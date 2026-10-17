//! Headless stress run: converts a folder with the real queue and the real
//! cjxl pipeline, output into a separate folder, and prints throughput.
//! The recycle bin is a fake (nothing is ever moved or deleted).
//!
//! Usage: `stress_run <input-dir> <output-dir> [workers]`
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{env, fs, io};

use jpeg_xl_konverter_lib::engine::originals::{Availability, FakeRecycleBin};
use jpeg_xl_konverter_lib::engine::queue::{
    Processor, ProgressSink, Queue, RealProcessor, default_workers,
};
use jpeg_xl_konverter_lib::engine::scan::scan_paths;
use jpeg_xl_konverter_lib::engine::tools::Tools;
use jpeg_xl_konverter_lib::engine::types::{
    CancelToken, DecodeSettings, EncodeSettings, ItemEvent, ItemStatus, JobSettings, Lang, Mode,
    OutputFormat, OutputLocation, Outcome, PauseReason, RunSummary, ScanEntry,
};

/// Wraps the real processor and records how many files run at the same time.
struct Counting {
    inner: RealProcessor,
    now: AtomicUsize,
    peak: AtomicUsize,
}

impl Processor for Counting {
    fn target_for(&self, m: Mode, e: &ScanEntry, s: &JobSettings) -> io::Result<PathBuf> {
        self.inner.target_for(m, e, s)
    }
    fn has_space(&self, m: Mode, e: &ScanEntry, t: &Path) -> bool {
        self.inner.has_space(m, e, t)
    }
    fn process(
        &self,
        m: Mode,
        e: &ScanEntry,
        t: &Path,
        s: &JobSettings,
        c: &CancelToken,
        p: &dyn Fn(u8),
    ) -> Outcome {
        let n = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(n, Ordering::SeqCst);
        let out = self.inner.process(m, e, t, s, c, p);
        self.now.fetch_sub(1, Ordering::SeqCst);
        out
    }
}

struct Sink {
    done: Sender<RunSummary>,
    finished_items: AtomicUsize,
}

impl ProgressSink for Sink {
    fn item(&self, ev: ItemEvent) {
        use ItemStatus::*;
        if matches!(ev.status, Done | DoneWithNote | Skipped | Error | VerifyFailed) {
            let n = self.finished_items.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 100 == 0 {
                eprintln!("{n} files finished");
            }
        }
    }
    fn paused(&self, _: PauseReason) {}
    fn finished(&self, s: RunSummary) {
        let _ = self.done.send(s);
    }
}

fn jxl_bytes(dir: &Path) -> (u64, usize) {
    let (mut bytes, mut n) = (0, 0);
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            let (b, c) = jxl_bytes(&p);
            bytes += b;
            n += c;
        } else if p.extension().is_some_and(|x| x == "jxl") {
            bytes += fs::metadata(&p).unwrap().len();
            n += 1;
        }
    }
    (bytes, n)
}

fn main() {
    let mut args = env::args().skip(1);
    let usage = "usage: stress_run <in> <out> [workers]";
    let input = PathBuf::from(args.next().expect(usage));
    let output = PathBuf::from(args.next().expect(usage));
    let workers: usize = args.next().map_or_else(default_workers, |w| w.parse().unwrap());
    fs::create_dir_all(&output).unwrap();

    let exe_dir = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let tools = Tools::locate(&exe_dir).expect("cjxl/djxl not found");
    let work_dir = output.join("_work");
    fs::create_dir_all(&work_dir).unwrap();
    let counting = Arc::new(Counting {
        inner: RealProcessor {
            tools,
            bin: Arc::new(FakeRecycleBin {
                availability: Availability::Available,
                fail: false,
                recycled: Mutex::new(Vec::new()),
            }),
            work_dir,
        },
        now: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    });

    let scan = scan_paths(&[input.clone()], Mode::Enc);
    println!("scanned {} files ({} rejected)", scan.entries.len(), scan.rejected);
    let (tx, rx) = channel();
    let sink = Arc::new(Sink { done: tx, finished_items: AtomicUsize::new(0) });
    let logs = output.join("_logs");
    let queue = Queue::new(counting.clone(), sink, workers, logs.clone(), logs);
    let added = queue.add(Mode::Enc, scan.entries).len();

    let settings = JobSettings {
        encode: EncodeSettings { distance: 1.2, effort: 7, transcode_jpeg: true },
        decode: DecodeSettings {
            format: OutputFormat::Auto,
            jpeg_quality: 92,
            reduce_to_8bit: false,
        },
        keep_metadata: true,
        trash_originals: true,
        output: OutputLocation::Folder { path: output.join("out") },
    };
    let t0 = Instant::now();
    queue.start(Mode::Enc, settings, Lang::De);
    let s = rx.recv_timeout(Duration::from_secs(4 * 3600)).expect("run did not finish");
    let secs = t0.elapsed().as_secs_f64();

    println!("workers: {workers} (default_workers() = {})", default_workers());
    println!("peak concurrent files: {}", counting.peak.load(Ordering::SeqCst));
    println!("elapsed: {secs:.1} s, {:.2} files/s", added as f64 / secs);
    println!(
        "summary: done {}, skipped {}, errors {}, bytes_in {}, bytes_out {} ({:.1} %)",
        s.done,
        s.skipped,
        s.errors,
        s.bytes_in,
        s.bytes_out,
        s.bytes_out as f64 * 100.0 / s.bytes_in.max(1) as f64
    );
    let (on_disk, n) = jxl_bytes(&output.join("out"));
    println!(
        "on disk: {n} .jxl files, {on_disk} bytes -> bytes_out matches: {}",
        on_disk == s.bytes_out
    );
    println!("log: {:?}", s.log_path);
}
