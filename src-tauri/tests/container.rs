//! `has_jpeg_reconstruction` against real files written by cjxl.

mod common;

use std::fs;
use std::path::Path;

use common::{JpegOpts, PngKind};
use jpeg_xl_konverter_lib::engine::args::{DjxlTarget, cjxl_args, djxl_args};
use jpeg_xl_konverter_lib::engine::jxl_container::has_jpeg_reconstruction;
use jpeg_xl_konverter_lib::engine::types::{CancelToken, EncodeSettings, InputKind};
use jpeg_xl_konverter_lib::engine::verify::files_identical;

fn cjxl(input: &Path, output: &Path, args: &[String]) {
    common::tools()
        .run_cjxl(input, output, args, &CancelToken::new())
        .expect("cjxl failed");
}

/// `JXL ` signature box that opens every container.
const CONTAINER_START: [u8; 12] = [0, 0, 0, 12, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A];
/// Start of a JXL file without a container.
const CODESTREAM_START: [u8; 2] = [0xFF, 0x0A];

fn settings(transcode_jpeg: bool) -> EncodeSettings {
    EncodeSettings {
        distance: 1.2,
        effort: 3,
        transcode_jpeg,
    }
}

#[test]
fn repacked_jpeg_has_reconstruction_data() {
    let dir = tempfile::tempdir().unwrap();
    let variants = [
        ("baseline", JpegOpts::default()),
        (
            "progressive",
            JpegOpts {
                progressive: true,
                ..JpegOpts::default()
            },
        ),
        (
            "exif",
            JpegOpts {
                exif: Some(common::minimal_exif("TESTMARKER")),
                ..JpegOpts::default()
            },
        ),
    ];
    for (name, opts) in variants {
        let jpg = dir.path().join(format!("{name}.jpg"));
        let jxl = dir.path().join(format!("{name}.jxl"));
        common::write_jpeg(&jpg, 64, 48, opts);
        cjxl(
            &jpg,
            &jxl,
            &cjxl_args(InputKind::Jpeg, &settings(true), true),
        );

        assert!(has_jpeg_reconstruction(&jxl).unwrap(), "{name}");
    }
}

#[test]
fn repacking_with_default_cjxl_options_works_too() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("a.jpg");
    let jxl = dir.path().join("a.jxl");
    common::write_jpeg(&jpg, 64, 48, JpegOpts::default());
    cjxl(&jpg, &jxl, &[]);

    assert!(has_jpeg_reconstruction(&jxl).unwrap());
}

#[test]
fn reencoded_jpeg_has_no_reconstruction_data() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("a.jpg");
    let jxl = dir.path().join("a.jxl");
    common::write_jpeg(
        &jpg,
        64,
        48,
        JpegOpts {
            exif: Some(common::minimal_exif("TESTMARKER")),
            ..JpegOpts::default()
        },
    );
    cjxl(
        &jpg,
        &jxl,
        &cjxl_args(InputKind::Jpeg, &settings(false), true),
    );
    // A real container, just without the JPEG data.
    assert!(fs::read(&jxl).unwrap().starts_with(&CONTAINER_START));

    assert!(!has_jpeg_reconstruction(&jxl).unwrap());
}

#[test]
fn converted_png_has_no_reconstruction_data() {
    let dir = tempfile::tempdir().unwrap();
    for kind in [
        PngKind::Rgb8,
        PngKind::Rgba8,
        PngKind::Rgb16,
        PngKind::Gray8,
    ] {
        let png = dir.path().join(format!("{kind:?}.png"));
        let jxl = dir.path().join(format!("{kind:?}.jxl"));
        common::write_png(&png, kind);
        cjxl(
            &png,
            &jxl,
            &cjxl_args(InputKind::Png, &settings(true), true),
        );
        // cjxl writes a bare codestream here.
        assert!(
            fs::read(&jxl).unwrap().starts_with(&CODESTREAM_START),
            "{kind:?}"
        );

        assert!(!has_jpeg_reconstruction(&jxl).unwrap(), "{kind:?}");
    }
}

#[test]
fn restored_jpeg_is_byte_identical() {
    // The reader says "reconstructible"; djxl must then really give the JPEG back.
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("a.jpg");
    let jxl = dir.path().join("a.jxl");
    let back = dir.path().join("back.jpg");
    common::write_jpeg(&jpg, 64, 48, JpegOpts::default());
    cjxl(
        &jpg,
        &jxl,
        &cjxl_args(InputKind::Jpeg, &settings(true), true),
    );

    assert!(has_jpeg_reconstruction(&jxl).unwrap());
    common::tools()
        .run_djxl(
            &jxl,
            &back,
            &djxl_args(DjxlTarget::ReconstructJpeg),
            &CancelToken::new(),
        )
        .expect("djxl failed");
    assert!(files_identical(&back, &jpg).unwrap());
    assert!(!files_identical(&jxl, &jpg).unwrap());
}
