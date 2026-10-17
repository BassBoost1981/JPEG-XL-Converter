//! Output and temporary paths for a converted file. Pure path computations;
//! nothing here touches the disk.

use std::path::{Path, PathBuf};

use super::types::{OutputLocation, ScanEntry};

/// Where the converted file goes, with the extension `ext` (without dot).
///
/// - `Next`: beside the original.
/// - `Folder`: below the chosen folder. For an entry that came from an added
///   folder the output mirrors the structure including that folder's own
///   name (`D:\Fotos\2024\Urlaub\a.jpg` added via `D:\Fotos\2024` ends up in
///   `<out>\2024\Urlaub\a.<ext>`; a drive root has no name to add). A single
///   file goes directly into the chosen folder.
pub fn target_path(entry: &ScanEntry, ext: &str, output: &OutputLocation) -> PathBuf {
    let target = match output {
        OutputLocation::Next => return entry.path.with_extension(ext),
        OutputLocation::Folder { path } => path,
    };
    let mirrored = entry.root.as_deref().and_then(|root| {
        let relative = entry.path.strip_prefix(root).ok()?;
        Some(match root.file_name() {
            Some(name) => Path::new(name).join(relative),
            None => relative.to_path_buf(),
        })
    });
    // Without a root (single file) only the file name is kept.
    let relative = mirrored.unwrap_or_else(|| entry.path.file_name().unwrap_or_default().into());
    target.join(relative).with_extension(ext)
}

/// The temporary file the converter writes to first: the target path with
/// `.tmp` appended (`foto.jxl` -> `foto.jxl.tmp`).
pub fn tmp_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::InputKind;

    fn entry(path: &str, root: Option<&str>) -> ScanEntry {
        ScanEntry {
            path: PathBuf::from(path),
            root: root.map(PathBuf::from),
            kind: InputKind::Jpeg,
            size: 0,
            width: 0,
            height: 0,
        }
    }

    fn folder(path: &str) -> OutputLocation {
        OutputLocation::Folder {
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn target_next_to_original() {
        let e = entry(r"D:\F\a.jpg", None);
        assert_eq!(
            target_path(&e, "jxl", &OutputLocation::Next),
            PathBuf::from(r"D:\F\a.jxl")
        );
    }

    #[test]
    fn target_next_to_original_ignores_root() {
        let e = entry(r"D:\F\sub\a.jpg", Some(r"D:\F"));
        assert_eq!(
            target_path(&e, "jxl", &OutputLocation::Next),
            PathBuf::from(r"D:\F\sub\a.jxl")
        );
    }

    #[test]
    fn target_mirrors_from_added_folder() {
        let e = entry(r"D:\Fotos\2024\Urlaub\a.jpg", Some(r"D:\Fotos\2024"));
        assert_eq!(
            target_path(&e, "jxl", &folder(r"E:\Out")),
            PathBuf::from(r"E:\Out\2024\Urlaub\a.jxl")
        );
        let single = entry(r"D:\Fotos\2024\Urlaub\a.jpg", None);
        assert_eq!(
            target_path(&single, "jxl", &folder(r"E:\Out")),
            PathBuf::from(r"E:\Out\a.jxl")
        );
    }

    #[test]
    fn file_directly_in_added_folder_goes_below_its_name() {
        let e = entry(r"D:\Fotos\2024\a.jpg", Some(r"D:\Fotos\2024"));
        assert_eq!(
            target_path(&e, "jxl", &folder(r"E:\Out")),
            PathBuf::from(r"E:\Out\2024\a.jxl")
        );
    }

    #[test]
    fn added_drive_root_has_no_name_to_mirror() {
        let e = entry(r"D:\Urlaub\a.jpg", Some(r"D:\"));
        assert_eq!(
            target_path(&e, "jxl", &folder(r"E:\Out")),
            PathBuf::from(r"E:\Out\Urlaub\a.jxl")
        );
    }

    #[test]
    fn only_the_last_extension_is_replaced() {
        let e = entry(r"D:\F\foto.v2.jpg", None);
        assert_eq!(
            target_path(&e, "jxl", &OutputLocation::Next),
            PathBuf::from(r"D:\F\foto.v2.jxl")
        );
        let back = ScanEntry {
            kind: InputKind::Jxl,
            ..entry(r"D:\F\foto.JXL", None)
        };
        assert_eq!(
            target_path(&back, "png", &folder(r"E:\Out")),
            PathBuf::from(r"E:\Out\foto.png")
        );
    }

    #[test]
    fn tmp_path_appends_tmp() {
        assert_eq!(
            tmp_path(Path::new(r"D:\F\foto.jxl")),
            PathBuf::from(r"D:\F\foto.jxl.tmp")
        );
    }
}
