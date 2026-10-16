//! `decode::decode_one` against real files and the real cjxl/djxl: JPEG
//! reconstruction, every output format, the checks and the recycle-bin rules.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::{FIXTURE_H, FIXTURE_W, JpegOpts, PngKind};
use image::{ColorType, ImageFormat, ImageReader};
use jpeg_xl_konverter_lib::engine::args::{DjxlTarget, djxl_args};
use jpeg_xl_konverter_lib::engine::decode::{decode_ext, decode_one};
use jpeg_xl_konverter_lib::engine::encode::{Ctx, encode_one};
use jpeg_xl_konverter_lib::engine::originals::{Availability, FakeRecycleBin};
use jpeg_xl_konverter_lib::engine::scan::scan_paths;
use jpeg_xl_konverter_lib::engine::targets::{target_path, tmp_path};
use jpeg_xl_konverter_lib::engine::tools::Tools;
use jpeg_xl_konverter_lib::engine::types::{
    CancelToken, DecodeSettings, EncodeSettings, ItemStatus, JobSettings, Mode, Outcome,
    OutputFormat, OutputLocation, ScanEntry,
};
use jpeg_xl_konverter_lib::engine::verify::same_pixels;
use tempfile::TempDir;

const MARKER: &str = "TESTMARKER";
const NOTE_METADATA: &str = "Metadaten nicht vollständig übernommen";
const NOTE_ANIMATION: &str = "Animation – .jxl behalten";

// ---------------------------------------------------------------- harness

struct Env {
    dir: TempDir,
    work: PathBuf,
    tools: Tools,
    bin: FakeRecycleBin,
}

impl Env {
    fn new() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("arbeit");
        fs::create_dir(&work).unwrap();
        Env {
            dir,
            work,
            tools: common::tools(),
            bin: FakeRecycleBin {
                availability: Availability::Available,
                fail: false,
                recycled: Mutex::new(Vec::new()),
            },
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn recycled(&self) -> Vec<PathBuf> {
        self.bin.recycled.lock().unwrap().clone()
    }

    fn ctx<'a>(&'a self, settings: &'a JobSettings, cancel: &'a CancelToken) -> Ctx<'a> {
        Ctx {
            tools: &self.tools,
            bin: &self.bin,
            settings,
            cancel,
            work_dir: &self.work,
            progress: &|_| {},
        }
    }

    /// cjxl `src` → `<name>.jxl` with the given options.
    fn cjxl(&self, src: &Path, name: &str, args: &[&str]) -> PathBuf {
        let jxl = self.path(name);
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        self.tools
            .run_cjxl(src, &jxl, &args, &CancelToken::new())
            .unwrap();
        jxl
    }

    /// `encode_one` (JPEG transcoding), then the JPEG is renamed away so the
    /// decoded one does not collide with it. Returns the `.jxl` and the moved JPEG.
    fn transcode(&self, jpeg: &Path) -> (PathBuf, PathBuf) {
        let settings = settings(OutputFormat::Auto, true, false);
        let cancel = CancelToken::new();
        let entry = entry_for(jpeg, Mode::Enc);
        let jxl = target_path(&entry, "jxl", &settings.output);
        let outcome = encode_one(&entry, &jxl, &self.ctx(&settings, &cancel));
        assert_eq!(outcome.status, ItemStatus::Done, "{outcome:?}");
        let moved = self.path("original-weg.jpg");
        fs::rename(jpeg, &moved).unwrap();
        (jxl, moved)
    }

    /// Runs `decode_one` for `jxl` and checks what must hold after every run:
    /// no `.tmp`, an empty work folder.
    fn decode(&self, jxl: &Path, settings: &JobSettings) -> (Outcome, PathBuf) {
        let entry = entry_for(jxl, Mode::Dec);
        let ext = decode_ext(&entry, settings.decode.format).unwrap();
        let target = target_path(&entry, ext, &settings.output);
        let cancel = CancelToken::new();
        let outcome = decode_one(&entry, &target, &self.ctx(settings, &cancel));
        assert!(!tmp_path(&target).exists(), "tmp left: {outcome:?}");
        let left: Vec<_> = fs::read_dir(&self.work).unwrap().collect();
        assert!(left.is_empty(), "work files left: {left:?}");
        (outcome, target)
    }
}

fn entry_for(src: &Path, mode: Mode) -> ScanEntry {
    let mut found = scan_paths(&[src.to_path_buf()], mode).entries;
    assert_eq!(found.len(), 1, "{} not collected", src.display());
    found.remove(0)
}

fn settings(format: OutputFormat, keep_metadata: bool, trash_originals: bool) -> JobSettings {
    JobSettings {
        encode: EncodeSettings {
            distance: 0.0,
            effort: 7,
            transcode_jpeg: true,
        },
        decode: DecodeSettings {
            format,
            jpeg_quality: 92,
            reduce_to_8bit: false,
        },
        keep_metadata,
        trash_originals,
        output: OutputLocation::Next,
    }
}

fn assert_done(outcome: &Outcome, target: &Path) {
    assert_eq!(outcome.status, ItemStatus::Done, "{outcome:?}");
    assert_eq!(outcome.note, None);
    assert_eq!(outcome.out_path.as_deref(), Some(target));
    assert_eq!(outcome.out_size, Some(fs::metadata(target).unwrap().len()));
}

/// Format (by content) and size of an image file.
fn format_and_dims(path: &Path) -> (ImageFormat, (u32, u32)) {
    let reader = ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap();
    let format = reader.format().unwrap();
    (format, reader.into_dimensions().unwrap())
}

fn has_chunk(png: &Path, kind: &[u8; 4]) -> bool {
    fs::read(png).unwrap().windows(4).any(|w| w == kind)
}

// ---------------------------------------------------------------- tests

#[test]
fn auto_restores_original_jpeg() {
    let env = Env::new();
    let jpeg = env.path("foto.jpg");
    common::write_jpeg(&jpeg, 640, 480, JpegOpts::default());
    let (jxl, original) = env.transcode(&jpeg);

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Auto, true, true));
    assert_eq!(target, jpeg);
    assert_done(&outcome, &target);
    assert_eq!(fs::read(&target).unwrap(), fs::read(&original).unwrap());
    // Byte-exact restore: the .jxl may go.
    assert_eq!(env.recycled(), vec![jxl]);
}

#[test]
fn oriented_jpeg_auto_decode_is_done() {
    let env = Env::new();
    let jpeg = env.path("hochkant.jpg");
    let opts = JpegOpts {
        exif: Some(common::orientation_exif(6)),
        ..JpegOpts::default()
    };
    common::write_jpeg(&jpeg, FIXTURE_W, FIXTURE_H, opts);
    let (jxl, original) = env.transcode(&jpeg);

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Auto, true, false));
    assert_done(&outcome, &target);
    assert_eq!(fs::read(&target).unwrap(), fs::read(&original).unwrap());
}

#[test]
fn auto_non_jpeg_gives_png() {
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png(&src, PngKind::Rgba8);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Auto, true, true));
    assert_eq!(target.extension().unwrap(), "png");
    assert_done(&outcome, &target);
    let expected = image::open(&src).unwrap();
    let actual = image::open(&target).unwrap();
    assert!(same_pixels(&expected, &actual));
    // Lossless PNG with all metadata: the .jxl may go.
    assert_eq!(env.recycled(), vec![jxl]);
}

#[test]
fn jpg_from_pixels_uses_quality() {
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png(&src, PngKind::Rgb8);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "1.2"]);

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Jpg, true, true));
    assert_eq!(target.extension().unwrap(), "jpg");
    assert_done(&outcome, &target);
    assert_eq!(
        format_and_dims(&target),
        (ImageFormat::Jpeg, (FIXTURE_W, FIXTURE_H))
    );
    // Same bytes as djxl at quality 92.
    let reference = env.path("referenz.jpg");
    let args = djxl_args(DjxlTarget::PixelsToJpeg { quality: 92 });
    env.tools
        .run_djxl(&jxl, &reference, &args, &CancelToken::new())
        .unwrap();
    assert_eq!(fs::read(&target).unwrap(), fs::read(&reference).unwrap());
    // Lossy: the .jxl stays.
    assert!(env.recycled().is_empty());
}

#[test]
fn tiff_and_webp_outputs() {
    for (format, ext, image_format) in [
        (OutputFormat::Tiff, "tif", ImageFormat::Tiff),
        (OutputFormat::Webp, "webp", ImageFormat::WebP),
    ] {
        let env = Env::new();
        let src = env.path("quelle.png");
        common::write_png(&src, PngKind::Rgb8);
        let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

        let (outcome, target) = env.decode(&jxl, &settings(format, true, true));
        assert_eq!(target.extension().unwrap(), ext);
        assert_done(&outcome, &target);
        assert_eq!(
            format_and_dims(&target),
            (image_format, (FIXTURE_W, FIXTURE_H))
        );
        let expected = image::open(&src).unwrap();
        let actual = image::open(&target).unwrap();
        if format == OutputFormat::Tiff {
            assert!(same_pixels(&expected, &actual));
            assert_eq!(env.recycled(), vec![jxl], "lossless TIFF");
        } else {
            assert!(env.recycled().is_empty(), "WebP is lossy");
        }
    }
}

#[test]
fn webp_output_notes_lost_exif() {
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png_with(&src, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

    let (outcome, _) = env.decode(&jxl, &settings(OutputFormat::Webp, true, false));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(outcome.note.as_deref(), Some(NOTE_METADATA));
}

#[test]
fn reduce_to_8bit_never_trashes() {
    let env = Env::new();
    let src = env.path("quelle16.png");
    common::write_png(&src, PngKind::Rgb16);
    let jxl = env.cjxl(&src, "tief.jxl", &["-d", "0"]);
    let mut settings = settings(OutputFormat::Png, true, true);
    settings.decode.reduce_to_8bit = true;

    let (outcome, target) = env.decode(&jxl, &settings);
    assert_done(&outcome, &target);
    assert_eq!(image::open(&target).unwrap().color(), ColorType::Rgb8);
    assert!(env.recycled().is_empty());
    assert!(jxl.exists());
}

#[test]
fn existing_target_skipped() {
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png(&src, PngKind::Rgb8);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);
    let existing = env.path("bild.png");
    fs::write(&existing, b"schon da").unwrap();

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Png, true, true));
    assert_eq!(target, existing);
    assert_eq!(outcome.status, ItemStatus::Skipped, "{outcome:?}");
    assert_eq!(fs::read(&existing).unwrap(), b"schon da");
    assert!(env.recycled().is_empty());
}

#[test]
fn metadata_off_strips_png_exif() {
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png_with(&src, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

    // With metadata on, djxl gives the EXIF back ...
    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Png, true, false));
    assert_done(&outcome, &target);
    assert!(common::read_png_exif(&target).is_some());
    fs::remove_file(&target).unwrap();

    // ... with metadata off it is gone, and the .jxl stays.
    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Png, false, true));
    assert_done(&outcome, &target);
    assert!(common::read_png_exif(&target).is_none());
    assert!(!has_chunk(&target, b"eXIf"));
    assert!(env.recycled().is_empty());
}

#[test]
fn animated_jxl_is_kept() {
    let env = Env::new();
    let gif = env.path("animation.gif");
    common::write_animated_gif(&gif, 2);
    let jxl = env.cjxl(&gif, "animation.jxl", &["-d", "0"]);

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Png, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(outcome.note.as_deref(), Some(NOTE_ANIMATION));
    assert!(target.exists());
    assert!(env.recycled().is_empty());

    // Without the recycle bin there is nothing to say.
    fs::remove_file(&target).unwrap();
    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Png, true, false));
    assert_done(&outcome, &target);
}

/// Marker codes of the segments in front of the image data (SOS).
fn jpeg_markers(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes[..2], [0xFF, 0xD8]);
    let mut markers = Vec::new();
    let mut pos = 2;
    while bytes[pos + 1] != 0xDA {
        assert_eq!(bytes[pos], 0xFF);
        markers.push(bytes[pos + 1]);
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 2 + len;
    }
    markers
}

#[test]
fn tiff_output_is_lossless_for_16_bit_and_alpha() {
    for (kind, color) in [
        (PngKind::Rgb16, ColorType::Rgb16),
        (PngKind::Rgba8, ColorType::Rgba8),
    ] {
        let env = Env::new();
        let src = env.path("quelle.png");
        common::write_png(&src, kind);
        let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

        let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Tiff, true, true));
        assert_done(&outcome, &target);
        let expected = image::open(&src).unwrap();
        let actual = image::open(&target).unwrap();
        assert_eq!(actual.color(), color, "{kind:?}");
        assert!(same_pixels(&expected, &actual), "{kind:?}");
        assert_eq!(env.recycled(), vec![jxl], "{kind:?}");
    }
}

#[test]
fn metadata_off_strips_pixel_jpeg_exif() {
    const APP1: u8 = 0xE1;
    let env = Env::new();
    let src = env.path("quelle.png");
    common::write_png_with(&src, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let jxl = env.cjxl(&src, "bild.jxl", &["-d", "0"]);

    // With metadata on, djxl writes the EXIF into the JPEG ...
    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Jpg, true, false));
    assert_done(&outcome, &target);
    assert!(jpeg_markers(&target).contains(&APP1));
    fs::remove_file(&target).unwrap();

    // ... with metadata off there is no APP1 segment left.
    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Jpg, false, false));
    assert_done(&outcome, &target);
    assert!(!jpeg_markers(&target).contains(&APP1));
    assert_eq!(
        format_and_dims(&target),
        (ImageFormat::Jpeg, (FIXTURE_W, FIXTURE_H))
    );
}

#[test]
fn corrupt_jbrd_box_gives_no_jpeg() {
    let env = Env::new();
    let jpeg = env.path("foto.jpg");
    common::write_jpeg(&jpeg, 640, 480, JpegOpts::default());
    let (jxl, _) = env.transcode(&jpeg);

    // Flip the payload of the jbrd box (behind its 8-byte header).
    let mut bytes = fs::read(&jxl).unwrap();
    let at = bytes.windows(4).position(|w| w == b"jbrd").unwrap() - 4;
    let size = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    for b in &mut bytes[at + 8..at + size] {
        *b ^= 0x5A;
    }
    fs::write(&jxl, bytes).unwrap();

    let (outcome, target) = env.decode(&jxl, &settings(OutputFormat::Auto, true, true));
    assert_eq!(target, jpeg);
    assert!(
        matches!(outcome.status, ItemStatus::Error | ItemStatus::VerifyFailed),
        "{outcome:?}"
    );
    assert!(!target.exists());
    assert!(env.recycled().is_empty());
}
