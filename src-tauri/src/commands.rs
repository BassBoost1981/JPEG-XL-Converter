//! Thin Tauri commands: map errors to `Msg` keys and call the engine.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use serde_json::Value;
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use windows::Win32::Globalization::{
    GetUserDefaultUILanguage, LCIDToLocaleName, LOCALE_ALLOW_NEUTRAL_NAMES,
};

use crate::engine::msg::{Msg, keys};
use crate::engine::originals::WindowsRecycleBin;
use crate::engine::preview::{make_preview, thumbnail as make_thumbnail};
use crate::engine::queue::{ProgressSink, Queue, RealProcessor, default_workers};
use crate::engine::scan::scan_paths;
use crate::engine::tools::Tools;
use crate::engine::types::{
    ItemEvent, JobSettings, Lang, Mode, PauseReason, QueueItem, RunSummary, ToolStatus,
};
use crate::settings_store;
use crate::storage::Storage;

const THUMBNAIL_SIZE: u32 = 80;

/// Shared app state, built once at startup.
pub struct AppState {
    tools: Option<Tools>,
    queue: Option<Queue>,
    storage: Storage,
}

impl AppState {
    pub fn new(app: AppHandle, tools: Option<Tools>, storage: Storage) -> Self {
        let queue = tools.clone().map(|tools| {
            let processor = RealProcessor {
                tools,
                bin: Arc::new(WindowsRecycleBin),
                work_dir: storage.work_dir.clone(),
            };
            Queue::new(
                Arc::new(processor),
                Arc::new(EmitSink(app)),
                default_workers(),
                storage.logs_dir.clone(),
                storage.temp_logs_dir.clone(),
            )
        });
        AppState {
            tools,
            queue,
            storage,
        }
    }

    /// Stops a running queue.
    pub fn cancel(&self) {
        if let Some(q) = &self.queue {
            q.cancel();
        }
    }

    /// Stops a running queue and waits (at most `timeout`) until its tools
    /// are killed and its scratch files removed (called on exit).
    pub fn cancel_and_wait(&self, timeout: Duration) -> bool {
        self.queue
            .as_ref()
            .is_none_or(|q| q.cancel_and_wait(timeout))
    }

    pub fn work_dir(&self) -> &Path {
        &self.storage.work_dir
    }

    fn queue(&self) -> Result<&Queue, Msg> {
        self.queue
            .as_ref()
            .ok_or_else(|| Msg::new(keys::TOOLS_MISSING))
    }

    fn tools(&self) -> Result<Tools, Msg> {
        self.tools
            .clone()
            .ok_or_else(|| Msg::new(keys::TOOLS_MISSING))
    }
}

struct EmitSink(AppHandle);

impl ProgressSink for EmitSink {
    fn item(&self, ev: ItemEvent) {
        let _ = self.0.emit("item-progress", ev);
    }
    fn paused(&self, r: PauseReason) {
        let _ = self.0.emit("run-paused", r);
    }
    fn finished(&self, s: RunSummary) {
        let _ = self.0.emit("run-finished", s);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    items: Vec<QueueItem>,
    rejected: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDto {
    before: String,
    after: String,
    lossless: bool,
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}

#[tauri::command]
pub fn check_tools(state: State<'_, AppState>) -> ToolStatus {
    match &state.tools {
        Some(tools) => tools.check(),
        None => ToolStatus {
            ok: false,
            cjxl_version: None,
            djxl_version: None,
            messages: vec![Msg::new(keys::TOOLS_MISSING)],
        },
    }
}

#[tauri::command]
pub async fn add_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
    mode: Mode,
) -> Result<AddResult, Msg> {
    let queue = state.queue()?;
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let scan = spawn_blocking(move || scan_paths(&paths, mode))
        .await
        .map_err(|_| Msg::new(keys::ERROR_INTERNAL))?;
    Ok(AddResult {
        items: queue.add(mode, scan.entries),
        rejected: scan.rejected,
    })
}

#[tauri::command]
pub fn clear_queue(state: State<'_, AppState>, mode: Mode) -> Result<(), Msg> {
    state.queue()?.clear(mode)
}

#[tauri::command]
pub fn start_run(
    state: State<'_, AppState>,
    mode: Mode,
    settings: JobSettings,
    lang: Lang,
) -> Result<(), Msg> {
    state.queue()?.start(mode, settings, lang);
    Ok(())
}

#[tauri::command]
pub fn retry_failed(
    state: State<'_, AppState>,
    mode: Mode,
    settings: JobSettings,
    lang: Lang,
) -> Result<(), Msg> {
    state.queue()?.retry_failed(mode, settings, lang);
    Ok(())
}

#[tauri::command]
pub fn pause_run(state: State<'_, AppState>) {
    if let Some(q) = &state.queue {
        q.pause();
    }
}

#[tauri::command]
pub fn resume_run(state: State<'_, AppState>) {
    if let Some(q) = &state.queue {
        q.resume();
    }
}

#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>) {
    state.cancel();
}

#[tauri::command]
pub fn is_running(state: State<'_, AppState>) -> bool {
    state.queue.as_ref().is_some_and(Queue::is_running)
}

#[tauri::command]
pub async fn preview(
    state: State<'_, AppState>,
    item_id: u64,
    mode: Mode,
    settings: JobSettings,
    view_w: u32,
    view_h: u32,
) -> Result<PreviewDto, Msg> {
    let entry = state
        .queue()?
        .entry(item_id)
        .ok_or_else(|| Msg::new(keys::UNKNOWN_ITEM))?;
    let tools = state.tools()?;
    let work_dir = state.storage.work_dir.clone();
    let p = spawn_blocking(move || {
        make_preview(&entry, mode, &settings, view_w, view_h, &tools, &work_dir)
    })
    .await
    .map_err(|_| Msg::new(keys::ERROR_INTERNAL))??;
    Ok(PreviewDto {
        before: data_url(&p.before_png),
        after: data_url(&p.after_png),
        lossless: p.lossless,
    })
}

#[tauri::command]
pub async fn thumbnail(state: State<'_, AppState>, item_id: u64) -> Result<String, Msg> {
    let entry = state
        .queue()?
        .entry(item_id)
        .ok_or_else(|| Msg::new(keys::UNKNOWN_ITEM))?;
    let tools = state.tools()?;
    let work_dir = state.storage.work_dir.clone();
    let png = spawn_blocking(move || make_thumbnail(&entry, THUMBNAIL_SIZE, &tools, &work_dir))
        .await
        .map_err(|_| Msg::new(keys::ERROR_INTERNAL))??;
    Ok(data_url(&png))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    read_only: bool,
}

/// Whether the data folder is read-only, so the UI can say that settings are
/// not saved.
#[tauri::command]
pub fn storage_info(state: State<'_, AppState>) -> StorageInfo {
    StorageInfo {
        read_only: state.storage.read_only,
    }
}

/// The Windows display language as a BCP-47 tag (`de-DE`), or `None` if
/// Windows does not tell.
#[tauri::command]
pub fn system_locale() -> Option<String> {
    // Longest locale name Windows returns, including the terminating NUL.
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    let mut name = [0u16; LOCALE_NAME_MAX_LENGTH];
    // SAFETY: plain Win32 calls; `name` is a valid buffer of the length passed.
    let len = unsafe {
        let ui_language = GetUserDefaultUILanguage();
        LCIDToLocaleName(
            u32::from(ui_language),
            Some(&mut name),
            LOCALE_ALLOW_NEUTRAL_NAMES,
        )
    };
    // `len` counts the NUL; 0 means failure.
    let len = usize::try_from(len).ok()?.checked_sub(1)?;
    let tag = String::from_utf16(name.get(..len)?).ok()?;
    (!tag.is_empty()).then_some(tag)
}

#[tauri::command]
pub fn load_settings(state: State<'_, AppState>) -> Option<Value> {
    settings_store::load(&state.storage.data_dir)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, value: Value) -> Result<(), Msg> {
    settings_store::save(&state.storage, &value)
}

/// True if `file` lies inside `logs_dir`, compared by canonical path (so `..`
/// and links cannot escape). A path that does not exist is not allowed.
fn log_path_allowed(logs_dir: &Path, file: &Path) -> bool {
    match (logs_dir.canonicalize(), file.canonicalize()) {
        (Ok(dir), Ok(file)) => file.starts_with(dir),
        _ => false,
    }
}

/// True if `file` lies in the active log folder or in the temp log folder a
/// run log falls back to; nothing else may be opened.
fn log_file_allowed(storage: &Storage, file: &Path) -> bool {
    log_path_allowed(&storage.logs_dir, file) || log_path_allowed(&storage.temp_logs_dir, file)
}

/// Opens a run log (in the log folder, or its temp fallback) with the default
/// program.
#[tauri::command]
pub fn open_log(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), Msg> {
    let file = PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| Msg::new(keys::OUTSIDE_LOG_DIR))?;
    if !log_file_allowed(&state.storage, &file) || !file.is_file() {
        return Err(Msg::new(keys::OUTSIDE_LOG_DIR));
    }
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|_| Msg::new(keys::OPEN_LOG))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::resolve;
    use std::fs;

    #[test]
    fn log_path_allowed_for_a_file_in_the_log_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let s = resolve(tmp.path(), &tmp.path().join("temp"), 1, &|_| true);
        fs::create_dir_all(&s.logs_dir).unwrap();
        let log = s.logs_dir.join("2026-10-17_12-00-00.txt");
        fs::write(&log, "log").unwrap();
        assert!(log_path_allowed(&s.logs_dir, &log));
    }

    #[test]
    fn log_path_allowed_rejects_paths_outside() {
        let tmp = tempfile::tempdir().unwrap();
        let s = resolve(tmp.path(), &tmp.path().join("temp"), 1, &|_| true);
        fs::create_dir_all(&s.logs_dir).unwrap();
        fs::write(s.data_dir.join("settings.json"), "{}").unwrap();

        // `..` out of the log folder, into the data folder next to it.
        let escape = s.logs_dir.join("..").join("settings.json");
        assert!(escape.is_file());
        assert!(!log_path_allowed(&s.logs_dir, &escape));

        // A sibling folder whose name merely starts like the log folder.
        let sibling = s.data_dir.join("logs-old");
        fs::create_dir_all(&sibling).unwrap();
        fs::write(sibling.join("a.txt"), "x").unwrap();
        assert!(!log_path_allowed(&s.logs_dir, &sibling.join("a.txt")));

        // Somewhere else entirely.
        let other = tmp.path().join("other.txt");
        fs::write(&other, "x").unwrap();
        assert!(!log_path_allowed(&s.logs_dir, &other));
    }

    #[test]
    fn log_path_allowed_rejects_missing_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let logs = tmp.path().join("logs");
        assert!(
            !log_path_allowed(&logs, &logs.join("a.txt")),
            "no log folder"
        );
        fs::create_dir_all(&logs).unwrap();
        assert!(
            !log_path_allowed(&logs, &logs.join("a.txt")),
            "no such file"
        );
    }

    #[test]
    fn log_path_allowed_in_read_only_mode_uses_the_temp_log_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let s = resolve(tmp.path(), &tmp.path().join("temp"), 1, &|_| false);
        assert!(s.read_only);
        assert_eq!(
            s.logs_dir,
            tmp.path().join("temp").join("jpeg-xl-konverter").join("logs")
        );
        fs::create_dir_all(&s.logs_dir).unwrap();
        let log = s.logs_dir.join("run.txt");
        fs::write(&log, "log").unwrap();
        assert!(log_path_allowed(&s.logs_dir, &log));

        // The (unwritable) data folder is still off limits.
        fs::create_dir_all(&s.data_dir).unwrap();
        let settings = s.data_dir.join("settings.json");
        fs::write(&settings, "{}").unwrap();
        assert!(!log_path_allowed(&s.logs_dir, &settings));
    }

    #[test]
    fn log_file_allowed_accepts_the_active_and_the_fallback_log_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let s = resolve(tmp.path(), &tmp.path().join("temp"), 1, &|_| true);
        assert_ne!(s.logs_dir, s.temp_logs_dir, "portable mode has two folders");
        fs::create_dir_all(&s.logs_dir).unwrap();
        fs::create_dir_all(&s.temp_logs_dir).unwrap();
        let primary = s.logs_dir.join("a.txt");
        let fallback = s.temp_logs_dir.join("b.txt");
        fs::write(&primary, "x").unwrap();
        fs::write(&fallback, "x").unwrap();

        assert!(log_file_allowed(&s, &primary));
        assert!(log_file_allowed(&s, &fallback));
    }

    #[test]
    fn log_file_allowed_still_rejects_everything_else() {
        let tmp = tempfile::tempdir().unwrap();
        let s = resolve(tmp.path(), &tmp.path().join("temp"), 1, &|_| true);
        fs::create_dir_all(&s.logs_dir).unwrap();
        fs::create_dir_all(&s.temp_logs_dir).unwrap();
        fs::write(s.data_dir.join("settings.json"), "{}").unwrap();

        // `..` out of either log folder.
        let up_from_primary = s.logs_dir.join("..").join("settings.json");
        assert!(up_from_primary.is_file());
        assert!(!log_file_allowed(&s, &up_from_primary));
        let work_file = s.work_dir.join("scratch.txt");
        fs::create_dir_all(&s.work_dir).unwrap();
        fs::write(&work_file, "x").unwrap();
        let up_from_fallback = s.temp_logs_dir.join("..").join("1").join("scratch.txt");
        assert!(up_from_fallback.is_file());
        assert!(!log_file_allowed(&s, &up_from_fallback));

        // Somewhere else entirely, and files that do not exist.
        let other = tmp.path().join("other.txt");
        fs::write(&other, "x").unwrap();
        assert!(!log_file_allowed(&s, &other));
        assert!(!log_file_allowed(&s, &s.temp_logs_dir.join("nicht-da.txt")));
    }

    #[test]
    fn storage_info_serializes_camel_case() {
        let info = StorageInfo { read_only: true };
        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"readOnly":true}"#
        );
    }

    #[test]
    fn system_locale_is_a_bcp47_tag() {
        let tag = system_locale().expect("Windows reports a display language");
        assert!(
            tag.len() >= 2
                && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && tag.chars().next().unwrap().is_ascii_alphabetic(),
            "unexpected locale tag: {tag:?}"
        );
    }
}
