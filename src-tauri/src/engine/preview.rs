//! Before/after crop for the comparison slider, and thumbnails for the queue.
//!
//! Previews only read: the source is never touched, and everything written
//! goes to `work_dir` under names unique to the call (previews run beside a
//! conversion run and beside each other). Those files are removed on every
//! way out. The images are handed back as PNG bytes.
//!
//! Orientation: both halves show the photo the way viewers do, with the EXIF
//! orientation applied. A JXL comes out of djxl already turned (the
//! orientation lives in its header, see docs/libjxl-notes.md); every other
//! source is turned here after loading. The crop is cut from the turned
//! image, so the crop PNG carries no orientation and cjxl/djxl give back the
//! same layout: before and after line up pixel for pixel.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};

use super::args::{DjxlTarget, cjxl_args, djxl_args};
use super::convert_io::from_png;
use super::jxl_container::has_jpeg_reconstruction;
use super::msg::{Msg, keys};
use super::pipeline_util::{Scratch, load, unique_prefix};
use super::tools::Tools;
use super::types::{CancelToken, InputKind, JobSettings, Mode, OutputFormat, ScanEntry};

const UNAVAILABLE: &str = keys::PREVIEW_UNAVAILABLE;
const UNSUPPORTED: &str = keys::PREVIEW_UNSUPPORTED;
const WRONG_MODE: &str = keys::PREVIEW_WRONG_MODE;
/// djxl output for previews: full bit depth, as the checks in encode use it.
const PREVIEW_PNG: DjxlTarget = DjxlTarget::Png {
    reduce_to_8bit: false,
};

/// The centered window of `view_w × view_h` in an `img_w × img_h` image,
/// reduced to the image where it is smaller: `(x, y, width, height)`.
pub fn crop_rect(img_w: u32, img_h: u32, view_w: u32, view_h: u32) -> (u32, u32, u32, u32) {
    let w = view_w.min(img_w);
    let h = view_h.min(img_h);
    ((img_w - w) / 2, (img_h - h) / 2, w, h)
}

pub struct Preview {
    pub before_png: Vec<u8>,
    pub after_png: Vec<u8>,
    /// The after image is identical to the before image (no loss to show).
    pub lossless: bool,
}

/// The crop of `entry` as it is now, and as the conversion in `mode` with the
/// settings `s` would leave it.
pub fn make_preview(
    entry: &ScanEntry,
    mode: Mode,
    s: &JobSettings,
    view_w: u32,
    view_h: u32,
    tools: &Tools,
    work_dir: &Path,
) -> Result<Preview, Msg> {
    if (mode == Mode::Dec) != (entry.kind == InputKind::Jxl) {
        return Err(Msg::new(WRONG_MODE));
    }
    let mut scratch = Scratch::default();
    let image = load_source(entry, tools, work_dir, &mut scratch)?;
    let (x, y, w, h) = crop_rect(image.width(), image.height(), view_w, view_h);
    if w == 0 || h == 0 {
        return Err(Msg::new(UNAVAILABLE));
    }
    let crop = image.crop_imm(x, y, w, h);
    let before_png = png_bytes(&crop)?;

    let same = |lossless| {
        Ok(Preview {
            after_png: before_png.clone(),
            before_png: before_png.clone(),
            lossless,
        })
    };
    let after_png = match mode {
        Mode::Enc => {
            let transcode = entry.kind == InputKind::Jpeg && s.encode.transcode_jpeg;
            if transcode || s.encode.distance == 0.0 {
                return same(true);
            }
            lossy_jxl(&before_png, s, tools, work_dir, &mut scratch)?
        }
        Mode::Dec => match s.decode.format {
            OutputFormat::Jpg | OutputFormat::Auto
                if has_jpeg_reconstruction(&entry.path).map_err(|_| Msg::new(UNAVAILABLE))? =>
            {
                // The real conversion gives back the original JPEG.
                return same(true);
            }
            OutputFormat::Jpg => as_jpeg(&crop, s.decode.jpeg_quality)?,
            OutputFormat::Webp => as_webp(&before_png, s, work_dir, &mut scratch)?,
            OutputFormat::Auto | OutputFormat::Png | OutputFormat::Tiff => return same(true),
        },
    };
    Ok(Preview {
        before_png,
        after_png,
        lossless: false,
    })
}

/// A PNG of `entry` with its longest side `size`, aspect ratio kept and
/// orientation applied (a small image is scaled up to `size`).
pub fn thumbnail(
    entry: &ScanEntry,
    size: u32,
    tools: &Tools,
    work_dir: &Path,
) -> Result<Vec<u8>, Msg> {
    let mut scratch = Scratch::default();
    let image = load_source(entry, tools, work_dir, &mut scratch)?;
    let longest = image.width().max(image.height());
    let scaled = |side: u32| {
        let v = (u64::from(side) * u64::from(size) + u64::from(longest) / 2) / u64::from(longest);
        u32::try_from(v).unwrap_or(size).clamp(1, size.max(1))
    };
    let thumb = image.thumbnail_exact(scaled(image.width()), scaled(image.height()));
    png_bytes(&thumb)
}

/// The first image of `entry`, turned upright. Float images are refused (the
/// conversions refuse them too), and anything the loader refuses.
fn load_source(
    entry: &ScanEntry,
    tools: &Tools,
    work_dir: &Path,
    scratch: &mut Scratch,
) -> Result<DynamicImage, Msg> {
    let (mut image, orientation) = if entry.kind == InputKind::Jxl {
        let png = work_file(work_dir, scratch, "quelle.png")?;
        let args = djxl_args(PREVIEW_PNG);
        tools
            .run_djxl(&entry.path, &png, &args, &CancelToken::new())
            .map_err(|_| Msg::new(UNAVAILABLE))?;
        load(&png)
    } else {
        load(&entry.path)
    }
    .map_err(|()| Msg::new(UNSUPPORTED))?;
    if matches!(
        image,
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_)
    ) {
        return Err(Msg::new(UNSUPPORTED));
    }
    image.apply_orientation(orientation);
    Ok(image)
}

/// The crop through cjxl with the settings of the real conversion and back
/// through djxl.
fn lossy_jxl(
    crop_png: &[u8],
    s: &JobSettings,
    tools: &Tools,
    work_dir: &Path,
    scratch: &mut Scratch,
) -> Result<Vec<u8>, Msg> {
    let png = work_file(work_dir, scratch, "ausschnitt.png")?;
    let jxl = work_file(work_dir, scratch, "ausschnitt.jxl")?;
    let out = work_file(work_dir, scratch, "nachher.png")?;
    std::fs::write(&png, crop_png).map_err(|_| Msg::new(UNAVAILABLE))?;
    let cancel = CancelToken::new();
    // The crop is a PNG whatever the source was; a JPEG source without
    // transcoding is encoded from these pixels at the chosen distance.
    let args = cjxl_args(InputKind::Png, &s.encode, s.keep_metadata);
    tools
        .run_cjxl(&png, &jxl, &args, &cancel)
        .and_then(|()| tools.run_djxl(&jxl, &out, &djxl_args(PREVIEW_PNG), &cancel))
        .map_err(|_| Msg::new(UNAVAILABLE))?;
    std::fs::read(&out).map_err(|_| Msg::new(UNAVAILABLE))
}

/// The crop as a JPEG of `quality`, read back.
fn as_jpeg(crop: &DynamicImage, quality: u8) -> Result<Vec<u8>, Msg> {
    let mut jpeg = Vec::new();
    crop.to_rgb8()
        .write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, quality))
        .map_err(|_| Msg::new(UNAVAILABLE))?;
    let back = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg)
        .map_err(|_| Msg::new(UNAVAILABLE))?;
    png_bytes(&back)
}

/// The crop as the WebP the real conversion writes, read back.
fn as_webp(
    crop_png: &[u8],
    s: &JobSettings,
    work_dir: &Path,
    scratch: &mut Scratch,
) -> Result<Vec<u8>, Msg> {
    let png = work_file(work_dir, scratch, "ausschnitt.png")?;
    let webp = work_file(work_dir, scratch, "nachher.webp")?;
    std::fs::write(&png, crop_png).map_err(|_| Msg::new(UNAVAILABLE))?;
    from_png(&png, &webp, OutputFormat::Webp, s.keep_metadata)?;
    let (back, _) = load(&webp).map_err(|()| Msg::new(UNAVAILABLE))?;
    png_bytes(&back)
}

/// A new work file path (unique to this call), removed when `scratch` drops.
fn work_file(dir: &Path, scratch: &mut Scratch, suffix: &str) -> Result<PathBuf, Msg> {
    std::fs::create_dir_all(dir).map_err(|_| Msg::new(UNAVAILABLE))?;
    let name = format!("{}-{suffix}", unique_prefix("vorschau"));
    Ok(scratch.add(dir.join(name)))
}

fn png_bytes(image: &DynamicImage) -> Result<Vec<u8>, Msg> {
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|_| Msg::new(UNAVAILABLE))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_rect_centers_and_clamps() {
        assert_eq!(crop_rect(6000, 4000, 1000, 240), (2500, 1880, 1000, 240));
        assert_eq!(crop_rect(500, 300, 1000, 240), (0, 30, 500, 240));
    }

    #[test]
    fn crop_rect_of_empty_view_is_empty() {
        assert_eq!(crop_rect(100, 100, 0, 0), (50, 50, 0, 0));
    }
}
