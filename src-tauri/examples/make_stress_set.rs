//! Writes a folder of test photos for the stress run (`scripts/make-stress-set.ps1`).
//!
//! Usage: `make_stress_set <out-dir> [count]` (default 1000 images).
//! Mix: about 82 % JPEG, 12 % PNG, 6 % TIFF, 2 to 12 megapixels each, spread
//! over subfolders. Content is a smooth picture plus noise, so the encoders
//! have real work to do. Deterministic: the same count gives the same set.
#![allow(dead_code)]

#[path = "../tests/common/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use image::{ImageBuffer, ImageFormat, Rgb};

const FOLDERS: usize = 25;

/// Small deterministic generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

struct Job {
    path: PathBuf,
    w: u32,
    h: u32,
    seed: u64,
    kind: Kind,
    smooth: bool,
}

#[derive(Clone, Copy)]
enum Kind {
    Jpeg,
    Png,
    Tiff,
}

/// RGB8 pixels: gradient, two sine waves and noise of amplitude `noise`.
fn picture(w: u32, h: u32, seed: u64, noise: i32) -> Vec<u8> {
    let mut rng = Rng(seed | 1);
    let (p1, p2) = (rng.unit() * 6.28, rng.unit() * 6.28);
    let (f1, f2) = (0.002 + rng.unit() * 0.01, 0.002 + rng.unit() * 0.01);
    let cols: Vec<f64> = (0..w).map(|x| 35.0 * (x as f64 * f1 + p1).sin()).collect();
    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for y in 0..h {
        let row = 35.0 * (y as f64 * f2 + p2).sin();
        for x in 0..w {
            let base = 128.0 + cols[x as usize] + row;
            for c in 0..3 {
                let n = (rng.next() % (2 * noise as u64 + 1)) as i32 - noise;
                let shade = (x * (c + 1) / w.max(1)) as f64 * 20.0;
                out.push((base + shade + n as f64).clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

fn write_one(job: &Job) {
    match job.kind {
        Kind::Jpeg if job.smooth => {
            common::write_jpeg(&job.path, job.w, job.h, common::JpegOpts::default())
        }
        Kind::Jpeg => {
            let rgb = picture(job.w, job.h, job.seed, 7);
            let mut out = Vec::new();
            jpeg_encoder::Encoder::new(&mut out, 90)
                .encode(&rgb, job.w as u16, job.h as u16, jpeg_encoder::ColorType::Rgb)
                .unwrap();
            fs::write(&job.path, out).unwrap();
        }
        Kind::Png | Kind::Tiff => {
            let rgb = picture(job.w, job.h, job.seed, 3);
            let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
                ImageBuffer::from_raw(job.w, job.h, rgb).unwrap();
            let format = if matches!(job.kind, Kind::Png) {
                ImageFormat::Png
            } else {
                ImageFormat::Tiff
            };
            img.save_with_format(&job.path, format).unwrap();
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: make_stress_set <out-dir> [count]"));
    let count: usize = args.next().map_or(1000, |c| c.parse().expect("count"));

    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let mut jobs = Vec::with_capacity(count);
    for i in 0..count {
        let mut folder = out.join(format!("Reise_{:02}", i % FOLDERS));
        if i % 3 == 0 {
            folder = folder.join("Auswahl");
        }
        fs::create_dir_all(&folder).unwrap();
        let (kind, ext) = match i % 50 {
            0..=5 => (Kind::Png, "png"),
            6..=8 => (Kind::Tiff, "tif"),
            _ => (Kind::Jpeg, "jpg"),
        };
        let mp = 2.0 + rng.unit() * 10.0;
        let ratio = if rng.next() % 2 == 0 { 1.5 } else { 4.0 / 3.0 };
        let w = (mp * 1e6 * ratio).sqrt() as u32;
        let h = (w as f64 / ratio) as u32;
        jobs.push(Job {
            path: folder.join(format!("IMG_{i:05}.{ext}")),
            w,
            h,
            seed: rng.next(),
            kind,
            smooth: matches!(kind, Kind::Jpeg) && i % 5 == 4,
        });
    }

    let next = AtomicUsize::new(0);
    let threads = thread::available_parallelism().map_or(4, |n| n.get());
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    write_one(job);
                    if (i + 1) % 100 == 0 {
                        eprintln!("{} / {count}", i + 1);
                    }
                }
            });
        }
    });

    let bytes: u64 = walk(&out).iter().map(|p| fs::metadata(p).unwrap().len()).sum();
    println!(
        "{count} images, {:.2} GB in {}",
        bytes as f64 / 1e9,
        out.display()
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files.extend(walk(&p));
        } else {
            files.push(p);
        }
    }
    files
}
