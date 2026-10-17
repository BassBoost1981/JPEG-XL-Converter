//! What a run did: the counts for the summary and, when something needs the
//! user's attention, a log file listing those files.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::types::{
    ItemStatus, JobSettings, Mode, Outcome, OutputFormat, OutputLocation, RunSummary,
};

pub struct RunReport {
    mode: Mode,
    settings: JobSettings,
    summary: RunSummary,
    /// Files that need attention: status, path, reason.
    problems: Vec<(ItemStatus, PathBuf, String)>,
}

impl RunReport {
    pub fn new(mode: Mode, settings: &JobSettings) -> Self {
        RunReport {
            mode,
            settings: settings.clone(),
            summary: RunSummary {
                done: 0,
                skipped: 0,
                errors: 0,
                bytes_in: 0,
                bytes_out: 0,
                log_path: None,
            },
            problems: Vec::new(),
        }
    }

    /// Counts one finished file. `Waiting` and `Running` (a cancelled file)
    /// are not counted. Bytes count only for finished files.
    pub fn add(&mut self, path: &Path, size_in: u64, o: &Outcome) {
        let s = &mut self.summary;
        match o.status {
            ItemStatus::Waiting | ItemStatus::Running => return,
            ItemStatus::Done | ItemStatus::DoneWithNote => {
                s.done += 1;
                s.bytes_in += size_in;
                s.bytes_out += o.out_size.unwrap_or(0);
            }
            ItemStatus::Skipped => s.skipped += 1,
            ItemStatus::Error | ItemStatus::VerifyFailed => s.errors += 1,
        }
        if o.status != ItemStatus::Done {
            let reason = o.note.clone().unwrap_or_default();
            self.problems.push((o.status, path.to_path_buf(), reason));
        }
    }

    pub fn summary(&self) -> RunSummary {
        self.summary.clone()
    }

    /// True when the run has a file for the log: skipped, failed, check
    /// failed, or done with a note.
    pub fn has_problems(&self) -> bool {
        !self.problems.is_empty()
    }

    /// Writes the log into `dir` (created if missing) as
    /// `JJJJ-MM-TT_HH-MM-SS.txt` and returns its path.
    pub fn write_log(&self, dir: &Path) -> io::Result<PathBuf> {
        fs::create_dir_all(dir)?;
        let name = chrono::Local::now()
            .format("%Y-%m-%d_%H-%M-%S.txt")
            .to_string();
        let path = dir.join(name);
        fs::write(&path, self.log_text())?;
        Ok(path)
    }

    fn log_text(&self) -> String {
        let s = &self.summary;
        let mut text = String::from("JPEG XL Konverter – Protokoll\r\n");
        let mode = match self.mode {
            Mode::Enc => "In JPEG XL umwandeln",
            Mode::Dec => "Aus JPEG XL zurückwandeln",
        };
        let _ = write!(text, "Modus: {mode}\r\n");
        let _ = write!(text, "Einstellungen: {}\r\n", self.settings_text());
        let _ = write!(
            text,
            "Summe: {} fertig, {} übersprungen, {} Fehler, {} Bytes → {} Bytes\r\n\r\n",
            s.done, s.skipped, s.errors, s.bytes_in, s.bytes_out
        );
        for (status, path, reason) in &self.problems {
            let _ = write!(
                text,
                "{}\t{}\t{reason}\r\n",
                status_word(*status),
                path.display()
            );
        }
        text
    }

    fn settings_text(&self) -> String {
        let s = &self.settings;
        let yes_no = |b: bool| if b { "ja" } else { "nein" };
        let specific = match self.mode {
            Mode::Enc => format!(
                "Distanz {}, Aufwand {}, JPEG verlustfrei umpacken: {}",
                s.encode.distance,
                s.encode.effort,
                yes_no(s.encode.transcode_jpeg)
            ),
            Mode::Dec => {
                let format = match s.decode.format {
                    OutputFormat::Auto => "Automatisch",
                    OutputFormat::Png => "PNG",
                    OutputFormat::Jpg => "JPEG",
                    OutputFormat::Webp => "WebP",
                    OutputFormat::Tiff => "TIFF",
                };
                format!(
                    "Format {format}, JPEG-Qualität {}, auf 8 Bit reduzieren: {}",
                    s.decode.jpeg_quality,
                    yes_no(s.decode.reduce_to_8bit)
                )
            }
        };
        let output = match &s.output {
            OutputLocation::Next => "neben dem Original".to_string(),
            OutputLocation::Folder { path } => path.display().to_string(),
        };
        format!(
            "{specific}, Metadaten behalten: {}, Originale in den Papierkorb: {}, Ziel: {output}",
            yes_no(s.keep_metadata),
            yes_no(s.trash_originals)
        )
    }
}

fn status_word(status: ItemStatus) -> &'static str {
    match status {
        ItemStatus::Skipped => "Übersprungen",
        ItemStatus::Error => "Fehler",
        ItemStatus::VerifyFailed => "Prüfung fehlgeschlagen",
        ItemStatus::DoneWithNote => "Fertig mit Hinweis",
        ItemStatus::Done => "Fertig",
        ItemStatus::Waiting => "Wartet",
        ItemStatus::Running => "Läuft",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::types::{DecodeSettings, EncodeSettings};

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

    fn outcome(status: ItemStatus, out_size: Option<u64>, note: Option<&str>) -> Outcome {
        Outcome {
            status,
            out_path: None,
            out_size,
            note: note.map(str::to_string),
        }
    }

    #[test]
    fn summary_counts_done_skipped_and_errors() {
        let mut report = RunReport::new(Mode::Enc, &settings());
        report.add(
            Path::new("a.jpg"),
            100,
            &outcome(ItemStatus::Done, Some(40), None),
        );
        report.add(
            Path::new("b.jpg"),
            200,
            &outcome(ItemStatus::DoneWithNote, Some(50), Some("Hinweis")),
        );
        report.add(
            Path::new("c.jpg"),
            300,
            &outcome(ItemStatus::Skipped, None, Some("x")),
        );
        report.add(
            Path::new("d.jpg"),
            400,
            &outcome(ItemStatus::Error, None, Some("y")),
        );
        report.add(
            Path::new("e.jpg"),
            500,
            &outcome(ItemStatus::VerifyFailed, None, Some("z")),
        );
        report.add(
            Path::new("f.jpg"),
            600,
            &outcome(ItemStatus::Waiting, None, None),
        );
        assert_eq!(
            report.summary(),
            RunSummary {
                done: 2,
                skipped: 1,
                errors: 2,
                bytes_in: 300,
                bytes_out: 90,
                log_path: None,
            }
        );
    }

    #[test]
    fn only_done_files_are_no_problem() {
        let mut report = RunReport::new(Mode::Enc, &settings());
        report.add(
            Path::new("a.jpg"),
            100,
            &outcome(ItemStatus::Done, Some(40), None),
        );
        assert!(!report.has_problems());
        report.add(
            Path::new("b.jpg"),
            100,
            &outcome(
                ItemStatus::DoneWithNote,
                Some(40),
                Some("Original behalten: kein Papierkorb"),
            ),
        );
        assert!(report.has_problems());
    }

    #[test]
    fn log_has_name_summary_and_problem_lines() {
        let dir = tempfile::tempdir().unwrap();
        let mut report = RunReport::new(Mode::Dec, &settings());
        report.add(
            Path::new("a.jxl"),
            100,
            &outcome(ItemStatus::Done, Some(400), None),
        );
        report.add(
            Path::new(r"D:\Bilder\b.jxl"),
            100,
            &outcome(
                ItemStatus::Error,
                None,
                Some("JPEG XL lässt sich nicht lesen"),
            ),
        );
        report.add(
            Path::new(r"D:\Bilder\c.jxl"),
            100,
            &outcome(
                ItemStatus::Skipped,
                None,
                Some("Zieldatei existiert bereits"),
            ),
        );
        report.add(
            Path::new(r"D:\Bilder\d.jxl"),
            100,
            &outcome(ItemStatus::VerifyFailed, None, Some("Pixel weichen ab")),
        );
        report.add(
            Path::new(r"D:\Bilder\e.jxl"),
            100,
            &outcome(
                ItemStatus::DoneWithNote,
                Some(10),
                Some("Animation – .jxl behalten"),
            ),
        );
        let path = report.write_log(&dir.path().join("Protokolle")).unwrap();

        assert_eq!(path.parent().unwrap(), dir.path().join("Protokolle"));
        assert!(is_log_name(path.file_name().unwrap().to_str().unwrap()));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Modus: Aus JPEG XL zurückwandeln"), "{text}");
        assert!(
            text.contains("Summe: 2 fertig, 1 übersprungen, 2 Fehler"),
            "{text}"
        );
        assert!(text.contains("Fehler\tD:\\Bilder\\b.jxl\tJPEG XL lässt sich nicht lesen"));
        assert!(text.contains("Übersprungen\tD:\\Bilder\\c.jxl\tZieldatei existiert bereits"));
        assert!(text.contains("Prüfung fehlgeschlagen\tD:\\Bilder\\d.jxl\tPixel weichen ab"));
        assert!(text.contains("Fertig mit Hinweis\tD:\\Bilder\\e.jxl\tAnimation – .jxl behalten"));
        assert!(!text.contains("a.jxl"), "{text}");
    }

    /// `^\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}\.txt$`
    pub(crate) fn is_log_name(name: &str) -> bool {
        let Some(stem) = name.strip_suffix(".txt") else {
            return false;
        };
        let pattern = "dddd-dd-dd_dd-dd-dd";
        stem.len() == pattern.len()
            && stem.chars().zip(pattern.chars()).all(|(c, p)| match p {
                'd' => c.is_ascii_digit(),
                _ => c == p,
            })
    }
}
