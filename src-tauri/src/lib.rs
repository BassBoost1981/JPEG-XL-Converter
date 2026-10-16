mod commands;
pub mod engine;
mod settings_store;

use std::path::PathBuf;
use std::time::Duration;
use std::{env, fs, process};

use tauri::{Manager, RunEvent};

use commands::AppState;
use engine::tools::Tools;

/// `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle`.
fn log_dir() -> PathBuf {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    base.join("JPEG XL Konverter").join("Protokolle")
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let work_dir = env::temp_dir()
                .join("jpeg-xl-konverter")
                .join(process::id().to_string());
            fs::create_dir_all(&work_dir)?;
            let exe_dir = env::current_exe()?
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default();
            let state = AppState::new(
                app.handle().clone(),
                Tools::locate(&exe_dir),
                work_dir,
                log_dir(),
                app.path().app_config_dir()?,
            );
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_tools,
            commands::add_paths,
            commands::clear_queue,
            commands::start_run,
            commands::retry_failed,
            commands::pause_run,
            commands::resume_run,
            commands::cancel_run,
            commands::is_running,
            commands::preview,
            commands::thumbnail,
            commands::load_settings,
            commands::save_settings,
            commands::open_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event
            && let Some(state) = handle.try_state::<AppState>()
        {
            // Running tools write next to the user's photos; let the workers
            // kill them and clean up before the process ends.
            state.cancel_and_wait(Duration::from_secs(3));
            let _ = fs::remove_dir_all(state.work_dir());
        }
    });
}
