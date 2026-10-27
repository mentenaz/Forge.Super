//! `settings.json` (in `branding::config_dir()`): what the user chooses.
//! Top-level shell keys (`ui_font_size`, `theme`, `show_menus`, …) and a
//! `panels` object keyed by panel id. Read at startup; written on every
//! change. Hand edits are picked up on the next start.

use std::path::PathBuf;

use gpui::{App, Global};
use serde::{Serialize, de::DeserializeOwned};

use forge_store::JsonStore;

pub struct Settings {
    store: JsonStore,
    /// What a key reads as when `settings.json` doesn't set it; filled in by
    /// the panels and features that own the key ([`Settings::set_default`]).
    defaults: JsonStore,
}

impl Global for Settings {}

impl Settings {
    pub(crate) fn load(path: PathBuf) -> Self {
        Self {
            store: JsonStore::load(path),
            defaults: JsonStore::in_memory(),
        }
    }

    /// Why `settings.json` couldn't be read at startup, if it couldn't.
    pub fn load_error(cx: &App) -> Option<String> {
        cx.global::<Settings>()
            .store
            .load_error()
            .map(str::to_string)
    }

    pub fn path(cx: &App) -> PathBuf {
        cx.global::<Settings>().store.path().to_path_buf()
    }

    /// The setting at `key` (dotted path), if set and of the right type;
    /// else its default, if one was registered.
    pub fn get<T: DeserializeOwned>(key: &str, cx: &App) -> Option<T> {
        let settings = cx.global::<Settings>();
        settings
            .store
            .get(key)
            .or_else(|| settings.defaults.get(key))
    }

    /// What `key` reads as while `settings.json` doesn't set it. A feature
    /// calls this from its `init` for the keys it owns, e.g.
    /// `Settings::set_default("panels.node.button", true, cx)`. Never
    /// written to the file.
    pub fn set_default(key: &str, value: impl Serialize, cx: &mut App) {
        cx.global_mut::<Settings>().defaults.set(key, value);
    }

    /// A panel's setting: `panels.<id>.<key>`.
    pub fn panel<T: DeserializeOwned>(id: &str, key: &str, cx: &App) -> Option<T> {
        Self::get(&format!("panels.{id}.{key}"), cx)
    }

    /// Sets `key` and writes the file. A write failure is returned for the
    /// caller to report; the in-memory value is kept either way.
    pub fn set(key: &str, value: impl Serialize, cx: &mut App) -> std::io::Result<()> {
        let settings = &mut cx.global_mut::<Settings>().store;
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn a_default_is_used_until_the_file_sets_the_key(cx: &mut TestAppContext) {
        let folder = tempfile::tempdir().unwrap();
        let settings_file = folder.path().join("settings.json");
        cx.update(|cx| {
            crate::init_at(settings_file.clone(), folder.path().join("state.json"), cx);
            Settings::set_default("panels.node.button", true, cx);
            assert_eq!(Settings::panel::<bool>("node", "button", cx), Some(true));
            assert_eq!(Settings::panel::<bool>("node", "other", cx), None);

            Settings::set_panel("node", "button", false, cx).unwrap();
            assert_eq!(Settings::panel::<bool>("node", "button", cx), Some(false));
        });
        let written = std::fs::read_to_string(&settings_file).unwrap();
        assert!(written.contains("false"), "the choice is saved: {written}");
        assert!(
            !written.contains("true"),
            "defaults aren't written: {written}"
        );
    }
}
