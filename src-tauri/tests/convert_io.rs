//! `convert_io` against real files: TIFF/WebP/BMP -> PNG, PNG -> TIFF/WebP,
//! metadata bookkeeping and the PNG metadata stripper. The end-to-end test
//! sends the intermediate PNG through the real cjxl and djxl.

mod common;

use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use common::{FIXTURE_H, FIXTURE_W, PngKind};
use image::{
    ColorType, DynamicImage, GrayAlphaImage, ImageDecoder, ImageFormat, ImageReader, Rgb32FImage,
};
use jpeg_xl_konverter_lib::engine::args::{DjxlTarget, cjxl_args, djxl_args};
use jpeg_xl_konverter_lib::engine::convert_io::{
    MetaState, MetaTransfer, from_png, strip_jpeg_metadata, strip_png_metadata, to_png,
};
use jpeg_xl_konverter_lib::engine::types::{CancelToken, EncodeSettings, InputKind, OutputFormat};
use jpeg_xl_konverter_lib::engine::verify::same_pixels;
use png::chunk::ChunkType;
use tiff::encoder::{TiffEncoder as RawTiffEncoder, colortype};
use tiff::tags::Tag;

const MARKER: &str = "TESTMARKER";
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const READ_ERROR: &str = "Bild lässt sich nicht lesen";
const OPEN_ERROR: &str = "Datei lässt sich nicht öffnen";

use MetaState::{Lost, NotPresent, Transferred};

// ---------------------------------------------------------------- fixtures

/// All chunks of a PNG as (type, data), in file order.
fn png_chunks(path: &Path) -> Vec<(String, Vec<u8>)> {
    let bytes = fs::read(path).unwrap();
    let mut rest = bytes.strip_prefix(SIGNATURE).expect("not a PNG");
    let mut chunks = Vec::new();
    while rest.len() >= 12 {
        let len = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        let kind = String::from_utf8_lossy(&rest[4..8]).into_owned();
        chunks.push((kind, rest[8..8 + len].to_vec()));
        rest = &rest[12 + len..];
    }
    chunks
}

fn chunk_names(path: &Path) -> Vec<String> {
    png_chunks(path).into_iter().map(|(kind, _)| kind).collect()
}

fn find_chunk(path: &Path, kind: &str) -> Option<Vec<u8>> {
    png_chunks(path)
        .into_iter()
        .find(|(k, _)| k == kind)
        .map(|(_, data)| data)
}

/// Keyword of a tEXt/zTXt/iTXt chunk: the bytes up to the first NUL.
fn keyword(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap();
    String::from_utf8_lossy(&data[..end]).into_owned()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A PNG (RGB8 test pattern) with `chunks` written between IHDR and the image data.
fn write_png_with(path: &Path, chunks: &[(&[u8; 4], Vec<u8>)]) {
    write_png_as(path, png::ColorType::Rgb, chunks);
}

/// Like `write_png_with`, 8 bit, for the given colour type.
fn write_png_as(path: &Path, color: png::ColorType, chunks: &[(&[u8; 4], Vec<u8>)]) {
    let (w, h) = (FIXTURE_W, FIXTURE_H);
    let pixels: Vec<u8> = (0..w * h * color.samples() as u32)
        .map(|i| (i * 7 % 251) as u8)
        .collect();
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path).unwrap()), w, h);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    for (kind, data) in chunks {
        writer.write_chunk(ChunkType(**kind), data).unwrap();
    }
    writer.write_image_data(&pixels).unwrap();
    writer.finish().unwrap();
}

fn zlib(bytes: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6)
}

fn iccp_chunk(profile: &[u8]) -> Vec<u8> {
    [b"ICC\0\0".as_slice(), &zlib(profile)].concat()
}

fn text_chunk(keyword: &str, text: &str) -> Vec<u8> {
    format!("{keyword}\0{text}").into_bytes()
}

fn ztxt_chunk(keyword: &str, text: &str) -> Vec<u8> {
    [format!("{keyword}\0\0").as_bytes(), &zlib(text.as_bytes())].concat()
}

fn itxt_chunk(keyword: &str, text: &str) -> Vec<u8> {
    // keyword, NUL, compression flag, compression method, language tag, NUL, translated keyword, NUL
    format!("{keyword}\0\0\0\0\0{text}").into_bytes()
}

/// ImageMagick-style raw profile text as djxl writes it for EXIF:
/// `"\nexif\n<length>\n<hex>\n"` where the bytes are `Exif\0\0` + TIFF block.
fn raw_profile_exif(exif: &[u8]) -> String {
    let bytes = [b"Exif\0\0".as_slice(), exif].concat();
    let hex: Vec<String> = bytes
        .chunks(36)
        .map(|line| line.iter().map(|b| format!("{b:02x}")).collect())
        .collect();
    format!("\nexif\n{:8}\n{}\n", bytes.len(), hex.join("\n"))
}

fn icc_blob() -> Vec<u8> {
    (0..300).map(|i| (i * 13 % 256) as u8).collect()
}

fn xmp_packet() -> String {
    format!("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">{MARKER}</x:xmpmeta>")
}

fn rgb8_bytes() -> Vec<u8> {
    DynamicImage::ImageRgba8(common::gradient_rgba8(FIXTURE_W, FIXTURE_H))
        .to_rgb8()
        .into_raw()
}

/// A tag in the first directory of a test TIFF.
#[derive(Debug, Clone)]
enum Extra {
    /// Descriptive tags a camera or editor writes, which make up EXIF in a TIFF.
    Make,
    Orientation(u16),
    ExifPointer,
    Xmp,
    Iptc,
    /// Any other tag, by number, with a dummy value.
    Other(u16),
    Icc(Vec<u8>),
}

/// RGB8 TIFF with extra tags in its first directory.
fn write_tiff_with(path: &Path, extras: &[Extra]) {
    let mut encoder = RawTiffEncoder::new(File::create(path).unwrap()).unwrap();
    let mut image = encoder
        .new_image::<colortype::RGB8>(FIXTURE_W, FIXTURE_H)
        .unwrap();
    for extra in extras {
        let directory = image.encoder();
        match extra {
            Extra::Make => directory.write_tag(Tag::Make, MARKER).unwrap(),
            Extra::Orientation(value) => directory.write_tag(Tag::Orientation, *value).unwrap(),
            Extra::ExifPointer => directory.write_tag(Tag::ExifDirectory, 0u32).unwrap(),
            Extra::Xmp => directory
                .write_tag(Tag::Unknown(700), xmp_packet().as_bytes())
                .unwrap(),
            Extra::Iptc => directory
                .write_tag(Tag::Unknown(33723), &b"IPTC-block"[..])
                .unwrap(),
            Extra::Other(number) => directory
                .write_tag(Tag::from_u16_exhaustive(*number), 7u32)
                .unwrap(),
            Extra::Icc(profile) => directory
                .write_tag(Tag::IccProfile, profile.as_slice())
                .unwrap(),
        }
    }
    image.write_data(&rgb8_bytes()).unwrap();
}

/// Adds an `XMP ` chunk to a WebP written by `write_webp_with_exif`.
fn add_webp_xmp(path: &Path, xmp: &[u8]) {
    const VP8X_FLAGS_AT: usize = 20; // RIFF header (12) + chunk header (8)
    const XMP_FLAG: u8 = 0x04;
    let mut bytes = fs::read(path).unwrap();
    bytes[VP8X_FLAGS_AT] |= XMP_FLAG;
    bytes.extend_from_slice(b"XMP ");
    bytes.extend_from_slice(&(xmp.len() as u32).to_le_bytes());
    bytes.extend_from_slice(xmp);
    if xmp.len() % 2 == 1 {
        bytes.push(0);
    }
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

// Colour space types of a BITMAPV5HEADER (`bV5CSType`).
const BMP_EMBEDDED: u32 = 0x4D42_4544; // 'MBED'
const BMP_LINKED: u32 = 0x4C49_4E4B; // 'LINK'
const BMP_SRGB: u32 = 0x7352_4742; // 'sRGB'

/// A 24-bit BMP with a BITMAPV5HEADER of the given colour space type; `profile`
/// (may be empty) is stored between the header and the pixels.
fn write_bmp_v5(path: &Path, cs_type: u32, profile: &[u8]) {
    const HEADER_LEN: u32 = 124;
    let rgb = rgb8_bytes();
    let mut pixels = Vec::new();
    for row in rgb.chunks(FIXTURE_W as usize * 3).rev() {
        // bottom-up rows, BGR; 64 * 3 bytes need no padding
        pixels.extend(row.chunks(3).flat_map(|p| [p[2], p[1], p[0]]));
    }
    let pixel_offset = 14 + HEADER_LEN + profile.len() as u32;

    let mut bmp = b"BM".to_vec();
    bmp.extend((pixel_offset + pixels.len() as u32).to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(pixel_offset.to_le_bytes());
    // BITMAPV5HEADER
    bmp.extend(HEADER_LEN.to_le_bytes());
    bmp.extend((FIXTURE_W as i32).to_le_bytes());
    bmp.extend((FIXTURE_H as i32).to_le_bytes());
    bmp.extend(1u16.to_le_bytes()); // planes
    bmp.extend(24u16.to_le_bytes()); // bits per pixel
    bmp.extend([0u8; 4]); // BI_RGB
    bmp.extend((pixels.len() as u32).to_le_bytes());
    bmp.extend([0u8; 16]); // resolution, palette size
    bmp.extend([0u8; 16]); // channel masks
    bmp.extend(cs_type.to_le_bytes());
    bmp.extend([0u8; 36 + 12]); // end points, gamma
    bmp.extend(4u32.to_le_bytes()); // intent
    bmp.extend((HEADER_LEN).to_le_bytes()); // profile data, from the start of the header
    bmp.extend((profile.len() as u32).to_le_bytes());
    bmp.extend([0u8; 4]);
    assert_eq!(bmp.len() as u32, 14 + HEADER_LEN);
    bmp.extend(profile);
    bmp.extend(pixels);
    fs::write(path, bmp).unwrap();
}

/// A TIFF with `pages` pages (one directory each).
fn write_tiff_pages(path: &Path, pages: usize) {
    let mut encoder = RawTiffEncoder::new(File::create(path).unwrap()).unwrap();
    for _ in 0..pages {
        encoder
            .write_image::<colortype::RGB8>(FIXTURE_W, FIXTURE_H, &rgb8_bytes())
            .unwrap();
    }
}

/// An animated WebP with `frames` frames that differ from each other (libwebp
/// merges frames that are equal).
fn write_animated_webp(path: &Path, frames: usize) {
    let config = webp::WebPConfig::new().unwrap();
    let mut encoder = webp::AnimEncoder::new(FIXTURE_W, FIXTURE_H, &config);
    let pictures: Vec<Vec<u8>> = (0..frames)
        .map(|i| {
            let shift = 60 * i as u8;
            rgb8_bytes().iter().map(|v| v.wrapping_add(shift)).collect()
        })
        .collect();
    for (i, rgb) in pictures.iter().enumerate() {
        let frame = webp::AnimFrame::from_rgb(rgb, FIXTURE_W, FIXTURE_H, 100 * i as i32);
        encoder.add_frame(frame);
    }
    fs::write(path, &*encoder.encode()).unwrap();
}

/// The ICC profile of the first directory of a TIFF. Read with the tiff crate:
/// the image crate's decoder misses the profile of a small image.
fn tiff_icc(path: &Path) -> Option<Vec<u8>> {
    let mut decoder =
        tiff::decoder::Decoder::new(BufReader::new(File::open(path).unwrap())).unwrap();
    let value = decoder.find_tag(Tag::IccProfile).unwrap()?;
    Some(value.into_u8_vec().unwrap())
}

/// Decodes by content, so `.tmp` files work.
fn open(path: &Path) -> DynamicImage {
    ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .unwrap()
        .decode()
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn color_of(path: &Path) -> ColorType {
    ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap()
        .color_type()
}

fn icc_of(path: &Path) -> Option<Vec<u8>> {
    ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap()
        .icc_profile()
        .unwrap()
}

fn meta(exif: MetaState, icc: MetaState) -> MetaTransfer {
    MetaTransfer {
        exif,
        icc,
        frames: NotPresent,
    }
}

// ------------------------------------------------------------- MetaTransfer

#[test]
fn complete_means_no_field_is_lost() {
    let states = [NotPresent, Transferred, Lost];
    for exif in states {
        for icc in states {
            for frames in states {
                let transfer = MetaTransfer { exif, icc, frames };
                assert_eq!(
                    transfer.complete(),
                    ![exif, icc, frames].contains(&Lost),
                    "{transfer:?}"
                );
            }
        }
    }
}

// --------------------------------------------------------------- to_png

#[test]
fn tiff16_to_png_keeps_16bit() {
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("a.tif");
    let png = dir.path().join("a.png");
    common::write_tiff16(&tiff);

    let transfer = to_png(&tiff, &png, true).unwrap();

    assert_eq!(transfer, meta(NotPresent, NotPresent));
    assert_eq!(color_of(&png), ColorType::Rgb16);
    let (before, after) = (open(&tiff), open(&png));
    assert_eq!(before.color(), ColorType::Rgb16);
    assert!(same_pixels(&before, &after));
    // The 16-bit values themselves, not just their 8-bit shadows.
    assert_eq!(before.to_rgb16().as_raw(), after.to_rgb16().as_raw());
}

#[test]
fn float_images_are_refused() {
    // 16 bit would round the values, and nothing afterwards could notice.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("float.png");
    let rgb = Rgb32FImage::from_fn(FIXTURE_W, FIXTURE_H, |x, y| {
        image::Rgb([
            x as f32 / (FIXTURE_W - 1) as f32,
            y as f32 / (FIXTURE_H - 1) as f32,
            0.5,
        ])
    });
    let rgba =
        image::Rgba32FImage::from_pixel(FIXTURE_W, FIXTURE_H, image::Rgba([0.1, 0.2, 0.3, 0.4]));
    for (name, image) in [
        ("rgb", DynamicImage::ImageRgb32F(rgb)),
        ("rgba", DynamicImage::ImageRgba32F(rgba)),
    ] {
        let tiff = dir.path().join(format!("{name}.tif"));
        image.save_with_format(&tiff, ImageFormat::Tiff).unwrap();
        assert!(matches!(
            color_of(&tiff),
            ColorType::Rgb32F | ColorType::Rgba32F
        ));

        let result = to_png(&tiff, &png, true);

        assert_eq!(
            result,
            Err("Gleitkomma-Bilder werden nicht unterstützt".to_string()),
            "{name}"
        );
        assert!(!png.exists());
    }
}

#[test]
fn cmyk_tiff_is_refused() {
    // The image crate turns CMYK into RGB with a naive formula and drops K.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("cmyk.png");
    let cmyk8 = dir.path().join("cmyk8.tif");
    let mut encoder = RawTiffEncoder::new(File::create(&cmyk8).unwrap()).unwrap();
    let pixels: Vec<u8> = (0..FIXTURE_W * FIXTURE_H * 4)
        .map(|i| (i % 251) as u8)
        .collect();
    encoder
        .write_image::<colortype::CMYK8>(FIXTURE_W, FIXTURE_H, &pixels)
        .unwrap();
    drop(encoder);
    let cmyk16 = dir.path().join("cmyk16.tif");
    let mut encoder = RawTiffEncoder::new(File::create(&cmyk16).unwrap()).unwrap();
    let pixels: Vec<u16> = (0..FIXTURE_W * FIXTURE_H * 4)
        .map(|i| (i * 17 % 65521) as u16)
        .collect();
    encoder
        .write_image::<colortype::CMYK16>(FIXTURE_W, FIXTURE_H, &pixels)
        .unwrap();
    drop(encoder);

    for tiff in [cmyk8, cmyk16] {
        // The image crate does read it, which is the trap.
        assert!(image::open(&tiff).is_ok(), "{}", tiff.display());

        let result = to_png(&tiff, &png, true);

        assert_eq!(
            result,
            Err("CMYK-Bilder werden nicht unterstützt".to_string()),
            "{}",
            tiff.display()
        );
        assert!(!png.exists());
    }
}

#[test]
fn to_png_only_takes_tiff_webp_and_bmp() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.png");
    let png = dir.path().join("a.png");
    let jpg = dir.path().join("a.jpg");
    common::write_png(&png, PngKind::Rgb8);
    common::write_jpeg(&jpg, FIXTURE_W, FIXTURE_H, common::JpegOpts::default());
    // Recognised by content: a TIFF named .png still counts, a PNG named .tif does not.
    let tiff_as_png = dir.path().join("tiff.png");
    let png_as_tiff = dir.path().join("png.tif");
    common::write_tiff16(&tiff_as_png);
    fs::copy(&png, &png_as_tiff).unwrap();

    for refused in [&png, &jpg, &png_as_tiff] {
        let result = to_png(refused, &out, true);
        assert_eq!(
            result,
            Err("Format wird hier nicht unterstützt".to_string()),
            "{}",
            refused.display()
        );
    }
    assert!(!out.exists());
    assert!(to_png(&tiff_as_png, &out, true).is_ok());
}

#[test]
fn webp_exif_goes_into_png() {
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    let exif = common::minimal_exif(MARKER);
    common::write_webp_with_exif(&webp, &exif);

    let transfer = to_png(&webp, &png, true).unwrap();

    assert_eq!(transfer, meta(Transferred, NotPresent));
    let chunk = find_chunk(&png, "eXIf").expect("no eXIf chunk");
    assert_eq!(chunk, exif);
    assert!(contains(&chunk, MARKER.as_bytes()));
    // cjxl only looks at what comes before the image data.
    let names = chunk_names(&png);
    let position = |name: &str| names.iter().position(|n| n == name).unwrap();
    assert!(position("eXIf") < position("IDAT"), "{names:?}");
    assert!(same_pixels(&open(&webp), &open(&png)));
}

#[test]
fn webp_exif_with_app1_prefix_is_stored_without_it() {
    // Some writers put `Exif\0\0` in front of the TIFF block; a PNG eXIf chunk must not have it.
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    let exif = common::minimal_exif(MARKER);
    common::write_webp_with_exif(&webp, &[b"Exif\0\0".as_slice(), &exif].concat());

    assert_eq!(to_png(&webp, &png, true).unwrap().exif, Transferred);

    assert_eq!(find_chunk(&png, "eXIf").unwrap(), exif);
}

#[test]
fn strip_when_metadata_off() {
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    common::write_webp_with_exif(&webp, &common::minimal_exif(MARKER));

    let transfer = to_png(&webp, &png, false).unwrap();

    assert_eq!(transfer.exif, NotPresent);
    assert!(!chunk_names(&png).contains(&"eXIf".to_string()));
    assert_eq!(common::read_png_exif(&png), None);
    assert!(same_pixels(&open(&webp), &open(&png)));
}

#[test]
fn bmp_to_png_without_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let bmp = dir.path().join("a.bmp");
    common::write_bmp(&bmp);

    for keep_metadata in [true, false] {
        let png = dir.path().join(format!("keep-{keep_metadata}.png"));
        let transfer = to_png(&bmp, &png, keep_metadata).unwrap();

        assert_eq!(transfer, meta(NotPresent, NotPresent));
        assert!(same_pixels(&open(&bmp), &open(&png)));
    }
}

#[test]
fn bmp_v5_with_a_colour_profile_reports_icc_lost() {
    // The image crate does not read the profile, so it cannot go into the PNG.
    let dir = tempfile::tempdir().unwrap();
    let profile = icc_blob();
    for (name, cs_type, profile) in [
        ("embedded", BMP_EMBEDDED, profile.as_slice()),
        ("linked", BMP_LINKED, b"profile.icc\0".as_slice()),
    ] {
        let bmp = dir.path().join(format!("{name}.bmp"));
        write_bmp_v5(&bmp, cs_type, profile);

        for keep_metadata in [true, false] {
            let png = dir.path().join(format!("{name}-{keep_metadata}.png"));
            let transfer = to_png(&bmp, &png, keep_metadata).unwrap();

            assert_eq!(
                transfer,
                meta(NotPresent, Lost),
                "{name} keep {keep_metadata}"
            );
            assert!(!transfer.complete());
            assert!(same_pixels(&open(&bmp), &open(&png)), "{name}");
        }
    }
}

#[test]
fn bmp_v5_without_a_colour_profile_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let bmp = dir.path().join("srgb.bmp");
    let png = dir.path().join("srgb.png");
    write_bmp_v5(&bmp, BMP_SRGB, &[]);

    let transfer = to_png(&bmp, &png, true).unwrap();

    assert_eq!(transfer, meta(NotPresent, NotPresent));
    assert!(same_pixels(&open(&bmp), &open(&png)));
}

#[test]
fn icc_profile_is_always_carried_into_the_png() {
    // The profile defines what the pixel values mean, so it is not "metadata" that
    // the user can switch off. The TIFF is tiny on purpose: the image crate's own
    // TIFF decoder drops the profile of a small image.
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("a.tif");
    let icc = icc_blob();
    write_tiff_with(&tiff, &[Extra::Icc(icc.clone())]);

    for keep_metadata in [true, false] {
        let png = dir.path().join(format!("keep-{keep_metadata}.png"));
        let transfer = to_png(&tiff, &png, keep_metadata).unwrap();

        assert_eq!(
            transfer,
            meta(NotPresent, Transferred),
            "keep {keep_metadata}"
        );
        assert_eq!(icc_of(&png), Some(icc.clone()), "keep {keep_metadata}");
        let names = chunk_names(&png);
        let position = |name: &str| names.iter().position(|n| n == name).unwrap();
        assert!(position("iCCP") < position("IDAT"), "{names:?}");
        assert!(same_pixels(&open(&tiff), &open(&png)));
    }
}

#[test]
fn tiff_exif_that_cannot_be_carried_is_reported_lost() {
    // The decoder cannot read EXIF out of a TIFF, and the metadata tags of a TIFF are
    // too many to list, so every tag of the first directory that does not just describe
    // how the pixels are stored counts.
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (Extra::Make, Lost),
        (Extra::Orientation(6), Lost),
        (Extra::ExifPointer, Lost),
        (Extra::Xmp, Lost),
        (Extra::Iptc, Lost),
        // Everything that is not about how the pixels are stored counts:
        (Extra::Other(305), Lost),   // Software
        (Extra::Other(316), Lost),   // HostComputer
        (Extra::Other(269), Lost),   // DocumentName
        (Extra::Other(285), Lost),   // PageName
        (Extra::Other(330), Lost),   // SubIFDs
        (Extra::Other(40091), Lost), // XPTitle
        (Extra::Other(18246), Lost), // Rating
        (Extra::Other(34377), Lost), // Photoshop image resources
        (Extra::Other(37724), Lost), // ImageSourceData
        // ... but tags about the storage do not:
        (Extra::Other(282), NotPresent),     // XResolution
        (Extra::Other(296), NotPresent),     // ResolutionUnit
        (Extra::Other(266), NotPresent),     // FillOrder
        (Extra::Other(254), NotPresent),     // NewSubfileType
        (Extra::Orientation(1), NotPresent), // "upright" is the default, nothing to lose
    ];
    for (extra, expected) in cases {
        let tiff = dir.path().join("a.tif");
        let png = dir.path().join("a.png");
        write_tiff_with(&tiff, std::slice::from_ref(&extra));

        let kept = to_png(&tiff, &png, true).unwrap();
        assert_eq!(kept.exif, expected, "{extra:?}");
        assert_eq!(kept.complete(), expected != Lost, "{extra:?}");

        let _ = fs::remove_file(&png);
        assert_eq!(
            to_png(&tiff, &png, false).unwrap().exif,
            NotPresent,
            "{extra:?}"
        );
    }
}

#[test]
fn tiff_colour_tags_without_a_profile_are_reported_as_lost_icc() {
    // TransferFunction, WhitePoint, PrimaryChromaticities: a colour description that
    // the PNG does not get. Colour, not metadata, so it counts with metadata off, too.
    let dir = tempfile::tempdir().unwrap();
    for number in [301, 318, 319] {
        let tiff = dir.path().join("a.tif");
        write_tiff_with(&tiff, &[Extra::Other(number)]);

        for keep_metadata in [true, false] {
            let png = dir.path().join("a.png");
            let transfer = to_png(&tiff, &png, keep_metadata).unwrap();

            assert_eq!(
                transfer,
                meta(NotPresent, Lost),
                "tag {number} keep {keep_metadata}"
            );
            assert!(!transfer.complete());
            fs::remove_file(&png).unwrap();
        }
    }
}

#[test]
fn tiff_icc_profile_is_carried_although_its_xmp_is_lost() {
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("a.tif");
    let png = dir.path().join("a.png");
    write_tiff_with(&tiff, &[Extra::Xmp, Extra::Icc(icc_blob())]);

    let transfer = to_png(&tiff, &png, true).unwrap();

    assert_eq!(transfer, meta(Lost, Transferred));
    assert_eq!(icc_of(&png), Some(icc_blob()));
}

#[test]
fn webp_xmp_is_reported_lost_and_its_exif_still_carried() {
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    let exif = common::minimal_exif(MARKER);
    common::write_webp_with_exif(&webp, &exif);
    add_webp_xmp(&webp, xmp_packet().as_bytes());

    let kept = to_png(&webp, &png, true).unwrap();
    assert_eq!(kept, meta(Lost, NotPresent));
    assert_eq!(find_chunk(&png, "eXIf"), Some(exif));

    let dropped = to_png(&webp, &png, false).unwrap();
    assert_eq!(dropped, meta(NotPresent, NotPresent));
}

#[test]
fn multi_page_tiff_converts_the_first_page_and_reports_the_rest_lost() {
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("pages.tif");
    write_tiff_pages(&tiff, 2);

    for keep_metadata in [true, false] {
        let png = dir.path().join("pages.png");
        let transfer = to_png(&tiff, &png, keep_metadata).unwrap();

        assert_eq!(
            transfer,
            MetaTransfer {
                exif: NotPresent,
                icc: NotPresent,
                frames: Lost
            },
            "keep {keep_metadata}"
        );
        assert!(!transfer.complete());
        assert!(same_pixels(&open(&tiff), &open(&png)), "first page");
        fs::remove_file(&png).unwrap();
    }
}

#[test]
fn animated_webp_converts_the_first_frame_and_reports_the_rest_lost() {
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("anim.webp");
    write_animated_webp(&webp, 2);
    let bytes = fs::read(&webp).unwrap();
    assert!(
        bytes.windows(4).filter(|w| w == b"ANMF").count() >= 2,
        "fixture is not an animation"
    );

    for keep_metadata in [true, false] {
        let png = dir.path().join("anim.png");
        let transfer = to_png(&webp, &png, keep_metadata).unwrap();

        assert_eq!(transfer.frames, Lost, "keep {keep_metadata}");
        assert!(!transfer.complete());
        assert!(same_pixels(&open(&webp), &open(&png)), "first frame");
        fs::remove_file(&png).unwrap();
    }
}

#[test]
fn single_page_and_single_frame_sources_lose_no_frames() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("out.png");
    let sources = [
        ("one page", dir.path().join("one.tif")),
        ("one frame", dir.path().join("one.webp")),
        ("bmp", dir.path().join("one.bmp")),
    ];
    write_tiff_pages(&sources[0].1, 1);
    write_animated_webp(&sources[1].1, 1);
    common::write_bmp(&sources[2].1);

    for (name, source) in sources {
        let transfer = to_png(&source, &png, true).unwrap();
        assert_eq!(transfer.frames, NotPresent, "{name}");
        assert!(transfer.complete(), "{name}");
    }
}

#[test]
fn to_png_errors_are_short_german_texts() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("out.png");

    let missing = to_png(&dir.path().join("fehlt.tif"), &png, true);
    assert_eq!(missing, Err(OPEN_ERROR.to_string()));

    let garbage = dir.path().join("kaputt.webp");
    fs::write(&garbage, b"this is not an image at all").unwrap();
    assert_eq!(to_png(&garbage, &png, true), Err(READ_ERROR.to_string()));

    let empty = dir.path().join("leer.tif");
    fs::write(&empty, b"").unwrap();
    assert_eq!(to_png(&empty, &png, true), Err(READ_ERROR.to_string()));

    assert!(
        !png.exists(),
        "a failed conversion must not leave output behind"
    );
}

#[test]
fn truncated_image_data_is_an_error_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("a.tif");
    let png = dir.path().join("a.png");
    common::write_tiff16(&tiff);
    let bytes = fs::read(&tiff).unwrap();
    fs::write(&tiff, &bytes[..bytes.len() / 2]).unwrap();

    assert_eq!(to_png(&tiff, &png, true), Err(READ_ERROR.to_string()));
    assert!(!png.exists());
}

// --------------------------------------------------------------- from_png

#[test]
fn png_to_webp_marks_exif_lost() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let webp = dir.path().join("a.webp");
    write_png_with(&png, &[(b"eXIf", common::minimal_exif(MARKER))]);

    let transfer = from_png(&png, &webp, OutputFormat::Webp, true).unwrap();

    assert_eq!(transfer, meta(Lost, NotPresent));
    assert!(!transfer.complete());
    let back = open(&webp);
    assert_eq!((back.width(), back.height()), (FIXTURE_W, FIXTURE_H));
}

#[test]
fn exif_in_a_raw_profile_text_chunk_counts_as_exif() {
    // djxl v0.12.0 writes EXIF as zTXt "Raw profile type exif", never as eXIf.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(
        &png,
        &[(
            b"zTXt",
            ztxt_chunk(
                "Raw profile type exif",
                &raw_profile_exif(&common::minimal_exif(MARKER)),
            ),
        )],
    );
    assert!(common::read_png_exif(&png).is_some(), "fixture has no EXIF");

    for (format, name) in [
        (OutputFormat::Webp, "a.webp"),
        (OutputFormat::Tiff, "a.tif"),
    ] {
        let out = dir.path().join(name);
        let transfer = from_png(&png, &out, format, true).unwrap();
        assert_eq!(transfer.exif, Lost, "{format:?}");
    }
}

#[test]
fn exif_in_any_text_chunk_kind_counts_as_exif() {
    let dir = tempfile::tempdir().unwrap();
    let text = raw_profile_exif(&common::minimal_exif(MARKER));
    let variants = [
        ("tEXt", text_chunk("Raw profile type exif", &text)),
        ("iTXt", itxt_chunk("Raw profile type exif", &text)),
    ];
    for (kind, data) in variants {
        let png = dir.path().join(format!("{kind}.png"));
        let out = dir.path().join(format!("{kind}.tif"));
        write_png_with(&png, &[(kind.as_bytes().try_into().unwrap(), data)]);

        let transfer = from_png(&png, &out, OutputFormat::Tiff, true).unwrap();
        assert_eq!(transfer.exif, Lost, "{kind}");
    }
}

#[test]
fn png_xmp_counts_like_exif() {
    // Both are the descriptive metadata "keep metadata" is about.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(
        &png,
        &[(b"iTXt", itxt_chunk("XML:com.adobe.xmp", &xmp_packet()))],
    );

    for (keep_metadata, expected) in [(true, Lost), (false, NotPresent)] {
        let out = dir.path().join("a.tif");
        let transfer = from_png(&png, &out, OutputFormat::Tiff, keep_metadata).unwrap();
        assert_eq!(transfer.exif, expected, "keep {keep_metadata}");
        let _ = fs::remove_file(&out);
    }
}

#[test]
fn png_without_exif_or_icc_converts_with_nothing_lost() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    // Text that is neither EXIF nor XMP is not metadata we track.
    write_png_with(&png, &[(b"tEXt", text_chunk("Comment", "hello"))]);

    for (format, name) in [
        (OutputFormat::Webp, "a.webp"),
        (OutputFormat::Tiff, "a.tif"),
    ] {
        let out = dir.path().join(name);
        let transfer = from_png(&png, &out, format, true).unwrap();
        assert_eq!(transfer, meta(NotPresent, NotPresent), "{format:?}");
        assert!(transfer.complete());
    }
}

#[test]
fn png_icc_profile_goes_into_tiff() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(&png, &[(b"iCCP", iccp_chunk(&icc_blob()))]);
    assert_eq!(icc_of(&png), Some(icc_blob()), "fixture has no ICC profile");

    // The profile is not metadata the user can switch off.
    for keep_metadata in [true, false] {
        let tiff = dir.path().join("a.tif");
        let transfer = from_png(&png, &tiff, OutputFormat::Tiff, keep_metadata).unwrap();

        assert_eq!(
            transfer,
            meta(NotPresent, Transferred),
            "keep {keep_metadata}"
        );
        assert!(transfer.complete());
        assert_eq!(tiff_icc(&tiff), Some(icc_blob()), "keep {keep_metadata}");
        assert!(same_pixels(&open(&png), &open(&tiff)));
        fs::remove_file(&tiff).unwrap();
    }
}

#[test]
fn png_icc_profile_is_lost_in_webp() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(&png, &[(b"iCCP", iccp_chunk(&icc_blob()))]);

    for keep_metadata in [true, false] {
        let webp = dir.path().join("a.webp");
        let transfer = from_png(&png, &webp, OutputFormat::Webp, keep_metadata).unwrap();

        // Lost either way: without the profile the colours change.
        assert_eq!(transfer.icc, Lost, "keep {keep_metadata}");
        assert!(!transfer.complete());
        assert_eq!(icc_of(&webp), None);
        fs::remove_file(&webp).unwrap();
    }
}

#[test]
fn icc_chunk_that_cannot_be_read_is_lost_in_tiff() {
    // There is a profile, but there are no bytes to put into the TIFF.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let tiff = dir.path().join("a.tif");
    write_png_with(&png, &[(b"iCCP", b"ICC\0\0this is not zlib".to_vec())]);

    let transfer = from_png(&png, &tiff, OutputFormat::Tiff, true).unwrap();

    assert_eq!(transfer.icc, Lost);
    assert_eq!(tiff_icc(&tiff), None);
}

#[test]
fn gray_png_keeps_its_profile_in_the_tiff_but_gray_alpha_cannot() {
    // Gray stays gray in a TIFF, so a gray profile still fits. Gray + alpha has to be
    // widened to RGBA, and a gray profile on RGB data would be wrong.
    let dir = tempfile::tempdir().unwrap();
    let profile = iccp_chunk(&icc_blob());
    for (name, color, expected) in [
        ("gray", png::ColorType::Grayscale, Transferred),
        ("gray-alpha", png::ColorType::GrayscaleAlpha, Lost),
    ] {
        let png = dir.path().join(format!("{name}.png"));
        let tiff = dir.path().join(format!("{name}.tif"));
        write_png_as(&png, color, &[(b"iCCP", profile.clone())]);

        let transfer = from_png(&png, &tiff, OutputFormat::Tiff, true).unwrap();

        assert_eq!(transfer.icc, expected, "{name}");
        let written = tiff_icc(&tiff);
        assert_eq!(written, (expected == Transferred).then(icc_blob), "{name}");
        assert!(same_pixels(&open(&png), &open(&tiff)), "{name}");
    }
}

#[test]
fn animated_png_reports_the_frames_it_drops() {
    // acTL carries the number of frames (APNG). A single-frame APNG loses nothing.
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("a.out");
    for (frames, expected) in [(1u32, NotPresent), (2, Lost), (10, Lost)] {
        let png = dir.path().join("a.png");
        let animation_control = [frames.to_be_bytes(), 0u32.to_be_bytes()].concat();
        write_png_with(&png, &[(b"acTL", animation_control)]);

        for format in [OutputFormat::Tiff, OutputFormat::Webp] {
            let transfer = from_png(&png, &out, format, true).unwrap();
            assert_eq!(transfer.frames, expected, "{frames} frames, {format:?}");
            assert_eq!(transfer.complete(), expected != Lost);
        }
    }
}

#[test]
fn metadata_off_reports_exif_as_not_present() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(&png, &[(b"eXIf", common::minimal_exif(MARKER))]);

    for (format, name) in [
        (OutputFormat::Webp, "a.webp"),
        (OutputFormat::Tiff, "a.tif"),
    ] {
        let out = dir.path().join(name);
        let transfer = from_png(&png, &out, format, false).unwrap();
        assert_eq!(transfer, meta(NotPresent, NotPresent), "{format:?}");
    }
}

#[test]
fn png_to_tiff_is_pixel_identical() {
    let dir = tempfile::tempdir().unwrap();
    for kind in [
        PngKind::Rgb8,
        PngKind::Rgba8,
        PngKind::Rgb16,
        PngKind::Gray8,
    ] {
        let png = dir.path().join(format!("{kind:?}.png"));
        let tiff = dir.path().join(format!("{kind:?}.tmp"));
        common::write_png(&png, kind);

        let transfer = from_png(&png, &tiff, OutputFormat::Tiff, true).unwrap();

        assert!(transfer.complete(), "{kind:?}");
        // Read by content: the output name `.tmp` is what the pipeline uses.
        assert_eq!(guess(&tiff), ImageFormat::Tiff);
        assert!(same_pixels(&open(&png), &open(&tiff)), "{kind:?}");
    }
}

#[test]
fn png_with_gray_and_alpha_to_tiff_is_pixel_identical() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("la.png");
    let tiff = dir.path().join("la.tif");
    GrayAlphaImage::from_fn(FIXTURE_W, FIXTURE_H, |x, y| {
        image::LumaA([(x * 3) as u8, (y * 5) as u8])
    })
    .save_with_format(&png, ImageFormat::Png)
    .unwrap();
    assert_eq!(color_of(&png), ColorType::La8);

    from_png(&png, &tiff, OutputFormat::Tiff, true).unwrap();

    assert!(same_pixels(&open(&png), &open(&tiff)));
}

#[test]
fn sixteen_bit_png_stays_sixteen_bit_in_tiff() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let tiff = dir.path().join("a.tif");
    common::write_png(&png, PngKind::Rgb16);

    from_png(&png, &tiff, OutputFormat::Tiff, true).unwrap();

    assert_eq!(color_of(&tiff), ColorType::Rgb16);
}

fn guess(path: &Path) -> ImageFormat {
    ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .format()
        .unwrap()
}

/// Largest per-channel difference at 8 bit.
fn max_diff(a: &DynamicImage, b: &DynamicImage) -> i32 {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    a.to_rgba8()
        .as_raw()
        .iter()
        .zip(b.to_rgba8().as_raw())
        .map(|(x, y)| (i32::from(*x) - i32::from(*y)).abs())
        .max()
        .unwrap()
}

#[test]
fn png_to_webp_is_lossy_but_close() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let webp = dir.path().join("a.tmp");
    common::write_png(&png, PngKind::Rgb8);

    from_png(&png, &webp, OutputFormat::Webp, true).unwrap();

    assert_eq!(guess(&webp), ImageFormat::WebP);
    assert!(max_diff(&open(&png), &open(&webp)) <= 24);
}

#[test]
fn png_alpha_survives_webp() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let webp = dir.path().join("a.webp");
    common::write_png(&png, PngKind::Rgba8);

    from_png(&png, &webp, OutputFormat::Webp, true).unwrap();

    let (before, after) = (open(&png).to_rgba8(), open(&webp).to_rgba8());
    assert!(
        after.pixels().any(|p| p[3] < 255),
        "alpha channel was dropped"
    );
    for (x, y, p) in before.enumerate_pixels() {
        let alpha_diff = (i32::from(p[3]) - i32::from(after.get_pixel(x, y)[3])).abs();
        assert!(alpha_diff <= 1, "({x},{y}): alpha {alpha_diff} off");
    }
}

#[test]
fn sixteen_bit_and_gray_png_convert_to_webp() {
    let dir = tempfile::tempdir().unwrap();
    for kind in [PngKind::Rgb16, PngKind::Gray8] {
        let png = dir.path().join(format!("{kind:?}.png"));
        let webp = dir.path().join(format!("{kind:?}.webp"));
        common::write_png(&png, kind);

        from_png(&png, &webp, OutputFormat::Webp, true).unwrap();

        assert!(max_diff(&open(&png), &open(&webp)) <= 24, "{kind:?}");
    }
}

#[test]
fn webp_refuses_images_larger_than_its_limit() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("wide.png");
    let webp = dir.path().join("wide.webp");
    image::RgbImage::new(16_384, 1).save(&png).unwrap();

    let result = from_png(&png, &webp, OutputFormat::Webp, true);

    let message = result.unwrap_err();
    assert!(
        message.starts_with("Bild ist für WebP zu groß"),
        "{message}"
    );
    assert!(!webp.exists());
}

#[test]
fn from_png_errors_are_short_german_texts() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.tif");

    let missing = from_png(
        &dir.path().join("fehlt.png"),
        &out,
        OutputFormat::Tiff,
        true,
    );
    assert_eq!(missing, Err(OPEN_ERROR.to_string()));

    let garbage = dir.path().join("kaputt.png");
    fs::write(&garbage, b"this is not an image at all").unwrap();
    for format in [OutputFormat::Tiff, OutputFormat::Webp] {
        assert_eq!(
            from_png(&garbage, &out, format, true),
            Err(READ_ERROR.to_string()),
            "{format:?}"
        );
    }

    let png = dir.path().join("a.png");
    common::write_png(&png, PngKind::Rgb8);
    for format in [OutputFormat::Auto, OutputFormat::Png, OutputFormat::Jpg] {
        assert_eq!(
            from_png(&png, &out, format, true),
            Err("Ausgabeformat wird nicht unterstützt".to_string()),
            "{format:?}"
        );
    }
    assert!(
        !out.exists(),
        "a failed conversion must not leave output behind"
    );
}

/// A PNG whose header says 40000 x 40000 RGB8 (4.8 GB decoded); the image data is junk.
fn write_huge_png(path: &Path) {
    let mut encoder =
        png::Encoder::new(BufWriter::new(File::create(path).unwrap()), 40_000, 40_000);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_chunk(ChunkType(*b"IDAT"), &[0u8; 8]).unwrap();
    writer.write_chunk(ChunkType(*b"IEND"), &[]).unwrap();
}

/// A TIFF whose header says 100000 x 100000 gray 16 bit (20 GB decoded); the
/// strip is 8 junk bytes.
fn write_huge_tiff(path: &Path) {
    const SIDE: u32 = 100_000;
    const DIRECTORY_AT: u32 = 8;
    const ENTRIES: u16 = 9;
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    let data_at = DIRECTORY_AT + 2 + 12 * u32::from(ENTRIES) + 4;
    let entries: [(u16, u16, u32); ENTRIES as usize] = [
        (256, LONG, SIDE),    // ImageWidth
        (257, LONG, SIDE),    // ImageLength
        (258, SHORT, 16),     // BitsPerSample
        (259, SHORT, 1),      // Compression: none
        (262, SHORT, 1),      // Photometric: black is zero
        (273, LONG, data_at), // StripOffsets
        (277, SHORT, 1),      // SamplesPerPixel
        (278, LONG, SIDE),    // RowsPerStrip
        (279, LONG, 8),       // StripByteCounts
    ];
    let mut bytes = b"II*\0".to_vec();
    bytes.extend(DIRECTORY_AT.to_le_bytes());
    bytes.extend(ENTRIES.to_le_bytes());
    for (tag, kind, value) in entries {
        bytes.extend(tag.to_le_bytes());
        bytes.extend(kind.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend([0u8; 4]); // no next directory
    bytes.extend([0u8; 8]);
    fs::write(path, bytes).unwrap();
}

#[test]
fn huge_image_is_refused_before_it_is_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("huge.png");
    let tiff = dir.path().join("huge.tif");
    let out = dir.path().join("huge.out");
    write_huge_png(&png);
    write_huge_tiff(&tiff);
    let too_big = Err("Bild ist zu groß zum Umwandeln".to_string());

    assert_eq!(to_png(&tiff, &out, true), too_big);
    assert_eq!(from_png(&png, &out, OutputFormat::Tiff, true), too_big);
    assert!(!out.exists());
}

#[test]
fn unwritable_destination_is_a_write_error() {
    let dir = tempfile::tempdir().unwrap();
    let bmp = dir.path().join("a.bmp");
    let png = dir.path().join("a.png");
    common::write_bmp(&bmp);
    common::write_png(&png, PngKind::Rgb8);
    let missing = dir.path().join("fehlt");
    let write_error = Err("Datei lässt sich nicht schreiben".to_string());

    assert_eq!(to_png(&bmp, &missing.join("out.png"), true), write_error);
    for format in [OutputFormat::Tiff, OutputFormat::Webp] {
        let result = from_png(&png, &missing.join("out"), format, true);
        assert_eq!(result, write_error, "{format:?}");
    }
}

// ------------------------------------------------------ strip_png_metadata

#[test]
fn strip_png_metadata_keeps_icc() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let icc = icc_blob();
    write_png_with(
        &png,
        &[
            (b"iCCP", iccp_chunk(&icc)),
            (b"eXIf", common::minimal_exif(MARKER)),
            (b"iTXt", itxt_chunk("XML:com.adobe.xmp", &xmp_packet())),
        ],
    );
    let before = open(&png);
    assert!(common::read_png_exif(&png).is_some());

    strip_png_metadata(&png).unwrap();

    let names = chunk_names(&png);
    assert!(!names.contains(&"eXIf".to_string()), "{names:?}");
    assert!(!names.contains(&"iTXt".to_string()), "{names:?}");
    assert!(names.contains(&"iCCP".to_string()), "{names:?}");
    assert_eq!(common::read_png_exif(&png), None);
    assert_eq!(icc_of(&png), Some(icc));
    assert!(same_pixels(&before, &open(&png)));
}

#[test]
fn strip_png_metadata_removes_every_exif_and_xmp_form() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    let exif_text = raw_profile_exif(&common::minimal_exif(MARKER));
    let removed: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"eXIf", common::minimal_exif(MARKER)),
        (b"zTXt", ztxt_chunk("Raw profile type exif", &exif_text)),
        (b"tEXt", text_chunk("Raw profile type exif", &exif_text)),
        (b"iTXt", itxt_chunk("Raw profile type exif", &exif_text)),
        (b"iTXt", itxt_chunk("XML:com.adobe.xmp", &xmp_packet())),
        (b"zTXt", ztxt_chunk("Raw profile type xmp", &xmp_packet())),
        (b"tEXt", text_chunk("Raw profile type xmp", &xmp_packet())),
    ];
    let kept: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"iCCP", iccp_chunk(&icc_blob())),
        (b"tEXt", text_chunk("Comment", "stays")),
        (b"zTXt", ztxt_chunk("Software", "stays too")),
        (b"iTXt", itxt_chunk("Title", "and this")),
        (b"zTXt", ztxt_chunk("Raw profile type iptc", "not ours")),
    ];
    // Interleave: removed and kept chunks alternate.
    let mut chunks = Vec::new();
    for i in 0..removed.len().max(kept.len()) {
        chunks.extend(removed.get(i).cloned());
        chunks.extend(kept.get(i).cloned());
    }
    write_png_with(&png, &chunks);
    let before_chunks = png_chunks(&png);
    let before = open(&png);

    strip_png_metadata(&png).unwrap();

    // Exactly the kept chunks remain, untouched and in their old order.
    let expected: Vec<_> = before_chunks
        .into_iter()
        .filter(|(kind, data)| match kind.as_str() {
            "eXIf" => false,
            "tEXt" | "zTXt" | "iTXt" => !matches!(
                keyword(data).as_str(),
                "Raw profile type exif" | "Raw profile type xmp" | "XML:com.adobe.xmp"
            ),
            _ => true,
        })
        .collect();
    assert_eq!(png_chunks(&png), expected);
    let kept_text = expected
        .iter()
        .filter(|(kind, _)| matches!(kind.as_str(), "tEXt" | "zTXt" | "iTXt"))
        .count();
    assert_eq!(kept_text, 4, "the fixture keeps four text chunks");
    assert!(same_pixels(&before, &open(&png)));
    assert_eq!(common::read_png_exif(&png), None);
}

#[test]
fn strip_png_metadata_leaves_a_plain_png_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    common::write_png(&png, PngKind::Rgb16);
    let before = fs::read(&png).unwrap();

    strip_png_metadata(&png).unwrap();

    assert_eq!(fs::read(&png).unwrap(), before);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        1,
        "no stray files next to the PNG"
    );
}

#[test]
fn strip_png_metadata_works_on_tmp_names() {
    // The pipeline strips its `.png.tmp` intermediates.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("foto.png.tmp");
    write_png_with(&png, &[(b"eXIf", common::minimal_exif(MARKER))]);

    strip_png_metadata(&png).unwrap();

    assert!(!chunk_names(&png).contains(&"eXIf".to_string()));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn strip_png_metadata_rejects_bad_files_and_leaves_them_alone() {
    let dir = tempfile::tempdir().unwrap();

    let missing = strip_png_metadata(&dir.path().join("fehlt.png"));
    assert_eq!(missing, Err(OPEN_ERROR.to_string()));

    let garbage = dir.path().join("kaputt.png");
    fs::write(&garbage, b"this is not an image at all").unwrap();
    assert_eq!(strip_png_metadata(&garbage), Err(READ_ERROR.to_string()));

    // A PNG cut off before IEND, once right before it, once inside the image data.
    let whole = dir.path().join("whole.png");
    write_png_with(&whole, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let bytes = fs::read(&whole).unwrap();
    for (name, len) in [
        ("short.png", bytes.len() - 12),
        ("cut.png", bytes.len() - 20),
    ] {
        let cut = dir.path().join(name);
        fs::write(&cut, &bytes[..len]).unwrap();
        assert_eq!(
            strip_png_metadata(&cut),
            Err(READ_ERROR.to_string()),
            "{name}"
        );
        assert_eq!(fs::read(&cut).unwrap(), &bytes[..len], "{name}");
    }

    assert_eq!(fs::read(&garbage).unwrap(), b"this is not an image at all");
    let files = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(files, 4, "whole, kaputt, short and cut - no stray files");
}

#[test]
#[allow(clippy::permissions_set_readonly_false)] // the app is Windows-only
fn strip_png_metadata_that_cannot_replace_the_file_cleans_up() {
    // Windows will not replace a read-only file.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    write_png_with(&png, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let before = fs::read(&png).unwrap();
    let mut permissions = fs::metadata(&png).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&png, permissions.clone()).unwrap();

    let result = strip_png_metadata(&png);

    permissions.set_readonly(false);
    fs::set_permissions(&png, permissions).unwrap();
    assert_eq!(result, Err("Datei lässt sich nicht bearbeiten".to_string()));
    assert_eq!(fs::read(&png).unwrap(), before);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

// ----------------------------------------------------------- end to end

/// Lossless cjxl then djxl: `input` (anything cjxl reads) -> `.jxl` -> PNG, all next to
/// it. Returns the PNG that djxl wrote. cjxl gets no strip options, so only what is in
/// the input can reach it.
fn through_cjxl_and_djxl(input: &Path) -> std::path::PathBuf {
    let jxl = input.with_extension("jxl");
    let back = input.with_file_name("back.png");
    let tools = common::tools();
    let cancel = CancelToken::new();
    let settings = EncodeSettings {
        distance: 0.0,
        effort: 3,
        transcode_jpeg: false,
    };
    tools
        .run_cjxl(
            input,
            &jxl,
            &cjxl_args(InputKind::Webp, &settings, true),
            &cancel,
        )
        .expect("cjxl failed");
    let djxl = djxl_args(DjxlTarget::Png {
        reduce_to_8bit: false,
    });
    tools
        .run_djxl(&jxl, &back, &djxl, &cancel)
        .expect("djxl failed");
    back
}

#[test]
fn webp_exif_survives_cjxl_and_djxl() {
    // The real proof that the metadata hand-over works: what cjxl reads from the
    // intermediate PNG is what djxl writes back out.
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    common::write_webp_with_exif(&webp, &common::minimal_exif(MARKER));

    let transfer = to_png(&webp, &png, true).unwrap();
    assert_eq!(transfer.exif, Transferred);
    let back = through_cjxl_and_djxl(&png);

    let restored = common::read_png_exif(&back).expect("EXIF did not survive cjxl + djxl");
    assert!(contains(&restored, MARKER.as_bytes()), "{restored:?}");
    assert!(same_pixels(&open(&webp), &open(&back)));
}

#[test]
fn metadata_off_leaves_no_exif_after_cjxl_and_djxl() {
    let dir = tempfile::tempdir().unwrap();
    let webp = dir.path().join("a.webp");
    let png = dir.path().join("a.png");
    common::write_webp_with_exif(&webp, &common::minimal_exif(MARKER));

    to_png(&webp, &png, false).unwrap();
    let back = through_cjxl_and_djxl(&png);

    assert_eq!(common::read_png_exif(&back), None);
}

#[test]
fn tiff_icc_profile_survives_cjxl_and_djxl() {
    // Adobe RGB: djxl writes the profile back as `iCCP` (an sRGB one would be
    // turned into plain sRGB chunks).
    let icc = moxcms::ColorProfile::new_adobe_rgb().encode().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let tiff = dir.path().join("a.tif");
    let png = dir.path().join("a.png");
    write_tiff_with(&tiff, &[Extra::Icc(icc.clone())]);

    let transfer = to_png(&tiff, &png, false).unwrap();
    assert_eq!(transfer.icc, Transferred);
    let back = through_cjxl_and_djxl(&png);

    assert_eq!(icc_of(&back), Some(icc));
    assert!(same_pixels(&open(&tiff), &open(&back)));
}

/// A GIF with `frames` frames that differ from each other. The frames get a delay:
/// djxl leaves out frames that last 0 ms.
fn write_animated_gif(path: &Path, frames: u8) {
    let frames: Vec<image::Frame> = (0..frames)
        .map(|i| {
            let picture = image::RgbaImage::from_fn(FIXTURE_W, FIXTURE_H, |x, y| {
                image::Rgba([(x * 4) as u8, (y * 5) as u8, (90 * u32::from(i)) as u8, 255])
            });
            let delay = image::Delay::from_numer_denom_ms(100, 1);
            image::Frame::from_parts(picture, 0, 0, delay)
        })
        .collect();
    let mut encoder = image::codecs::gif::GifEncoder::new(File::create(path).unwrap());
    encoder.encode_frames(frames).unwrap();
}

#[test]
fn animated_jxl_becomes_an_animated_png_and_its_extra_frames_are_reported_lost() {
    // The real chain: GIF -> cjxl -> JXL animation -> djxl -> PNG -> TIFF.
    let dir = tempfile::tempdir().unwrap();
    let gif = dir.path().join("anim.gif");
    let tiff = dir.path().join("anim.tif");
    write_animated_gif(&gif, 2);

    let apng = through_cjxl_and_djxl(&gif);
    let names = chunk_names(&apng);
    assert!(
        names.contains(&"acTL".to_string()),
        "djxl wrote no APNG: {names:?}"
    );

    let transfer = from_png(&apng, &tiff, OutputFormat::Tiff, true).unwrap();

    assert_eq!(transfer.frames, Lost);
    assert!(!transfer.complete());
    let first = open(&tiff);
    assert_eq!((first.width(), first.height()), (FIXTURE_W, FIXTURE_H));
}

#[test]
fn strip_jpeg_metadata_drops_app1_and_app13_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("foto.jpg");
    common::write_jpeg(
        &path,
        FIXTURE_W,
        FIXTURE_H,
        common::JpegOpts {
            exif: Some(common::minimal_exif("TESTMARKER")),
            ..Default::default()
        },
    );
    // Insert an APP13 (Photoshop/IPTC) segment and fill bytes right after SOI.
    let mut bytes = fs::read(&path).unwrap();
    let app13 = [0xFF, 0xFF, 0xED, 0x00, 0x06, b'I', b'P', b'T', b'C'];
    bytes.splice(2..2, app13);
    fs::write(&path, &bytes).unwrap();
    let before = image::open(&path).unwrap();

    strip_jpeg_metadata(&path).unwrap();
    let stripped = fs::read(&path).unwrap();
    assert!(!stripped.windows(6).any(|w| w == b"Exif\0\0"));
    assert!(!stripped.windows(4).any(|w| w == b"IPTC"));
    assert!(stripped.windows(2).any(|w| w == [0xFF, 0xDB]), "DQT kept");
    assert_eq!(image::open(&path).unwrap(), before, "image data untouched");

    // Nothing left to strip: the file stays as it is.
    strip_jpeg_metadata(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), stripped);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "no tmp left");

    fs::write(&path, b"kein jpeg").unwrap();
    assert!(strip_jpeg_metadata(&path).is_err());
}
