//! `settings.json` (in `branding::config_dir()`): what the user chooses.
//! Top-level shell keys (`ui_font_size`, `theme`, `show_menus`, …) and a
//! `panels` object keyed by panel id. Read at startup; written on every
//! change. Hand edits are picked up on the next start.

use std::path::PathBuf;

use gpui::{App, Global};
use serde::{Serialize, de::DeserializeOwned};

use crate::store::JsonStore;

pub struct Settings(JsonStore);

impl Global for Settings {}

impl Settings {
    pub(crate) fn load(path: PathBuf) -> Self {
        Self(JsonStore::load(path))
    }

    /// Why `settings.json` couldn't be read at startup, if it couldn't.
    pub fn load_error(cx: &App) -> Option<String> {
        cx.global::<Settings>().0.load_error().map(str::to_string)
    }

    pub fn path(cx: &App) -> PathBuf {
        cx.global::<Settings>().0.path().to_path_buf()
    }

    /// The setting at `key` (dotted path), if set and of the right type.
    pub fn get<T: DeserializeOwned>(key: &str, cx: &App) -> Option<T> {
        cx.global::<Settings>().0.get(key)
    }

    /// A panel's setting: `panels.<id>.<key>`.
    pub fn panel<T: DeserializeOwned>(id: &str, key: &str, cx: &App) -> Option<T> {
        Self::get(&format!("panels.{id}.{key}"), cx)
    }

    /// Sets `key` and writes the file. A write failure is returned for the
    /// caller to report; the in-memory value is kept either way.
    pub fn set(key: &str, value: impl Serialize, cx: &mut App) -> std::io::Result<()> {
        let settings = &mut cx.global_mut::<Settings>().0;
        if settings.get::<serde_json::Value>(key) == serde_json::to_value(&value).ok() {
            return Ok(());
        }
        settings.set(key, value);
        settings.save()
    }

    pub fn set_panel(
        id: &str,
        key: &str,
        value: impl Serialize,
        cx: &mut App,
    ) -> std::io::Result<()> {
        Self::set(&format!("panels.{id}.{key}"), value, cx)
    }
}
