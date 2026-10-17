//! Types shared by all engine modules and, via serde, with the TypeScript side
//! (`src/api/types.ts`). Field and variant names follow the plan's
//! "Gemeinsame Typen" section and must not change.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Enc,
    Dec,
}

/// Display language of the app and of the log file. Callers always pass one; there is no default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    De,
    En,
}

/// Supported input formats. EXR is not among them because the official
/// libjxl v0.12.0 static cjxl cannot read it (Ruling R8, docs/libjxl-notes.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputKind {
    Jpeg,
    Png,
    Gif,
    Tiff,
    Webp,
    Bmp,
    Jxl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    Auto,
    Png,
    Jpg,
    Webp,
    Tiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputLocation {
    Next,
    Folder { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeSettings {
    pub distance: f32,
    pub effort: u8,
    pub transcode_jpeg: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodeSettings {
    pub format: OutputFormat,
    pub jpeg_quality: u8,
    pub reduce_to_8bit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSettings {
    pub encode: EncodeSettings,
    pub decode: DecodeSettings,
    pub keep_metadata: bool,
    pub trash_originals: bool,
    pub output: OutputLocation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEntry {
    pub path: PathBuf,
    pub root: Option<PathBuf>,
    pub kind: InputKind,
    pub size: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: u64,
    pub entry: ScanEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemStatus {
    Waiting,
    Running,
    Done,
    DoneWithNote,
    Skipped,
    Error,
    VerifyFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub status: ItemStatus,
    pub out_path: Option<PathBuf>,
    pub out_size: Option<u64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEvent {
    pub id: u64,
    pub status: ItemStatus,
    pub progress: u8,
    pub out_size: Option<u64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub done: u32,
    pub skipped: u32,
    pub errors: u32,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub log_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PauseReason {
    User,
    DiskFull { drive: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub ok: bool,
    pub cjxl_version: Option<String>,
    pub djxl_version: Option<String>,
    pub message: Option<String>,
}

/// Shared cancel flag. Clones observe the same flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_shapes_match_shared_contract() {
        assert_eq!(serde_json::to_value(Mode::Enc).unwrap(), json!("enc"));
        assert_eq!(
            serde_json::to_value(ItemStatus::DoneWithNote).unwrap(),
            json!("doneWithNote")
        );
        assert_eq!(
            serde_json::to_value(ItemStatus::VerifyFailed).unwrap(),
            json!("verifyFailed")
        );
        assert_eq!(
            serde_json::to_value(OutputLocation::Next).unwrap(),
            json!({"kind": "next"})
        );
        assert_eq!(
            serde_json::to_value(OutputLocation::Folder {
                path: PathBuf::from("E:\\Out")
            })
            .unwrap(),
            json!({"kind": "folder", "path": "E:\\Out"})
        );
        assert_eq!(
            serde_json::to_value(PauseReason::User).unwrap(),
            json!({"type": "user"})
        );
        assert_eq!(
            serde_json::to_value(PauseReason::DiskFull { drive: "D:".into() }).unwrap(),
            json!({"type": "diskFull", "drive": "D:"})
        );
        let enc = EncodeSettings {
            distance: 0.5,
            effort: 7,
            transcode_jpeg: true,
        };
        assert_eq!(
            serde_json::to_value(enc).unwrap(),
            json!({"distance": 0.5, "effort": 7, "transcodeJpeg": true})
        );
    }
}
