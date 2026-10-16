//! Locates, checks and runs cjxl.exe / djxl.exe.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use super::types::{CancelToken, ToolStatus};

/// Process creation flag: no console window for the child.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub cjxl: PathBuf,
    pub djxl: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub cancelled: bool,
}

impl Tools {
    /// Looks in `exe_dir` first (portable app: `cjxl.exe`), then in the
    /// development sidecar folder `src-tauri/binaries`. Both tools must come
    /// from the same folder.
    pub fn locate(exe_dir: &Path) -> Option<Tools> {
        let dev_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        [exe_dir, dev_dir.as_path()].into_iter().find_map(|dir| {
            Some(Tools {
                cjxl: find_tool(dir, "cjxl")?,
                djxl: find_tool(dir, "djxl")?,
            })
        })
    }

    pub fn check(&self) -> ToolStatus {
        match (tool_version(&self.cjxl), tool_version(&self.djxl)) {
            (Ok(c), Ok(d)) => ToolStatus {
                ok: true,
                cjxl_version: Some(c),
                djxl_version: Some(d),
                message: None,
            },
            (c, d) => {
                let message = [&c, &d]
                    .into_iter()
                    .filter_map(|r| r.as_ref().err().cloned())
                    .collect::<Vec<_>>();
                ToolStatus {
                    ok: false,
                    cjxl_version: c.ok(),
                    djxl_version: d.ok(),
                    message: Some(message.join(" ")),
                }
            }
        }
    }

    pub fn run_cjxl(
        &self,
        input: &Path,
        output: &Path,
        args: &[String],
        cancel: &CancelToken,
    ) -> Result<(), ToolError> {
        run_tool(&self.cjxl, input, output, args, cancel)
    }

    pub fn run_djxl(
        &self,
        input: &Path,
        output: &Path,
        args: &[String],
        cancel: &CancelToken,
    ) -> Result<(), ToolError> {
        run_tool(&self.djxl, input, output, args, cancel)
    }
}

fn find_tool(dir: &Path, name: &str) -> Option<PathBuf> {
    [
        format!("{name}.exe"),
        format!("{name}-x86_64-pc-windows-msvc.exe"),
    ]
    .into_iter()
    .map(|file| dir.join(file))
    .find(|p| p.is_file())
}

/// Runs `exe --version` and returns the version number, e.g. `0.12.0`
/// (first line of the output is `cjxl v0.12.0 4128790 [...]`).
fn tool_version(exe: &Path) -> Result<String, String> {
    let name = exe.file_name().unwrap_or_default().to_string_lossy();
    let out = Command::new(exe)
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("{name} startet nicht: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text.split_whitespace().find_map(|w| {
        w.strip_prefix('v')
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
    });
    match version {
        Some(v) if out.status.success() => Ok(v.to_string()),
        _ => Err(format!("{name} liefert keine Versionsangabe.")),
    }
}

/// cjxl/djxl are not long-path aware, so a path over 260 characters fails
/// unless it is given in extended-length form (`\\?\C:\…`, `\\?\UNC\server\…`),
/// which has no length limit. Umlauts and emoji work either way (the tools
/// run with the UTF-8 code page). See docs/libjxl-notes.md.
fn extended_length(path: &Path) -> PathBuf {
    let Ok(abs) = std::path::absolute(path) else {
        return path.to_path_buf();
    };
    let Some(s) = abs.to_str() else { return abs };
    if s.starts_with(r"\\?\") || s.starts_with(r"\\.\") {
        abs
    } else if let Some(unc) = s.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{unc}"))
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

/// Runs `exe INPUT OUTPUT ARGS...` without a console window, polls every
/// 50 ms and kills the process as soon as `cancel` is set.
fn run_tool(
    exe: &Path,
    input: &Path,
    output: &Path,
    args: &[String],
    cancel: &CancelToken,
) -> Result<(), ToolError> {
    let fail = |stderr: String| ToolError {
        exit_code: None,
        stderr,
        cancelled: false,
    };
    let mut child = Command::new(exe)
        .arg(extended_length(input))
        .arg(extended_length(output))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| fail(e.to_string()))?;

    // Read stderr on its own thread so a full pipe can never block the tool.
    let mut pipe = child.stderr.take().expect("stderr is piped");
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let stderr = move || reader.join().unwrap_or_default();

    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ToolError {
                exit_code: None,
                stderr: stderr(),
                cancelled: true,
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr = stderr();
                if status.success() {
                    return Ok(());
                }
                return Err(ToolError {
                    exit_code: status.code(),
                    stderr,
                    cancelled: false,
                });
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail(e.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Instant;

    use super::*;
    use crate::engine::args::{DjxlTarget, cjxl_args, djxl_args};
    use crate::engine::types::{EncodeSettings, InputKind};

    fn tools() -> Tools {
        let empty = tempfile::tempdir().unwrap();
        Tools::locate(empty.path()).expect("run scripts/fetch-libjxl.ps1 first")
    }

    fn lossless(effort: u8) -> Vec<String> {
        let enc = EncodeSettings {
            distance: 0.0,
            effort,
            transcode_jpeg: true,
        };
        cjxl_args(InputKind::Png, &enc, true)
    }

    /// Writes an RGB PNG filled with pseudo-random noise (hard to compress).
    fn write_noise_png(path: &Path, w: u32, h: u32) {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            let v = (x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77))
                .wrapping_mul(0xC2B2_AE3D);
            image::Rgb([v as u8, (v >> 8) as u8, (v >> 16) as u8])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn locate_finds_dev_binaries() {
        let empty = tempfile::tempdir().unwrap();
        let t = Tools::locate(empty.path()).expect("binaries under CARGO_MANIFEST_DIR/binaries");
        let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        assert_eq!(t.cjxl, dev.join("cjxl-x86_64-pc-windows-msvc.exe"));
        assert_eq!(t.djxl, dev.join("djxl-x86_64-pc-windows-msvc.exe"));
    }

    #[test]
    fn locate_prefers_exe_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("cjxl.exe"), b"").unwrap();
        fs::write(dir.path().join("djxl-x86_64-pc-windows-msvc.exe"), b"").unwrap();
        let t = Tools::locate(dir.path()).unwrap();
        assert_eq!(t.cjxl, dir.path().join("cjxl.exe"));
        assert_eq!(t.djxl, dir.path().join("djxl-x86_64-pc-windows-msvc.exe"));
    }

    #[test]
    fn check_reports_versions() {
        let s = tools().check();
        assert!(s.ok, "{s:?}");
        assert!(s.cjxl_version.as_deref().unwrap().contains("0.12"), "{s:?}");
        assert!(s.djxl_version.as_deref().unwrap().contains("0.12"), "{s:?}");
    }

    #[test]
    fn failure_reports_exit_code_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("kaputt.png");
        fs::write(&input, b"not an image").unwrap();
        let err = tools()
            .run_cjxl(
                &input,
                &dir.path().join("x.jxl"),
                &lossless(7),
                &CancelToken::new(),
            )
            .unwrap_err();
        assert_eq!(err.exit_code, Some(1));
        assert!(!err.stderr.is_empty());
        assert!(!err.cancelled);
    }

    #[test]
    fn cancel_kills_running_tool() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("gross.png");
        let output = dir.path().join("gross.jxl");
        write_noise_png(&input, 2400, 1800);
        let t = tools();

        let cancel = CancelToken::new();
        let trigger = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            trigger.cancel();
        });
        let start = Instant::now();
        let err = t
            .run_cjxl(&input, &output, &lossless(9), &cancel)
            .unwrap_err();

        assert!(err.cancelled, "{err:?}");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        );
        assert!(!output.exists());
    }

    fn decoded_dims(path: &Path, format: image::ImageFormat) -> (u32, u32) {
        let img = image::load_from_memory_with_format(&fs::read(path).unwrap(), format).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn djxl_writes_to_tmp_paths() {
        let dir = tempfile::tempdir().unwrap();
        let jpg = dir.path().join("foto.jpg");
        let jxl = dir.path().join("foto.jxl");
        image::RgbImage::from_fn(64, 48, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 5) as u8, 128])
        })
        .save(&jpg)
        .unwrap();
        let t = tools();
        let none = CancelToken::new();
        let transcode = EncodeSettings {
            distance: 0.0,
            effort: 7,
            transcode_jpeg: true,
        };
        t.run_cjxl(
            &jpg,
            &jxl,
            &cjxl_args(InputKind::Jpeg, &transcode, true),
            &none,
        )
        .unwrap();

        let rec = dir.path().join("x.jpg.tmp");
        t.run_djxl(&jxl, &rec, &djxl_args(DjxlTarget::ReconstructJpeg), &none)
            .unwrap();
        assert_eq!(fs::read(&rec).unwrap(), fs::read(&jpg).unwrap());

        let pixels = dir.path().join("pixels.jpg.tmp");
        let to_jpeg = djxl_args(DjxlTarget::PixelsToJpeg { quality: 92 });
        t.run_djxl(&jxl, &pixels, &to_jpeg, &none).unwrap();
        assert_eq!(decoded_dims(&pixels, image::ImageFormat::Jpeg), (64, 48));

        let png = dir.path().join("x.png.tmp");
        let to_png = djxl_args(DjxlTarget::Png {
            reduce_to_8bit: false,
        });
        t.run_djxl(&jxl, &png, &to_png, &none).unwrap();
        assert_eq!(decoded_dims(&png, image::ImageFormat::Png), (64, 48));
    }

    #[test]
    fn paths_are_passed_in_extended_length_form() {
        assert_eq!(
            extended_length(Path::new(r"C:\a\..\b.png")),
            PathBuf::from(r"\\?\C:\b.png")
        );
        assert_eq!(
            extended_length(Path::new(r"\\nas\fotos\a.jpg")),
            PathBuf::from(r"\\?\UNC\nas\fotos\a.jpg")
        );
        assert_eq!(
            extended_length(Path::new(r"\\?\C:\x.png")),
            PathBuf::from(r"\\?\C:\x.png")
        );
    }

    #[test]
    fn unicode_and_long_paths_work() {
        let root = tempfile::tempdir().unwrap();
        let unicode = root.path().join("Ünïcödé 📷");
        let long = (0..5).fold(root.path().to_path_buf(), |p, i| {
            p.join(format!(
                "{i}_ein_ziemlich_langer_ordnername_0123456789_0123456789"
            ))
        });
        assert!(long.join("bild 📷.png").to_str().unwrap().chars().count() > 260);
        let t = tools();

        for dir in [unicode, long] {
            fs::create_dir_all(&dir).unwrap();
            let png = dir.join("bild 📷.png");
            let jxl = dir.join("bild 📷.jxl");
            let back = dir.join("zurück.png");
            write_noise_png(&png, 64, 48);

            t.run_cjxl(&png, &jxl, &lossless(7), &CancelToken::new())
                .unwrap_or_else(|e| panic!("cjxl {}: {e:?}", png.display()));
            assert!(jxl.is_file(), "{}", jxl.display());
            t.run_djxl(
                &jxl,
                &back,
                &djxl_args(DjxlTarget::Png {
                    reduce_to_8bit: false,
                }),
                &CancelToken::new(),
            )
            .unwrap_or_else(|e| panic!("djxl {}: {e:?}", jxl.display()));
            assert!(back.is_file(), "{}", back.display());
        }
    }
}
