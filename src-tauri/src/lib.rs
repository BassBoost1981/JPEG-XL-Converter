mod commands;
pub mod engine;
mod settings_store;
mod storage;

use std::path::PathBuf;
use std::time::Duration;
use std::{env, fs, process};

use tauri::{Manager, RunEvent};

use commands::AppState;
use engine::tools::Tools;

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let exe_dir = env::current_exe()?
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default();
            let storage = storage::resolve(
                &exe_dir,
                &env::temp_dir(),
                process::id(),
                &storage::probe_writable,
            );
            fs::create_dir_all(&storage.work_dir)?;
            let state = AppState::new(app.handle().clone(), Tools::locate(&exe_dir), storage);
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
            commands::storage_info,
            commands::system_locale,
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
