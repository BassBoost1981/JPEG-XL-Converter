//! `preview::make_preview` and `preview::thumbnail` against real files and the
//! real cjxl/djxl.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{JpegOpts, PngKind};
use image::{DynamicImage, GenericImageView};
use jpeg_xl_konverter_lib::engine::preview::{make_preview, thumbnail};
use jpeg_xl_konverter_lib::engine::scan::scan_paths;
use jpeg_xl_konverter_lib::engine::tools::Tools;
use jpeg_xl_konverter_lib::engine::types::{
    CancelToken, DecodeSettings, EncodeSettings, InputKind, JobSettings, Mode, OutputFormat,
    OutputLocation, ScanEntry,
};
use jpeg_xl_konverter_lib::engine::verify::same_pixels;
use tempfile::TempDir;

struct Env {
    dir: TempDir,
    work: PathBuf,
    tools: Tools,
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
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn assert_work_empty(&self) {
        let left: Vec<_> = fs::read_dir(&self.work).unwrap().collect();
        assert!(left.is_empty(), "work files left: {left:?}");
    }

    fn jxl_from(&self, src: &Path, name: &str, args: &[&str]) -> PathBuf {
        let jxl = self.path(name);
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        self.tools
            .run_cjxl(src, &jxl, &args, &CancelToken::new())
            .unwrap();
        jxl
    }

    fn preview(
        &self,
        entry: &ScanEntry,
        mode: Mode,
        s: &JobSettings,
        view: (u32, u32),
    ) -> Result<jpeg_xl_konverter_lib::engine::preview::Preview, String> {
        make_preview(entry, mode, s, view.0, view.1, &self.tools, &self.work)
    }
}

fn entry_for(src: &Path, mode: Mode) -> ScanEntry {
    let mut found = scan_paths(&[src.to_path_buf()], mode).entries;
    assert_eq!(found.len(), 1, "{} not collected", src.display());
    found.remove(0)
}

fn settings(distance: f32, transcode_jpeg: bool, format: OutputFormat) -> JobSettings {
    JobSettings {
        encode: EncodeSettings {
            distance,
            effort: 7,
            transcode_jpeg,
        },
        decode: DecodeSettings {
            format,
            jpeg_quality: 92,
            reduce_to_8bit: false,
        },
        keep_metadata: true,
        trash_originals: false,
        output: OutputLocation::Next,
    }
}

fn decode_png(bytes: &[u8]) -> DynamicImage {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png).unwrap()
}

#[test]
fn jpeg_transcode_preview_is_lossless() {
    let env = Env::new();
    let src = env.path("a.jpg");
    common::write_jpeg(&src, 200, 120, JpegOpts::default());
    let s = settings(2.6, true, OutputFormat::Auto);
    let p = env
        .preview(&entry_for(&src, Mode::Enc), Mode::Enc, &s, (100, 60))
        .unwrap();
    assert!(p.lossless);
    assert_eq!(p.before_png, p.after_png);
    assert_eq!(decode_png(&p.before_png).dimensions(), (100, 60));
    env.assert_work_empty();
}

#[test]
fn lossy_preview_differs() {
    let env = Env::new();
    let src = env.path("a.png");
    common::write_png(&src, PngKind::Rgb8);
    let s = settings(2.6, true, OutputFormat::Auto);
    let p = env
        .preview(&entry_for(&src, Mode::Enc), Mode::Enc, &s, (40, 30))
        .unwrap();
    assert!(!p.lossless);
    let (before, after) = (decode_png(&p.before_png), decode_png(&p.after_png));
    assert_eq!(before.dimensions(), (40, 30));
    assert_eq!(after.dimensions(), (40, 30));
    assert!(!same_pixels(&before, &after));
    env.assert_work_empty();
}

#[test]
fn lossy_jpeg_without_transcoding_encodes_the_crop() {
    let env = Env::new();
    let src = env.path("a.jpg");
    common::write_jpeg(&src, 200, 120, JpegOpts::default());
    let s = settings(2.6, false, OutputFormat::Auto);
    let p = env
        .preview(&entry_for(&src, Mode::Enc), Mode::Enc, &s, (100, 60))
        .unwrap();
    assert!(!p.lossless);
    let (before, after) = (decode_png(&p.before_png), decode_png(&p.after_png));
    assert!(!same_pixels(&before, &after));
    env.assert_work_empty();
}

#[test]
fn distance_zero_preview_is_lossless() {
    let env = Env::new();
    let src = env.path("a.png");
    common::write_png(&src, PngKind::Rgb8);
    let s = settings(0.0, true, OutputFormat::Auto);
    let p = env
        .preview(&entry_for(&src, Mode::Enc), Mode::Enc, &s, (40, 30))
        .unwrap();
    assert!(p.lossless);
    assert_eq!(p.before_png, p.after_png);
}

#[test]
fn orientation_is_applied_to_both_halves() {
    let env = Env::new();
    let src = env.path("gedreht.png");
    common::write_png_with(&src, &[(b"eXIf", common::orientation_exif(6))]);
    // 64x48 stored, shown 48x64.
    let s = settings(2.6, true, OutputFormat::Auto);
    let p = env
        .preview(&entry_for(&src, Mode::Enc), Mode::Enc, &s, (1000, 1000))
        .unwrap();
    assert_eq!(decode_png(&p.before_png).dimensions(), (48, 64));
    assert_eq!(decode_png(&p.after_png).dimensions(), (48, 64));
}

#[test]
fn decode_preview_per_target() {
    let env = Env::new();
    let png = env.path("q.png");
    common::write_png(&png, PngKind::Rgb8);
    let jxl = env.jxl_from(&png, "q.jxl", &["-d", "1"]);
    let entry = entry_for(&jxl, Mode::Dec);

    for format in [OutputFormat::Auto, OutputFormat::Png, OutputFormat::Tiff] {
        let s = settings(0.0, true, format);
        let p = env.preview(&entry, Mode::Dec, &s, (40, 30)).unwrap();
        assert!(p.lossless, "{format:?}");
        assert_eq!(p.before_png, p.after_png);
    }
    for format in [OutputFormat::Jpg, OutputFormat::Webp] {
        let s = settings(0.0, true, format);
        let p = env.preview(&entry, Mode::Dec, &s, (40, 30)).unwrap();
        assert!(!p.lossless, "{format:?}");
        let (before, after) = (decode_png(&p.before_png), decode_png(&p.after_png));
        assert_eq!(before.dimensions(), after.dimensions());
        assert!(!same_pixels(&before, &after), "{format:?}");
    }
    env.assert_work_empty();
}

#[test]
fn decode_of_transcoded_jpeg_is_lossless() {
    let env = Env::new();
    let jpg = env.path("t.jpg");
    common::write_jpeg(&jpg, 64, 48, JpegOpts::default());
    let jxl = env.jxl_from(&jpg, "t.jxl", &[]);
    let s = settings(0.0, true, OutputFormat::Jpg);
    let p = env
        .preview(&entry_for(&jxl, Mode::Dec), Mode::Dec, &s, (40, 30))
        .unwrap();
    assert!(p.lossless);
    assert_eq!(p.before_png, p.after_png);
}

#[test]
fn unsupported_sources_are_refused_without_leftovers() {
    let env = Env::new();
    let cmyk = env.path("c.jpg");
    common::write_jpeg(
        &cmyk,
        64,
        48,
        JpegOpts {
            cmyk: true,
            ..JpegOpts::default()
        },
    );
    let entry = ScanEntry {
        path: cmyk,
        root: None,
        kind: InputKind::Jpeg,
        size: 1,
        width: 64,
        height: 48,
    };
    let s = settings(2.6, true, OutputFormat::Auto);
    let err = env
        .preview(&entry, Mode::Enc, &s, (40, 30))
        .err()
        .expect("CMYK must be refused");
    assert!(!err.is_empty());
    assert!(thumbnail(&entry, 80, &env.tools, &env.work).is_err());
    env.assert_work_empty();
}

#[test]
fn thumbnail_size() {
    let env = Env::new();
    let jpg = env.path("w.jpg");
    common::write_jpeg(&jpg, 400, 300, JpegOpts::default());
    let t = thumbnail(&entry_for(&jpg, Mode::Enc), 80, &env.tools, &env.work).unwrap();
    assert_eq!(decode_png(&t).dimensions(), (80, 60));

    // Portrait via orientation, from a JXL.
    let png = env.path("p.png");
    common::write_png_with(&png, &[(b"eXIf", common::orientation_exif(6))]);
    let jxl = env.jxl_from(&png, "p.jxl", &["-d", "0"]);
    let t = thumbnail(&entry_for(&jxl, Mode::Dec), 80, &env.tools, &env.work).unwrap();
    assert_eq!(decode_png(&t).dimensions(), (60, 80));
    env.assert_work_empty();
}

#[test]
fn concurrent_previews_do_not_clash() {
    let env = Env::new();
    let src = env.path("a.png");
    common::write_png(&src, PngKind::Rgb8);
    let entry = entry_for(&src, Mode::Enc);
    let s = settings(2.6, true, OutputFormat::Auto);
    std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| env.preview(&entry, Mode::Enc, &s, (40, 30))))
            .collect();
        for job in jobs {
            assert!(job.join().unwrap().is_ok());
        }
    });
    env.assert_work_empty();
}
