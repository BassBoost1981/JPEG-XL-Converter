//! The work queue: worker threads take waiting entries of the running mode,
//! reserve their target, and run the per-file pipeline. Pause, cancel, and a
//! full disk stop the hand-out; the run ends with a summary (and a log file
//! when something needs attention).
//!
//! Locking: all state sits behind one `Mutex`. The sink and the processor are
//! only ever called with that lock released.

use std::collections::HashSet;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use super::decode::{decode_ext, decode_one};
use super::disk::free_bytes;
use super::encode::{Ctx, encode_one};
use super::originals::RecycleBin;
use super::pipeline_util::TARGET_EXISTS;
use super::report::RunReport;
use super::scan::identity;
use super::targets::target_path;
use super::tools::Tools;
use super::types::{
    CancelToken, ItemEvent, ItemStatus, JobSettings, Mode, Outcome, PauseReason, QueueItem,
    RunSummary, ScanEntry,
};

const TARGET_UNKNOWN: &str = "Zielpfad lässt sich nicht bestimmen";
const CLEAR_WHILE_RUNNING: &str =
    "Die Warteschlange lässt sich während eines Durchlaufs nicht leeren";

pub trait Processor: Send + Sync {
    fn target_for(&self, mode: Mode, e: &ScanEntry, s: &JobSettings) -> io::Result<PathBuf>;
    fn has_space(&self, e: &ScanEntry, target: &Path) -> bool;
    fn process(
        &self,
        mode: Mode,
        e: &ScanEntry,
        target: &Path,
        s: &JobSettings,
        cancel: &CancelToken,
        progress: &dyn Fn(u8),
    ) -> Outcome;
}

/// Runs the real pipelines (`encode_one` / `decode_one`).
pub struct RealProcessor {
    pub tools: Tools,
    pub bin: Arc<dyn RecycleBin>,
    pub work_dir: PathBuf,
}

impl Processor for RealProcessor {
    fn target_for(&self, mode: Mode, e: &ScanEntry, s: &JobSettings) -> io::Result<PathBuf> {
        let ext = match mode {
            Mode::Enc => "jxl",
            Mode::Dec => decode_ext(e, s.decode.format)?,
        };
        Ok(target_path(e, ext, &s.output))
    }

    /// At least twice the source size free where the target goes (checked on
    /// the nearest existing folder above it). Unknown free space does not
    /// block the run.
    fn has_space(&self, e: &ScanEntry, target: &Path) -> bool {
        target
            .ancestors()
            .skip(1)
            .find(|dir| dir.is_dir())
            .and_then(|dir| free_bytes(dir).ok())
            .is_none_or(|free| free >= e.size.saturating_mul(2))
    }

    fn process(
        &self,
        mode: Mode,
        e: &ScanEntry,
        target: &Path,
        s: &JobSettings,
        cancel: &CancelToken,
        progress: &dyn Fn(u8),
    ) -> Outcome {
        let ctx = Ctx {
            tools: &self.tools,
            bin: &*self.bin,
            settings: s,
            cancel,
            work_dir: &self.work_dir,
            progress,
        };
        match mode {
            Mode::Enc => encode_one(e, target, &ctx),
            Mode::Dec => decode_one(e, target, &ctx),
        }
    }
}

pub trait ProgressSink: Send + Sync {
    fn item(&self, ev: ItemEvent);
    fn paused(&self, r: PauseReason);
    fn finished(&self, s: RunSummary);
}

/// Half the logical processors, at least one.
pub fn default_workers() -> usize {
    let cpus = thread::available_parallelism().map_or(1, |n| n.get());
    (cpus / 2).max(1)
}

pub struct Queue {
    inner: Arc<Inner>,
}

struct Inner {
    processor: Arc<dyn Processor>,
    sink: Arc<dyn ProgressSink>,
    workers: usize,
    log_dir: PathBuf,
    state: Mutex<State>,
    /// Signalled whenever waiting workers may find something new.
    wake: Condvar,
}

#[derive(Default)]
struct State {
    items: Vec<Item>,
    next_id: u64,
    /// Targets of the files being worked on (lower-cased identity).
    reserved: HashSet<String>,
    run: Option<Run>,
}

struct Item {
    id: u64,
    mode: Mode,
    /// `scan::identity` of the source, for the duplicate check.
    key: PathBuf,
    entry: ScanEntry,
    status: ItemStatus,
}

struct Run {
    mode: Mode,
    settings: Arc<JobSettings>,
    cancel: CancelToken,
    report: RunReport,
    /// No new files are handed out (by the user, or for a full disk).
    paused: bool,
    /// Files handed out and not yet back.
    in_flight: usize,
    /// Worker threads of this run still alive.
    alive: usize,
    /// Nothing left to do; the workers leave.
    over: bool,
}

/// One file handed to a worker.
struct Job {
    id: u64,
    mode: Mode,
    entry: ScanEntry,
    settings: Arc<JobSettings>,
    cancel: CancelToken,
}

enum JobEnd {
    Finished(Outcome),
    /// Not enough space on this drive; the file goes back to waiting.
    NoSpace(String),
}

impl Queue {
    pub fn new(
        processor: Arc<dyn Processor>,
        sink: Arc<dyn ProgressSink>,
        workers: usize,
        log_dir: PathBuf,
    ) -> Self {
        Queue {
            inner: Arc::new(Inner {
                processor,
                sink,
                workers: workers.max(1),
                log_dir,
                state: Mutex::new(State::default()),
                wake: Condvar::new(),
            }),
        }
    }

    /// Adds `entries` as waiting items of `mode`. Paths already in this mode's
    /// list (or repeated within `entries`) are left out; the first occurrence
    /// wins. Items added during a run of `mode` are part of that run.
    pub fn add(&self, mode: Mode, entries: Vec<ScanEntry>) -> Vec<QueueItem> {
        // `identity` touches the disk; do it before taking the lock.
        let keyed: Vec<(PathBuf, ScanEntry)> = entries
            .into_iter()
            .map(|e| (identity(&e.path), e))
            .collect();
        let mut st = self.inner.lock();
        let mut seen: HashSet<PathBuf> = st
            .items
            .iter()
            .filter(|item| item.mode == mode)
            .map(|item| item.key.clone())
            .collect();
        let mut added = Vec::new();
        for (key, entry) in keyed {
            if !seen.insert(key.clone()) {
                continue;
            }
            let id = st.next_id;
            st.next_id += 1;
            added.push(QueueItem {
                id,
                entry: entry.clone(),
            });
            st.items.push(Item {
                id,
                mode,
                key,
                entry,
                status: ItemStatus::Waiting,
            });
        }
        drop(st);
        self.inner.wake.notify_all();
        added
    }

    pub fn entry(&self, id: u64) -> Option<ScanEntry> {
        let st = self.inner.lock();
        st.items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.entry.clone())
    }

    /// Removes every item of `mode`. Refused while a run is active.
    pub fn clear(&self, mode: Mode) -> Result<(), String> {
        let mut st = self.inner.lock();
        if st.run.is_some() {
            return Err(CLEAR_WHILE_RUNNING.to_string());
        }
        st.items.retain(|item| item.mode != mode);
        Ok(())
    }

    /// Starts a run over the waiting items of `mode`. Does nothing while a
    /// run is active.
    pub fn start(&self, mode: Mode, s: JobSettings) {
        Inner::start(&self.inner, mode, s, false);
    }

    /// Puts the failed items of `mode` (error, check failed) back to waiting
    /// and starts a run. Does nothing while a run is active.
    pub fn retry_failed(&self, mode: Mode, s: JobSettings) {
        Inner::start(&self.inner, mode, s, true);
    }

    /// Running files finish; no new ones start until `resume`.
    pub fn pause(&self) {
        let mut st = self.inner.lock();
        let Some(run) = st.run.as_mut().filter(|run| !run.over && !run.paused) else {
            return;
        };
        run.paused = true;
        drop(st);
        self.inner.sink.paused(PauseReason::User);
    }

    pub fn resume(&self) {
        let mut st = self.inner.lock();
        if let Some(run) = st.run.as_mut() {
            run.paused = false;
        }
        drop(st);
        self.inner.wake.notify_all();
    }

    /// Asks the running files to stop; they go back to waiting. Returns at
    /// once, `finished` follows when the last worker is done.
    pub fn cancel(&self) {
        let st = self.inner.lock();
        if let Some(run) = &st.run {
            run.cancel.cancel();
        }
        drop(st);
        self.inner.wake.notify_all();
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().run.is_some()
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, id: u64, status: ItemStatus, progress: u8, o: Option<&Outcome>) {
        self.sink.item(ItemEvent {
            id,
            status,
            progress,
            out_size: o.and_then(|o| o.out_size),
            note: o.and_then(|o| o.note.clone()),
        });
    }

    fn start(this: &Arc<Inner>, mode: Mode, s: JobSettings, retry: bool) {
        let mut st = this.lock();
        if st.run.is_some() {
            return;
        }
        let mut reset = Vec::new();
        if retry {
            for item in st.items.iter_mut().filter(|item| item.mode == mode) {
                if matches!(item.status, ItemStatus::Error | ItemStatus::VerifyFailed) {
                    item.status = ItemStatus::Waiting;
                    reset.push(item.id);
                }
            }
        }
        st.run = Some(Run {
            mode,
            report: RunReport::new(mode, &s),
            settings: Arc::new(s),
            // A token cannot be reset, so every run gets its own.
            cancel: CancelToken::new(),
            paused: false,
            in_flight: 0,
            alive: this.workers,
            over: false,
        });
        drop(st);
        for id in reset {
            this.emit(id, ItemStatus::Waiting, 0, None);
        }
        for _ in 0..this.workers {
            let inner = Arc::clone(this);
            thread::spawn(move || inner.work());
        }
    }

    fn work(&self) {
        while let Some(job) = self.next_job() {
            self.run_job(job);
        }
        self.leave();
    }

    /// Waits for the next file to hand out; `None` when the worker should
    /// leave (run over or cancelled).
    fn next_job(&self) -> Option<Job> {
        let mut st = self.lock();
        loop {
            let State { items, run, .. } = &mut *st;
            let run = run.as_mut().expect("a run exists while its workers live");
            if run.over || run.cancel.is_cancelled() {
                return None;
            }
            let waiting = items
                .iter_mut()
                .find(|item| item.mode == run.mode && item.status == ItemStatus::Waiting);
            match waiting {
                // Also ends a paused run that has nothing left.
                None if run.in_flight == 0 => {
                    run.over = true;
                    self.wake.notify_all();
                    return None;
                }
                Some(item) if !run.paused => {
                    item.status = ItemStatus::Running;
                    run.in_flight += 1;
                    let job = Job {
                        id: item.id,
                        mode: run.mode,
                        entry: item.entry.clone(),
                        settings: Arc::clone(&run.settings),
                        cancel: run.cancel.clone(),
                    };
                    drop(st);
                    self.emit(job.id, ItemStatus::Running, 0, None);
                    return Some(job);
                }
                _ => st = self.wake.wait(st).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    fn run_job(&self, job: Job) {
        let end = self.process(&job);
        let mut st = self.lock();
        let State { items, run, .. } = &mut *st;
        let run = run.as_mut().expect("a run exists while its workers live");
        run.in_flight -= 1;
        let mut newly_full = None;
        let outcome = match end {
            JobEnd::Finished(outcome) => outcome,
            JobEnd::NoSpace(drive) => {
                if !run.paused {
                    run.paused = true;
                    newly_full = Some(drive);
                }
                Outcome {
                    status: ItemStatus::Waiting,
                    out_path: None,
                    out_size: None,
                    note: None,
                }
            }
        };
        // A cancelled file comes back as `Waiting`; the report does not count it.
        let status = outcome.status;
        run.report.add(&job.entry.path, job.entry.size, &outcome);
        if let Some(item) = items.iter_mut().find(|item| item.id == job.id) {
            item.status = status;
        }
        drop(st);
        self.wake.notify_all();

        let progress = if status == ItemStatus::Waiting {
            0
        } else {
            100
        };
        self.emit(job.id, status, progress, Some(&outcome));
        if let Some(drive) = newly_full {
            self.sink.paused(PauseReason::DiskFull { drive });
        }
    }

    /// Target, reservation, space check, and the pipeline itself.
    fn process(&self, job: &Job) -> JobEnd {
        let target = match self
            .processor
            .target_for(job.mode, &job.entry, &job.settings)
        {
            Ok(target) => target,
            Err(_) => return JobEnd::Finished(stopped(ItemStatus::Error, TARGET_UNKNOWN)),
        };
        // Windows paths ignore letter case.
        let key = identity(&target).to_string_lossy().to_lowercase();
        if !self.lock().reserved.insert(key.clone()) {
            return JobEnd::Finished(stopped(ItemStatus::Skipped, TARGET_EXISTS));
        }
        let end = if target.exists() {
            JobEnd::Finished(stopped(ItemStatus::Skipped, TARGET_EXISTS))
        } else if !self.processor.has_space(&job.entry, &target) {
            JobEnd::NoSpace(drive_of(&target))
        } else {
            let progress = |p: u8| self.emit(job.id, ItemStatus::Running, p, None);
            JobEnd::Finished(self.processor.process(
                job.mode,
                &job.entry,
                &target,
                &job.settings,
                &job.cancel,
                &progress,
            ))
        };
        self.lock().reserved.remove(&key);
        end
    }

    /// The last worker of a run ends it: summary, log, `finished`.
    fn leave(&self) {
        let mut st = self.lock();
        let run = st
            .run
            .as_mut()
            .expect("a run exists while its workers live");
        run.alive -= 1;
        if run.alive > 0 {
            return;
        }
        let run = st.run.take().expect("checked above");
        drop(st);
        let mut summary = run.report.summary();
        if run.report.has_problems() {
            summary.log_path = run.report.write_log(&self.log_dir).ok();
        }
        self.sink.finished(summary);
    }
}

fn stopped(status: ItemStatus, note: &str) -> Outcome {
    Outcome {
        status,
        out_path: None,
        out_size: None,
        note: Some(note.to_string()),
    }
}

/// `D:` for a drive path, otherwise the path's prefix (a UNC share) or the
/// whole path.
fn drive_of(path: &Path) -> String {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                format!("{}:", char::from(d).to_ascii_uppercase())
            }
            _ => prefix.as_os_str().to_string_lossy().into_owned(),
        },
        _ => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::report::tests::is_log_name;
    use crate::engine::types::{
        DecodeSettings, EncodeSettings, InputKind, ItemStatus, OutputFormat, OutputLocation,
    };
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::sync::{Condvar, Mutex};
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(10);
    /// How long a test waits to see that something does *not* happen.
    const QUIET: Duration = Duration::from_millis(300);

    fn settings() -> JobSettings {
        JobSettings {
            encode: EncodeSettings {
                distance: 1.2,
                effort: 7,
                transcode_jpeg: true,
            },
            decode: DecodeSettings {
                format: OutputFormat::Auto,
                jpeg_quality: 92,
                reduce_to_8bit: false,
            },
            keep_metadata: true,
            trash_originals: false,
            output: OutputLocation::Next,
        }
    }

    /// Entry for a path that does not exist (nothing touches the disk).
    fn entry(name: &str) -> ScanEntry {
        ScanEntry {
            path: PathBuf::from(format!(r"Q:\bilder\{name}")),
            root: None,
            kind: InputKind::Png,
            size: 100,
            width: 1,
            height: 1,
        }
    }

    fn file_name(e: &ScanEntry) -> String {
        e.path.file_name().unwrap().to_string_lossy().into_owned()
    }

    /// Answers per file name (default `Done`). Its target is `<stem>.jxl`
    /// beside the source. While the gate is closed, `process` waits (or
    /// returns `Waiting` once cancelled), so tests decide when files finish.
    struct FakeProcessor {
        results: Mutex<HashMap<String, ItemStatus>>,
        no_space: HashSet<String>,
        gate: Mutex<bool>,
        gate_cv: Condvar,
        /// File names in the order `process` was called.
        started: Mutex<Vec<String>>,
    }

    impl FakeProcessor {
        fn new(results: &[(&str, ItemStatus)], no_space: &[&str], open: bool) -> Arc<Self> {
            Arc::new(FakeProcessor {
                results: Mutex::new(
                    results
                        .iter()
                        .map(|(name, status)| (name.to_string(), *status))
                        .collect(),
                ),
                no_space: no_space.iter().map(|name| name.to_string()).collect(),
                gate: Mutex::new(open),
                gate_cv: Condvar::new(),
                started: Mutex::new(Vec::new()),
            })
        }

        fn open(&self) {
            *self.gate.lock().unwrap() = true;
            self.gate_cv.notify_all();
        }

        fn set_result(&self, name: &str, status: ItemStatus) {
            self.results
                .lock()
                .unwrap()
                .insert(name.to_string(), status);
        }

        fn started(&self) -> Vec<String> {
            self.started.lock().unwrap().clone()
        }
    }

    impl Processor for FakeProcessor {
        fn target_for(&self, _mode: Mode, e: &ScanEntry, _s: &JobSettings) -> io::Result<PathBuf> {
            Ok(e.path.with_extension("jxl"))
        }

        fn has_space(&self, e: &ScanEntry, _target: &Path) -> bool {
            !self.no_space.contains(&file_name(e))
        }

        fn process(
            &self,
            _mode: Mode,
            e: &ScanEntry,
            target: &Path,
            _s: &JobSettings,
            cancel: &CancelToken,
            progress: &dyn Fn(u8),
        ) -> Outcome {
            let name = file_name(e);
            self.started.lock().unwrap().push(name.clone());
            progress(5);
            let mut open = self.gate.lock().unwrap();
            while !*open {
                if cancel.is_cancelled() {
                    return Outcome {
                        status: ItemStatus::Waiting,
                        out_path: None,
                        out_size: None,
                        note: None,
                    };
                }
                // The token has no wake-up; look again shortly.
                open = self
                    .gate_cv
                    .wait_timeout(open, Duration::from_millis(10))
                    .unwrap()
                    .0;
            }
            drop(open);
            progress(100);
            let status = self
                .results
                .lock()
                .unwrap()
                .get(&name)
                .copied()
                .unwrap_or(ItemStatus::Done);
            let done = matches!(status, ItemStatus::Done | ItemStatus::DoneWithNote);
            Outcome {
                status,
                out_path: done.then(|| target.to_path_buf()),
                out_size: done.then_some(e.size / 2),
                note: (!done).then(|| format!("Grund für {name}")),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Rec {
        Item(ItemEvent),
        Paused(PauseReason),
        Finished(RunSummary),
    }

    #[derive(Default)]
    struct RecordingSink {
        records: Mutex<Vec<Rec>>,
        cv: Condvar,
    }

    impl RecordingSink {
        fn push(&self, rec: Rec) {
            self.records.lock().unwrap().push(rec);
            self.cv.notify_all();
        }

        /// Waits up to `limit` until `pred` holds for the records so far.
        fn wait_until(&self, limit: Duration, pred: impl Fn(&[Rec]) -> bool) -> bool {
            let deadline = Instant::now() + limit;
            let mut records = self.records.lock().unwrap();
            while !pred(&records) {
                let now = Instant::now();
                if now >= deadline {
                    return false;
                }
                records = self.cv.wait_timeout(records, deadline - now).unwrap().0;
            }
            true
        }

        fn wait(&self, what: &str, pred: impl Fn(&[Rec]) -> bool) {
            assert!(
                self.wait_until(TIMEOUT, pred),
                "timed out waiting for {what}: {:#?}",
                self.records()
            );
        }

        fn records(&self) -> Vec<Rec> {
            self.records.lock().unwrap().clone()
        }

        fn finished(&self) -> Vec<RunSummary> {
            self.records()
                .into_iter()
                .filter_map(|r| match r {
                    Rec::Finished(s) => Some(s),
                    _ => None,
                })
                .collect()
        }

        /// Waits for the `n`-th `finished` and returns it.
        fn wait_finished(&self, n: usize) -> RunSummary {
            self.wait("finished", |r| count_finished(r) >= n);
            self.finished()[n - 1].clone()
        }

        /// The last status reported for `id`.
        fn status(&self, id: u64) -> Option<ItemStatus> {
            last_status(&self.records(), id)
        }
    }

    fn count_finished(records: &[Rec]) -> usize {
        records
            .iter()
            .filter(|r| matches!(r, Rec::Finished(_)))
            .count()
    }

    fn last_status(records: &[Rec], id: u64) -> Option<ItemStatus> {
        records.iter().rev().find_map(|r| match r {
            Rec::Item(ev) if ev.id == id => Some(ev.status),
            _ => None,
        })
    }

    fn has_status(records: &[Rec], id: u64, status: ItemStatus) -> bool {
        records
            .iter()
            .any(|r| matches!(r, Rec::Item(ev) if ev.id == id && ev.status == status))
    }

    impl ProgressSink for RecordingSink {
        fn item(&self, ev: ItemEvent) {
            self.push(Rec::Item(ev));
        }

        fn paused(&self, r: PauseReason) {
            self.push(Rec::Paused(r));
        }

        fn finished(&self, s: RunSummary) {
            self.push(Rec::Finished(s));
        }
    }

    struct Setup {
        queue: Queue,
        fake: Arc<FakeProcessor>,
        sink: Arc<RecordingSink>,
        log_dir: tempfile::TempDir,
    }

    fn setup(fake: Arc<FakeProcessor>, workers: usize) -> Setup {
        let sink = Arc::new(RecordingSink::default());
        let log_dir = tempfile::tempdir().unwrap();
        let queue = Queue::new(
            fake.clone(),
            sink.clone(),
            workers,
            log_dir.path().join("Protokolle"),
        );
        Setup {
            queue,
            fake,
            sink,
            log_dir,
        }
    }

    fn add(t: &Setup, names: &[&str]) -> Vec<u64> {
        t.queue
            .add(Mode::Enc, names.iter().map(|n| entry(n)).collect())
            .into_iter()
            .map(|item| item.id)
            .collect()
    }

    #[test]
    fn default_workers_is_half_the_processors() {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        assert_eq!(default_workers(), (cpus / 2).max(1));
    }

    #[test]
    fn processes_all_and_reports_summary() {
        let fake = FakeProcessor::new(
            &[("d.png", ItemStatus::Skipped), ("e.png", ItemStatus::Error)],
            &[],
            true,
        );
        let t = setup(fake, 2);
        let ids = add(&t, &["a.png", "b.png", "c.png", "d.png", "e.png"]);
        t.queue.start(Mode::Enc, settings());
        let summary = t.sink.wait_finished(1);

        assert_eq!(summary.done, 3);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.bytes_in, 300);
        assert_eq!(summary.bytes_out, 150);
        assert!(summary.log_path.is_some());
        let records = t.sink.records();
        for id in &ids {
            assert!(has_status(&records, *id, ItemStatus::Running));
        }
        assert_eq!(last_status(&records, ids[0]), Some(ItemStatus::Done));
        assert_eq!(last_status(&records, ids[3]), Some(ItemStatus::Skipped));
        assert_eq!(last_status(&records, ids[4]), Some(ItemStatus::Error));
        assert!(
            records
                .iter()
                .any(|r| matches!(r, Rec::Item(ev) if ev.id == ids[0] && ev.progress == 5)),
            "progress from the pipeline is forwarded"
        );
        assert!(!t.queue.is_running());
        assert_eq!(count_finished(&records), 1);
    }

    /// Review Focus 1: two sources that want the same target, two workers.
    #[test]
    fn same_target_reserved_once() {
        let fake = FakeProcessor::new(&[], &[], false);
        let t = setup(fake, 2);
        let ids = add(&t, &["foto.jpg", "foto.png"]);
        t.queue.start(Mode::Enc, settings());
        // The first holds its reservation until the gate opens.
        t.sink.wait("one skipped", |r| {
            ids.iter().any(|id| has_status(r, *id, ItemStatus::Skipped))
        });
        t.fake.open();
        let summary = t.sink.wait_finished(1);

        assert_eq!((summary.done, summary.skipped), (1, 1));
        assert_eq!(t.fake.started().len(), 1);
        let records = t.sink.records();
        let skipped = records.iter().find_map(|r| match r {
            Rec::Item(ev) if ev.status == ItemStatus::Skipped => Some(ev.clone()),
            _ => None,
        });
        assert_eq!(
            skipped.unwrap().note.as_deref(),
            Some("Zieldatei existiert bereits")
        );
    }

    /// Review Focus 5: the same file twice, and a folder plus a file from it.
    #[test]
    fn duplicates_not_added() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Fotos");
        fs::create_dir(&folder).unwrap();
        let a = folder.join("a.png");
        let b = folder.join("b.png");
        fs::write(&a, b"x").unwrap();
        fs::write(&b, b"x").unwrap();
        let real = |path: &Path, root: Option<&Path>| ScanEntry {
            path: path.to_path_buf(),
            root: root.map(Path::to_path_buf),
            ..entry("unused.png")
        };
        let t = setup(FakeProcessor::new(&[], &[], true), 1);

        // Folder with both files, then the same file once more in one call.
        let first = t.queue.add(
            Mode::Enc,
            vec![
                real(&a, Some(&folder)),
                real(&b, Some(&folder)),
                real(&a, None),
            ],
        );
        // The folder again, and one file from it written differently.
        let upper = PathBuf::from(a.to_string_lossy().to_uppercase());
        let dotted = folder.join(".").join("b.png");
        let second = t.queue.add(
            Mode::Enc,
            vec![
                real(&upper, None),
                real(&dotted, None),
                real(&a, Some(&folder)),
            ],
        );

        assert_eq!(first.len(), 2);
        assert!(second.is_empty(), "{second:?}");
        let ids: HashSet<u64> = first.iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(t.queue.entry(first[0].id).unwrap().path, a);
        assert_eq!(t.queue.entry(first[1].id).unwrap().path, b);
        // The other mode has its own list.
        assert_eq!(t.queue.add(Mode::Dec, vec![real(&a, None)]).len(), 1);
    }

    #[test]
    fn pause_finishes_running_then_stops() {
        let t = setup(FakeProcessor::new(&[], &[], false), 1);
        let ids = add(&t, &["a.png", "b.png", "c.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink
            .wait("a running", |r| has_status(r, ids[0], ItemStatus::Running));
        t.queue.pause();
        t.fake.open();
        t.sink
            .wait("a done", |r| has_status(r, ids[0], ItemStatus::Done));

        assert!(!t.sink.wait_until(QUIET, |r| {
            has_status(r, ids[1], ItemStatus::Running) || count_finished(r) > 0
        }));
        assert!(t.sink.records().contains(&Rec::Paused(PauseReason::User)));
        assert_eq!(t.fake.started(), ["a.png"]);
        assert!(t.queue.is_running());
    }

    #[test]
    fn resume_continues() {
        let t = setup(FakeProcessor::new(&[], &[], false), 1);
        let ids = add(&t, &["a.png", "b.png", "c.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink
            .wait("a running", |r| has_status(r, ids[0], ItemStatus::Running));
        t.queue.pause();
        t.fake.open();
        t.sink
            .wait("a done", |r| has_status(r, ids[0], ItemStatus::Done));
        t.queue.resume();
        let summary = t.sink.wait_finished(1);

        assert_eq!(summary.done, 3);
        assert_eq!(t.fake.started(), ["a.png", "b.png", "c.png"]);
        assert!(!t.queue.is_running());
    }

    #[test]
    fn cancel_returns_running_to_waiting() {
        let t = setup(FakeProcessor::new(&[], &[], false), 2);
        let ids = add(&t, &["a.png", "b.png", "c.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink.wait("two running", |r| {
            has_status(r, ids[0], ItemStatus::Running) && has_status(r, ids[1], ItemStatus::Running)
        });
        t.queue.cancel();
        let summary = t.sink.wait_finished(1);

        assert_eq!((summary.done, summary.skipped, summary.errors), (0, 0, 0));
        assert_eq!(summary.log_path, None);
        assert_eq!(t.sink.status(ids[0]), Some(ItemStatus::Waiting));
        assert_eq!(t.sink.status(ids[1]), Some(ItemStatus::Waiting));
        assert_eq!(t.sink.status(ids[2]), None, "c never started");
        assert!(!t.queue.is_running());

        // A new run gets a fresh token and does all three.
        t.fake.open();
        t.queue.start(Mode::Enc, settings());
        assert_eq!(t.sink.wait_finished(2).done, 3);
    }

    #[test]
    fn cancel_while_paused_finishes() {
        let t = setup(FakeProcessor::new(&[], &[], false), 1);
        let ids = add(&t, &["a.png", "b.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink
            .wait("a running", |r| has_status(r, ids[0], ItemStatus::Running));
        t.queue.pause();
        t.fake.open();
        t.sink
            .wait("a done", |r| has_status(r, ids[0], ItemStatus::Done));
        t.queue.cancel();

        assert_eq!(t.sink.wait_finished(1).done, 1);
        assert_eq!(t.fake.started(), ["a.png"]);
    }

    #[test]
    fn disk_full_pauses_with_reason() {
        let t = setup(FakeProcessor::new(&[], &["b.png"], true), 1);
        let ids = add(&t, &["a.png", "b.png", "c.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink
            .wait("paused", |r| r.iter().any(|r| matches!(r, Rec::Paused(_))));

        let records = t.sink.records();
        assert!(records.contains(&Rec::Paused(PauseReason::DiskFull { drive: "Q:".into() })));
        assert_eq!(last_status(&records, ids[0]), Some(ItemStatus::Done));
        assert_eq!(last_status(&records, ids[1]), Some(ItemStatus::Waiting));
        assert_eq!(count_finished(&records), 0);
        assert!(t.queue.is_running());

        t.queue.cancel();
        let summary = t.sink.wait_finished(1);
        assert_eq!(summary.done, 1);
        assert_eq!(t.fake.started(), ["a.png"], "b and c never ran");
    }

    #[test]
    fn add_during_run_is_processed() {
        let t = setup(FakeProcessor::new(&[], &[], false), 1);
        let first = add(&t, &["a.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink.wait("a running", |r| {
            has_status(r, first[0], ItemStatus::Running)
        });
        let second = add(&t, &["b.png"]);
        t.fake.open();
        let summary = t.sink.wait_finished(1);

        assert_eq!(summary.done, 2);
        assert_eq!(t.sink.status(second[0]), Some(ItemStatus::Done));
    }

    #[test]
    fn retry_failed_only_reruns_errors() {
        let fake = FakeProcessor::new(
            &[
                ("b.png", ItemStatus::Skipped),
                ("c.png", ItemStatus::Error),
                ("d.png", ItemStatus::VerifyFailed),
            ],
            &[],
            true,
        );
        let t = setup(fake, 1);
        let ids = add(&t, &["a.png", "b.png", "c.png", "d.png"]);
        t.queue.start(Mode::Enc, settings());
        let first = t.sink.wait_finished(1);
        assert_eq!((first.done, first.skipped, first.errors), (1, 1, 2));

        t.fake.set_result("c.png", ItemStatus::Done);
        t.fake.set_result("d.png", ItemStatus::Done);
        t.queue.retry_failed(Mode::Enc, settings());
        let second = t.sink.wait_finished(2);

        assert_eq!((second.done, second.skipped, second.errors), (2, 0, 0));
        assert_eq!(&t.fake.started()[4..], ["c.png", "d.png"]);
        assert_eq!(t.sink.status(ids[1]), Some(ItemStatus::Skipped));
        assert_eq!(t.sink.status(ids[2]), Some(ItemStatus::Done));
    }

    #[test]
    fn clear_rejected_while_running() {
        let t = setup(FakeProcessor::new(&[], &[], false), 1);
        let ids = add(&t, &["a.png"]);
        t.queue.start(Mode::Enc, settings());
        t.sink
            .wait("a running", |r| has_status(r, ids[0], ItemStatus::Running));

        assert!(t.queue.clear(Mode::Enc).is_err());
        assert!(t.queue.entry(ids[0]).is_some());

        t.queue.cancel();
        t.sink.wait_finished(1);
        assert_eq!(t.queue.clear(Mode::Enc), Ok(()));
        assert_eq!(t.queue.entry(ids[0]), None);
        // Cleared paths may be added again.
        assert_eq!(add(&t, &["a.png"]).len(), 1);
    }

    #[test]
    fn log_written_only_with_problems() {
        let fake = FakeProcessor::new(&[], &[], true);
        let t = setup(fake, 1);
        add(&t, &["a.png"]);
        t.queue.start(Mode::Enc, settings());
        assert_eq!(t.sink.wait_finished(1).log_path, None);
        assert!(!t.log_dir.path().join("Protokolle").exists());

        t.fake.set_result("e.png", ItemStatus::Error);
        add(&t, &["e.png"]);
        t.queue.start(Mode::Enc, settings());
        let summary = t.sink.wait_finished(2);
        let log = summary.log_path.expect("log for the error");

        assert_eq!(log.parent().unwrap(), t.log_dir.path().join("Protokolle"));
        assert!(is_log_name(log.file_name().unwrap().to_str().unwrap()));
        let text = fs::read_to_string(&log).unwrap();
        assert!(
            text.contains("Summe: 0 fertig, 0 übersprungen, 1 Fehler"),
            "{text}"
        );
        assert!(
            text.contains("Fehler\tQ:\\bilder\\e.png\tGrund für e.png"),
            "{text}"
        );
    }

    fn real_processor() -> RealProcessor {
        RealProcessor {
            tools: Tools {
                cjxl: PathBuf::from("cjxl.exe"),
                djxl: PathBuf::from("djxl.exe"),
            },
            bin: Arc::new(crate::engine::originals::FakeRecycleBin {
                availability: crate::engine::originals::Availability::NoRecycleBin,
                fail: false,
                recycled: Mutex::new(Vec::new()),
            }),
            work_dir: std::env::temp_dir(),
        }
    }

    #[test]
    fn real_processor_encode_target_is_jxl() {
        let target = real_processor()
            .target_for(Mode::Enc, &entry("foto.png"), &settings())
            .unwrap();
        assert_eq!(target, PathBuf::from(r"Q:\bilder\foto.jxl"));
    }

    #[test]
    fn real_processor_space_needs_twice_the_size() {
        let dir = tempfile::tempdir().unwrap();
        let p = real_processor();
        // The target folder does not exist yet: the nearest existing one counts.
        let target = dir.path().join("neu").join("a.jxl");
        let small = ScanEntry {
            size: 1,
            ..entry("a.png")
        };
        let huge = ScanEntry {
            size: u64::MAX / 2,
            ..entry("a.png")
        };
        assert!(p.has_space(&small, &target));
        assert!(!p.has_space(&huge, &target));
        // Unknown free space (no existing folder) does not block.
        assert!(p.has_space(&huge, Path::new(r"Q:\nirgends\a.jxl")));
    }
}
