//! Collects the files to convert from dropped files and folders.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use jxl_oxide::{InitializeResult, JxlImage};
use walkdir::WalkDir;

use super::formats::kind_from_path;
use super::types::{InputKind, Mode, ScanEntry};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanResult {
    pub entries: Vec<ScanEntry>,
    /// Files and paths that were not added: unsupported or wrong format for
    /// the mode, or not readable.
    pub rejected: usize,
}

/// Collects convertible files from `paths` (files, or folders searched
/// recursively).
///
/// `Enc` accepts every supported format except JXL, `Dec` only JXL.
/// Entries from a folder carry that folder as `root`, single files have none.
/// A path that appears more than once is handled once (the first occurrence in
/// `paths` wins, duplicates are neither listed nor counted as rejected).
/// The result is sorted by path. Width and height come from the file header
/// and are 0 when it cannot be read; the entry stays.
pub fn scan_paths(paths: &[PathBuf], mode: Mode) -> ScanResult {
    let mut scan = Scan {
        mode,
        seen: HashSet::new(),
        result: ScanResult::default(),
    };
    for path in paths {
        if path.is_dir() {
            for item in WalkDir::new(path) {
                match item {
                    Ok(item) if item.file_type().is_dir() => {}
                    Ok(item) => scan.add_file(item.path(), Some(path)),
                    Err(_) => scan.result.rejected += 1,
                }
            }
        } else {
            scan.add_file(path, None);
        }
    }
    scan.result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    scan.result
}

struct Scan {
    mode: Mode,
    /// Identity of every file handled so far, to skip repeated paths.
    seen: HashSet<PathBuf>,
    result: ScanResult,
}

impl Scan {
    fn add_file(&mut self, path: &Path, root: Option<&Path>) {
        if !self.seen.insert(identity(path)) {
            return;
        }
        let entry = kind_from_path(path)
            .filter(|kind| (*kind == InputKind::Jxl) == (self.mode == Mode::Dec))
            .zip(fs::metadata(path).ok().filter(fs::Metadata::is_file));
        let Some((kind, meta)) = entry else {
            self.result.rejected += 1;
            return;
        };
        let (width, height) = dimensions(path, kind).unwrap_or((0, 0));
        self.result.entries.push(ScanEntry {
            path: path.to_path_buf(),
            root: root.map(Path::to_path_buf),
            kind,
            size: meta.len(),
            width,
            height,
        });
    }
}

/// The canonical path (resolves `..`, links and letter case), or just the
/// absolute path when the file does not exist.
fn identity(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn dimensions(path: &Path, kind: InputKind) -> Option<(u32, u32)> {
    if kind == InputKind::Jxl {
        return jxl_dimensions(path);
    }
    let size = imagesize::size(path).ok()?;
    Some((
        u32::try_from(size.width).ok()?,
        u32::try_from(size.height).ok()?,
    ))
}

/// Reads just enough of a JXL file to parse its image header; nothing is
/// decoded. `None` when the header is missing, broken or truncated.
fn jxl_dimensions(path: &Path) -> Option<(u32, u32)> {
    let mut file = fs::File::open(path).ok()?;
    let mut uninit = JxlImage::builder().build_uninit();
    let mut buf = vec![0u8; 4096];
    let mut valid = 0;
    loop {
        let count = file.read(&mut buf[valid..]).ok()?;
        if count == 0 {
            return None;
        }
        valid += count;
        let consumed = uninit.feed_bytes(&buf[..valid]).ok()?;
        buf.copy_within(consumed..valid, 0);
        valid -= consumed;
        match uninit.try_init().ok()? {
            InitializeResult::Initialized(image) => return Some((image.width(), image.height())),
            InitializeResult::NeedMoreData(more) => uninit = more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::InputKind;
    use std::fs;
    use std::path::Path;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"not really an image").unwrap();
    }

    fn sample_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.jpg"));
        touch(&dir.path().join("sub").join("b.png"));
        touch(&dir.path().join("sub").join("c.jxl"));
        touch(&dir.path().join("notes.txt"));
        dir
    }

    fn names(result: &ScanResult) -> Vec<String> {
        result
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn scan_recurses_and_rejects() {
        let dir = sample_tree();
        let paths = [dir.path().to_path_buf()];

        let enc = scan_paths(&paths, Mode::Enc);
        assert_eq!(enc.entries.len(), 2);
        assert_eq!(enc.rejected, 2);
        assert_eq!(names(&enc), ["a.jpg", "b.png"]);
        assert!(
            enc.entries
                .iter()
                .all(|e| e.root.as_deref() == Some(dir.path()))
        );
        assert_eq!(enc.entries[0].kind, InputKind::Jpeg);
        assert_eq!(enc.entries[1].kind, InputKind::Png);

        let dec = scan_paths(&paths, Mode::Dec);
        assert_eq!(dec.entries.len(), 1);
        assert_eq!(dec.rejected, 3);
        assert_eq!(names(&dec), ["c.jxl"]);
        assert_eq!(dec.entries[0].kind, InputKind::Jxl);
    }

    #[test]
    fn single_file_has_no_root() {
        let dir = sample_tree();
        let file = dir.path().join("a.jpg");

        let result = scan_paths(std::slice::from_ref(&file), Mode::Enc);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].path, file);
        assert_eq!(result.entries[0].root, None);
        assert_eq!(result.entries[0].size, fs::metadata(&file).unwrap().len());
    }

    #[test]
    fn dropped_files_of_the_wrong_kind_are_rejected() {
        let dir = sample_tree();
        let jxl = dir.path().join("sub").join("c.jxl");
        let jpg = dir.path().join("a.jpg");

        let enc = scan_paths(&[jxl.clone(), jpg.clone()], Mode::Enc);
        assert_eq!(names(&enc), ["a.jpg"]);
        assert_eq!(enc.rejected, 1);

        let dec = scan_paths(&[jxl, jpg], Mode::Dec);
        assert_eq!(names(&dec), ["c.jxl"]);
        assert_eq!(dec.rejected, 1);
    }

    #[test]
    fn missing_path_is_rejected_without_aborting() {
        let dir = sample_tree();
        let missing = dir.path().join("gone.jpg");

        let result = scan_paths(&[missing, dir.path().join("a.jpg")], Mode::Enc);
        assert_eq!(names(&result), ["a.jpg"]);
        assert_eq!(result.rejected, 1);
    }

    #[test]
    fn empty_input_gives_empty_result() {
        assert_eq!(scan_paths(&[], Mode::Enc), ScanResult::default());
    }

    #[test]
    fn repeated_paths_are_listed_once() {
        let dir = sample_tree();
        let folder = dir.path().to_path_buf();
        let file = dir.path().join("a.jpg");

        // Same folder twice, plus a file from inside it, plus the file twice.
        let result = scan_paths(
            &[folder.clone(), folder, file.clone(), file.clone(), file],
            Mode::Enc,
        );
        assert_eq!(names(&result), ["a.jpg", "b.png"]);
        // Duplicates are not "rejected"; only notes.txt and c.jxl are.
        assert_eq!(result.rejected, 2);
        // The first occurrence wins, so the folder's root is kept.
        assert_eq!(result.entries[0].root.as_deref(), Some(dir.path()));
    }

    #[test]
    fn same_file_with_different_spelling_is_listed_once() {
        let dir = sample_tree();
        let plain = dir.path().join("a.jpg");
        let dotted = dir.path().join("sub").join("..").join("a.jpg");

        let result = scan_paths(&[plain, dotted], Mode::Enc);
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn entries_are_sorted_by_path() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["z.png", "m/b.png", "a.png", "m/a.png"] {
            touch(&dir.path().join(name));
        }
        // Given in reverse order on purpose.
        let paths = [dir.path().join("z.png"), dir.path().to_path_buf()];

        let result = scan_paths(&paths, Mode::Enc);
        let listed: Vec<_> = result.entries.iter().map(|e| e.path.clone()).collect();
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
        assert_eq!(listed.len(), 4);
    }

    #[test]
    fn dimensions_come_from_the_header() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("p.png");
        image::RgbImage::new(7, 3).save(&png).unwrap();

        let result = scan_paths(std::slice::from_ref(&png), Mode::Enc);
        assert_eq!(result.entries.len(), 1);
        let e = &result.entries[0];
        assert_eq!((e.width, e.height), (7, 3));
        assert_eq!(e.size, fs::metadata(&png).unwrap().len());
    }

    #[test]
    fn unreadable_dimensions_become_zero_but_the_file_stays() {
        let dir = sample_tree();
        let result = scan_paths(
            &[
                dir.path().join("a.jpg"),
                dir.path().join("sub").join("c.jxl"),
            ],
            Mode::Enc,
        );
        // c.jxl is rejected in Enc; a.jpg is garbage but stays with 0 x 0.
        assert_eq!(result.entries.len(), 1);
        assert_eq!((result.entries[0].width, result.entries[0].height), (0, 0));

        let dec = scan_paths(&[dir.path().join("sub").join("c.jxl")], Mode::Dec);
        assert_eq!(dec.entries.len(), 1);
        assert_eq!((dec.entries[0].width, dec.entries[0].height), (0, 0));
    }
}
