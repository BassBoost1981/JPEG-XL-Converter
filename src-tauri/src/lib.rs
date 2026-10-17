mod commands;
pub mod engine;
mod settings_store;
mod storage;
mod window_state;

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs, io, process, thread};

use tauri::{
    Manager, PhysicalPosition, PhysicalSize, RunEvent, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use serde_json::Value;

use commands::AppState;
use engine::i18n;
use engine::msg::keys;
use engine::tools::Tools;
use engine::types::Lang;
use window_state::WindowState;

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            let webview_dir = storage.webview_dir.clone();
            let window_file = storage.data_dir.join("window.json");
            let read_only = storage.read_only;
            // The title is built in the language the UI will show, so it does
            // not flicker when the frontend sets it a moment later.
            let title = i18n::key(
                startup_lang(
                    settings_store::load(&storage.data_dir).as_ref(),
                    commands::system_locale().as_deref(),
                ),
                keys::APP_TITLE,
            );
            let state = AppState::new(app.handle().clone(), Tools::locate(&exe_dir), storage);
            app.manage(state);

            // The window is built here, not from tauri.conf.json, so that its
            // WebView2 profile can live in the scratch folder instead of the
            // user profile.
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(title)
                .inner_size(1200.0, 760.0)
                .min_inner_size(1000.0, 640.0)
                .decorations(false)
                .visible(false)
                .data_directory(webview_dir)
                .build()?;
            restore_or_center(&window, window_state::load(&window_file));
            if !read_only {
                let closing = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { .. } = event {
                        save_window_state(&closing, &window_file);
                    }
                });
            }
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
            remove_work_dir(state.work_dir());
        }
    });
}

/// The language the UI starts in: the saved `lang` (`"de"` or `"en"`) from the
/// settings, otherwise the Windows display language (`system`, a tag such as
/// `de-DE`; German for `de`, `de-…` or `de_…` in any case, else English).
fn startup_lang(saved: Option<&Value>, system: Option<&str>) -> Lang {
    match saved.and_then(|s| s.get("lang")).and_then(Value::as_str) {
        Some("de") => Lang::De,
        Some("en") => Lang::En,
        _ => lang_from_locale(system),
    }
}

fn lang_from_locale(tag: Option<&str>) -> Lang {
    let tag = tag.unwrap_or_default().trim().to_ascii_lowercase();
    match tag.strip_prefix("de") {
        Some(rest) if rest.is_empty() || rest.starts_with(['-', '_']) => Lang::De,
        _ => Lang::En,
    }
}

/// Puts the (still hidden) window where it was, or in the middle of the
/// screen when that place is gone, and shows it.
///
/// The window is shown before it is resized: a window that was never shown
/// still has its frame measured wrong, so the new height would come out too
/// big (by the height of a title bar) and grow with every start.
fn restore_or_center(window: &WebviewWindow, saved: Option<WindowState>) {
    let monitors: Vec<_> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let (pos, size) = (m.position(), m.size());
            (pos.x, pos.y, size.width, size.height)
        })
        .collect();
    match saved.filter(|s| window_state::visible_on(s, &monitors)) {
        Some(s) => {
            let _ = window.set_position(PhysicalPosition::new(s.x, s.y));
            let _ = window.show();
            let _ = window.set_size(PhysicalSize::new(s.width, s.height));
        }
        None => {
            let _ = window.center();
            let _ = window.show();
        }
    }
    // A maximized window fits any screen. Windows reports its outer position
    // a few pixels above the screen, so a window that was first maximized
    // never passes `visible_on`; it must be maximized all the same.
    if saved.is_some_and(|s| s.maximized) {
        let _ = window.maximize();
    }
}

/// Stores the window's place in `file`. A minimized window has no place worth
/// keeping, so the old file stays. Errors are ignored: closing never waits on
/// this.
fn save_window_state(window: &WebviewWindow, file: &Path) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(pos), Ok(size), Ok(maximized)) = (
        window.outer_position(),
        window.inner_size(),
        window.is_maximized(),
    ) else {
        return;
    };
    let current = WindowState {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
        maximized,
    };
    let to_save = window_state::state_to_save(&current, window_state::load(file));
    let _ = window_state::save(file, &to_save);
}

/// Deletes the scratch folder. WebView2 lets go of its files a little after
/// the window is gone, so a failed attempt is repeated a few times.
fn remove_work_dir(dir: &Path) {
    const ATTEMPTS: u32 = 10;
    for attempt in 1..=ATTEMPTS {
        match fs::remove_dir_all(dir) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(_) if attempt < ATTEMPTS => thread::sleep(Duration::from_millis(200)),
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saved_language_wins_over_the_system_language() {
        let en = json!({ "lang": "en" });
        let de = json!({ "lang": "de" });
        assert_eq!(startup_lang(Some(&en), Some("de-DE")), Lang::En);
        assert_eq!(startup_lang(Some(&de), Some("en-US")), Lang::De);
        assert_eq!(startup_lang(Some(&de), None), Lang::De);
    }

    #[test]
    fn missing_or_invalid_saved_language_follows_the_system() {
        for saved in [
            None,
            Some(json!({})),
            Some(json!({ "lang": null })),
            Some(json!({ "lang": "fr" })),
            Some(json!({ "lang": "DE" })),
            Some(json!({ "lang": "" })),
            Some(json!({ "lang": 5 })),
            Some(json!("de")),
            Some(json!(["de"])),
        ] {
            assert_eq!(
                startup_lang(saved.as_ref(), Some("de-DE")),
                Lang::De,
                "{saved:?}"
            );
            assert_eq!(
                startup_lang(saved.as_ref(), Some("en-US")),
                Lang::En,
                "{saved:?}"
            );
            assert_eq!(startup_lang(saved.as_ref(), None), Lang::En, "{saved:?}");
        }
    }

    #[test]
    fn windows_language_tags_in_all_spellings() {
        for tag in ["de-DE", "de", "DE", "de_CH", "de-AT", "De-De", " de-DE "] {
            assert_eq!(startup_lang(None, Some(tag)), Lang::De, "{tag:?}");
        }
        for tag in ["en-US", "fr-FR", "dsb-DE", "den", "", "-", "d"] {
            assert_eq!(startup_lang(None, Some(tag)), Lang::En, "{tag:?}");
        }
        assert_eq!(startup_lang(None, None), Lang::En);
    }

    #[test]
    fn window_title_comes_from_the_catalog_in_that_language() {
        assert_eq!(i18n::key(Lang::De, keys::APP_TITLE), "JPEG XL Konverter");
        assert_eq!(i18n::key(Lang::En, keys::APP_TITLE), "JPEG XL Converter");
    }
}
