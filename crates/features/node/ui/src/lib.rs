//! Node.js views: the dock panel ([`NodePanel`]) and the main-area
//! view ([`NodeWorkspace`]). Both get their data from `node-core`.
//!
//! [`init`] registers the panel with the shell through `forge-host`.

mod panel;
mod workspace;

use forge_host::{DockPosition, PanelInfo, register_panel};
use gpui::{App, AppContext};
use gpui_component::Icon;

pub use panel::NodePanel;
pub use workspace::NodeWorkspace;

/// Registers the Node panel. Call once at startup, after `forge_host::init`.
pub fn init(cx: &mut App) {
    let info = PanelInfo {
        id: "node",
        title: "Node",
        icon: Icon::empty().path("icons/zed_node.svg"),
        tooltip: None,
        default_dock: DockPosition::Left,
        order: 40,
    };
    register_panel(cx, info, |_, cx| cx.new(|_| NodePanel::new()).into());
}
