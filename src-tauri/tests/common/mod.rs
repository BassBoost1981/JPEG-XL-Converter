//! Shared fixture helpers for the integration tests: locating the libjxl
//! tools and writing small test images in every input format.
//!
//! Each test binary compiles this module on its own and uses only part of it.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use image::codecs::gif::GifEncoder;
use image::codecs::webp::WebPEncoder;
use image::{
    Delay, DynamicImage, ExtendedColorType, Frame, GrayImage, ImageBuffer, ImageEncoder,
    ImageFormat, Rgb, RgbImage, Rgba, RgbaImage,
};
use jpeg_xl_konverter_lib::engine::msg::Msg;
use jpeg_xl_konverter_lib::engine::tools::Tools;
use png::chunk::ChunkType;

/// Size of every image written by the helpers that take no size argument.
pub const FIXTURE_W: u32 = 64;
pub const FIXTURE_H: u32 = 48;

/// The catalog keys of `msgs`, in order.
pub fn keys_of(msgs: &[Msg]) -> Vec<&str> {
    msgs.iter().map(|m| m.key.as_str()).collect()
}

/// Finds `cjxl`/`djxl` the way the app does (next to the test binary, then in
/// `src-tauri/binaries`). Panics with a hint when they are missing.
pub fn tools() -> Tools {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    Tools::locate(&exe_dir)
        .expect("cjxl/djxl not found in src-tauri/binaries - run scripts/fetch-libjxl.ps1 first")
}

/// `v` in `0..n` scaled to `0..=max` (a single step maps to 0).
fn scale(v: u32, n: u32, max: u32) -> u32 {
    v * max / (n - 1).max(1)
}

/// Smooth RGB gradient whose alpha also varies, so every channel is exercised.
pub fn gradient_rgba8(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([
            scale(x, w, 255) as u8,
            scale(y, h, 255) as u8,
            scale(x + y, w + h - 1, 255) as u8,
            (255 - scale(x, w, 127)) as u8,
        ])
    })
}

fn gradient_rgb8(w: u32, h: u32) -> RgbImage {
    DynamicImage::ImageRgba8(gradient_rgba8(w, h)).to_rgb8()
}

/// Gradient with real 16-bit precision (not an 8-bit image widened by 257).
fn gradient_rgb16(w: u32, h: u32) -> ImageBuffer<Rgb<u16>, Vec<u16>> {
    ImageBuffer::from_fn(w, h, |x, y| {
        Rgb([
            scale(x, w, 65535) as u16,
            scale(y, h, 65535) as u16,
            scale(x + y, w + h - 1, 65535) as u16,
        ])
    })
}

#[derive(Debug, Clone, Default)]
pub struct JpegOpts {
    pub progressive: bool,
    /// Four-component CMYK JPEG instead of YCbCr.
    pub cmyk: bool,
    /// Raw TIFF-structured EXIF data; written to an APP1 segment behind `Exif\0\0`.
    pub exif: Option<Vec<u8>>,
}

pub fn write_jpeg(path: &Path, w: u32, h: u32, opts: JpegOpts) {
    let (width, height) = (u16::try_from(w).unwrap(), u16::try_from(h).unwrap());
    let rgb = gradient_rgb8(w, h);

    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, 90);
    encoder.set_progressive(opts.progressive);
    if let Some(exif) = &opts.exif {
        encoder.add_exif_metadata(exif).unwrap();
    }
    if opts.cmyk {
        let cmyk: Vec<u8> = rgb
            .pixels()
            .flat_map(|p| [p[0], p[1], p[2], p[0] / 4])
            .collect();
        encoder
            .encode(&cmyk, width, height, jpeg_encoder::ColorType::Cmyk)
            .unwrap();
    } else {
        encoder
            .encode(rgb.as_raw(), width, height, jpeg_encoder::ColorType::Rgb)
            .unwrap();
    }
    fs::write(path, out).unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngKind {
    Rgb8,
    Rgba8,
    Rgb16,
    Gray8,
}

pub fn write_png(path: &Path, kind: PngKind) {
    let (w, h) = (FIXTURE_W, FIXTURE_H);
    match kind {
        PngKind::Rgb8 => gradient_rgb8(w, h).save_with_format(path, ImageFormat::Png),
        PngKind::Rgba8 => gradient_rgba8(w, h).save_with_format(path, ImageFormat::Png),
        PngKind::Rgb16 => gradient_rgb16(w, h).save_with_format(path, ImageFormat::Png),
        PngKind::Gray8 => GrayImage::from_fn(w, h, |x, y| {
            image::Luma([((scale(x, w, 255) + scale(y, h, 255)) / 2) as u8])
        })
        .save_with_format(path, ImageFormat::Png),
    }
    .unwrap();
}

/// 16-bit RGB TIFF.
pub fn write_tiff16(path: &Path) {
    gradient_rgb16(FIXTURE_W, FIXTURE_H)
        .save_with_format(path, ImageFormat::Tiff)
        .unwrap();
}

pub fn write_bmp(path: &Path) {
    gradient_rgb8(FIXTURE_W, FIXTURE_H)
        .save_with_format(path, ImageFormat::Bmp)
        .unwrap();
}

/// Single-frame GIF.
pub fn write_gif(path: &Path) {
    gradient_rgba8(FIXTURE_W, FIXTURE_H)
        .save_with_format(path, ImageFormat::Gif)
        .unwrap();
}

/// Appends a RIFF chunk: fourcc, little-endian size, payload, pad byte to even length.
fn push_riff_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

/// Lossless WebP in the extended container: `VP8X` (EXIF flag set), `VP8L`, `EXIF`.
/// `exif` is the raw TIFF-structured data without the `Exif\0\0` prefix.
pub fn write_webp_with_exif(path: &Path, exif: &[u8]) {
    const EXIF_FLAG: u8 = 0x08;

    let rgb = gradient_rgb8(FIXTURE_W, FIXTURE_H);
    let mut plain = Vec::new();
    WebPEncoder::new_lossless(&mut plain)
        .write_image(rgb.as_raw(), FIXTURE_W, FIXTURE_H, ExtendedColorType::Rgb8)
        .unwrap();

    // Find the VP8L chunk inside the simple container the encoder wrote.
    assert!(plain.starts_with(b"RIFF") && &plain[8..12] == b"WEBP");
    let mut pos = 12;
    let vp8l = loop {
        let len = u32::from_le_bytes(plain[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if &plain[pos..pos + 4] == b"VP8L" {
            break &plain[pos + 8..pos + 8 + len];
        }
        pos += 8 + len + len % 2;
    };

    // VP8X payload: flags, 3 reserved bytes, canvas width - 1, canvas height - 1 (24 bit each).
    let mut vp8x = vec![EXIF_FLAG, 0, 0, 0];
    vp8x.extend_from_slice(&(FIXTURE_W - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(FIXTURE_H - 1).to_le_bytes()[..3]);

    let mut body = b"WEBP".to_vec();
    push_riff_chunk(&mut body, b"VP8X", &vp8x);
    push_riff_chunk(&mut body, b"VP8L", vp8l);
    push_riff_chunk(&mut body, b"EXIF", exif);

    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    fs::write(path, file).unwrap();
}

/// Smallest valid EXIF block: a big-endian TIFF header and one IFD0 entry,
/// `ImageDescription` (0x010E, ASCII) holding `marker`.
pub fn minimal_exif(marker: &str) -> Vec<u8> {
    const IFD0_OFFSET: u32 = 8;
    const ASCII: u16 = 2;
    // Header (8) + entry count (2) + one entry (12) + next-IFD offset (4).
    const VALUE_OFFSET: u32 = IFD0_OFFSET + 2 + 12 + 4;

    let mut text = marker.as_bytes().to_vec();
    text.push(0);

    let mut exif = b"MM\0*".to_vec();
    exif.extend_from_slice(&IFD0_OFFSET.to_be_bytes());
    exif.extend_from_slice(&1u16.to_be_bytes());
    exif.extend_from_slice(&0x010Eu16.to_be_bytes());
    exif.extend_from_slice(&ASCII.to_be_bytes());
    exif.extend_from_slice(&(text.len() as u32).to_be_bytes());
    if text.len() <= 4 {
        // Short values are stored left-aligned in the entry's value field.
        text.resize(4, 0);
        exif.extend_from_slice(&text);
        exif.extend_from_slice(&0u32.to_be_bytes());
    } else {
        exif.extend_from_slice(&VALUE_OFFSET.to_be_bytes());
        exif.extend_from_slice(&0u32.to_be_bytes());
        exif.extend_from_slice(&text);
    }
    exif
}

/// The EXIF block (TIFF structure, no `Exif\0\0` prefix) of a PNG, wherever it
/// sits in the file. Looks for an `eXIf` chunk and for the `zTXt` chunk
/// "Raw profile type exif": djxl v0.12.0 writes the latter, never `eXIf`.
/// `None` if the file is missing, not a PNG, or carries no EXIF.
pub fn read_png_exif(path: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;
    let mut rest = bytes.strip_prefix(b"\x89PNG\r\n\x1a\n")?;
    // Each chunk: length (4, big endian), type (4), data, CRC (4).
    while rest.len() >= 12 {
        let len = u32::from_be_bytes(rest[..4].try_into().ok()?) as usize;
        let data = rest.get(8..8 + len)?;
        match &rest[4..8] {
            b"eXIf" => return Some(data.to_vec()),
            b"zTXt" => {
                if let Some(exif) = raw_profile_exif(data) {
                    return Some(exif);
                }
            }
            b"IEND" => return None,
            _ => {}
        }
        rest = rest.get(12 + len..)?;
    }
    None
}

/// Decodes a `zTXt` chunk holding an ImageMagick-style raw profile:
/// zlib-compressed text `"\nexif\n<length>\n<hex digits, wrapped>\n"` whose bytes
/// are `Exif\0\0` followed by the TIFF block.
fn raw_profile_exif(chunk: &[u8]) -> Option<Vec<u8>> {
    let keyword_end = chunk.iter().position(|&b| b == 0)?;
    if &chunk[..keyword_end] != b"Raw profile type exif" || *chunk.get(keyword_end + 1)? != 0 {
        return None; // not the exif profile, or not zlib compression
    }
    let text = miniz_oxide::inflate::decompress_to_vec_zlib(&chunk[keyword_end + 2..]).ok()?;
    let text = String::from_utf8(text).ok()?;
    let mut words = text.split_whitespace();
    if words.next()? != "exif" {
        return None;
    }
    let declared: usize = words.next()?.parse().ok()?;
    let hex: String = words.collect();
    let bytes = (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    if bytes.len() != declared {
        return None;
    }
    Some(bytes.strip_prefix(b"Exif\0\0").unwrap_or(&bytes).to_vec())
}

/// An RGB8 PNG with `chunks` written between IHDR and the image data.
pub fn write_png_with(path: &Path, chunks: &[(&[u8; 4], Vec<u8>)]) {
    let (w, h) = (FIXTURE_W, FIXTURE_H);
    let pixels: Vec<u8> = (0..w * h * 3).map(|i| (i * 7 % 251) as u8).collect();
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path).unwrap()), w, h);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    for (kind, data) in chunks {
        writer.write_chunk(ChunkType(**kind), data).unwrap();
    }
    writer.write_image_data(&pixels).unwrap();
    writer.finish().unwrap();
}

/// EXIF block (big-endian TIFF structure) with only the Orientation tag.
pub fn orientation_exif(orientation: u16) -> Vec<u8> {
    const SHORT: u16 = 3;
    let mut exif = b"MM\0*".to_vec();
    exif.extend_from_slice(&8u32.to_be_bytes());
    exif.extend_from_slice(&1u16.to_be_bytes());
    exif.extend_from_slice(&0x0112u16.to_be_bytes());
    exif.extend_from_slice(&SHORT.to_be_bytes());
    exif.extend_from_slice(&1u32.to_be_bytes());
    exif.extend_from_slice(&orientation.to_be_bytes());
    exif.extend_from_slice(&[0, 0]);
    exif.extend_from_slice(&0u32.to_be_bytes());
    exif
}

/// A GIF with `frames` different frames.
pub fn write_animated_gif(path: &Path, frames: u32) {
    let mut encoder = GifEncoder::new(File::create(path).unwrap());
    encoder
        .encode_frames((0..frames).map(|i| {
            let img = RgbaImage::from_fn(FIXTURE_W, FIXTURE_H, |x, y| {
                Rgba([(x * 4) as u8, (y * 5) as u8, (i * 90) as u8, 255])
            });
            Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))
        }))
        .unwrap();
}
