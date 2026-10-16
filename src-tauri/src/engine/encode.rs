//! One file to JPEG XL (specification 5.1): convert into `<target>.tmp`, prove
//! the result, rename it, carry the file dates over, and only then decide
//! about the original.
//!
//! The original goes to the recycle bin only when the result is proven
//! lossless (byte-exact JPEG restore, or identical pixels at distance 0), all
//! metadata arrived, and nothing else of the source was left behind (further
//! pages, unverified animation frames). Everything this module writes besides
//! the target (the `.tmp` and the work files) is removed on every way out.

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
use windows::core::HSTRING;

use super::args::{DjxlTarget, cjxl_args, djxl_args};
use super::convert_io::{MetaState, MetaTransfer, PngMeta, scan_png, to_png};
use super::jxl_container::has_jpeg_reconstruction;
use super::originals::{RecycleBin, TrashContext, copy_file_times, handle_original};
use super::targets::tmp_path;
use super::tools::{ToolError, Tools, extended_length};
use super::types::{CancelToken, InputKind, ItemStatus, JobSettings, Outcome, ScanEntry};
use super::verify::{dims_match, files_identical, image_dims, same_pixels};

pub struct Ctx<'a> {
    pub tools: &'a Tools,
    pub bin: &'a dyn RecycleBin,
    pub settings: &'a JobSettings,
    pub cancel: &'a CancelToken,
    /// Intermediate PNGs and check outputs go here, under names unique to
    /// this call.
    pub work_dir: &'a Path,
    pub progress: &'a dyn Fn(u8),
}

const TARGET_EXISTS: &str = "Zieldatei existiert bereits";
const NOT_TRANSCODABLE: &str = "JPEG lässt sich nicht verlustfrei umpacken";
const NOTE_METADATA: &str = "Metadaten nicht vollständig übernommen";
const NOTE_FIRST_PAGE: &str = "Nur die erste Seite übernommen – Original behalten";
const NOTE_ANIMATION: &str = "Animation – Original behalten";
const NOTE_DATES: &str = "Dateidatum nicht übernommen";
const READ_ERROR: &str = "Bild lässt sich nicht lesen";
const NOTE_SEPARATOR: &str = " · ";

/// Largest decoded image the pixel check accepts (as in `convert_io`).
const MAX_DECODED_BYTES: u64 = 4 << 30;

/// Makes the work file names of parallel calls distinct.
static NEXT_WORK_ID: AtomicU64 = AtomicU64::new(0);

/// Why a file did not get to the end.
enum Stop {
    Skipped(&'static str),
    Error(String),
    VerifyFailed(&'static str),
    Cancelled,
}

impl Stop {
    fn error(reason: &str) -> Stop {
        Stop::Error(reason.to_string())
    }

    fn into_outcome(self) -> Outcome {
        let (status, note) = match self {
            Stop::Skipped(note) => (ItemStatus::Skipped, Some(note.to_string())),
            Stop::Error(reason) => (ItemStatus::Error, Some(reason)),
            Stop::VerifyFailed(reason) => (ItemStatus::VerifyFailed, Some(reason.to_string())),
            // The queue puts the entry back.
            Stop::Cancelled => (ItemStatus::Waiting, None),
        };
        Outcome {
            status,
            out_path: None,
            out_size: None,
            note,
        }
    }
}

/// Files removed when the call ends, however it ends.
#[derive(Default)]
struct Scratch(Vec<PathBuf>);

impl Scratch {
    fn add(&mut self, path: PathBuf) -> PathBuf {
        self.0.push(path.clone());
        path
    }

    fn keep(&mut self, path: &Path) {
        self.0.retain(|p| p != path);
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

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

    if fs::symlink_metadata(target).is_ok() {
        return Err(Stop::Skipped(TARGET_EXISTS));
    }
    check_cancel(ctx)?;
    check_source(source, entry.kind)?;

    let tmp = tmp_path(target);
    match fs::remove_file(&tmp) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            return Err(Stop::error("Alte Zwischendatei lässt sich nicht löschen"));
        }
        _ => {}
    }
    scratch.add(tmp.clone());
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|_| Stop::error("Zielordner lässt sich nicht anlegen"))?;
    }
    fs::create_dir_all(ctx.work_dir)
        .map_err(|_| Stop::error("Arbeitsordner lässt sich nicht anlegen"))?;
    let work = format!(
        "enc-{}-{}",
        std::process::id(),
        NEXT_WORK_ID.fetch_add(1, Ordering::Relaxed)
    );
    let work_file = |suffix: &str| ctx.work_dir.join(format!("{work}-{suffix}"));

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
    let mut metadata_lost =
        transfer.is_some_and(|t| t.exif == MetaState::Lost || t.icc == MetaState::Lost);
    if transcode {
        let restored = scratch.add(work_file("pruefung.jpg"));
        let failed = "JPEG lässt sich nicht wiederherstellen";
        decode(ctx, &tmp, &restored, DjxlTarget::ReconstructJpeg, failed)?;
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
        decode(
            ctx,
            &tmp,
            &decoded,
            to_png,
            "JPEG XL lässt sich nicht dekodieren",
        )?;
        if lossless {
            check_pixels(&input, &decoded, keep_metadata)?;
        } else {
            check_dims(&input, &decoded)?;
        }
        if keep_metadata {
            metadata_lost |= facts.metadata_lost(&decoded)?;
        }
    }
    (ctx.progress)(90);
    check_cancel(ctx)?;

    // ---- finish
    if move_new(&tmp, target).is_err() {
        return Err(if fs::symlink_metadata(target).is_ok() {
            Stop::Skipped(TARGET_EXISTS)
        } else {
            Stop::error("Zieldatei lässt sich nicht anlegen")
        });
    }
    scratch.keep(&tmp);
    let dates_copied = copy_file_times(source, target).is_ok();

    let pages_lost = transfer.is_some_and(|t| t.frames == MetaState::Lost);
    let trash = TrashContext {
        enabled: settings.trash_originals,
        // Only the first frame of an animation was compared.
        verified: !facts.animated,
        lossless,
        metadata_complete: dates_copied
            && (transcode || (keep_metadata && !metadata_lost && !pages_lost)),
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

    Ok(Outcome {
        status: if notes.is_empty() {
            ItemStatus::Done
        } else {
            ItemStatus::DoneWithNote
        },
        out_path: Some(target.to_path_buf()),
        out_size: fs::metadata(target).ok().map(|m| m.len()),
        note: (!notes.is_empty()).then(|| notes.join(NOTE_SEPARATOR)),
    })
}

fn check_cancel(ctx: &Ctx) -> Result<(), Stop> {
    if ctx.cancel.is_cancelled() {
        Err(Stop::Cancelled)
    } else {
        Ok(())
    }
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

fn open_reason(e: &io::Error) -> &'static str {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    match (e.kind(), e.raw_os_error()) {
        (io::ErrorKind::NotFound, _) => "Datei nicht gefunden",
        (_, Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)) => {
            "Datei wird von einem anderen Programm verwendet"
        }
        _ => "Datei lässt sich nicht öffnen",
    }
}

/// djxl `jxl` → `out`, as part of the check: a failure is a failed check.
fn decode(
    ctx: &Ctx,
    jxl: &Path,
    out: &Path,
    target: DjxlTarget,
    failed: &'static str,
) -> Result<(), Stop> {
    match ctx.tools.run_djxl(jxl, out, &djxl_args(target), ctx.cancel) {
        Ok(()) => Ok(()),
        Err(ToolError {
            cancelled: true, ..
        }) => Err(Stop::Cancelled),
        Err(_) => Err(Stop::VerifyFailed(failed)),
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

/// The first image of a file and its EXIF orientation. Refused (not aborted)
/// when it would not fit into `MAX_DECODED_BYTES`.
fn load(path: &Path) -> Result<(DynamicImage, Orientation), ()> {
    let mut reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(drop)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(drop)?;
    if decoder.total_bytes() > MAX_DECODED_BYTES {
        return Err(());
    }
    let orientation = decoder.orientation().map_err(drop)?;
    let image = DynamicImage::from_decoder(decoder).map_err(drop)?;
    Ok((image, orientation))
}

/// `same_pixels`, without its two 16-bit copies when the layouts already agree.
fn pixels_equal(a: &DynamicImage, b: &DynamicImage) -> bool {
    if a.color() == b.color() {
        a.width() == b.width() && a.height() == b.height() && a.as_bytes() == b.as_bytes()
    } else {
        same_pixels(a, b)
    }
}

/// Renames `from` to `to`, failing if `to` exists (`fs::rename` would replace it).
fn move_new(from: &Path, to: &Path) -> windows::core::Result<()> {
    let from = HSTRING::from(extended_length(from).as_path());
    let to = HSTRING::from(extended_length(to).as_path());
    // SAFETY: both arguments are valid, NUL-terminated wide strings that live
    // until the call returns.
    unsafe { MoveFileExW(&from, &to, MOVE_FILE_FLAGS(0)) }
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
