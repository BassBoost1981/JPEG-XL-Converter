//! A user-visible message as data: a catalog key plus parameters. The engine
//! produces these instead of text; the frontend (`src/i18n`) or `engine::i18n`
//! turns them into the current language. Serde shape matches `Msg` in
//! `src/i18n/index.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Msg {
    pub key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, MsgParam>,
}

/// A parameter is plain text, or a message of its own (a nested reason).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MsgParam {
    Text(String),
    Msg(Box<Msg>),
}

impl Msg {
    pub fn new(key: &str) -> Msg {
        Msg {
            key: key.to_owned(),
            params: BTreeMap::new(),
        }
    }

    pub fn with(mut self, name: &str, value: impl Into<String>) -> Msg {
        self.params
            .insert(name.to_owned(), MsgParam::Text(value.into()));
        self
    }

    pub fn with_msg(mut self, name: &str, inner: Msg) -> Msg {
        self.params
            .insert(name.to_owned(), MsgParam::Msg(Box::new(inner)));
        self
    }
}

/// Defines one `pub const NAME: &str = "key";` per entry and `ALL` with all of them.
macro_rules! define_keys {
    ($($name:ident = $key:literal,)*) => {
        $(pub const $name: &str = $key;)*
        /// Every key the engine produces or writes into the log file.
        pub const ALL: &[&str] = &[$($name),*];
    };
}

/// Catalog keys (`src/i18n/de.json`, `en.json`) that the Rust side produces. UI-only
/// keys are not listed. Names drop the key's group (`note.`, `error.`, ...) except for
/// `log.*`, `preview.*` and where two would collide (`error.internal`/`reason.internal`).
pub mod keys {
    define_keys! {
        APP_TITLE = "app.title",
        TOOLS_MISSING = "banner.toolsMissing",

        KEPT_NO_RECYCLE_BIN = "note.keptNoRecycleBin",
        KEPT_TOO_LARGE = "note.keptTooLarge",
        KEPT_RECYCLE_FAILED = "note.keptRecycleFailed",
        FIRST_PAGE_ONLY = "note.firstPageOnly",
        ANIMATION_KEPT_ORIGINAL = "note.animationKeptOriginal",
        ANIMATION_KEPT_JXL = "note.animationKeptJxl",
        NOT_EXACT_KEPT_JXL = "note.notExactKeptJxl",
        METADATA_INCOMPLETE = "note.metadataIncomplete",
        DATES_NOT_KEPT = "note.datesNotKept",

        ERROR_INTERNAL = "error.internal",
        UNKNOWN_ITEM = "error.unknownItem",
        OUTSIDE_LOG_DIR = "error.outsideLogDir",
        OPEN_LOG = "error.openLog",
        SAVE_SETTINGS = "error.saveSettings",
        TARGET_UNKNOWN = "error.targetUnknown",
        CLEAR_WHILE_RUNNING = "error.clearWhileRunning",
        TARGET_EXISTS = "error.targetExists",
        TARGET_DIR_FAILED = "error.targetDirFailed",
        WORK_DIR_FAILED = "error.workDirFailed",
        TARGET_CREATE_FAILED = "error.targetCreateFailed",
        STALE_TEMP_FILE = "error.staleTempFile",
        CANNOT_OPEN = "error.cannotOpen",
        CANNOT_READ = "error.cannotRead",
        CANNOT_WRITE = "error.cannotWrite",
        CANNOT_EDIT = "error.cannotEdit",
        IMAGE_TOO_BIG = "error.imageTooBig",
        OUTPUT_FORMAT_UNSUPPORTED = "error.outputFormatUnsupported",
        FORMAT_UNSUPPORTED = "error.formatUnsupported",
        CMYK_UNSUPPORTED = "error.cmykUnsupported",
        FLOAT_UNSUPPORTED = "error.floatUnsupported",
        WEBP_TOO_LARGE = "error.webpTooLarge",
        WEBP_ENCODE = "error.webpEncode",
        NOT_TRANSCODABLE = "error.notTranscodable",
        ENCODE_FAILED = "error.encodeFailed",
        DECODE_FAILED = "error.decodeFailed",
        CONTENT_MISMATCH = "error.contentMismatch",
        JXL_UNREADABLE = "error.jxlUnreadable",
        RESULT_UNREADABLE = "error.resultUnreadable",
        JPEG_RESTORE_FAILED = "error.jpegRestoreFailed",
        JPEG_DIFFERS = "error.jpegDiffers",
        JXL_DECODE_FAILED = "error.jxlDecodeFailed",
        PIXEL_COMPARE_FAILED = "error.pixelCompareFailed",
        PIXELS_DIFFER = "error.pixelsDiffer",
        DIMS_CHECK_FAILED = "error.dimsCheckFailed",
        DIMS_MISMATCH = "error.dimsMismatch",
        META_CHECK_FAILED = "error.metaCheckFailed",

        NOT_REGULAR = "reason.notRegular",
        REASON_INTERNAL = "reason.internal",
        READ_ONLY = "reason.readOnly",
        NOT_FOUND = "reason.notFound",
        UNREADABLE = "reason.unreadable",
        CANCELLED = "reason.cancelled",
        STILL_THERE = "reason.stillThere",
        UNVERIFIABLE = "reason.unverifiable",
        ACCESS_DENIED = "reason.accessDenied",
        IN_USE = "reason.inUse",
        WINDOWS_ERROR = "reason.windowsError",

        WONT_START = "tool.wontStart",
        NO_RESPONSE = "tool.noResponse",
        NO_VERSION = "tool.noVersion",

        PREVIEW_UNAVAILABLE = "preview.unavailable",
        PREVIEW_UNSUPPORTED = "preview.unsupported",
        PREVIEW_WRONG_MODE = "preview.wrongMode",

        LOG_TITLE = "log.title",
        LOG_MODE = "log.mode",
        LOG_MODE_ENC = "log.mode.enc",
        LOG_MODE_DEC = "log.mode.dec",
        LOG_SETTINGS = "log.settings",
        LOG_SETTINGS_ENC = "log.settings.enc",
        LOG_SETTINGS_DEC = "log.settings.dec",
        LOG_SETTINGS_COMMON = "log.settings.common",
        LOG_TOTALS = "log.totals",
        LOG_YES = "log.yes",
        LOG_NO = "log.no",
        LOG_AUTOMATIC = "log.automatic",
        LOG_NEXT_TO_ORIGINAL = "log.nextToOriginal",
        LOG_STATUS_WAITING = "log.status.waiting",
        LOG_STATUS_RUNNING = "log.status.running",
        LOG_STATUS_DONE = "log.status.done",
        LOG_STATUS_DONE_WITH_NOTE = "log.status.doneWithNote",
        LOG_STATUS_SKIPPED = "log.status.skipped",
        LOG_STATUS_ERROR = "log.status.error",
        LOG_STATUS_VERIFY_FAILED = "log.status.verifyFailed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(msg: &Msg) -> String {
        serde_json::to_string(msg).unwrap()
    }

    #[test]
    fn msg_serializes_compact() {
        assert_eq!(
            json(&Msg::new("error.targetExists")),
            r#"{"key":"error.targetExists"}"#
        );
        assert_eq!(
            json(&Msg::new("footer.noSpace").with("drive", "D:")),
            r#"{"key":"footer.noSpace","params":{"drive":"D:"}}"#
        );
        assert_eq!(
            json(
                &Msg::new("note.keptRecycleFailed").with_msg("reason", Msg::new("reason.readOnly"))
            ),
            r#"{"key":"note.keptRecycleFailed","params":{"reason":{"key":"reason.readOnly"}}}"#
        );
    }

    #[test]
    fn params_serialize_in_name_order() {
        let msg = Msg::new("x").with("b", "2").with("a", "1");
        assert_eq!(json(&msg), r#"{"key":"x","params":{"a":"1","b":"2"}}"#);
    }

    #[test]
    fn msg_round_trips_through_json() {
        let messages = [
            Msg::new("error.targetExists"),
            Msg::new("footer.noSpace").with("drive", "D:"),
            Msg::new("note.keptRecycleFailed").with_msg(
                "reason",
                Msg::new("reason.windowsError").with("code", "0x80270058"),
            ),
            Msg::new("mixed")
                .with("text", "t")
                .with_msg("inner", Msg::new("reason.inUse")),
        ];
        for msg in messages {
            let back: Msg = serde_json::from_str(&json(&msg)).unwrap();
            assert_eq!(back, msg);
        }
    }

    #[test]
    fn deserializes_what_the_frontend_may_send() {
        let bare: Msg = serde_json::from_str(r#"{"key":"error.internal"}"#).unwrap();
        assert_eq!(bare, Msg::new("error.internal"));
        assert!(bare.params.is_empty());

        let nested: Msg = serde_json::from_str(
            r#"{"key":"a","params":{"t":"text","m":{"key":"b","params":{"n":"1"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            nested,
            Msg::new("a")
                .with("t", "text")
                .with_msg("m", Msg::new("b").with("n", "1"))
        );
    }

    #[test]
    fn with_replaces_a_parameter_of_the_same_name() {
        let msg = Msg::new("x").with("a", "1").with("a", "2");
        assert_eq!(json(&msg), r#"{"key":"x","params":{"a":"2"}}"#);
    }

    #[test]
    fn all_keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for key in keys::ALL {
            assert!(seen.insert(*key), "duplicate key in keys::ALL: {key}");
        }
    }

    #[test]
    fn constants_carry_their_catalog_key() {
        assert_eq!(keys::TARGET_EXISTS, "error.targetExists");
        assert_eq!(keys::KEPT_NO_RECYCLE_BIN, "note.keptNoRecycleBin");
        assert_eq!(keys::LOG_AUTOMATIC, "log.automatic");
        assert!(keys::ALL.contains(&keys::TARGET_EXISTS));
        assert!(keys::ALL.contains(&keys::LOG_STATUS_VERIFY_FAILED));
    }
}
