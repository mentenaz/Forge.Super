//! The Git panel: the repository's changes, staging and committing, on
//! `forge-host`'s workspace model (git through the `git` CLI).
//!
//! A stand-in until phase 5, when Zed's git panel takes over. There is no
//! `git-core`: git lives in the shared workspace backbone
//! (`forge-workspace`).

mod panel;

pub use panel::GitPanel;

use forge_host::{DockPosition, PanelInfo, register_panel};
use gpui::{App, AppContext};
use gpui_component::Icon;

/// Registers the Git panel. Call once at startup, after `forge_host::init`.
pub fn init(cx: &mut App) {
    let info = PanelInfo {
        id: "git",
        title: "Git",
        icon: Icon::empty().path("icons/zed_git_branch.svg"),
        tooltip: Some("Git Panel"),
        default_dock: DockPosition::Right,
        order: 0,
    };
    register_panel(cx, info, |window, cx| {
        cx.new(|cx| GitPanel::new(window, cx)).into()
    });
}
