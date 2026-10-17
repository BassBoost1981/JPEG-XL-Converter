//! `encode::encode_one` against real files and the real cjxl/djxl: every
//! input format, the checks, the recycle-bin rules and the clean-up.

mod common;

use std::cell::RefCell;
use std::fs::{self, File, FileTimes};
use std::os::windows::fs::{FileTimesExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use common::{FIXTURE_H, FIXTURE_W, JpegOpts, PngKind, keys_of};
use image::Frame;
use image::codecs::gif::{GifEncoder, Repeat};
use jpeg_xl_konverter_lib::engine::args::{DjxlTarget, djxl_args};
use jpeg_xl_konverter_lib::engine::encode::{Ctx, encode_one};
use jpeg_xl_konverter_lib::engine::msg::keys;
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
use tiff::encoder::{TiffEncoder as RawTiffEncoder, colortype};

const MARKER: &str = "TESTMARKER";

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

    /// Runs `encode_one` for `src` (collected like the app does) and checks
    /// what must hold after every run: no `.tmp`, an empty work folder.
    fn encode(&self, src: &Path, settings: &JobSettings) -> (Outcome, PathBuf) {
        self.encode_with(src, settings, &CancelToken::new(), &|_| {})
    }

    fn encode_with(
        &self,
        src: &Path,
        settings: &JobSettings,
        cancel: &CancelToken,
        progress: &dyn Fn(u8),
    ) -> (Outcome, PathBuf) {
        let entry = entry_for(src);
        let target = target_path(&entry, "jxl", &settings.output);
        let ctx = Ctx {
            tools: &self.tools,
            bin: &self.bin,
            settings,
            cancel,
            work_dir: &self.work,
            progress,
        };
        let outcome = encode_one(&entry, &target, &ctx);
        assert!(!tmp_path(&target).exists(), "tmp left: {outcome:?}");
        let left: Vec<_> = fs::read_dir(&self.work).unwrap().collect();
        assert!(left.is_empty(), "work files left: {left:?}");
        (outcome, target)
    }

    /// djxl → PNG, read back.
    fn decode_png(&self, jxl: &Path) -> PathBuf {
        let out = self.path("zurück.png");
        let _ = fs::remove_file(&out);
        let args = djxl_args(DjxlTarget::Png {
            reduce_to_8bit: false,
        });
        self.tools
            .run_djxl(jxl, &out, &args, &CancelToken::new())
            .unwrap();
        out
    }
}

fn entry_for(src: &Path) -> ScanEntry {
    let mut found = scan_paths(&[src.to_path_buf()], Mode::Enc).entries;
    assert_eq!(found.len(), 1, "{} not collected", src.display());
    found.remove(0)
}

fn settings(distance: f32, keep_metadata: bool, trash_originals: bool) -> JobSettings {
    JobSettings {
        encode: EncodeSettings {
            distance,
            effort: 7,
            transcode_jpeg: true,
        },
        decode: DecodeSettings {
            format: OutputFormat::Auto,
            jpeg_quality: 92,
            reduce_to_8bit: false,
        },
        keep_metadata,
        trash_originals,
        output: OutputLocation::Next,
    }
}

fn lossless() -> JobSettings {
    settings(0.0, true, false)
}

fn assert_done(outcome: &Outcome, target: &Path) {
    assert_eq!(outcome.status, ItemStatus::Done, "{outcome:?}");
    assert!(outcome.notes.is_empty(), "{outcome:?}");
    assert_eq!(outcome.out_path.as_deref(), Some(target));
    assert_eq!(outcome.out_size, Some(fs::metadata(target).unwrap().len()));
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------- fixtures

/// A TIFF with `pages` pages (one directory each).
fn write_tiff_pages(path: &Path, pages: usize) {
    let pixels: Vec<u8> = (0..FIXTURE_W * FIXTURE_H * 3)
        .map(|i| (i * 7 % 251) as u8)
        .collect();
    let mut encoder = RawTiffEncoder::new(File::create(path).unwrap()).unwrap();
    for _ in 0..pages {
        encoder
            .write_image::<colortype::RGB8>(FIXTURE_W, FIXTURE_H, &pixels)
            .unwrap();
    }
}

// ---------------------------------------------------------------- JPEG

#[test]
fn jpeg_baseline_roundtrip_is_byte_identical() {
    let env = Env::new();
    let variants = [
        ("baseline.jpg", JpegOpts::default()),
        (
            "progressiv.jpg",
            JpegOpts {
                progressive: true,
                ..Default::default()
            },
        ),
        (
            "exif.jpg",
            JpegOpts {
                exif: Some(common::minimal_exif(MARKER)),
                ..Default::default()
            },
        ),
    ];
    for (name, opts) in variants {
        let src = env.path(name);
        common::write_jpeg(&src, 640, 480, opts);
        let seen = RefCell::new(Vec::new());
        let (outcome, target) = env.encode_with(&src, &lossless(), &CancelToken::new(), &|p| {
            seen.borrow_mut().push(p)
        });
        assert_done(&outcome, &target);
        assert_eq!(*seen.borrow(), [5, 60, 90, 100], "{name}");

        let rec = env.path("rekonstruiert.jpg");
        env.tools
            .run_djxl(
                &target,
                &rec,
                &djxl_args(DjxlTarget::ReconstructJpeg),
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(fs::read(&rec).unwrap(), fs::read(&src).unwrap(), "{name}");
        fs::remove_file(&rec).unwrap();
    }
}

#[test]
fn cmyk_jpeg_skipped() {
    let env = Env::new();
    let src = env.path("cmyk.jpg");
    common::write_jpeg(
        &src,
        64,
        48,
        JpegOpts {
            cmyk: true,
            ..Default::default()
        },
    );
    let (outcome, target) = env.encode(&src, &lossless());
    assert_eq!(outcome.status, ItemStatus::Skipped, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::NOT_TRANSCODABLE]);
    assert!(!target.exists());
}

#[test]
fn corrupt_file_errors() {
    let env = Env::new();
    let src = env.path("kaputt.jpg");
    let garbage: Vec<u8> = (0..5000u32).map(|i| (i * 7919 % 253) as u8).collect();
    fs::write(&src, &garbage).unwrap();
    let (outcome, target) = env.encode(&src, &lossless());
    assert_eq!(outcome.status, ItemStatus::Error, "{outcome:?}");
    assert!(!outcome.notes.is_empty());
    assert!(!target.exists());
    assert_eq!(fs::read(&src).unwrap(), garbage);
}

#[test]
fn png_named_jpg_errors_or_converts_without_loss() {
    let env = Env::new();
    let src = env.path("x.jpg");
    common::write_png(&src, PngKind::Rgb8);
    let (outcome, target) = env.encode(&src, &lossless());
    assert!(
        matches!(outcome.status, ItemStatus::Error | ItemStatus::Skipped),
        "{outcome:?}"
    );
    assert!(!target.exists());
}

// ---------------------------------------------------------------- PNG and others

#[test]
fn png_variants_lossless_pixel_identical() {
    let env = Env::new();
    for kind in [
        PngKind::Rgb8,
        PngKind::Rgba8,
        PngKind::Rgb16,
        PngKind::Gray8,
    ] {
        let src = env.path(&format!("{kind:?}.png"));
        common::write_png(&src, kind);
        let (outcome, target) = env.encode(&src, &lossless());
        assert_done(&outcome, &target);

        let back = env.decode_png(&target);
        let a = image::open(&src).unwrap();
        let b = image::open(&back).unwrap();
        assert!(same_pixels(&a, &b), "{kind:?}");
        assert_eq!(b.color().bytes_per_pixel() > 4, kind == PngKind::Rgb16);
    }
}

#[test]
fn tiff_webp_bmp_gif_encode() {
    let env = Env::new();
    let tif = env.path("bild.tif");
    common::write_tiff16(&tif);
    let webp = env.path("bild.webp");
    common::write_webp_with_exif(&webp, &common::minimal_exif(MARKER));
    let bmp = env.path("bild.bmp");
    common::write_bmp(&bmp);
    let gif = env.path("bild.gif");
    common::write_gif(&gif);

    for src in [&tif, &webp, &bmp, &gif] {
        let (outcome, target) = env.encode(src, &lossless());
        assert_done(&outcome, &target);
        // Same base name: move the result aside for the next source.
        let kept = target.with_extension(format!(
            "{}.jxl",
            src.extension().unwrap().to_str().unwrap()
        ));
        fs::rename(&target, &kept).unwrap();
        if src == &webp {
            let back = env.decode_png(&kept);
            let exif = common::read_png_exif(&back).expect("EXIF lost");
            assert!(contains(&exif, MARKER.as_bytes()));
        }
    }
}

#[test]
fn oriented_png_lossless_verifies() {
    let env = Env::new();
    for orientation in [6u16, 3, 8] {
        let src = env.path(&format!("gedreht{orientation}.png"));
        common::write_png_with(&src, &[(b"eXIf", common::orientation_exif(orientation))]);
        let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
        assert_done(&outcome, &target);
        assert!(env.recycled().contains(&src), "{orientation}");
    }
    // Without metadata cjxl drops the orientation; still pixel-identical.
    let src = env.path("ohne.png");
    common::write_png_with(&src, &[(b"eXIf", common::orientation_exif(6))]);
    let (outcome, target) = env.encode(&src, &settings(0.0, false, false));
    assert_done(&outcome, &target);
}

#[test]
fn animated_gif_is_never_trashed() {
    let env = Env::new();
    let src = env.path("animiert.gif");
    common::write_animated_gif(&src, 2);
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::ANIMATION_KEPT_ORIGINAL]);
    assert!(target.is_file());
    assert!(env.recycled().is_empty());
    assert!(src.is_file());

    // Without the recycle bin there is nothing to say.
    fs::remove_file(&target).unwrap();
    let (outcome, target) = env.encode(&src, &lossless());
    assert_done(&outcome, &target);
}

#[test]
fn multi_page_tiff_keeps_original() {
    let env = Env::new();
    let src = env.path("seiten.tif");
    write_tiff_pages(&src, 2);
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::FIRST_PAGE_ONLY]);
    assert!(target.is_file());
    assert!(env.recycled().is_empty());
}

#[test]
fn png_text_chunk_keeps_original() {
    let env = Env::new();
    let src = env.path("text.png");
    common::write_png_with(&src, &[(b"tEXt", b"Title\0Urlaub".to_vec())]);
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::METADATA_INCOMPLETE]);
    assert!(target.is_file());
    assert!(env.recycled().is_empty());
}

#[test]
fn png_display_chunks_do_not_keep_original() {
    // cjxl drops pHYs (DPI), but by Ruling R20 that is not metadata loss.
    let env = Env::new();
    let src = env.path("dpi.png");
    common::write_png_with(
        &src,
        &[(b"pHYs", vec![0, 0, 0x0b, 0x13, 0, 0, 0x0b, 0x13, 1])],
    );
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_done(&outcome, &target);
    assert_eq!(env.recycled(), [src]);
}

#[test]
fn gif_loop_extension_is_recyclable() {
    let env = Env::new();
    let src = env.path("schleife.gif");
    {
        let mut encoder = GifEncoder::new(File::create(&src).unwrap());
        encoder.set_repeat(Repeat::Infinite).unwrap();
        let img = common::gradient_rgba8(FIXTURE_W, FIXTURE_H);
        encoder.encode_frame(Frame::new(img)).unwrap();
    }
    assert!(contains(&fs::read(&src).unwrap(), b"NETSCAPE2.0"));
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_done(&outcome, &target);
    assert_eq!(env.recycled(), [src]);
}

#[test]
fn gif_comment_keeps_original() {
    let env = Env::new();
    let src = env.path("kommentar.gif");
    common::write_gif(&src);
    let mut bytes = fs::read(&src).unwrap();
    assert_eq!(bytes.pop(), Some(0x3B)); // trailer
    bytes.extend_from_slice(b"\x21\xFE\x05hallo\x00\x3B");
    fs::write(&src, bytes).unwrap();
    let (outcome, _) = env.encode(&src, &settings(0.0, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::METADATA_INCOMPLETE]);
    assert!(env.recycled().is_empty());
}

#[test]
fn png_exif_survives_and_original_goes() {
    let env = Env::new();
    let src = env.path("exif.png");
    common::write_png_with(&src, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let (outcome, target) = env.encode(&src, &settings(0.0, true, true));
    assert_done(&outcome, &target);
    assert_eq!(env.recycled(), [src]);
    let exif = common::read_png_exif(&env.decode_png(&target)).expect("EXIF lost");
    assert!(contains(&exif, MARKER.as_bytes()));
}

// ---------------------------------------------------------------- rules

#[test]
fn existing_target_skipped() {
    let env = Env::new();
    let src = env.path("foto.png");
    common::write_png(&src, PngKind::Rgb8);
    let target = env.path("foto.jxl");
    fs::write(&target, b"schon da").unwrap();
    let (outcome, _) = env.encode(&src, &lossless());
    assert_eq!(outcome.status, ItemStatus::Skipped, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::TARGET_EXISTS]);
    assert_eq!(fs::read(&target).unwrap(), b"schon da");
}

#[test]
fn stale_tmp_is_replaced() {
    let env = Env::new();
    let src = env.path("foto.png");
    common::write_png(&src, PngKind::Rgb8);
    fs::write(env.path("foto.jxl.tmp"), b"alt").unwrap();
    let (outcome, target) = env.encode(&src, &lossless());
    assert_done(&outcome, &target);
}

#[test]
fn missing_or_locked_source_errors() {
    let env = Env::new();
    let gone = env.path("weg.jpg");
    common::write_jpeg(&gone, 64, 48, JpegOpts::default());
    let entry = entry_for(&gone);
    fs::remove_file(&gone).unwrap();
    let target = target_path(&entry, "jxl", &OutputLocation::Next);
    let cancel = CancelToken::new();
    let job = lossless();
    let ctx = Ctx {
        tools: &env.tools,
        bin: &env.bin,
        settings: &job,
        cancel: &cancel,
        work_dir: &env.work,
        progress: &|_| {},
    };
    let outcome = encode_one(&entry, &target, &ctx);
    assert_eq!(outcome.status, ItemStatus::Error, "{outcome:?}");
    assert!(!target.exists());

    for name in ["gesperrt.jpg", "gesperrt.png"] {
        let locked = env.path(name);
        if name.ends_with(".jpg") {
            common::write_jpeg(&locked, 64, 48, JpegOpts::default());
        } else {
            common::write_png(&locked, PngKind::Rgb8);
        }
        let handle = File::options()
            .read(true)
            .share_mode(0)
            .open(&locked)
            .unwrap();
        let (outcome, target) = env.encode(&locked, &lossless());
        drop(handle);
        assert_eq!(outcome.status, ItemStatus::Error, "{name}: {outcome:?}");
        assert!(!target.exists());
    }
}

#[test]
fn unicode_and_long_paths() {
    let env = Env::new();
    let unicode = env.path("Ünïcödé 📷");
    let long = (0..5).fold(env.path("lang"), |p, i| {
        p.join(format!("{i}_ein_ziemlich_langer_ordnername_0123456789"))
    });
    assert!(
        long.join("bild 📷.jxl.tmp")
            .to_str()
            .unwrap()
            .chars()
            .count()
            > 260
    );
    for dir in [unicode, long] {
        fs::create_dir_all(&dir).unwrap();
        let png = dir.join("bild 📷.png");
        let jpg = dir.join("foto 📷.jpg");
        common::write_png(&png, PngKind::Rgba8);
        common::write_jpeg(&jpg, 64, 48, JpegOpts::default());
        for src in [&png, &jpg] {
            let (outcome, target) = env.encode(src, &settings(0.0, true, true));
            assert_done(&outcome, &target);
            assert!(env.recycled().contains(src), "{}", src.display());
        }
    }
}

#[test]
fn output_folder_is_created() {
    let env = Env::new();
    let src = env.path("foto.png");
    common::write_png(&src, PngKind::Rgb8);
    let mut job = lossless();
    job.output = OutputLocation::Folder {
        path: env.path("neu").join("tief"),
    };
    let (outcome, target) = env.encode(&src, &job);
    assert_done(&outcome, &target);
    assert!(target.starts_with(env.path("neu")));
}

#[test]
fn dates_copied() {
    let env = Env::new();
    let src = env.path("alt.jpg");
    common::write_jpeg(&src, 64, 48, JpegOpts::default());
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_100_000_000);
    File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_times(FileTimes::new().set_created(created).set_modified(modified))
        .unwrap();

    let (outcome, target) = env.encode(&src, &lossless());
    assert_done(&outcome, &target);
    let meta = fs::metadata(&target).unwrap();
    assert_eq!(meta.created().unwrap(), created);
    assert_eq!(meta.modified().unwrap(), modified);
}

#[test]
fn trash_only_when_lossless() {
    let env = Env::new();
    let lossy = env.path("verlustbehaftet.png");
    common::write_png(&lossy, PngKind::Rgb8);
    let (outcome, target) = env.encode(&lossy, &settings(1.2, true, true));
    assert_done(&outcome, &target);
    assert!(env.recycled().is_empty());
    assert!(lossy.is_file());

    let exact = env.path("verlustfrei.png");
    common::write_png(&exact, PngKind::Rgb8);
    let (outcome, target) = env.encode(&exact, &settings(0.0, true, true));
    assert_done(&outcome, &target);
    assert_eq!(env.recycled(), [exact]);
}

#[test]
fn lossy_jpeg_reencode_checks_dims_and_keeps_original() {
    let env = Env::new();
    let src = env.path("neu.jpg");
    common::write_jpeg(&src, 64, 48, JpegOpts::default());
    let mut job = settings(0.0, true, true);
    job.encode.transcode_jpeg = false;
    let (outcome, target) = env.encode(&src, &job);
    assert_done(&outcome, &target);
    assert!(env.recycled().is_empty());
}

#[test]
fn metadata_off_never_trashes() {
    let env = Env::new();
    let src = env.path("exif.png");
    common::write_png_with(&src, &[(b"eXIf", common::minimal_exif(MARKER))]);
    let (outcome, target) = env.encode(&src, &settings(0.0, false, true));
    assert_done(&outcome, &target);
    assert!(env.recycled().is_empty());
    assert!(src.is_file());
    // cjxl ignores unknown strip values silently: prove the EXIF is gone.
    assert_eq!(common::read_png_exif(&env.decode_png(&target)), None);
}

#[test]
fn recycle_bin_note_is_passed_on() {
    let mut env = Env::new();
    env.bin.availability = Availability::NoRecycleBin;
    let src = env.path("usb.png");
    common::write_png(&src, PngKind::Rgb8);
    let (outcome, _) = env.encode(&src, &settings(0.0, true, true));
    assert_eq!(outcome.status, ItemStatus::DoneWithNote, "{outcome:?}");
    assert_eq!(keys_of(&outcome.notes), [keys::KEPT_NO_RECYCLE_BIN]);
}

#[test]
fn cancelled_run_waits_and_cleans_up() {
    let env = Env::new();
    let src = env.path("abbruch.tif");
    common::write_tiff16(&src);
    // Cancel right after the conversion: the check is interrupted, the
    // .tmp and the intermediate PNG must go (`encode_with` checks both).
    for at in [5, 60, 90] {
        let cancel = CancelToken::new();
        let trigger = cancel.clone();
        let (outcome, target) = env.encode_with(&src, &settings(0.0, true, true), &cancel, &|p| {
            if p == at {
                trigger.cancel();
            }
        });
        assert_eq!(outcome.status, ItemStatus::Waiting, "{at}: {outcome:?}");
        assert!(outcome.notes.is_empty(), "{at}: {outcome:?}");
        assert!(!target.exists(), "{at}");
        assert!(env.recycled().is_empty());
    }
}
