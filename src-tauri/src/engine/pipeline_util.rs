//! Steps shared by `encode` and `decode`: how a call stops, the clean-up of
//! its own files, preparing the `.tmp` and the work file names, running djxl,
//! and moving the checked result into place.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
use windows::core::HSTRING;

use super::args::{DjxlTarget, djxl_args};
use super::encode::Ctx;
use super::targets::tmp_path;
use super::tools::{ToolError, extended_length};
use super::types::{ItemStatus, Outcome};

pub(super) const TARGET_EXISTS: &str = "Zieldatei existiert bereits";
pub(super) const NOTE_METADATA: &str = "Metadaten nicht vollständig übernommen";
pub(super) const NOTE_DATES: &str = "Dateidatum nicht übernommen";
const NOTE_SEPARATOR: &str = " · ";

/// Makes the work file names of parallel calls distinct.
static NEXT_WORK_ID: AtomicU64 = AtomicU64::new(0);

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
    let prefix = format!(
        "{kind}-{}-{}",
        std::process::id(),
        NEXT_WORK_ID.fetch_add(1, Ordering::Relaxed)
    );
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
