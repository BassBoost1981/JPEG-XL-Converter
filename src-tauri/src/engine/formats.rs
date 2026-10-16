//! Maps file extensions to the supported input formats.

use std::path::Path;

use super::types::InputKind;

/// Detects the input format from the file extension, ignoring case.
/// Returns `None` for everything else, including `.exr` (Ruling R8).
pub fn kind_from_path(p: &Path) -> Option<InputKind> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some(InputKind::Jpeg),
        "png" => Some(InputKind::Png),
        "gif" => Some(InputKind::Gif),
        "tif" | "tiff" => Some(InputKind::Tiff),
        "webp" => Some(InputKind::Webp),
        "bmp" => Some(InputKind::Bmp),
        "jxl" => Some(InputKind::Jxl),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_extensions_are_recognized() {
        assert_eq!(kind_from_path(Path::new("A.JPG")), Some(InputKind::Jpeg));
        assert_eq!(kind_from_path(Path::new("b.Jpeg")), Some(InputKind::Jpeg));
        assert_eq!(kind_from_path(Path::new("c.Tif")), Some(InputKind::Tiff));
        assert_eq!(kind_from_path(Path::new("d.JXL")), Some(InputKind::Jxl));
        assert_eq!(kind_from_path(Path::new("e.txt")), None);
    }

    #[test]
    fn every_supported_extension_maps_to_its_kind() {
        let cases = [
            ("jpg", InputKind::Jpeg),
            ("jpeg", InputKind::Jpeg),
            ("png", InputKind::Png),
            ("gif", InputKind::Gif),
            ("tif", InputKind::Tiff),
            ("tiff", InputKind::Tiff),
            ("webp", InputKind::Webp),
            ("bmp", InputKind::Bmp),
            ("jxl", InputKind::Jxl),
        ];
        for (ext, kind) in cases {
            assert_eq!(
                kind_from_path(Path::new(&format!("x.{ext}"))),
                Some(kind),
                "{ext}"
            );
        }
    }

    #[test]
    fn exr_and_extensionless_files_are_rejected() {
        assert_eq!(kind_from_path(Path::new("scene.exr")), None);
        assert_eq!(kind_from_path(Path::new("scene.EXR")), None);
        assert_eq!(kind_from_path(Path::new("README")), None);
        assert_eq!(kind_from_path(Path::new("jpg")), None);
        assert_eq!(kind_from_path(Path::new(".png")), None);
    }

    #[test]
    fn only_the_last_extension_counts() {
        assert_eq!(kind_from_path(Path::new("foto.jpg.txt")), None);
        assert_eq!(
            kind_from_path(Path::new("foto.txt.jpg")),
            Some(InputKind::Jpeg)
        );
    }
}
