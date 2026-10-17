//! Thin Tauri commands: map errors to German strings and call the engine.

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

use crate::engine::originals::WindowsRecycleBin;
use crate::engine::preview::{make_preview, thumbnail as make_thumbnail};
use crate::engine::queue::{ProgressSink, Queue, RealProcessor, default_workers};
use crate::engine::scan::scan_paths;
use crate::engine::tools::Tools;
use crate::engine::types::{
    ItemEvent, JobSettings, Mode, PauseReason, QueueItem, RunSummary, ToolStatus,
};
use crate::settings_store;

pub const TOOLS_MISSING: &str = "cjxl.exe oder djxl.exe fehlt neben dem Programm.";
const THUMBNAIL_SIZE: u32 = 80;
const UNKNOWN_ITEM: &str = "Eintrag nicht gefunden";
const OUTSIDE_LOG_DIR: &str = "Nur Protokolldateien lassen sich öffnen";
const INTERNAL: &str = "Interner Fehler";

/// Shared app state, built once at startup.
pub struct AppState {
    tools: Option<Tools>,
    queue: Option<Queue>,
    work_dir: PathBuf,
    log_dir: PathBuf,
    config_dir: PathBuf,
}

impl AppState {
    pub fn new(
        app: AppHandle,
        tools: Option<Tools>,
        work_dir: PathBuf,
        log_dir: PathBuf,
        config_dir: PathBuf,
    ) -> Self {
        let queue = tools.clone().map(|tools| {
            let processor = RealProcessor {
                tools,
                bin: Arc::new(WindowsRecycleBin),
                work_dir: work_dir.clone(),
            };
            Queue::new(
                Arc::new(processor),
                Arc::new(EmitSink(app)),
                default_workers(),
                log_dir.clone(),
            )
        });
        AppState {
            tools,
            queue,
            work_dir,
            log_dir,
            config_dir,
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
        &self.work_dir
    }

    fn queue(&self) -> Result<&Queue, String> {
        self.queue.as_ref().ok_or_else(|| TOOLS_MISSING.to_string())
    }

    fn tools(&self) -> Result<Tools, String> {
        self.tools.clone().ok_or_else(|| TOOLS_MISSING.to_string())
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
            message: Some(TOOLS_MISSING.to_string()),
        },
    }
}

#[tauri::command]
pub async fn add_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
    mode: Mode,
) -> Result<AddResult, String> {
    let queue = state.queue()?;
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let scan = spawn_blocking(move || scan_paths(&paths, mode))
        .await
        .map_err(|_| INTERNAL.to_string())?;
    Ok(AddResult {
        items: queue.add(mode, scan.entries),
        rejected: scan.rejected,
    })
}

#[tauri::command]
pub fn clear_queue(state: State<'_, AppState>, mode: Mode) -> Result<(), String> {
    state.queue()?.clear(mode)
}

#[tauri::command]
pub fn start_run(
    state: State<'_, AppState>,
    mode: Mode,
    settings: JobSettings,
) -> Result<(), String> {
    state.queue()?.start(mode, settings);
    Ok(())
}

#[tauri::command]
pub fn retry_failed(
    state: State<'_, AppState>,
    mode: Mode,
    settings: JobSettings,
) -> Result<(), String> {
    state.queue()?.retry_failed(mode, settings);
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
) -> Result<PreviewDto, String> {
    let entry = state
        .queue()?
        .entry(item_id)
        .ok_or_else(|| UNKNOWN_ITEM.to_string())?;
    let tools = state.tools()?;
    let work_dir = state.work_dir.clone();
    let p = spawn_blocking(move || {
        make_preview(&entry, mode, &settings, view_w, view_h, &tools, &work_dir)
    })
    .await
    .map_err(|_| INTERNAL.to_string())??;
    Ok(PreviewDto {
        before: data_url(&p.before_png),
        after: data_url(&p.after_png),
        lossless: p.lossless,
    })
}

#[tauri::command]
pub async fn thumbnail(state: State<'_, AppState>, item_id: u64) -> Result<String, String> {
    let entry = state
        .queue()?
        .entry(item_id)
        .ok_or_else(|| UNKNOWN_ITEM.to_string())?;
    let tools = state.tools()?;
    let work_dir = state.work_dir.clone();
    let png = spawn_blocking(move || make_thumbnail(&entry, THUMBNAIL_SIZE, &tools, &work_dir))
        .await
        .map_err(|_| INTERNAL.to_string())??;
    Ok(data_url(&png))
}

#[tauri::command]
pub fn load_settings(state: State<'_, AppState>) -> Option<Value> {
    settings_store::load(&state.config_dir)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, value: Value) -> Result<(), String> {
    settings_store::save(&state.config_dir, &value)
        .map_err(|e| format!("Einstellungen lassen sich nicht speichern: {e}"))
}

/// Opens a file inside the log folder with the default program.
#[tauri::command]
pub fn open_log(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let dir = state
        .log_dir
        .canonicalize()
        .map_err(|_| OUTSIDE_LOG_DIR.to_string())?;
    let file = PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| OUTSIDE_LOG_DIR.to_string())?;
    if !file.starts_with(&dir) || !file.is_file() {
        return Err(OUTSIDE_LOG_DIR.to_string());
    }
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("Protokoll lässt sich nicht öffnen: {e}"))
}
