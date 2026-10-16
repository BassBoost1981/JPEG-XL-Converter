//! `scan_paths` reads the size of real JXL files (made with cjxl) from the
//! header only.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{FIXTURE_H, FIXTURE_W, JpegOpts, PngKind};
use jpeg_xl_konverter_lib::engine::scan::scan_paths;
use jpeg_xl_konverter_lib::engine::types::{CancelToken, Mode};

fn cjxl(input: &Path, output: &Path) {
    common::tools()
        .run_cjxl(input, output, &[], &CancelToken::new())
        .expect("cjxl failed");
}

fn dims_of(path: &Path) -> (u32, u32) {
    let result = scan_paths(&[path.to_path_buf()], Mode::Dec);
    assert_eq!(result.rejected, 0);
    assert_eq!(result.entries.len(), 1, "the file must stay in the list");
    (result.entries[0].width, result.entries[0].height)
}

#[test]
fn jxl_size_is_read_from_a_bare_codestream() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let jxl = dir.path().join("a.jxl");
    common::write_png(&png, PngKind::Rgb8);
    cjxl(&png, &jxl);

    assert_eq!(dims_of(&jxl), (FIXTURE_W, FIXTURE_H));
    let entry = &scan_paths(std::slice::from_ref(&jxl), Mode::Dec).entries[0];
    assert_eq!(entry.size, fs::metadata(&jxl).unwrap().len());
}

#[test]
fn jxl_size_is_read_from_a_container_with_jpeg_reconstruction() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("b.jpg");
    let jxl = dir.path().join("b.jxl");
    common::write_jpeg(&jpg, FIXTURE_W, FIXTURE_H, JpegOpts::default());
    cjxl(&jpg, &jxl);
    assert_eq!(
        &fs::read(&jxl).unwrap()[4..8],
        b"JXL ",
        "expected a container"
    );

    assert_eq!(dims_of(&jxl), (FIXTURE_W, FIXTURE_H));
}

#[test]
fn only_the_header_is_needed() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("c.png");
    let jxl = dir.path().join("c.jxl");
    common::write_png(&png, PngKind::Rgb8);
    cjxl(&png, &jxl);
    let bytes = fs::read(&jxl).unwrap();
    assert!(bytes.len() > 64, "fixture too small to truncate");

    // The image data is cut off, the header at the start is intact.
    let cut: PathBuf = dir.path().join("cut.jxl");
    fs::write(&cut, &bytes[..32]).unwrap();
    assert_eq!(dims_of(&cut), (FIXTURE_W, FIXTURE_H));
}

#[test]
fn broken_jxl_stays_in_the_list_with_zero_size() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("d.png");
    let jxl = dir.path().join("d.jxl");
    common::write_png(&png, PngKind::Rgb8);
    cjxl(&png, &jxl);
    let bytes = fs::read(&jxl).unwrap();

    let stub = dir.path().join("stub.jxl");
    fs::write(&stub, &bytes[..1]).unwrap();
    assert_eq!(dims_of(&stub), (0, 0));

    let garbage = dir.path().join("garbage.jxl");
    fs::write(&garbage, b"this is not a jpeg xl file at all").unwrap();
    assert_eq!(dims_of(&garbage), (0, 0));

    let empty = dir.path().join("empty.jxl");
    fs::write(&empty, b"").unwrap();
    assert_eq!(dims_of(&empty), (0, 0));
}
