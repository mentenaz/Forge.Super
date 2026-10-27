//! A JSON file read once at startup and written back on change. `Settings`
//! and `State` in `forge-host` sit on it, and any `core` crate can use it
//! without gpui. Keys are dotted paths into the document, e.g.
//! `panels.cockpit.dock`.
//!
//! A missing file is an empty document. An unreadable or invalid file is
//! also treated as empty, with the error kept for the shell to report, and
//! is moved aside (`*.invalid.json`) on the first write so it isn't lost.

use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

pub struct JsonStore {
    path: PathBuf,
    value: Value,
    load_error: Option<String>,
}

impl JsonStore {
    pub fn load(path: PathBuf) -> Self {
        let (value, load_error) = match std::fs::read_to_string(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                (Value::Object(Map::new()), None)
            }
            Err(err) => (
                Value::Object(Map::new()),
                Some(format!("{}: {err}", path.display())),
            ),
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(value @ Value::Object(_)) => (value, None),
                Ok(_) => (
                    Value::Object(Map::new()),
                    Some(format!("{}: not a JSON object", path.display())),
                ),
                Err(err) => (
                    Value::Object(Map::new()),
                    Some(format!("{}: {err}", path.display())),
                ),
            },
        };
        Self {
            path,
            value,
            load_error,
        }
    }

    /// A store that is never read from or written to disk, for values kept
    /// only while the program runs (defaults).
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            value: Value::Object(Map::new()),
            load_error: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// The value at `key`, if present and of the right type.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut node = &self.value;
        for part in key.split('.') {
            node = node.get(part)?;
        }
        serde_json::from_value(node.clone()).ok()
    }

    /// Sets `key`, creating intermediate objects (and replacing non-objects
    /// in the way).
    pub fn set(&mut self, key: &str, value: impl Serialize) {
        let Ok(value) = serde_json::to_value(value) else {
            return;
        };
        let mut parts = key.split('.').peekable();
        let mut node = &mut self.value;
        while let Some(part) = parts.next() {
            if !node.is_object() {
                *node = Value::Object(Map::new());
            }
            let map = node.as_object_mut().expect("just made an object");
            if parts.peek().is_none() {
                map.insert(part.to_string(), value);
                return;
            }
            node = map
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
        }
    }

    /// Writes the document, pretty-printed. On the first write after a
    /// failed load, the bad file is kept as `<name>.invalid.json`.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if self.load_error.take().is_some() && self.path.exists() {
            let _ = std::fs::rename(&self.path, self.path.with_extension("invalid.json"));
        }
        let text = serde_json::to_string_pretty(&self.value).map_err(std::io::Error::other)?;
        std::fs::write(&self.path, text + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("forge-store-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("store.json")
    }

    #[test]
    fn missing_file_is_empty_without_error() {
        let store = JsonStore::load(temp_file("missing"));
        assert!(store.load_error().is_none());
        assert_eq!(store.get::<u32>("a.b"), None);
    }

    #[test]
    fn set_get_and_round_trip_dotted_keys() {
        let path = temp_file("round-trip");
        let mut store = JsonStore::load(path.clone());
        store.set("panels.cockpit.dock", "right");
        store.set("ui_font_size", 13);
        assert_eq!(
            store.get::<String>("panels.cockpit.dock").as_deref(),
            Some("right")
        );
        store.save().unwrap();

        let reloaded = JsonStore::load(path);
        assert_eq!(reloaded.get::<u32>("ui_font_size"), Some(13));
        assert_eq!(
            reloaded.get::<String>("panels.cockpit.dock").as_deref(),
            Some("right")
        );
        assert_eq!(
            reloaded.get::<u32>("panels.cockpit.dock"),
            None,
            "wrong type reads as missing"
        );
    }

    #[test]
    fn in_memory_store_holds_values() {
        let mut store = JsonStore::in_memory();
        store.set("a.b", 3);
        assert_eq!(store.get::<u32>("a.b"), Some(3));
        assert!(store.load_error().is_none());
    }

    #[test]
    fn invalid_file_is_reported_and_kept_aside() {
        let path = temp_file("invalid");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();

        let mut store = JsonStore::load(path.clone());
        assert!(store.load_error().is_some());
        store.set("theme", "dark");
        store.save().unwrap();

        assert!(path.with_extension("invalid.json").exists());
        assert_eq!(
            JsonStore::load(path).get::<String>("theme").as_deref(),
            Some("dark")
        );
    }
}
