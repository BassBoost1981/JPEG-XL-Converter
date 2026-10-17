//! Smoke test for the shared fixture helpers in `tests/common`.
//! Every helper writes into a temp dir; the files are then checked with
//! independent readers (the `image` crate and small byte walkers below).

mod common;

use std::fs;
use std::io::BufReader;
use std::path::Path;

use common::{FIXTURE_H, FIXTURE_W, JpegOpts, PngKind};
use image::{ColorType, ImageDecoder};

const MARKER: &str = "TESTMARKER";

/// Returns the (marker, payload) pairs of a JPEG up to and including the SOS header.
fn jpeg_segments(bytes: &[u8]) -> Vec<(u8, &[u8])> {
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "missing SOI");
    let mut segments = Vec::new();
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        assert_eq!(bytes[pos], 0xFF, "expected a marker at offset {pos}");
        let marker = bytes[pos + 1];
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        segments.push((marker, &bytes[pos + 4..pos + 2 + len]));
        if marker == 0xDA {
            break;
        }
        pos += 2 + len;
    }
    segments
}

/// SOF marker (0xC0 baseline, 0xC2 progressive) and component count of a JPEG.
fn jpeg_frame(path: &Path) -> (u8, u8) {
    let bytes = fs::read(path).unwrap();
    let (marker, payload) = jpeg_segments(&bytes)
        .into_iter()
        .find(|(m, _)| matches!(m, 0xC0..=0xC2))
        .expect("no SOF segment");
    // payload: precision, height (2), width (2), component count
    (marker, payload[5])
}

fn assert_opens_as(path: &Path, color: ColorType) {
    let img = image::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!((img.width(), img.height()), (FIXTURE_W, FIXTURE_H));
    assert_eq!(img.color(), color, "{}", path.display());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Inserts an `eXIf` chunk into a PNG, directly before the first `IDAT`
/// chunk or directly before `IEND`.
fn splice_exif(png: &[u8], exif: &[u8], before_idat: bool) -> Vec<u8> {
    let mut chunk = (exif.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(b"eXIf");
    chunk.extend_from_slice(exif);
    let crc = crc32(&chunk[4..]);
    chunk.extend_from_slice(&crc.to_be_bytes());

    let target: &[u8] = if before_idat { b"IDAT" } else { b"IEND" };
    let mut pos = 8;
    loop {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        if &png[pos + 4..pos + 8] == target {
            let mut out = png[..pos].to_vec();
            out.extend_from_slice(&chunk);
            out.extend_from_slice(&png[pos..]);
            return out;
        }
        pos += 12 + len;
    }
}

#[test]
fn tools_are_found_and_runnable() {
    let tools = common::tools();
    assert!(tools.cjxl.is_file() && tools.djxl.is_file());
    let status = tools.check();
    assert!(status.ok, "{:?}", status.messages);
}

#[test]
fn gradient_varies_in_every_channel() {
    let img = common::gradient_rgba8(FIXTURE_W, FIXTURE_H);
    assert_eq!((img.width(), img.height()), (FIXTURE_W, FIXTURE_H));
    let first = img.get_pixel(0, 0).0;
    let last = img.get_pixel(FIXTURE_W - 1, FIXTURE_H - 1).0;
    for channel in 0..4 {
        assert_ne!(
            first[channel], last[channel],
            "channel {channel} is constant"
        );
    }
}

#[test]
fn baseline_jpeg_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baseline.jpg");
    common::write_jpeg(&path, FIXTURE_W, FIXTURE_H, JpegOpts::default());
    assert_opens_as(&path, ColorType::Rgb8);
    assert_eq!(jpeg_frame(&path), (0xC0, 3));
}

#[test]
fn progressive_jpeg_is_progressive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("progressive.jpg");
    let opts = JpegOpts {
        progressive: true,
        ..JpegOpts::default()
    };
    common::write_jpeg(&path, FIXTURE_W, FIXTURE_H, opts);
    assert_opens_as(&path, ColorType::Rgb8);
    assert_eq!(jpeg_frame(&path), (0xC2, 3));
}

#[test]
fn cmyk_jpeg_has_four_components() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cmyk.jpg");
    let opts = JpegOpts {
        cmyk: true,
        ..JpegOpts::default()
    };
    common::write_jpeg(&path, FIXTURE_W, FIXTURE_H, opts);
    // The `image` crate cannot read CMYK JPEGs, so only the frame header is checked.
    assert_eq!(jpeg_frame(&path), (0xC0, 4));
}

#[test]
fn jpeg_carries_exif_in_app1() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exif.jpg");
    let exif = common::minimal_exif(MARKER);
    let opts = JpegOpts {
        exif: Some(exif.clone()),
        ..JpegOpts::default()
    };
    common::write_jpeg(&path, FIXTURE_W, FIXTURE_H, opts);
    assert_opens_as(&path, ColorType::Rgb8);

    let bytes = fs::read(&path).unwrap();
    let app1: Vec<_> = jpeg_segments(&bytes)
        .into_iter()
        .filter(|(m, _)| *m == 0xE1)
        .collect();
    assert_eq!(app1.len(), 1, "expected exactly one APP1 segment");
    let payload = app1[0].1;
    assert_eq!(&payload[..6], b"Exif\0\0");
    assert_eq!(&payload[6..], exif.as_slice());
    assert!(
        payload
            .windows(MARKER.len())
            .any(|w| w == MARKER.as_bytes())
    );
}

#[test]
fn jpeg_without_exif_has_no_app1() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.jpg");
    common::write_jpeg(&path, FIXTURE_W, FIXTURE_H, JpegOpts::default());
    let bytes = fs::read(&path).unwrap();
    assert!(jpeg_segments(&bytes).iter().all(|(m, _)| *m != 0xE1));
}

#[test]
fn minimal_exif_is_a_big_endian_tiff_with_image_description() {
    for marker in [MARKER, "AB"] {
        let exif = common::minimal_exif(marker);
        assert_eq!(&exif[..4], b"MM\0*");
        assert_eq!(
            u32::from_be_bytes(exif[4..8].try_into().unwrap()),
            8,
            "IFD0 offset"
        );
        assert_eq!(
            u16::from_be_bytes(exif[8..10].try_into().unwrap()),
            1,
            "entry count"
        );
        assert_eq!(
            u16::from_be_bytes(exif[10..12].try_into().unwrap()),
            0x010E,
            "tag"
        );
        assert_eq!(
            u16::from_be_bytes(exif[12..14].try_into().unwrap()),
            2,
            "type ASCII"
        );
        let count = u32::from_be_bytes(exif[14..18].try_into().unwrap()) as usize;
        assert_eq!(count, marker.len() + 1, "count includes the NUL terminator");
        assert_eq!(
            u32::from_be_bytes(exif[22..26].try_into().unwrap()),
            0,
            "no IFD1"
        );
        // Values of up to four bytes live in the entry itself, longer ones at an offset.
        let value = if count <= 4 {
            &exif[18..18 + count]
        } else {
            let offset = u32::from_be_bytes(exif[18..22].try_into().unwrap()) as usize;
            &exif[offset..offset + count]
        };
        assert_eq!(value, [marker.as_bytes(), b"\0"].concat());
    }
}

#[test]
fn png_kinds_open_with_the_right_color_type() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (PngKind::Rgb8, ColorType::Rgb8),
        (PngKind::Rgba8, ColorType::Rgba8),
        (PngKind::Rgb16, ColorType::Rgb16),
        (PngKind::Gray8, ColorType::L8),
    ];
    for (kind, color) in cases {
        let path = dir.path().join(format!("{kind:?}.png"));
        common::write_png(&path, kind);
        assert_opens_as(&path, color);
    }
}

#[test]
fn rgba8_png_matches_the_gradient() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rgba.png");
    common::write_png(&path, PngKind::Rgba8);
    let decoded = image::open(&path).unwrap().to_rgba8();
    assert_eq!(decoded, common::gradient_rgba8(FIXTURE_W, FIXTURE_H));
}

#[test]
fn rgb16_png_has_real_16_bit_precision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rgb16.png");
    common::write_png(&path, PngKind::Rgb16);
    let decoded = image::open(&path).unwrap().to_rgb16();
    // A sample that was widened from 8 bit would always be a multiple of 257.
    assert!(decoded.as_raw().iter().any(|v| v % 257 != 0));
    assert_eq!(decoded.get_pixel(FIXTURE_W - 1, 0).0[0], u16::MAX);
}

#[test]
fn tiff16_opens_as_16_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.tif");
    common::write_tiff16(&path);
    assert_opens_as(&path, ColorType::Rgb16);
    let decoded = image::open(&path).unwrap().to_rgb16();
    assert!(decoded.as_raw().iter().any(|v| v % 257 != 0));
}

#[test]
fn bmp_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.bmp");
    common::write_bmp(&path);
    assert_eq!(
        image::guess_format(&fs::read(&path).unwrap()).unwrap(),
        image::ImageFormat::Bmp
    );
    assert_opens_as(&path, ColorType::Rgb8);
}

#[test]
fn gif_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.gif");
    common::write_gif(&path);
    assert_eq!(
        image::guess_format(&fs::read(&path).unwrap()).unwrap(),
        image::ImageFormat::Gif
    );
    assert_opens_as(&path, ColorType::Rgba8);
}

#[test]
fn webp_carries_exif_in_an_exif_chunk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exif.webp");
    let exif = common::minimal_exif(MARKER);
    common::write_webp_with_exif(&path, &exif);
    assert_opens_as(&path, ColorType::Rgb8);

    // Container layout: RIFF header, then VP8X (EXIF flag 0x08), VP8L, EXIF.
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8
    );
    assert_eq!(&bytes[8..12], b"WEBP");
    assert_eq!(&bytes[12..16], b"VP8X");
    assert_ne!(bytes[20] & 0x08, 0, "EXIF flag not set in VP8X");
    assert!(bytes.windows(4).any(|w| w == b"VP8L"));
    let exif_at = bytes
        .windows(4)
        .position(|w| w == b"EXIF")
        .expect("no EXIF chunk");
    let len = u32::from_le_bytes(bytes[exif_at + 4..exif_at + 8].try_into().unwrap()) as usize;
    assert_eq!(&bytes[exif_at + 8..exif_at + 8 + len], exif.as_slice());
    assert!(bytes.windows(MARKER.len()).any(|w| w == MARKER.as_bytes()));

    // An independent parser must find the same metadata.
    let file = fs::File::open(&path).unwrap();
    let mut decoder = image::codecs::webp::WebPDecoder::new(BufReader::new(file)).unwrap();
    assert_eq!(decoder.exif_metadata().unwrap(), Some(exif));
}

#[test]
fn read_png_exif_finds_the_chunk_before_and_after_idat() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.png");
    common::write_png(&plain, PngKind::Rgb8);
    assert_eq!(common::read_png_exif(&plain), None);

    let exif = common::minimal_exif(MARKER);
    let original = fs::read(&plain).unwrap();
    for (name, before_idat) in [("before.png", true), ("after.png", false)] {
        let path = dir.path().join(name);
        fs::write(&path, splice_exif(&original, &exif, before_idat)).unwrap();
        assert_opens_as(&path, ColorType::Rgb8);
        assert_eq!(common::read_png_exif(&path), Some(exif.clone()), "{name}");
    }
}

#[test]
fn read_png_exif_reads_what_djxl_writes() {
    use jpeg_xl_konverter_lib::engine::types::CancelToken;

    let tools = common::tools();
    let cancel = CancelToken::new();
    let dir = tempfile::tempdir().unwrap();
    let exif = common::minimal_exif(MARKER);

    let jpg = dir.path().join("exif.jpg");
    let opts = JpegOpts {
        exif: Some(exif.clone()),
        ..JpegOpts::default()
    };
    common::write_jpeg(&jpg, FIXTURE_W, FIXTURE_H, opts);
    let jxl = dir.path().join("exif.jxl");
    tools.run_cjxl(&jpg, &jxl, &[], &cancel).unwrap();
    let png = dir.path().join("exif.png");
    tools
        .run_djxl(&jxl, &png, &["--output_format=png".to_string()], &cancel)
        .unwrap();

    assert_eq!(common::read_png_exif(&png), Some(exif));
}

#[test]
fn read_png_exif_returns_none_for_missing_or_non_png_files() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(common::read_png_exif(&dir.path().join("missing.png")), None);
    let not_png = dir.path().join("not.png");
    fs::write(&not_png, b"definitely not a png").unwrap();
    assert_eq!(common::read_png_exif(&not_png), None);
}
