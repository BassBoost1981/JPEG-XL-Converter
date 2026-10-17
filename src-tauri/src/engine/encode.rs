//! One file to JPEG XL (specification 5.1): convert into `<target>.tmp`, prove
//! the result, rename it, carry the file dates over, and only then decide
//! about the original.
//!
//! The original goes to the recycle bin only when the result is proven
//! lossless (byte-exact JPEG restore, or identical pixels at distance 0), all
//! metadata arrived, and nothing else of the source was left behind (further
//! pages, unverified animation frames). Everything this module writes besides
//! the target (the `.tmp` and the work files) is removed on every way out.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use image::ImageFormat;

use super::args::{DjxlTarget, cjxl_args};
use super::convert_io::{MetaState, MetaTransfer, PngMeta, scan_png, to_png};
use super::jxl_container::has_jpeg_reconstruction;
use super::originals::{TrashContext, copy_file_times, handle_original};
pub use super::pipeline_util::Ctx;
use super::pipeline_util::{
    NOTE_DATES, NOTE_METADATA, Scratch, Stop, check_cancel, check_target, finished, load,
    open_reason, pixels_equal, place, prepare, run_djxl,
};
use super::tools::ToolError;
use super::types::{InputKind, Outcome, ScanEntry};
use super::verify::{dims_match, files_identical, image_dims};

const NOT_TRANSCODABLE: &str = "JPEG lässt sich nicht verlustfrei umpacken";
const NOTE_FIRST_PAGE: &str = "Nur die erste Seite übernommen – Original behalten";
const NOTE_ANIMATION: &str = "Animation – Original behalten";
const READ_ERROR: &str = "Bild lässt sich nicht lesen";

/// Converts `entry` to `target` (which must not exist yet).
/// Progress: 5 at the start, 60 after converting, 90 after the check, 100 done.
pub fn encode_one(entry: &ScanEntry, target: &Path, ctx: &Ctx) -> Outcome {
    (ctx.progress)(5);
    let mut scratch = Scratch::default();
    run(entry, target, ctx, &mut scratch).unwrap_or_else(Stop::into_outcome)
}

fn run(
    entry: &ScanEntry,
    target: &Path,
    ctx: &Ctx,
    scratch: &mut Scratch,
) -> Result<Outcome, Stop> {
    let settings = ctx.settings;
    let keep_metadata = settings.keep_metadata;
    let source = entry.path.as_path();

    check_target(target)?;
    check_cancel(ctx)?;
    check_source(source, entry.kind)?;

    let work = prepare(target, ctx, scratch, "enc")?;
    let tmp = work.tmp.clone();
    let work_file = |suffix: &str| work.file(suffix);

    // ---- convert
    let transcode = entry.kind == InputKind::Jpeg && settings.encode.transcode_jpeg;
    let mut transfer: Option<MetaTransfer> = None;
    let (input, cjxl_kind) = match entry.kind {
        InputKind::Tiff | InputKind::Webp | InputKind::Bmp => {
            let png = scratch.add(work_file("quelle.png"));
            transfer = Some(to_png(source, &png, keep_metadata).map_err(Stop::Error)?);
            (png, InputKind::Png)
        }
        kind => (source.to_path_buf(), kind),
    };
    let facts = SourceFacts::read(&input, cjxl_kind)?;
    check_cancel(ctx)?;

    let args = cjxl_args(cjxl_kind, &settings.encode, keep_metadata);
    match ctx.tools.run_cjxl(&input, &tmp, &args, ctx.cancel) {
        Ok(()) => {}
        Err(ToolError {
            cancelled: true, ..
        }) => return Err(Stop::Cancelled),
        Err(_) if transcode => return Err(Stop::Skipped(NOT_TRANSCODABLE)),
        Err(_) => return Err(Stop::error("Umwandlung nach JPEG XL fehlgeschlagen")),
    }
    // A cheap first look; the byte-exact restore below is the real proof.
    if transcode && !has_jpeg_reconstruction(&tmp).unwrap_or(false) {
        return Err(Stop::Skipped(NOT_TRANSCODABLE));
    }
    (ctx.progress)(60);
    check_cancel(ctx)?;

    // ---- check
    let lossless = transcode || (entry.kind != InputKind::Jpeg && settings.encode.distance == 0.0);
    // Metadata cjxl dropped (seen in the decoded result).
    let mut decoded_metadata_lost = false;
    if transcode {
        let restored = scratch.add(work_file("pruefung.jpg"));
        let failed = "JPEG lässt sich nicht wiederherstellen";
        run_djxl(
            ctx,
            &tmp,
            &restored,
            DjxlTarget::ReconstructJpeg,
            Stop::VerifyFailed(failed),
        )?;
        if !files_identical(source, &restored).unwrap_or(false) {
            return Err(Stop::VerifyFailed(
                "Wiederhergestelltes JPEG weicht vom Original ab",
            ));
        }
    } else {
        let decoded = scratch.add(work_file("pruefung.png"));
        let to_png = DjxlTarget::Png {
            reduce_to_8bit: false,
        };
        run_djxl(
            ctx,
            &tmp,
            &decoded,
            to_png,
            Stop::VerifyFailed("JPEG XL lässt sich nicht dekodieren"),
        )?;
        if lossless {
            check_pixels(&input, &decoded, keep_metadata)?;
        } else {
            check_dims(&input, &decoded)?;
        }
        if keep_metadata {
            decoded_metadata_lost = facts.metadata_lost(&decoded)?;
        }
    }
    (ctx.progress)(90);
    check_cancel(ctx)?;

    // ---- finish
    place(&tmp, target, scratch)?;
    let dates_copied = copy_file_times(source, target).is_ok();

    // The per-field flags only choose the notes.
    let pages_lost = transfer.is_some_and(|t| t.frames == MetaState::Lost);
    let metadata_lost = decoded_metadata_lost
        || transfer.is_some_and(|t| t.exif == MetaState::Lost || t.icc == MetaState::Lost);
    let trash = TrashContext {
        enabled: settings.trash_originals,
        // Only the first frame of an animation was compared.
        verified: !facts.animated,
        lossless,
        metadata_complete: dates_copied
            && (transcode
                || (keep_metadata
                    && !decoded_metadata_lost
                    && transfer.is_none_or(|t| t.complete()))),
    };
    let mut notes: Vec<String> = Vec::new();
    if pages_lost {
        notes.push(NOTE_FIRST_PAGE.to_string());
    }
    if metadata_lost {
        notes.push(NOTE_METADATA.to_string());
    }
    if !dates_copied {
        notes.push(NOTE_DATES.to_string());
    }
    if facts.animated && settings.trash_originals {
        notes.push(NOTE_ANIMATION.to_string());
    }
    notes.extend(handle_original(&trash, source, ctx.bin));
    (ctx.progress)(100);

    Ok(finished(target, notes))
}

/// The source can be opened, and for the formats cjxl reads directly the
/// content is what the extension says (TIFF, WebP and BMP: `to_png` checks).
fn check_source(path: &Path, kind: InputKind) -> Result<(), Stop> {
    let file = File::open(path).map_err(|e| Stop::error(open_reason(&e)))?;
    let expected = match kind {
        InputKind::Jpeg => ImageFormat::Jpeg,
        InputKind::Png => ImageFormat::Png,
        InputKind::Gif => ImageFormat::Gif,
        InputKind::Tiff | InputKind::Webp | InputKind::Bmp => return Ok(()),
        InputKind::Jxl => return Err(Stop::error("Format wird hier nicht unterstützt")),
    };
    let mut head = Vec::new();
    file.take(16)
        .read_to_end(&mut head)
        .map_err(|e| Stop::error(open_reason(&e)))?;
    match image::guess_format(&head) {
        Ok(found) if found == expected => Ok(()),
        Ok(_) => Err(Stop::error("Inhalt passt nicht zur Dateiendung")),
        Err(_) => Err(Stop::error(READ_ERROR)),
    }
}

/// Distance 0: the decoded JXL has the pixels of the cjxl input. cjxl takes
/// the EXIF orientation into the JXL header and djxl turns the pixels
/// accordingly; with metadata off (`-x strip=exif`) the orientation is dropped
/// and the pixels come back as stored (docs/libjxl-notes.md).
fn check_pixels(input: &Path, decoded: &Path, keep_metadata: bool) -> Result<(), Stop> {
    const FAILED: &str = "Pixelvergleich nicht möglich";
    let (mut expected, orientation) = load(input).map_err(|_| Stop::VerifyFailed(FAILED))?;
    let (actual, _) = load(decoded).map_err(|_| Stop::VerifyFailed(FAILED))?;
    if keep_metadata {
        expected.apply_orientation(orientation);
    }
    if pixels_equal(&expected, &actual) {
        Ok(())
    } else {
        Err(Stop::VerifyFailed("Pixel weichen vom Original ab"))
    }
}

/// Lossy: the decoded JXL has the size of the input (possibly turned).
fn check_dims(input: &Path, decoded: &Path) -> Result<(), Stop> {
    const FAILED: &str = "Abmessungen lassen sich nicht prüfen";
    let expected = image_dims(input).map_err(|_| Stop::VerifyFailed(FAILED))?;
    let actual = image_dims(decoded).map_err(|_| Stop::VerifyFailed(FAILED))?;
    if dims_match(expected, actual) {
        Ok(())
    } else {
        Err(Stop::VerifyFailed("Abmessungen stimmen nicht"))
    }
}

/// What the cjxl input holds besides its first image.
struct SourceFacts {
    /// More than one frame (GIF, APNG).
    animated: bool,
    /// The chunk list of a PNG input, to compare with the decoded result.
    png: Option<PngMeta>,
    /// Metadata cjxl does not carry over.
    uncarried: bool,
}

impl SourceFacts {
    fn read(input: &Path, cjxl_kind: InputKind) -> Result<SourceFacts, Stop> {
        Ok(match cjxl_kind {
            InputKind::Png => {
                let png = scan_png(input).map_err(Stop::Error)?;
                SourceFacts {
                    animated: png.animated,
                    png: Some(png),
                    uncarried: png.other_metadata,
                }
            }
            InputKind::Gif => {
                let gif = scan_gif(input).map_err(|_| Stop::error(READ_ERROR))?;
                SourceFacts {
                    animated: gif.frames > 1,
                    png: None,
                    uncarried: gif.other_metadata,
                }
            }
            // JPEG: transcoding keeps everything; a re-encoded JPEG is never
            // lossless, so its original is not looked at further.
            _ => SourceFacts {
                animated: false,
                png: None,
                uncarried: false,
            },
        })
    }

    /// `true` if metadata of the source is missing from the decoded result:
    /// EXIF or XMP the source had and djxl does not give back, or anything
    /// cjxl does not carry at all.
    fn metadata_lost(&self, decoded: &Path) -> Result<bool, Stop> {
        let Some(src) = self.png else {
            return Ok(self.uncarried);
        };
        let out = scan_png(decoded)
            .map_err(|_| Stop::VerifyFailed("Metadaten lassen sich nicht prüfen"))?;
        Ok(self.uncarried || (src.exif && !out.exif) || (src.xmp && !out.xmp))
    }
}

#[derive(Debug, Default)]
struct GifFacts {
    frames: u32,
    /// Comments, and application data other than the loop count (XMP, ICC
    /// profiles): cjxl does not carry them (Ruling R20).
    other_metadata: bool,
}

/// Walks the blocks of a GIF without decoding the image data.
fn scan_gif(path: &Path) -> io::Result<GifFacts> {
    const IMAGE: u8 = 0x2C;
    const EXTENSION: u8 = 0x21;
    const TRAILER: u8 = 0x3B;
    const APPLICATION: u8 = 0xFF;
    const COMMENT: u8 = 0xFE;

    let mut r = BufReader::new(File::open(path)?);
    // Signature (6) and logical screen descriptor (7).
    let mut header = [0u8; 13];
    r.read_exact(&mut header)?;
    if &header[..3] != b"GIF" {
        return Err(io::ErrorKind::InvalidData.into());
    }
    skip_colour_table(&mut r, header[10])?;
    let mut facts = GifFacts::default();
    loop {
        match read_u8(&mut r)? {
            TRAILER => return Ok(facts),
            IMAGE => {
                facts.frames += 1;
                let mut descriptor = [0u8; 9];
                r.read_exact(&mut descriptor)?;
                skip_colour_table(&mut r, descriptor[8])?;
                read_u8(&mut r)?; // LZW minimum code size
                skip_sub_blocks(&mut r)?;
            }
            EXTENSION => match read_u8(&mut r)? {
                APPLICATION => {
                    let len = read_u8(&mut r)?;
                    let mut id = vec![0u8; usize::from(len)];
                    r.read_exact(&mut id)?;
                    if !(id.starts_with(b"NETSCAPE2.0") || id.starts_with(b"ANIMEXTS1.0")) {
                        facts.other_metadata = true;
                    }
                    skip_sub_blocks(&mut r)?;
                }
                COMMENT => {
                    facts.other_metadata = true;
                    skip_sub_blocks(&mut r)?;
                }
                // Graphic control, plain text: display, not metadata.
                _ => skip_sub_blocks(&mut r)?,
            },
            _ => return Err(io::ErrorKind::InvalidData.into()),
        }
    }
}

fn read_u8(r: &mut impl Read) -> io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

/// Skips the colour table announced in a packed-fields byte.
fn skip_colour_table(r: &mut BufReader<File>, packed: u8) -> io::Result<()> {
    if packed & 0x80 != 0 {
        r.seek_relative(3 << ((packed & 0x07) + 1))?;
    }
    Ok(())
}

/// Skips data sub-blocks up to the terminating empty one.
fn skip_sub_blocks(r: &mut BufReader<File>) -> io::Result<()> {
    loop {
        match read_u8(r)? {
            0 => return Ok(()),
            len => r.seek_relative(i64::from(len))?,
        }
    }
}
