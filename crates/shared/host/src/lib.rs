//! The host API: what Forge Super's panels need from the shell, replacing
//! the thin Zed layer the Zed edition's panels used (`impl Panel`,
//! `workspace`, `project`, `settings`, `db`). Design: `forge-super-design.md`
//! §13.
//!
//! - [`panel`]: describe a panel and register it ([`register_panel`]).
//! - [`host`]: ask the shell to open tabs, show panels, notify, run commands
//!   ([`Host`]).
//! - [`workspace`]: the workspace model: root, folder listings and git,
//!   kept current by a file watcher.
//! - [`settings`] / [`state`]: `settings.json` and `state.json`.
//! - [`tab_kind`]: tabs that are reopened at the next start.
//!
//! The shell calls [`init`] once at startup, before any panel registers.

pub mod host;
pub mod panel;
pub mod settings;
pub mod state;
pub mod tab_kind;
pub mod workspace;

pub use host::{
    CommandError, CommandOutcome, CommandRunner, CommandSpec, Host, HostDelegate, NoticeKind,
    TabRequest,
};
pub use panel::{BuildView, DockPosition, PanelInfo, PanelRegistry, register_panel};
pub use settings::Settings;
pub use state::State;
pub use tab_kind::{RestoreTab, TabRestore, register_tab_kind, restore_tab};
pub use workspace::{
    DirEntry, FileState, FileStatus, GitError, GitState, Workspace, WorkspaceEvent, Worktree,
};

use gpui::App;

/// Installs the host's globals and loads `settings.json` and `state.json`.
pub fn init(cx: &mut App) {
    init_at(branding::settings_file(), branding::state_file(), cx);
}

/// [`init`] with the settings and state in other files, for tests.
pub fn init_at(settings_file: std::path::PathBuf, state_file: std::path::PathBuf, cx: &mut App) {
    cx.set_global(Settings::load(settings_file));
    cx.set_global(State::load(state_file));
    cx.set_global(PanelRegistry::default());
    cx.set_global(Host::default());
    workspace::init(cx);
}
