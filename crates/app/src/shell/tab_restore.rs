//! Saves the open tabs in `state.json` and reopens them at the next start.
//! Only tabs that carry a `TabRestore` are saved; the rest (placeholders,
//! views that can't be rebuilt) are left out.

use forge_host::{State, TabRestore, restore_tab};
use gpui::{Context, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ForgeShell;
use super::tabs::{TabGroup, WorkspaceTab};

const KEY: &str = "tabs";

#[derive(Serialize, Deserialize)]
struct SavedTab {
    kind: String,
    payload: Value,
    pinned: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct SavedTabs {
    primary: Vec<SavedTab>,
    split: Vec<SavedTab>,
    /// Positions in the lists above, not in the open tab strip.
    active_primary: Option<usize>,
    active_split: Option<usize>,
    split_focused: bool,
}

/// The restorable tabs of a group and the position of the active one among
/// them.
fn saved_group(tabs: &[WorkspaceTab], active: Option<usize>) -> (Vec<SavedTab>, Option<usize>) {
    let mut saved = Vec::new();
    let mut saved_active = None;
    for (idx, tab) in tabs.iter().enumerate() {
        let Some(restore) = &tab.restore else {
            continue;
        };
        if active == Some(idx) {
            saved_active = Some(saved.len());
        }
        saved.push(SavedTab {
            kind: restore.kind.to_string(),
            payload: restore.payload.clone(),
            pinned: tab.pinned,
        });
    }
    (saved, saved_active)
}

impl ForgeShell {
    /// Writes the open tabs to `state.json` (a no-op until the saved ones
    /// have been reopened, so start-up doesn't overwrite them).
    pub(super) fn save_tabs(&self, cx: &mut Context<Self>) {
        if !self.tabs_ready {
            return;
        }
        let (primary, active_primary) = saved_group(&self.tabs, self.active_tab);
        let (split, active_split) = saved_group(&self.split_tabs, self.split_active_tab);
        State::set(
            KEY,
            SavedTabs {
                primary,
                split,
                active_primary,
                active_split,
                split_focused: self.active_group == TabGroup::Split,
            },
            cx,
        );
    }

    /// Reopens the tabs saved by [`Self::save_tabs`], then starts saving.
    pub(super) fn restore_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved: SavedTabs = State::get(KEY, cx).unwrap_or_default();
        let primary_active = self.restore_group(
            TabGroup::Primary,
            &saved.primary,
            saved.active_primary,
            window,
            cx,
        );
        let split_active = self.restore_group(
            TabGroup::Split,
            &saved.split,
            saved.active_split,
            window,
            cx,
        );
        self.active_tab = primary_active;
        self.split_active_tab = split_active;
        self.active_group = if saved.split_focused {
            TabGroup::Split
        } else {
            TabGroup::Primary
        };
        self.normalize_groups();
        self.tabs_ready = true;
        cx.notify();
    }

    /// Reopens one group's tabs; returns the index of the one to make
    /// active, if it came back.
    fn restore_group(
        &mut self,
        group: TabGroup,
        saved: &[SavedTab],
        saved_active: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.active_group = group;
        let mut active = None;
        for (position, tab) in saved.iter().enumerate() {
            let restore = TabRestore {
                kind: tab.kind.clone().into(),
                payload: tab.payload.clone(),
            };
            let Some(spec) = restore_tab(&restore, window, cx) else {
                continue;
            };
            let before = self.group(group).len();
            self.open_tab(spec, window, cx);
            // Opening a tab that is already open adds nothing.
            if let Some(opened) = self.group_mut(group).0.get_mut(before) {
                opened.pinned = tab.pinned;
                if saved_active == Some(position) {
                    active = Some(before);
                }
            }
        }
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_host::Host;
    use gpui::{AppContext as _, TestAppContext, WindowOptions};

    fn open_shell(cx: &mut TestAppContext) -> gpui::WindowHandle<ForgeShell> {
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                ForgeShell::view(window, cx)
            })
            .unwrap()
        })
    }

    #[gpui::test]
    fn open_file_tabs_come_back_in_a_new_window(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let folder = tempfile::tempdir().unwrap();
        let (kept, gone) = (
            folder.path().join("kept.txt"),
            folder.path().join("gone.txt"),
        );
        std::fs::write(&kept, "hello").unwrap();
        std::fs::write(&gone, "bye").unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            forge_host::init_at(
                folder.path().join("settings.json"),
                folder.path().join("state.json"),
                cx,
            );
            explorer_ui::init(cx);
            super::super::init(cx);
        });

        let first = open_shell(cx);
        for file in [&kept, &gone] {
            let file = file.clone();
            cx.update_window(first.into(), |_, window, cx| {
                Host::open_file(file, window, cx)
            })
            .unwrap();
        }
        cx.run_until_parked();
        let open = first.read_with(cx, |shell, _| shell.tabs.len()).unwrap();
        assert_eq!(open, 2);

        // The second file is deleted before the next start.
        std::fs::remove_file(&gone).unwrap();
        let second = open_shell(cx);
        cx.run_until_parked();
        let titles = second
            .read_with(cx, |shell, _| {
                shell
                    .tabs
                    .iter()
                    .map(|t| t.title.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(
            titles,
            ["kept.txt"],
            "only the file that still exists returns"
        );
    }
}
