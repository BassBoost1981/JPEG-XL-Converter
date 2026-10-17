//! Where the app keeps its files. Portable: everything it keeps lives in
//! `data\` next to the exe, scratch files in `%TEMP%\jpeg-xl-konverter\`.
//! Nothing goes into the user profile. If `data\` cannot be written (a
//! read-only stick), the app runs in read-only mode: nothing is saved and the
//! run logs go to the temp folder.

use std::path::{Path, PathBuf};
use std::{fs, process};

#[derive(Debug)]
pub struct Storage {
    /// `<exe folder>\data`: `settings.json`, `window.json`, `logs\`.
    pub data_dir: PathBuf,
    /// `data_dir` cannot be written; settings are not saved.
    pub read_only: bool,
    /// Run logs: `data\logs`, or `%TEMP%\jpeg-xl-konverter\logs` when read-only.
    pub logs_dir: PathBuf,
    /// `%TEMP%\jpeg-xl-konverter\logs`: where a run log goes when `logs_dir`
    /// cannot be written mid-session. The same folder as `logs_dir` when
    /// read-only.
    pub temp_logs_dir: PathBuf,
    /// Scratch folder of this process: `%TEMP%\jpeg-xl-konverter\<pid>`.
    pub work_dir: PathBuf,
    /// WebView2 profile folder (inside `work_dir`, so it vanishes on exit).
    pub webview_dir: PathBuf,
}

/// Decides the folders. `writable` tells whether a folder can be written
/// (`probe_writable` in the app, a stub in tests); it is asked about `data_dir`.
pub fn resolve(
    exe_dir: &Path,
    temp_dir: &Path,
    pid: u32,
    writable: &dyn Fn(&Path) -> bool,
) -> Storage {
    let data_dir = exe_dir.join("data");
    let temp_root = temp_dir.join("jpeg-xl-konverter");
    let work_dir = temp_root.join(pid.to_string());
    let read_only = !writable(&data_dir);
    let temp_logs_dir = temp_root.join("logs");
    let logs_dir = if read_only {
        temp_logs_dir.clone()
    } else {
        data_dir.join("logs")
    };
    Storage {
        data_dir,
        read_only,
        logs_dir,
        temp_logs_dir,
        webview_dir: work_dir.join("webview"),
        work_dir,
    }
}

/// Creates `dir` if needed, writes a small file into it and removes it again.
/// `false` on any failure.
pub fn probe_writable(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".write-test-{}", process::id()));
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (PathBuf, PathBuf) {
        (PathBuf::from("E:\\stick\\app"), PathBuf::from("C:\\temp"))
    }

    #[test]
    fn resolve_portable() {
        let (exe, temp) = dirs();
        let s = resolve(&exe, &temp, 4711, &|_| true);
        assert_eq!(s.data_dir, exe.join("data"));
        assert_eq!(s.logs_dir, exe.join("data").join("logs"));
        assert_eq!(
            s.temp_logs_dir,
            temp.join("jpeg-xl-konverter").join("logs"),
            "the fallback for logs is always the temp folder"
        );
        assert!(!s.read_only);
        assert_eq!(s.work_dir, temp.join("jpeg-xl-konverter").join("4711"));
        assert_eq!(s.webview_dir, s.work_dir.join("webview"));
    }

    #[test]
    fn resolve_read_only() {
        let (exe, temp) = dirs();
        let s = resolve(&exe, &temp, 4711, &|_| false);
        assert!(s.read_only);
        assert_eq!(s.data_dir, exe.join("data"));
        assert_eq!(s.logs_dir, temp.join("jpeg-xl-konverter").join("logs"));
        assert_eq!(s.temp_logs_dir, s.logs_dir);
        assert_eq!(s.work_dir, temp.join("jpeg-xl-konverter").join("4711"));
        assert_eq!(s.webview_dir, s.work_dir.join("webview"));
    }

    #[test]
    fn resolve_asks_about_the_data_folder() {
        let (exe, temp) = dirs();
        let asked = std::cell::RefCell::new(Vec::new());
        resolve(&exe, &temp, 1, &|p| {
            asked.borrow_mut().push(p.to_path_buf());
            true
        });
        assert_eq!(*asked.borrow(), vec![exe.join("data")]);
    }

    #[test]
    fn probe_writable_true_for_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("data");
        assert!(probe_writable(&dir));
        assert!(dir.is_dir(), "the folder is created on a writable stick");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "the probe file is removed again"
        );
    }

    #[test]
    fn probe_writable_false_when_data_is_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("data");
        std::fs::write(&dir, "i am a file").unwrap();
        assert!(!probe_writable(&dir));
    }
}
