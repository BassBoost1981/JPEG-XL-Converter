//! UI settings as one JSON file in the app config folder.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

const FILE_NAME: &str = "settings.json";

/// The stored settings, with `trashOriginals` forced to `false` (the switch
/// starts off at every program start). A missing or unreadable file gives
/// `None`.
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

pub fn save(dir: &Path, value: &Value) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(dir.join(FILE_NAME), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn load_forces_trash_off() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &json!({ "trashOriginals": true, "effort": 7 })).unwrap();
        let loaded = load(dir.path()).unwrap();
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
    fn roundtrip_keeps_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let value = json!({ "trashOriginals": false, "preset": "balanced", "nested": { "a": [1, 2] } });
        save(dir.path().join("neu").as_path(), &value).unwrap();
        assert_eq!(load(&dir.path().join("neu")), Some(value));
    }
}
