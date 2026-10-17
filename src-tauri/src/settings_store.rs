//! UI settings as one JSON file in the app's data folder (`data\settings.json`
//! next to the exe).

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::engine::msg::{Msg, keys};
use crate::storage::Storage;

const FILE_NAME: &str = "settings.json";

/// The stored settings, with `trashOriginals` forced to `false` (the switch
/// starts off at every program start). A missing or unreadable file gives
/// `None`. Values are not validated here; the UI does that.
pub fn load(dir: &Path) -> Option<Value> {
    let text = fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    let mut value: Value = serde_json::from_str(&text).ok()?;
    if let Some(obj) = value.as_object_mut()
        && obj.contains_key("trashOriginals")
    {
        obj.insert("trashOriginals".to_string(), Value::Bool(false));
    }
    Some(value)
}

/// Writes the settings into the data folder. In read-only mode nothing is
/// written and that is not an error. Fails with `error.saveSettings` when
/// the folder cannot be written, e.g. because the stick was pulled.
pub fn save(storage: &Storage, value: &Value) -> Result<(), Msg> {
    if storage.read_only {
        return Ok(());
    }
    write(&storage.data_dir, value).map_err(|_| Msg::new(keys::SAVE_SETTINGS))
}

/// Creates the data folder itself if it is missing, but not its parents: if
/// the stick is gone, the folders must not be rebuilt on another drive.
fn write(dir: &Path, value: &Value) -> io::Result<()> {
    match fs::create_dir(dir) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
        _ => {}
    }
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(dir.join(FILE_NAME), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::resolve;
    use serde_json::json;

    /// A portable `Storage` whose data folder is `<exe_dir>\data`.
    fn storage(exe_dir: &Path, read_only: bool) -> Storage {
        resolve(exe_dir, &exe_dir.join("temp"), 1, &|_| !read_only)
    }

    #[test]
    fn load_forces_trash_off() {
        let tmp = tempfile::tempdir().unwrap();
        let s = storage(tmp.path(), false);
        save(&s, &json!({ "trashOriginals": true, "effort": 7 })).unwrap();
        let loaded = load(&s.data_dir).unwrap();
        assert_eq!(loaded["trashOriginals"], json!(false));
    }

    #[test]
    fn missing_file_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), None);
        assert_eq!(load(&dir.path().join("nicht-da")), None);
    }

    #[test]
    fn corrupt_file_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), "{ kaputt").unwrap();
        assert_eq!(load(dir.path()), None);
    }

    #[test]
    fn load_corrupt_or_foreign() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);

        fs::write(&file, "{ \"lang\": \"de\", ").unwrap();
        assert_eq!(load(dir.path()), None, "broken JSON is ignored");

        fs::write(&file, "").unwrap();
        assert_eq!(load(dir.path()), None, "an empty file is ignored");

        // A foreign or old file loads as it is; the UI validates the fields.
        fs::write(&file, r#"{"lang":"fr","preset":"bal"}"#).unwrap();
        assert_eq!(
            load(dir.path()),
            Some(json!({ "lang": "fr", "preset": "bal" }))
        );
    }

    #[test]
    fn roundtrip_keeps_other_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let s = storage(tmp.path(), false);
        let value = json!({ "trashOriginals": false, "preset": "balanced", "nested": { "a": [1, 2] } });
        save(&s, &value).unwrap();
        assert!(s.data_dir.join(FILE_NAME).is_file());
        assert_eq!(load(&s.data_dir), Some(value));
    }

    #[test]
    fn save_overwrites_an_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = storage(tmp.path(), false);
        save(&s, &json!({ "preset": "fast" })).unwrap();
        save(&s, &json!({ "preset": "best" })).unwrap();
        assert_eq!(load(&s.data_dir), Some(json!({ "preset": "best" })));
    }

    #[test]
    fn save_read_only_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let s = storage(tmp.path(), true);
        assert!(save(&s, &json!({ "preset": "fast" })).is_ok());
        assert!(!s.data_dir.exists(), "nothing is written in read-only mode");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn save_into_vanished_dir_errors() {
        // The stick was pulled: the data folder and its parent are gone.
        let tmp = tempfile::tempdir().unwrap();
        let exe_dir = tmp.path().join("stick");
        let s = storage(&exe_dir, false);
        fs::create_dir_all(&s.data_dir).unwrap();
        fs::remove_dir_all(&exe_dir).unwrap();

        let err = save(&s, &json!({ "preset": "fast" })).unwrap_err();
        assert_eq!(err.key, "error.saveSettings");
        assert!(!exe_dir.exists(), "the missing folders are not recreated");
    }
}
