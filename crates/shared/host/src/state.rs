//! `state.json` (in `branding::data_dir()`): what the app remembers, as
//! opposed to what the user chooses: dock sizes and which docks are open,
//! recent projects, a panel's last selection. Replaces Zed's `db::kvp`.
//!
//! Writes are debounced (half a second after the last change), since things
//! like dragging a dock divider change it many times a second.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{App, Global};
use serde::{Serialize, de::DeserializeOwned};

use forge_store::JsonStore;

const SAVE_DELAY: Duration = Duration::from_millis(500);

pub struct State {
    store: JsonStore,
    /// Bumped on each change; a pending save only writes if it's still the
    /// latest.
    generation: u64,
}

impl Global for State {}

impl State {
    pub(crate) fn load(path: PathBuf) -> Self {
        Self {
            store: JsonStore::load(path),
            generation: 0,
        }
    }

    /// Why `state.json` couldn't be read at startup, if it couldn't.
    pub fn load_error(cx: &App) -> Option<String> {
        cx.global::<State>().store.load_error().map(str::to_string)
    }

    pub fn get<T: DeserializeOwned>(key: &str, cx: &App) -> Option<T> {
        cx.global::<State>().store.get(key)
    }

    /// Sets `key`; the file is written shortly after the last change.
    pub fn set(key: &str, value: impl Serialize, cx: &mut App) {
        let state = cx.global_mut::<State>();
        if state.store.get::<serde_json::Value>(key) == serde_json::to_value(&value).ok() {
            return;
        }
        state.store.set(key, value);
        state.generation += 1;
        let generation = state.generation;

        cx.spawn(async move |cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            cx.update(|cx| {
                let state = cx.global_mut::<State>();
                if state.generation == generation {
                    let _ = state.store.save();
                }
            });
        })
        .detach();
    }

    /// Writes any pending change now (on quit).
    pub fn flush(cx: &mut App) {
        let _ = cx.global_mut::<State>().store.save();
    }
}
