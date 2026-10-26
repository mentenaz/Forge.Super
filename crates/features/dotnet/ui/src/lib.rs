//! .NET views: the dock panel ([`DotnetPanel`]) and the main-area
//! view ([`DotnetWorkspace`]). Both get their data from `dotnet-core`.
//!
//! [`init`] registers the panel with the shell through `forge-host`.

mod panel;
mod workspace;

use forge_host::{DockPosition, PanelInfo, register_panel};
use gpui::{App, AppContext};
use gpui_component::Icon;

pub use panel::DotnetPanel;
pub use workspace::DotnetWorkspace;

/// Registers the .NET panel. Call once at startup, after `forge_host::init`.
pub fn init(cx: &mut App) {
    let info = PanelInfo {
        id: "dotnet",
        title: ".NET",
        icon: Icon::empty().path("icons/zed_dotnet.svg"),
        tooltip: None,
        default_dock: DockPosition::Left,
        order: 50,
    };
    register_panel(cx, info, |_, cx| cx.new(|_| DotnetPanel::new()).into());
}
