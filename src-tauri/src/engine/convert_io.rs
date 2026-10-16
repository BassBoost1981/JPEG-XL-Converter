//! TIFF, WebP and BMP <-> PNG. cjxl cannot read those three, so they take a
//! detour through an intermediate PNG that carries ICC profile and EXIF, and
//! djxl's PNG is turned into TIFF or WebP on the way back. Every conversion
//! says what happened to the metadata (`MetaTransfer`); the pipelines use
//! that to decide whether an original may be moved to the recycle bin, so a
//! field is only `Transferred` or `NotPresent` when that is certain.
//!
//! The `exif` field stands for the descriptive metadata the user can switch
//! off with "keep metadata": EXIF and XMP. The `icc` field is the colour
//! profile, which defines what the pixel values mean; it is therefore carried
//! into the PNG whatever the metadata switch says. The `frames` field is not
//! metadata but belongs to the same question, "may the original go?": a
//! multi-page TIFF or animated WebP is converted from its first page or frame
//! only, and the rest is `Lost`.
//!
//! User-facing errors are short German texts without library detail.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::tiff::TiffEncoder;
use image::{
    ColorType, DynamicImage, ImageDecoder, ImageEncoder, ImageError, ImageFormat, ImageReader,
    Limits,
};
use tiff::tags::Tag;

use super::types::OutputFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaState {
    /// The source had nothing of this kind (or the user switched it off).
    NotPresent,
    /// Written to the result.
    Transferred,
    /// The source had it and the result does not.
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetaTransfer {
    /// EXIF and XMP.
    pub exif: MetaState,
    pub icc: MetaState,
    /// Further pages (multi-page TIFF) or frames (animated WebP) of the source.
    /// A conversion keeps the first one only, so these are `Lost` or `NotPresent`.
    pub frames: MetaState,
}

impl MetaTransfer {
    /// `true` if nothing was lost.
    pub fn complete(&self) -> bool {
        [self.exif, self.icc, self.frames]
            .iter()
            .all(|&state| state != MetaState::Lost)
    }
}

const OPEN_ERROR: &str = "Datei lässt sich nicht öffnen";
const READ_ERROR: &str = "Bild lässt sich nicht lesen";
const WRITE_ERROR: &str = "Datei lässt sich nicht schreiben";
const EDIT_ERROR: &str = "Datei lässt sich nicht bearbeiten";
const TOO_BIG_ERROR: &str = "Bild ist zu groß zum Umwandeln";
const FORMAT_ERROR: &str = "Ausgabeformat wird nicht unterstützt";

/// Largest decoded image accepted. The image crate's default of 512 MiB would
/// turn away big 16-bit scans.
const MAX_DECODED_BYTES: u64 = 4 << 30;
/// libwebp cannot store a side longer than this.
const WEBP_MAX_SIDE: u32 = 16_383;
const WEBP_QUALITY: f32 = 90.0;
/// Some writers put this APP1 header in front of the TIFF block of an EXIF chunk.
const EXIF_HEADER: &[u8] = b"Exif\0\0";

/// `Lost` if the source had something that is not in the result, else `NotPresent`.
fn lost(present: bool) -> MetaState {
    if present {
        MetaState::Lost
    } else {
        MetaState::NotPresent
    }
}

/// Converts a TIFF, WebP or BMP (any format the image crate reads) to a PNG
/// that cjxl can take. 16-bit stays 16-bit, float becomes 16-bit.
///
/// ICC profile: carried when the source has one, whatever `keep_metadata` says.
/// EXIF and XMP: only looked at when `keep_metadata` is set (otherwise
/// `NotPresent`). EXIF goes into an `eXIf` chunk; XMP cannot be carried and
/// counts as `Lost`.
///
/// What the decoders can tell (image 0.25.10):
/// - WebP: EXIF, ICC and XMP, read from the RIFF chunks.
/// - TIFF: ICC and XMP, but no EXIF, because TIFF keeps EXIF as tags of its
///   first directory. If it has such tags the EXIF is `Lost` (see
///   `tiff_metadata`, which also explains why the image crate is not asked).
/// - BMP: neither. (Profiles embedded in BMP v5 files are not read; they are rare.)
///
/// Only the first page of a TIFF or the first frame of an animated WebP is
/// converted; if there are more, `frames` is `Lost`.
///
/// A failed conversion leaves no output file behind.
pub fn to_png(src: &Path, dst_png: &Path, keep_metadata: bool) -> Result<MetaTransfer, String> {
    let reader = open_reader(src)?;
    let format = reader.format();
    let mut decoder = decoder_for(reader)?;
    let mut meta = if format == Some(ImageFormat::Tiff) {
        tiff_metadata(src, keep_metadata)?
    } else {
        decoder_metadata(&mut decoder, keep_metadata)?
    };
    if format == Some(ImageFormat::WebP) {
        meta.more_frames = webp_has_more_frames(src)?;
    }
    let image = DynamicImage::from_decoder(decoder).map_err(|e| image_error(&e))?;

    let exif_state = if !keep_metadata {
        MetaState::NotPresent
    } else if meta.uncarried {
        MetaState::Lost
    } else if meta.exif.is_some() {
        MetaState::Transferred
    } else {
        MetaState::NotPresent
    };
    let icc_state = if meta.icc.is_some() {
        MetaState::Transferred
    } else {
        MetaState::NotPresent
    };

    write_png(&integer_image(image), dst_png, meta.icc, meta.exif)?;
    Ok(MetaTransfer {
        exif: exif_state,
        icc: icc_state,
        frames: lost(meta.more_frames),
    })
}

/// Converts the PNG djxl wrote to TIFF (lossless) or WebP (lossy, quality 90).
/// Other formats are refused.
///
/// The TIFF takes the ICC profile along (`Transferred`; `Lost` only if the PNG
/// has an `iCCP` chunk that cannot be read). Neither output carries EXIF or XMP,
/// so these are `Lost` if the PNG has them and the user did not switch metadata
/// off; WebP does not carry the ICC profile either. `frames` is always
/// `NotPresent`: a PNG from djxl has one image.
///
/// A failed conversion leaves no output file behind.
pub fn from_png(
    src_png: &Path,
    dst: &Path,
    format: OutputFormat,
    keep_metadata: bool,
) -> Result<MetaTransfer, String> {
    if !matches!(format, OutputFormat::Tiff | OutputFormat::Webp) {
        return Err(FORMAT_ERROR.to_string());
    }
    let found = scan_png(src_png)?;
    let mut decoder = decoder_for(open_reader(src_png)?)?;
    let icc = non_empty(decoder.icc_profile().map_err(|e| image_error(&e))?);
    let image = DynamicImage::from_decoder(decoder).map_err(|e| image_error(&e))?;

    let icc_state = if format == OutputFormat::Tiff {
        let carried = icc.is_some();
        write_tiff(image, icc, dst)?;
        if carried {
            MetaState::Transferred
        } else {
            lost(found.icc)
        }
    } else {
        write_webp(image, dst)?;
        lost(found.icc)
    };
    Ok(MetaTransfer {
        exif: lost(keep_metadata && (found.exif || found.xmp)),
        icc: icc_state,
        frames: MetaState::NotPresent,
    })
}

/// Rewrites a PNG without EXIF and XMP: the `eXIf` chunk and the `tEXt`,
/// `zTXt` and `iTXt` chunks with the keywords `Raw profile type exif`,
/// `Raw profile type xmp` and `XML:com.adobe.xmp` (djxl writes EXIF as
/// a `zTXt` chunk with the first of those keywords, never as `eXIf`).
/// Everything else, the ICC profile included, is copied byte for byte. A PNG
/// without such chunks is not touched.
pub fn strip_png_metadata(path: &Path) -> Result<(), String> {
    let mut input = BufReader::new(File::open(path).map_err(|_| OPEN_ERROR.to_string())?);
    let found = scan(&mut input).map_err(|_| READ_ERROR.to_string())?;
    if !found.exif && !found.xmp {
        return Ok(());
    }

    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".strip.tmp");
    let tmp = path.with_file_name(tmp_name);
    input.rewind().map_err(|_| EDIT_ERROR.to_string())?;
    write_file(&tmp, |out| {
        copy_without_metadata(&mut input, out).map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData => READ_ERROR.to_string(),
            _ => EDIT_ERROR.to_string(),
        })
    })?;
    drop(input); // Windows cannot replace a file that is still open
    fs::rename(&tmp, path).map_err(|_| {
        let _ = fs::remove_file(&tmp);
        EDIT_ERROR.to_string()
    })
}

// ------------------------------------------------------------------ reading

fn open_reader(path: &Path) -> Result<ImageReader<BufReader<File>>, String> {
    let mut reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(|_| OPEN_ERROR.to_string())?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    Ok(reader)
}

/// The decoder for `reader`, refused if the decoded image would be bigger than
/// `MAX_DECODED_BYTES`. `ImageReader::decode` makes that check on its own,
/// `into_decoder` followed by `DynamicImage::from_decoder` does not.
fn decoder_for(reader: ImageReader<BufReader<File>>) -> Result<impl ImageDecoder, String> {
    let decoder = reader.into_decoder().map_err(|e| image_error(&e))?;
    if decoder.total_bytes() > MAX_DECODED_BYTES {
        return Err(TOO_BIG_ERROR.to_string());
    }
    Ok(decoder)
}

fn image_error(error: &ImageError) -> String {
    match error {
        ImageError::Limits(_) => TOO_BIG_ERROR,
        _ => READ_ERROR,
    }
    .to_string()
}

/// What a source file says about its metadata.
#[derive(Default)]
struct SourceMeta {
    icc: Option<Vec<u8>>,
    /// The TIFF block for an `eXIf` chunk.
    exif: Option<Vec<u8>>,
    /// The source has XMP, or EXIF that the PNG cannot take along.
    uncarried: bool,
    /// The source has more pages or frames than the first.
    more_frames: bool,
}

fn non_empty(bytes: Option<Vec<u8>>) -> Option<Vec<u8>> {
    bytes.filter(|b| !b.is_empty())
}

/// The TIFF block of an EXIF chunk, without the optional `Exif\0\0` header.
fn tiff_block(exif: Vec<u8>) -> Vec<u8> {
    match exif.strip_prefix(EXIF_HEADER) {
        Some(block) => block.to_vec(),
        None => exif,
    }
}

/// The metadata an image-crate decoder reports. EXIF and XMP only with `keep_metadata`.
fn decoder_metadata(
    decoder: &mut impl ImageDecoder,
    keep_metadata: bool,
) -> Result<SourceMeta, String> {
    let icc = decoder.icc_profile().map_err(|e| image_error(&e))?;
    let mut meta = SourceMeta {
        icc: non_empty(icc),
        ..SourceMeta::default()
    };
    if keep_metadata {
        let exif = decoder.exif_metadata().map_err(|e| image_error(&e))?;
        let xmp = decoder.xmp_metadata().map_err(|e| image_error(&e))?;
        meta.exif = non_empty(exif.map(tiff_block));
        meta.uncarried = non_empty(xmp).is_some();
    }
    Ok(meta)
}

/// The metadata of a TIFF, read with the tiff crate itself. The image crate's
/// TIFF decoder is no use here: it sizes the limit for tag values by the pixel
/// data, so on a small image it silently drops the ICC profile and fails on an
/// XMP packet.
///
/// A TIFF keeps EXIF as tags of its first directory, not as a block we could
/// copy. They count as EXIF when the directory has a pointer to the EXIF or GPS
/// directory or one of the descriptive tags a camera or editor writes (an
/// orientation of 1, "upright", is the default and does not count); then the
/// EXIF is uncarried. A tag that exists but cannot be read counts as present.
fn tiff_metadata(path: &Path, keep_metadata: bool) -> Result<SourceMeta, String> {
    const EXIF_TAGS: [Tag; 8] = [
        Tag::ExifDirectory,
        Tag::GpsDirectory,
        Tag::ImageDescription,
        Tag::Make,
        Tag::Model,
        Tag::DateTime,
        Tag::Artist,
        Tag::Copyright,
    ];
    const XMP_TAG: Tag = Tag::Unknown(700);
    fn present<T>(found: Result<Option<T>, tiff::TiffError>) -> bool {
        !matches!(found, Ok(None))
    }

    let file = File::open(path).map_err(|_| OPEN_ERROR.to_string())?;
    let mut decoder =
        tiff::decoder::Decoder::new(BufReader::new(file)).map_err(|_| READ_ERROR.to_string())?;
    let icc = match decoder.find_tag(Tag::IccProfile) {
        Ok(None) => None,
        Ok(Some(profile)) => Some(profile.into_u8_vec().map_err(|_| READ_ERROR.to_string())?),
        Err(_) => return Err(READ_ERROR.to_string()),
    };
    let mut meta = SourceMeta {
        icc: non_empty(icc),
        more_frames: decoder.more_images(),
        ..SourceMeta::default()
    };
    if keep_metadata {
        let rotated = match decoder.find_tag(Tag::Orientation) {
            Ok(None) => false,
            Ok(Some(value)) => value.into_u16().map_or(true, |o| o != 1),
            Err(_) => true,
        };
        meta.uncarried = rotated
            || present(decoder.find_tag(XMP_TAG))
            || EXIF_TAGS
                .into_iter()
                .any(|tag| present(decoder.find_tag(tag)));
    }
    Ok(meta)
}

/// `true` if a WebP is an animation with more than one frame (`ANMF` chunk).
fn webp_has_more_frames(path: &Path) -> Result<bool, String> {
    let mut input = BufReader::new(File::open(path).map_err(|_| OPEN_ERROR.to_string())?);
    let mut riff = [0u8; 12];
    input
        .read_exact(&mut riff)
        .map_err(|_| READ_ERROR.to_string())?;
    if &riff[..4] != b"RIFF" || &riff[8..] != b"WEBP" {
        return Err(READ_ERROR.to_string());
    }
    let mut frames = 0;
    loop {
        let mut header = [0u8; 8];
        match input.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(_) => return Err(READ_ERROR.to_string()),
        }
        if &header[..4] == b"ANMF" {
            frames += 1;
            if frames > 1 {
                return Ok(true);
            }
        }
        // Chunk data is padded to an even length.
        let size = i64::from(u32::from_le_bytes(header[4..].try_into().unwrap()));
        input
            .seek_relative(size + (size & 1))
            .map_err(|_| READ_ERROR.to_string())?;
    }
}

// ------------------------------------------------------------------ writing

/// Runs `write` on a buffered new file at `dst` and flushes it. On any error
/// the half-written file is removed again.
fn write_file(
    dst: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<(), String>,
) -> Result<(), String> {
    let mut out = BufWriter::new(File::create(dst).map_err(|_| WRITE_ERROR.to_string())?);
    let result = write(&mut out).and_then(|()| out.flush().map_err(|_| WRITE_ERROR.to_string()));
    drop(out);
    if result.is_err() {
        let _ = fs::remove_file(dst);
    }
    result
}

/// PNG and TIFF output take 8 and 16 bit integers; float becomes 16 bit.
fn integer_image(image: DynamicImage) -> DynamicImage {
    match image.color() {
        ColorType::Rgb32F => image.to_rgb16().into(),
        ColorType::Rgba32F => image.to_rgba16().into(),
        _ => image,
    }
}

/// `eXIf` comes before the image data, where cjxl looks for it, and after `iCCP`.
fn write_png(
    image: &DynamicImage,
    dst: &Path,
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
) -> Result<(), String> {
    write_file(dst, |out| {
        // A temporary file for cjxl: fast to write matters more than small.
        let mut encoder =
            PngEncoder::new_with_quality(out, CompressionType::Fast, FilterType::Adaptive);
        if let Some(icc) = icc {
            encoder
                .set_icc_profile(icc)
                .map_err(|_| WRITE_ERROR.to_string())?;
        }
        if let Some(exif) = exif {
            encoder
                .set_exif_metadata(exif)
                .map_err(|_| WRITE_ERROR.to_string())?;
        }
        image
            .write_with_encoder(encoder)
            .map_err(|_| WRITE_ERROR.to_string())
    })
}

fn write_tiff(image: DynamicImage, icc: Option<Vec<u8>>, dst: &Path) -> Result<(), String> {
    // The TIFF encoder has no gray + alpha.
    let image = match image.color() {
        ColorType::La8 => image.to_rgba8().into(),
        ColorType::La16 => image.to_rgba16().into(),
        _ => image,
    };
    write_file(dst, |out| {
        let mut encoder = TiffEncoder::new(out);
        if let Some(icc) = icc {
            encoder
                .set_icc_profile(icc)
                .map_err(|_| WRITE_ERROR.to_string())?;
        }
        image
            .write_with_encoder(encoder)
            .map_err(|_| WRITE_ERROR.to_string())
    })
}

fn write_webp(image: DynamicImage, dst: &Path) -> Result<(), String> {
    let (width, height) = (image.width(), image.height());
    if width > WEBP_MAX_SIDE || height > WEBP_MAX_SIDE {
        return Err(format!(
            "Bild ist für WebP zu groß (höchstens {WEBP_MAX_SIDE} Pixel pro Seite)"
        ));
    }
    // WebP is 8 bit, with or without alpha.
    let (rgb, rgba);
    let encoder = if image.color().has_alpha() {
        rgba = image.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), width, height)
    } else {
        rgb = image.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), width, height)
    };
    let data = encoder
        .encode_simple(false, WEBP_QUALITY)
        .map_err(|_| "WebP-Kodierung fehlgeschlagen".to_string())?;
    write_file(dst, |out| {
        out.write_all(&data).map_err(|_| WRITE_ERROR.to_string())
    })
}

// ---------------------------------------------------------- PNG chunk level

const PNG_SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";
/// A PNG text keyword has at most 79 bytes, followed by a NUL.
const KEYWORD_WINDOW: usize = 80;
/// PNG chunks are shorter than 2^31 bytes.
const MAX_CHUNK_LEN: u32 = i32::MAX as u32;

/// What a PNG holds that TIFF and WebP output cannot.
#[derive(Debug, Default, Clone, Copy)]
struct PngMeta {
    exif: bool,
    xmp: bool,
    icc: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Payload {
    Exif,
    Xmp,
}

/// The start of a chunk: its 8-byte header and, for the types that can carry
/// EXIF or XMP, the first bytes of its data (where the keyword is).
struct ChunkStart {
    header: [u8; 8],
    head: Vec<u8>,
}

impl ChunkStart {
    fn kind(&self) -> [u8; 4] {
        self.header[4..].try_into().unwrap()
    }

    fn len(&self) -> u64 {
        u32::from_be_bytes(self.header[..4].try_into().unwrap()).into()
    }

    /// Bytes of the chunk not read yet: the rest of the data and the CRC.
    fn remaining(&self) -> u64 {
        self.len() + 4 - self.head.len() as u64
    }

    fn payload(&self) -> Option<Payload> {
        match &self.kind() {
            b"eXIf" => Some(Payload::Exif),
            b"tEXt" | b"zTXt" | b"iTXt" => {
                let keyword = &self.head[..self.head.iter().position(|&b| b == 0)?];
                match keyword {
                    b"Raw profile type exif" => Some(Payload::Exif),
                    b"Raw profile type xmp" | b"XML:com.adobe.xmp" => Some(Payload::Xmp),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn read_chunk_start(input: &mut impl Read) -> io::Result<ChunkStart> {
    let mut header = [0u8; 8];
    input.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header[..4].try_into().unwrap());
    if len > MAX_CHUNK_LEN {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut head = Vec::new();
    if matches!(&header[4..], b"eXIf" | b"tEXt" | b"zTXt" | b"iTXt") {
        head = vec![0; (len as usize).min(KEYWORD_WINDOW)];
        input.read_exact(&mut head)?;
    }
    Ok(ChunkStart { header, head })
}

fn read_signature(input: &mut impl Read) -> io::Result<()> {
    let mut signature = [0u8; 8];
    input.read_exact(&mut signature)?;
    if signature == PNG_SIGNATURE {
        Ok(())
    } else {
        Err(io::ErrorKind::InvalidData.into())
    }
}

fn scan_png(path: &Path) -> Result<PngMeta, String> {
    let mut input = BufReader::new(File::open(path).map_err(|_| OPEN_ERROR.to_string())?);
    scan(&mut input).map_err(|_| READ_ERROR.to_string())
}

/// Walks the chunks up to `IEND`, skipping over the data.
fn scan(input: &mut BufReader<File>) -> io::Result<PngMeta> {
    read_signature(input)?;
    let mut found = PngMeta::default();
    loop {
        let chunk = read_chunk_start(input)?;
        match (&chunk.kind(), chunk.payload()) {
            (b"IEND", _) => return Ok(found),
            (b"iCCP", _) => found.icc = true,
            (_, Some(Payload::Exif)) => found.exif = true,
            (_, Some(Payload::Xmp)) => found.xmp = true,
            _ => {}
        }
        input.seek_relative(chunk.remaining() as i64)?;
    }
}

/// Copies a PNG up to and including `IEND`, leaving out the chunks that carry
/// EXIF or XMP. Fails with `UnexpectedEof` if the file ends early.
fn copy_without_metadata(input: &mut BufReader<File>, out: &mut impl Write) -> io::Result<()> {
    read_signature(input)?;
    out.write_all(&PNG_SIGNATURE)?;
    loop {
        let chunk = read_chunk_start(input)?;
        if chunk.payload().is_some() {
            input.seek_relative(chunk.remaining() as i64)?;
            continue;
        }
        out.write_all(&chunk.header)?;
        out.write_all(&chunk.head)?;
        let remaining = chunk.remaining();
        if io::copy(&mut input.by_ref().take(remaining), out)? != remaining {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if &chunk.kind() == b"IEND" {
            return Ok(());
        }
    }
}
