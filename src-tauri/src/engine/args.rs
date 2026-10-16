//! Maps job settings to cjxl/djxl command line options (spellings verified
//! against libjxl v0.12.0, see docs/libjxl-notes.md).

use super::types::{EncodeSettings, InputKind};

/// Worker threads per cjxl/djxl process.
pub const TOOL_THREADS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DjxlTarget {
    ReconstructJpeg,
    Png { reduce_to_8bit: bool },
    PixelsToJpeg { quality: u8 },
}

/// Options for `cjxl INPUT OUTPUT [OPTIONS...]`.
///
/// JPEG transcoding ignores the distance (cjxl rejects a non-zero distance
/// together with `--lossless_jpeg=1`) and always keeps metadata, because
/// stripping would make byte-exact JPEG reconstruction impossible.
pub fn cjxl_args(kind: InputKind, enc: &EncodeSettings, keep_metadata: bool) -> Vec<String> {
    let transcode = kind == InputKind::Jpeg && enc.transcode_jpeg;
    let mut a = Vec::new();
    if kind == InputKind::Jpeg {
        a.push(format!("--lossless_jpeg={}", u8::from(transcode)));
    }
    if !transcode {
        a.push("-d".to_string());
        a.push(format!("{}", enc.distance));
    }
    a.push("-e".to_string());
    a.push(enc.effort.to_string());
    if !keep_metadata && !transcode {
        a.extend(["-x", "strip=exif", "-x", "strip=xmp"].map(String::from));
    }
    a.push(format!("--num_threads={TOOL_THREADS}"));
    a
}

/// Options for `djxl INPUT OUTPUT [OPTIONS...]`. The output format is always
/// given explicitly, because djxl otherwise picks it from the OUTPUT
/// extension and fails on `.tmp` files (Ruling R9, docs/libjxl-notes.md).
pub fn djxl_args(target: DjxlTarget) -> Vec<String> {
    let mut a = match target {
        DjxlTarget::ReconstructJpeg => vec!["--reconstruct_jpeg".to_string()],
        DjxlTarget::PixelsToJpeg { quality } => {
            vec![
                "--pixels_to_jpeg".to_string(),
                format!("--jpeg_quality={quality}"),
            ]
        }
        DjxlTarget::Png {
            reduce_to_8bit: true,
        } => vec!["--bits_per_sample=8".to_string()],
        DjxlTarget::Png {
            reduce_to_8bit: false,
        } => Vec::new(),
    };
    let format = match target {
        DjxlTarget::ReconstructJpeg | DjxlTarget::PixelsToJpeg { .. } => "jpg",
        DjxlTarget::Png { .. } => "png",
    };
    a.push(format!("--output_format={format}"));
    a.push(format!("--num_threads={TOOL_THREADS}"));
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(distance: f32, effort: u8, transcode_jpeg: bool) -> EncodeSettings {
        EncodeSettings {
            distance,
            effort,
            transcode_jpeg,
        }
    }

    fn has_pair(a: &[String], first: &str, second: &str) -> bool {
        a.windows(2).any(|w| w == [first, second])
    }

    #[test]
    fn jpeg_transcode_has_no_distance() {
        let a = cjxl_args(InputKind::Jpeg, &enc(1.2, 7, true), true);
        assert!(a.contains(&"--lossless_jpeg=1".into()));
        assert!(!a.contains(&"-d".into()));
        assert!(a.windows(2).any(|w| w == ["-e", "7"]));
        assert!(a.contains(&"--num_threads=2".into()));
    }

    #[test]
    fn jpeg_without_transcode_uses_distance() {
        let a = cjxl_args(InputKind::Jpeg, &enc(2.6, 8, false), true);
        assert!(a.contains(&"--lossless_jpeg=0".into()));
        assert!(has_pair(&a, "-d", "2.6"));
        assert!(has_pair(&a, "-e", "8"));
    }

    #[test]
    fn png_lossless_is_distance_zero() {
        let a = cjxl_args(InputKind::Png, &enc(0.0, 7, true), true);
        assert!(has_pair(&a, "-d", "0"));
    }

    #[test]
    fn distance_is_printed_plainly() {
        for (d, s) in [(0.5, "0.5"), (1.2, "1.2"), (2.6, "2.6")] {
            assert!(has_pair(
                &cjxl_args(InputKind::Png, &enc(d, 7, true), true),
                "-d",
                s
            ));
        }
    }

    #[test]
    fn strip_only_when_not_transcoding() {
        let png = cjxl_args(InputKind::Png, &enc(0.0, 7, true), false);
        assert!(has_pair(&png, "-x", "strip=exif"));
        assert!(has_pair(&png, "-x", "strip=xmp"));

        let jpeg = cjxl_args(InputKind::Jpeg, &enc(0.0, 7, true), false);
        assert!(!jpeg.iter().any(|s| s.starts_with("strip=")));

        let kept = cjxl_args(InputKind::Png, &enc(0.0, 7, true), true);
        assert!(!kept.iter().any(|s| s.starts_with("strip=")));
    }

    #[test]
    fn djxl_targets() {
        assert_eq!(
            djxl_args(DjxlTarget::ReconstructJpeg),
            vec![
                "--reconstruct_jpeg",
                "--output_format=jpg",
                "--num_threads=2"
            ]
        );
        assert_eq!(
            djxl_args(DjxlTarget::PixelsToJpeg { quality: 92 }),
            vec![
                "--pixels_to_jpeg",
                "--jpeg_quality=92",
                "--output_format=jpg",
                "--num_threads=2"
            ]
        );
        assert_eq!(
            djxl_args(DjxlTarget::Png {
                reduce_to_8bit: true
            }),
            vec![
                "--bits_per_sample=8",
                "--output_format=png",
                "--num_threads=2"
            ]
        );
        assert_eq!(
            djxl_args(DjxlTarget::Png {
                reduce_to_8bit: false
            }),
            vec!["--output_format=png", "--num_threads=2"]
        );
    }
}
