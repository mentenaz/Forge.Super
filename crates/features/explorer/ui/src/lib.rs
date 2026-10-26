//! The Explorer: a file tree of the workspace ([`ExplorerPanel`]) that opens
//! files read-only ([`FileView`]). Built on `forge-host`'s workspace model,
//! so the tree follows file changes and shows git status.
//!
//! A stand-in until phase 5, when Zed's project panel and editor take over.
//! There is no `explorer-core`: the file tree and git live in the shared
//! workspace backbone (`forge-workspace`).

mod file_view;
mod panel;

pub use file_view::{FileView, open_file_tab};
pub use panel::ExplorerPanel;

use forge_host::{DockPosition, Host, PanelInfo, register_panel};
use gpui::{App, AppContext};
use gpui_component::Icon;

/// Registers the Explorer panel. Call once at startup, after
/// `forge_host::init`.
pub fn init(cx: &mut App) {
    let info = PanelInfo {
        id: "explorer",
        title: "Explorer",
        icon: Icon::empty().path("icons/zed_file_tree.svg"),
        tooltip: Some("Project Panel"),
        default_dock: DockPosition::Left,
        order: 0,
    };
    register_panel(cx, info, |_, cx| cx.new(ExplorerPanel::new).into());
    Host::set_file_opener(open_file_tab, cx);
}
