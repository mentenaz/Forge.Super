//! [`Host`]: how panels ask the shell to do things. The shell installs a
//! [`HostDelegate`] at startup; until then (and in tests) calls are no-ops.

use std::rc::Rc;

use gpui::{App, Global, SharedString, Window};
use gpui_component::Icon;

use crate::panel::BuildView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A tab to open (or focus, when one with the same `key` is already open).
#[derive(Clone)]
pub struct TabRequest {
    /// Identifies the content, e.g. a file path.
    pub key: SharedString,
    pub title: SharedString,
    /// Shown for non-file tabs only.
    pub icon: Icon,
    /// Workspace-relative `/`-separated path, for file tabs.
    pub path: Option<SharedString>,
    /// Breadcrumb symbol, until the editor provides it.
    pub symbol: Option<SharedString>,
    pub dirty: bool,
    pub build: BuildView,
}

/// A shell command for the command runner (Script Runner, phase 4.1).
#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub label: SharedString,
    pub command: SharedString,
    pub cwd: Option<std::path::PathBuf>,
}

/// What the shell implements for the host.
pub trait HostDelegate {
    fn open_tab(&self, tab: TabRequest, window: &mut Window, cx: &mut App);
    /// Opens the panel's dock on the panel.
    fn show_panel(&self, id: &str, window: &mut Window, cx: &mut App);
    /// Shows the panel, or closes its dock if it's already showing.
    fn toggle_panel(&self, id: &str, window: &mut Window, cx: &mut App);
    /// A toast plus an entry in the notification centre.
    fn notify(&self, kind: NoticeKind, title: SharedString, message: SharedString, window: &mut Window, cx: &mut App);
}

type CommandRunner = Rc<dyn Fn(CommandSpec, &mut Window, &mut App)>;
type FileOpener = Rc<dyn Fn(std::path::PathBuf, &mut Window, &mut App)>;

#[derive(Default)]
pub struct Host {
    delegate: Option<Rc<dyn HostDelegate>>,
    command_runner: Option<CommandRunner>,
    file_opener: Option<FileOpener>,
}

impl Global for Host {}

impl Host {
    pub fn set_delegate(delegate: Rc<dyn HostDelegate>, cx: &mut App) {
        cx.global_mut::<Host>().delegate = Some(delegate);
    }

    /// Registers the panel that runs commands (Script Runner).
    pub fn set_command_runner(runner: impl Fn(CommandSpec, &mut Window, &mut App) + 'static, cx: &mut App) {
        cx.global_mut::<Host>().command_runner = Some(Rc::new(runner));
    }

    /// Registers what opens files (the Explorer's read-only view now, Zed's
    /// editor in phase 5).
    pub fn set_file_opener(opener: impl Fn(std::path::PathBuf, &mut Window, &mut App) + 'static, cx: &mut App) {
        cx.global_mut::<Host>().file_opener = Some(Rc::new(opener));
    }

    /// Opens `path` (absolute) in a tab with the registered file opener.
    pub fn open_file(path: std::path::PathBuf, window: &mut Window, cx: &mut App) {
        let opener = cx.try_global::<Host>().and_then(|host| host.file_opener.clone());
        match opener {
            Some(open) => open(path, window, cx),
            None => Self::notify(NoticeKind::Warning, "Can't open files", "No file viewer is registered.", window, cx),
        }
    }

    fn delegate(cx: &App) -> Option<Rc<dyn HostDelegate>> {
        cx.try_global::<Host>().and_then(|host| host.delegate.clone())
    }

    pub fn open_tab(tab: TabRequest, window: &mut Window, cx: &mut App) {
        if let Some(delegate) = Self::delegate(cx) {
            delegate.open_tab(tab, window, cx);
        }
    }

    pub fn show_panel(id: &str, window: &mut Window, cx: &mut App) {
        if let Some(delegate) = Self::delegate(cx) {
            delegate.show_panel(id, window, cx);
        }
    }

    pub fn toggle_panel(id: &str, window: &mut Window, cx: &mut App) {
        if let Some(delegate) = Self::delegate(cx) {
            delegate.toggle_panel(id, window, cx);
        }
    }

    pub fn notify(
        kind: NoticeKind,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(delegate) = Self::delegate(cx) {
            delegate.notify(kind, title.into(), message.into(), window, cx);
        }
    }

    /// Runs `spec` through the command runner. Without one (before Script
    /// Runner is ported), tells the user instead.
    pub fn run_command(spec: CommandSpec, window: &mut Window, cx: &mut App) {
        let runner = cx.try_global::<Host>().and_then(|host| host.command_runner.clone());
        match runner {
            Some(run) => run(spec, window, cx),
            None => Self::notify(
                NoticeKind::Warning,
                "Can't run commands yet",
                format!("`{}` needs the Script Runner, which isn't available yet.", spec.command),
                window,
                cx,
            ),
        }
    }
}
