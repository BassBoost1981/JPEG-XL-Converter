//! One JPEG XL back to JPEG, PNG, TIFF or WebP (specification 5.2): convert
//! into `<target>.tmp`, check the result, rename it, carry the file dates
//! over, and only then decide about the `.jxl`.
//!
//! The `.jxl` goes to the recycle bin only when the result holds everything
//! it had: a reconstructed JPEG (byte for byte the original), or a PNG/TIFF at
//! full bit depth with all metadata, and never for an animation (only its
//! first frame is written and checked). Everything this module writes besides
//! the target is removed on every way out.

use std::fs::File;
use std::io;
use std::path::Path;

use super::args::DjxlTarget;
use super::convert_io::{
    MetaState, MetaTransfer, from_png, strip_jpeg_metadata, strip_png_metadata,
};
use super::jxl_container::{has_jpeg_reconstruction, read_header};
use super::originals::{TrashContext, copy_file_times, handle_original};
use super::pipeline_util::{
    Ctx, NOTE_DATES, NOTE_METADATA, Scratch, Stop, check_cancel, check_target, finished, load,
    open_reason, pixels_equal, place, prepare, run_djxl,
};
use super::types::{Outcome, OutputFormat, ScanEntry};
use super::verify::{dims_match, image_dims};

const NOTE_ANIMATION: &str = "Animation – .jxl behalten";
const DECODE_FAILED: &str = "Umwandlung aus JPEG XL fehlgeschlagen";
const READ_ERROR: &str = "JPEG XL lässt sich nicht lesen";

/// File extension of the result: `Auto` gives back the original JPEG when the
/// file carries reconstruction data, otherwise a PNG.
pub fn decode_ext(entry: &ScanEntry, format: OutputFormat) -> io::Result<&'static str> {
    Ok(match format {
        OutputFormat::Auto if has_jpeg_reconstruction(&entry.path)? => "jpg",
        OutputFormat::Auto | OutputFormat::Png => "png",
        OutputFormat::Jpg => "jpg",
        OutputFormat::Webp => "webp",
        OutputFormat::Tiff => "tif",
    })
}

/// Converts `entry` to `target` (which must not exist yet).
/// Progress: 5 at the start, 60 after converting, 90 after the check, 100 done.
pub fn decode_one(entry: &ScanEntry, target: &Path, ctx: &Ctx) -> Outcome {
    (ctx.progress)(5);
    let mut scratch = Scratch::default();
    run(entry, target, ctx, &mut scratch).unwrap_or_else(Stop::into_outcome)
}

/// What djxl is asked to write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    /// The original JPEG from the `jbrd` box.
    Reconstructed,
    /// The pixels as a new JPEG.
    PixelsJpeg,
    Png,
    /// djxl writes a PNG, `from_png` turns it into TIFF or WebP.
    Converted(OutputFormat),
}

fn run(
    entry: &ScanEntry,
    target: &Path,
    ctx: &Ctx,
    scratch: &mut Scratch,
) -> Result<Outcome, Stop> {
    let settings = ctx.settings;
    let decode = &settings.decode;
    let keep_metadata = settings.keep_metadata;
    let source = entry.path.as_path();

    check_target(target)?;
    check_cancel(ctx)?;
    let animated = is_animated(source)?;
    let jbrd = has_jpeg_reconstruction(source).map_err(|_| Stop::error(READ_ERROR))?;
    let output = match decode.format {
        OutputFormat::Auto | OutputFormat::Jpg if jbrd => Output::Reconstructed,
        OutputFormat::Jpg => Output::PixelsJpeg,
        OutputFormat::Auto | OutputFormat::Png => Output::Png,
        format => Output::Converted(format),
    };

    let work = prepare(target, ctx, scratch, "dec")?;
    let tmp = &work.tmp;

    // ---- convert
    let png_target = DjxlTarget::Png {
        reduce_to_8bit: decode.reduce_to_8bit,
    };
    let mut transfer: Option<MetaTransfer> = None;
    let mut intermediate = None;
    match output {
        Output::Converted(format) => {
            let png = scratch.add(work.file("zwischen.png"));
            run_djxl(ctx, source, &png, png_target, Stop::error(DECODE_FAILED))?;
            check_cancel(ctx)?;
            transfer = Some(from_png(&png, tmp, format, keep_metadata).map_err(Stop::Error)?);
            intermediate = Some(png);
        }
        _ => {
            let djxl_target = match output {
                Output::Reconstructed => DjxlTarget::ReconstructJpeg,
                Output::PixelsJpeg => DjxlTarget::PixelsToJpeg {
                    quality: decode.jpeg_quality,
                },
                _ => png_target,
            };
            run_djxl(ctx, source, tmp, djxl_target, Stop::error(DECODE_FAILED))?;
            if !keep_metadata {
                match output {
                    Output::Png => strip_png_metadata(tmp).map_err(Stop::Error)?,
                    Output::PixelsJpeg => strip_jpeg_metadata(tmp).map_err(Stop::Error)?,
                    // A reconstructed JPEG is the original, byte for byte.
                    _ => {}
                }
            }
        }
    }
    (ctx.progress)(60);
    check_cancel(ctx)?;

    // ---- check
    let dims =
        image_dims(tmp).map_err(|_| Stop::VerifyFailed("Ergebnis lässt sich nicht lesen"))?;
    if !dims_match((entry.width, entry.height), dims) {
        return Err(Stop::VerifyFailed("Abmessungen stimmen nicht"));
    }
    // A TIFF counts as lossless only with the pixels djxl gave.
    if let (Output::Converted(OutputFormat::Tiff), Some(png)) = (output, &intermediate) {
        check_pixels(png, tmp)?;
    }
    (ctx.progress)(90);
    check_cancel(ctx)?;

    // ---- finish
    place(tmp, target, scratch)?;
    let dates_copied = copy_file_times(source, target).is_ok();

    let reconstructed = output == Output::Reconstructed;
    let lossless = reconstructed
        || (matches!(output, Output::Png | Output::Converted(OutputFormat::Tiff))
            && !decode.reduce_to_8bit);
    let metadata_lost = keep_metadata
        && transfer.is_some_and(|t| t.exif == MetaState::Lost || t.icc == MetaState::Lost);
    let trash = TrashContext {
        enabled: settings.trash_originals,
        // Only the first frame of an animation is written and checked.
        verified: !animated,
        lossless,
        metadata_complete: dates_copied
            && (reconstructed || (keep_metadata && transfer.is_none_or(|t| t.complete()))),
    };
    let mut notes: Vec<String> = Vec::new();
    if metadata_lost {
        notes.push(NOTE_METADATA.to_string());
    }
    if !dates_copied {
        notes.push(NOTE_DATES.to_string());
    }
    if animated && settings.trash_originals {
        notes.push(NOTE_ANIMATION.to_string());
    }
    notes.extend(handle_original(&trash, source, ctx.bin));
    (ctx.progress)(100);

    Ok(finished(target, notes))
}

/// The TIFF has the pixels of the PNG djxl wrote.
fn check_pixels(png: &Path, tiff: &Path) -> Result<(), Stop> {
    const FAILED: &str = "Pixelvergleich nicht möglich";
    let (expected, _) = load(png).map_err(|_| Stop::VerifyFailed(FAILED))?;
    let (actual, _) = load(tiff).map_err(|_| Stop::VerifyFailed(FAILED))?;
    if pixels_equal(&expected, &actual) {
        Ok(())
    } else {
        Err(Stop::VerifyFailed("Pixel weichen vom Original ab"))
    }
}

/// The source can be opened; `true` if its image header announces an
/// animation. Read from the file itself, because djxl drops frames of 0 ms
/// when it writes an animated PNG.
fn is_animated(path: &Path) -> Result<bool, Stop> {
    File::open(path).map_err(|e| Stop::error(open_reason(&e)))?;
    let image = read_header(path).ok_or_else(|| Stop::error(READ_ERROR))?;
    Ok(image.image_header().metadata.animation.is_some())
}
