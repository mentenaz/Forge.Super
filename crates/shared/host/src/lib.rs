//! The host API: what Forge Super's panels need from the shell, replacing
//! the thin Zed layer the Zed edition's panels used (`impl Panel`,
//! `workspace`, `project`, `settings`, `db`). Design: `forge-super-design.md`
//! §13.
//!
//! - [`panel`]: describe a panel and register it ([`register_panel`]).
//! - [`host`]: ask the shell to open tabs, show panels, notify, run commands
//!   ([`Host`]).
//! - [`workspace`]: the workspace model (root now; files and git in phase
//!   1.5).
//! - [`settings`] / [`state`]: `settings.json` and `state.json`.
//!
//! The shell calls [`init`] once at startup, before any panel registers.

pub mod host;
pub mod panel;
pub mod settings;
pub mod state;
mod store;
pub mod workspace;

pub use host::{CommandSpec, Host, HostDelegate, NoticeKind, TabRequest};
pub use panel::{BuildView, DockPosition, PanelInfo, PanelRegistry, register_panel};
pub use settings::Settings;
pub use state::State;
pub use workspace::Workspace;

use gpui::App;

/// Installs the host's globals and loads `settings.json` and `state.json`.
pub fn init(cx: &mut App) {
    cx.set_global(Settings::load(branding::settings_file()));
    cx.set_global(State::load(branding::state_file()));
    cx.set_global(PanelRegistry::default());
    cx.set_global(Host::default());
    workspace::init(cx);
}
