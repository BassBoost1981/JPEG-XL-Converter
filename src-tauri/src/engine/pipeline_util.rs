//! Steps shared by `encode` and `decode`: the call context, how a call stops,
//! the clean-up of its own files, preparing the `.tmp` and the work file names,
//! running djxl, the pixel comparison, and moving the checked result into place.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
use windows::core::HSTRING;

use super::args::{DjxlTarget, djxl_args};
use super::originals::RecycleBin;
use super::targets::tmp_path;
use super::tools::{ToolError, Tools, extended_length};
use super::types::{CancelToken, ItemStatus, JobSettings, Outcome};
use super::verify::same_pixels;

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

/// Largest decoded image the pixel check accepts (as in `convert_io`).
const MAX_DECODED_BYTES: u64 = 4 << 30;

pub(super) const TARGET_EXISTS: &str = "Zieldatei existiert bereits";
pub(super) const NOTE_METADATA: &str = "Metadaten nicht vollständig übernommen";
pub(super) const NOTE_DATES: &str = "Dateidatum nicht übernommen";
const NOTE_SEPARATOR: &str = " · ";

/// Makes the work file names of parallel calls distinct.
static NEXT_WORK_ID: AtomicU64 = AtomicU64::new(0);

/// A prefix for work file names, unique within the process (and across
/// processes sharing a work folder).
pub(super) fn unique_prefix(kind: &str) -> String {
    format!(
        "{kind}-{}-{}",
        std::process::id(),
        NEXT_WORK_ID.fetch_add(1, Ordering::Relaxed)
    )
}

/// Why a file did not get to the end.
pub(super) enum Stop {
    Skipped(&'static str),
    Error(String),
    VerifyFailed(&'static str),
    Cancelled,
}

impl Stop {
    pub(super) fn error(reason: &str) -> Stop {
        Stop::Error(reason.to_string())
    }

    pub(super) fn into_outcome(self) -> Outcome {
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
pub(super) struct Scratch(Vec<PathBuf>);

impl Scratch {
    pub(super) fn add(&mut self, path: PathBuf) -> PathBuf {
        self.0.push(path.clone());
        path
    }

    pub(super) fn keep(&mut self, path: &Path) {
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

/// The `.tmp` of a call and the names of its work files.
pub(super) struct Work<'a> {
    pub(super) tmp: PathBuf,
    dir: &'a Path,
    prefix: String,
}

impl Work<'_> {
    /// A work file name unique to this call (the caller registers it in `Scratch`).
    pub(super) fn file(&self, suffix: &str) -> PathBuf {
        self.dir.join(format!("{}-{suffix}", self.prefix))
    }
}

pub(super) fn check_cancel(ctx: &Ctx) -> Result<(), Stop> {
    if ctx.cancel.is_cancelled() {
        Err(Stop::Cancelled)
    } else {
        Ok(())
    }
}

/// An existing target is never replaced.
pub(super) fn check_target(target: &Path) -> Result<(), Stop> {
    if fs::symlink_metadata(target).is_ok() {
        Err(Stop::Skipped(TARGET_EXISTS))
    } else {
        Ok(())
    }
}

pub(super) fn open_reason(e: &io::Error) -> &'static str {
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

/// Removes a stale `.tmp` of `target`, registers the new one in `scratch`,
/// and creates the target and work folders. `kind` starts the work file names.
pub(super) fn prepare<'a>(
    target: &Path,
    ctx: &Ctx<'a>,
    scratch: &mut Scratch,
    kind: &str,
) -> Result<Work<'a>, Stop> {
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
    let prefix = unique_prefix(kind);
    Ok(Work {
        tmp,
        dir: ctx.work_dir,
        prefix,
    })
}

/// djxl `jxl` → `out`; `failed` is the answer when djxl fails.
pub(super) fn run_djxl(
    ctx: &Ctx,
    jxl: &Path,
    out: &Path,
    target: DjxlTarget,
    failed: Stop,
) -> Result<(), Stop> {
    match ctx.tools.run_djxl(jxl, out, &djxl_args(target), ctx.cancel) {
        Ok(()) => Ok(()),
        Err(ToolError {
            cancelled: true, ..
        }) => Err(Stop::Cancelled),
        Err(_) => Err(failed),
    }
}

/// Renames the checked `tmp` to `target`. A target that appeared in the
/// meantime is kept and the item is skipped.
pub(super) fn place(tmp: &Path, target: &Path, scratch: &mut Scratch) -> Result<(), Stop> {
    if move_new(tmp, target).is_err() {
        return Err(if fs::symlink_metadata(target).is_ok() {
            Stop::Skipped(TARGET_EXISTS)
        } else {
            Stop::error("Zieldatei lässt sich nicht anlegen")
        });
    }
    scratch.keep(tmp);
    Ok(())
}

/// The outcome of a finished item: `Done`, or `DoneWithNote` with the notes joined.
pub(super) fn finished(target: &Path, notes: Vec<String>) -> Outcome {
    Outcome {
        status: if notes.is_empty() {
            ItemStatus::Done
        } else {
            ItemStatus::DoneWithNote
        },
        out_path: Some(target.to_path_buf()),
        out_size: fs::metadata(target).ok().map(|m| m.len()),
        note: (!notes.is_empty()).then(|| notes.join(NOTE_SEPARATOR)),
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

/// The first image of a file and its EXIF orientation. Refused (not aborted)
/// when it would not fit into `MAX_DECODED_BYTES` or is CMYK.
pub(super) fn load(path: &Path) -> Result<(DynamicImage, Orientation), ()> {
    // The image crate turns CMYK into RGB by a naive formula (and does not say
    // so for JPEG); never compare or show that.
    if jpeg_component_count(path) == Some(4) {
        return Err(());
    }
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
pub(super) fn pixels_equal(a: &DynamicImage, b: &DynamicImage) -> bool {
    if a.color() == b.color() {
        a.width() == b.width() && a.height() == b.height() && a.as_bytes() == b.as_bytes()
    } else {
        same_pixels(a, b)
    }
}

/// The number of colour components a JPEG file announces in its frame header
/// (4 = CMYK), or `None` if the file is not a JPEG or the header is unreadable.
fn jpeg_component_count(path: &Path) -> Option<u8> {
    let mut file = io::BufReader::new(fs::File::open(path).ok()?);
    let mut head = [0u8; 2];
    file.read_exact(&mut head).ok()?;
    if head != [0xFF, 0xD8] {
        return None;
    }
    loop {
        // Segments follow each other: 0xFF, marker code, length, payload.
        let mut marker = [0u8; 2];
        file.read_exact(&mut marker).ok()?;
        if marker[0] != 0xFF {
            return None;
        }
        let code = marker[1];
        // Start of scan or end of image: no frame header came before.
        if matches!(code, 0xD9 | 0xDA) {
            return None;
        }
        let mut len = [0u8; 2];
        file.read_exact(&mut len).ok()?;
        let len = u64::from(u16::from_be_bytes(len));
        let is_frame = matches!(code, 0xC0..=0xCF) && !matches!(code, 0xC4 | 0xC8 | 0xCC);
        if is_frame {
            // precision (1), height (2), width (2), component count (1)
            let mut frame = [0u8; 6];
            file.read_exact(&mut frame).ok()?;
            return Some(frame[5]);
        }
        io::copy(
            &mut (&mut file).take(len.saturating_sub(2)),
            &mut io::sink(),
        )
        .ok()?;
    }
}
