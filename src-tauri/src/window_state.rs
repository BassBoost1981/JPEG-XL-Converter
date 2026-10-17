//! Position, size and maximized state of the main window, kept in
//! `data\window.json` next to the exe. Pixels are physical pixels.

use std::io;
use std::path::Path;
use std::{cmp, fs};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    /// Outer position of the window.
    pub x: i32,
    pub y: i32,
    /// Inner size of the window.
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

/// The stored state; a missing or unreadable file gives `None`.
pub fn load(path: &Path) -> Option<WindowState> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the state; the folder must exist (a pulled stick is not rebuilt).
pub fn save(path: &Path, s: &WindowState) -> io::Result<()> {
    let text = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    fs::write(path, text)
}

/// Whether the top strip of the window (its title bar and a bit more) shows on
/// at least one monitor, so the user can still grab it. `monitors` are
/// `(x, y, width, height)`.
pub fn visible_on(s: &WindowState, monitors: &[(i32, i32, u32, u32)]) -> bool {
    /// The part of the title strip that has to show, in pixels (both ways).
    const MIN_SHOWN: i64 = 100;
    /// How far above a monitor's top edge the window may start and still count
    /// as sitting on it. A snapped window reports its outer position a few
    /// pixels outside the monitor (Windows' invisible border, e.g. y = -8).
    const TOP_OVERSHOOT: i64 = 16;
    let (left, right) = (i64::from(s.x), i64::from(s.x) + i64::from(s.width));
    monitors.iter().any(|&(mx, my, mw, mh)| {
        let (mx, my) = (i64::from(mx), i64::from(my));
        let mut top = i64::from(s.y);
        if (my - TOP_OVERSHOOT..my).contains(&top) {
            top = my;
        }
        let bottom = top + MIN_SHOWN;
        let shown_w = cmp::min(right, mx + i64::from(mw)) - cmp::max(left, mx);
        let shown_h = cmp::min(bottom, my + i64::from(mh)) - cmp::max(top, my);
        shown_w >= MIN_SHOWN && shown_h >= MIN_SHOWN
    })
}

/// What to store when the window closes. A maximized window keeps the
/// geometry of its last normal state (when one is stored), so that un-maximizing
/// after the next start puts it back where it was.
pub fn state_to_save(current: &WindowState, stored: Option<WindowState>) -> WindowState {
    match stored {
        Some(old) if current.maximized => WindowState {
            maximized: true,
            ..old
        },
        _ => *current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(x: i32, y: i32, width: u32, height: u32) -> WindowState {
        WindowState {
            x,
            y,
            width,
            height,
            maximized: false,
        }
    }

    const PRIMARY: (i32, i32, u32, u32) = (0, 0, 1920, 1080);
    /// A second monitor to the right of the primary one.
    const SECOND: (i32, i32, u32, u32) = (1920, 0, 1920, 1080);

    #[test]
    fn load_missing_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("window.json")), None);
        assert_eq!(load(&dir.path().join("nicht-da").join("window.json")), None);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("window.json");
        let s = WindowState {
            x: -1700,
            y: 40,
            width: 1200,
            height: 760,
            maximized: true,
        };
        save(&file, &s).unwrap();
        assert_eq!(load(&file), Some(s));

        let t = state(10, 20, 1000, 640);
        save(&file, &t).unwrap();
        assert_eq!(load(&file), Some(t), "saving again replaces the file");
    }

    #[test]
    fn load_corrupt_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("window.json");
        for text in [
            "",
            "{ kaputt",
            "null",
            r#"{"x":1,"y":2}"#,
            r#"{"x":"a","y":2,"width":3,"height":4,"maximized":false}"#,
            r#"{"x":1,"y":2,"width":-3,"height":4,"maximized":false}"#,
        ] {
            fs::write(&file, text).unwrap();
            assert_eq!(load(&file), None, "{text:?}");
        }
    }

    #[test]
    fn save_into_missing_folder_fails() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nicht-da").join("window.json");
        assert!(save(&file, &state(0, 0, 1200, 760)).is_err());
    }

    #[test]
    fn visible_on_primary() {
        assert!(visible_on(&state(100, 100, 1200, 760), &[PRIMARY]));
        assert!(visible_on(&state(0, 0, 1920, 1080), &[PRIMARY]));
    }

    #[test]
    fn visible_on_detached_monitor() {
        // Saved on a second monitor that is no longer connected.
        let s = state(2200, 100, 1200, 760);
        assert!(!visible_on(&s, &[PRIMARY]));
        assert!(visible_on(&s, &[PRIMARY, SECOND]));
        assert!(!visible_on(&s, &[]), "no monitors at all");
    }

    #[test]
    fn visible_on_monitor_left_of_the_primary_one() {
        let left = (-1920, 0, 1920, 1080);
        assert!(visible_on(&state(-1700, 40, 1200, 760), &[left, PRIMARY]));
        assert!(!visible_on(&state(-1700, 40, 1200, 760), &[PRIMARY]));
    }

    #[test]
    fn visible_on_needs_100_by_100_of_the_title_strip() {
        // 100 px of the width at the right edge of the monitor: just enough.
        assert!(visible_on(&state(1820, 0, 1200, 760), &[PRIMARY]));
        // 99 px: not enough.
        assert!(!visible_on(&state(1821, 0, 1200, 760), &[PRIMARY]));
        // Only the left 100 px of a window that starts left of the monitor.
        assert!(visible_on(&state(-1100, 0, 1200, 760), &[PRIMARY]));
        assert!(!visible_on(&state(-1101, 0, 1200, 760), &[PRIMARY]));
        // Title strip pushed under the bottom edge: only 99 px of it remain.
        assert!(visible_on(&state(0, 980, 1200, 760), &[PRIMARY]));
        assert!(!visible_on(&state(0, 981, 1200, 760), &[PRIMARY]));
        // Title strip well above the top edge.
        assert!(!visible_on(&state(0, -17, 1200, 760), &[PRIMARY]));
    }

    #[test]
    fn visible_on_edge_snapped_window_with_negative_border_overshoot() {
        // Windows reports the outer position of a snapped window a few pixels
        // outside the monitor (invisible border), e.g. y = -7 or -8.
        assert!(visible_on(&state(0, -8, 960, 1080), &[PRIMARY]));
        assert!(visible_on(&state(-8, -8, 960, 1080), &[PRIMARY]));
        assert!(visible_on(&state(0, -16, 1200, 760), &[PRIMARY]));
        // Same on a monitor that does not start at y = 0.
        assert!(visible_on(
            &state(100, 592, 1200, 760),
            &[(0, 600, 1920, 1080)]
        ));
        // A window genuinely off-screen above stays invisible.
        assert!(!visible_on(&state(0, -200, 1200, 760), &[PRIMARY]));
        assert!(!visible_on(&state(0, -100, 1200, 760), &[PRIMARY]));
        assert!(!visible_on(&state(0, -17, 1200, 760), &[PRIMARY]));
    }

    #[test]
    fn visible_on_extreme_values_do_not_overflow() {
        let s = WindowState {
            x: i32::MAX,
            y: i32::MAX,
            width: u32::MAX,
            height: u32::MAX,
            maximized: false,
        };
        assert!(!visible_on(&s, &[PRIMARY]));
        let t = WindowState {
            x: i32::MIN,
            y: i32::MIN,
            width: u32::MAX,
            height: u32::MAX,
            maximized: false,
        };
        assert!(!visible_on(&t, &[PRIMARY]));
    }

    #[test]
    fn normal_window_is_saved_as_it_is() {
        let current = state(50, 60, 1300, 800);
        let old = state(1, 2, 1000, 640);
        assert_eq!(state_to_save(&current, Some(old)), current);
        assert_eq!(state_to_save(&current, None), current);
    }

    #[test]
    fn maximized_window_keeps_the_stored_geometry() {
        let current = WindowState {
            x: -8,
            y: -8,
            width: 1936,
            height: 1096,
            maximized: true,
        };
        let old = state(50, 60, 1300, 800);
        let saved = state_to_save(&current, Some(old));
        assert_eq!(
            saved,
            WindowState {
                maximized: true,
                ..old
            }
        );
        // Without a stored state the current values are all there is.
        assert_eq!(state_to_save(&current, None), current);
    }
}
