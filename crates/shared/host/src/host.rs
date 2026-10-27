//! [`Host`]: how panels ask the shell to do things. The shell installs a
//! [`HostDelegate`] at startup; until then (and in tests) calls are no-ops.

use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, Global, SharedString, Task, Window};
use gpui_component::Icon;

use crate::panel::BuildView;
use crate::tab_kind::TabRestore;

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
    /// How to reopen the tab after a restart; `None` for a tab that isn't
    /// reopened.
    pub restore: Option<TabRestore>,
    pub build: BuildView,
}

/// A shell command for the command runner (the Script Runner): one line,
/// run through the platform's shell.
#[derive(Clone, Debug, Default)]
pub struct CommandSpec {
    /// What the run is, for messages ("Install react").
    pub label: SharedString,
    /// The command line. Check anything spliced into it with
    /// `forge_script_runner::command`.
    pub command: SharedString,
    /// Where it runs; the workspace root when `None`.
    pub cwd: Option<PathBuf>,
    /// Variables added to the IDE's environment for this run only. The IDE's
    /// own runtimes go first on `PATH` unless this sets `PATH` itself.
    pub env: Vec<(OsString, OsString)>,
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Exited(i32),
    /// Stopped by the user.
    Stopped,
    /// The shell couldn't be started.
    FailedToStart(String),
}

impl CommandOutcome {
    pub fn succeeded(&self) -> bool {
        *self == CommandOutcome::Exited(0)
    }
}

/// Why a command didn't start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// No command runner is registered.
    NoRunner,
    /// The runner is running another command; it runs one at a time.
    Busy,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CommandError::NoRunner => "No command runner is available.",
            CommandError::Busy => "The Script Runner is busy. Stop the current run first.",
        })
    }
}

/// What runs commands for the host: the Script Runner registers one with
/// [`Host::set_command_runner`].
pub trait CommandRunner {
    /// Starts `spec` and shows its output. The task finishes when the run
    /// does.
    fn run(
        &self,
        spec: CommandSpec,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Task<CommandOutcome>, CommandError>;
    fn is_running(&self, cx: &App) -> bool;
}

/// What the shell implements for the host.
pub trait HostDelegate {
    fn open_tab(&self, tab: TabRequest, window: &mut Window, cx: &mut App);
    /// Opens the panel's dock on the panel.
    fn show_panel(&self, id: &str, window: &mut Window, cx: &mut App);
    /// Shows the panel, or closes its dock if it's already showing.
    fn toggle_panel(&self, id: &str, window: &mut Window, cx: &mut App);
    /// A toast plus an entry in the notification centre.
    fn notify(
        &self,
        kind: NoticeKind,
        title: SharedString,
        message: SharedString,
        window: &mut Window,
        cx: &mut App,
    );
}

type FileOpener = Rc<dyn Fn(std::path::PathBuf, &mut Window, &mut App)>;

#[derive(Default)]
pub struct Host {
    delegate: Option<Rc<dyn HostDelegate>>,
    command_runner: Option<Rc<dyn CommandRunner>>,
    file_opener: Option<FileOpener>,
}

impl Global for Host {}

impl Host {
    pub fn set_delegate(delegate: Rc<dyn HostDelegate>, cx: &mut App) {
        cx.global_mut::<Host>().delegate = Some(delegate);
    }

    /// Registers what runs commands (the Script Runner).
    pub fn set_command_runner(runner: impl CommandRunner + 'static, cx: &mut App) {
        cx.global_mut::<Host>().command_runner = Some(Rc::new(runner));
    }

    /// Registers what opens files (the Explorer's read-only view now, Zed's
    /// editor in phase 5).
    pub fn set_file_opener(
        opener: impl Fn(std::path::PathBuf, &mut Window, &mut App) + 'static,
        cx: &mut App,
    ) {
        cx.global_mut::<Host>().file_opener = Some(Rc::new(opener));
    }

    /// Opens `path` (absolute) in a tab with the registered file opener.
    pub fn open_file(path: std::path::PathBuf, window: &mut Window, cx: &mut App) {
        let opener = cx
            .try_global::<Host>()
            .and_then(|host| host.file_opener.clone());
        match opener {
            Some(open) => open(path, window, cx),
            None => Self::notify(
                NoticeKind::Warning,
                "Can't open files",
                "No file viewer is registered.",
                window,
                cx,
            ),
        }
    }

    fn delegate(cx: &App) -> Option<Rc<dyn HostDelegate>> {
        cx.try_global::<Host>()
            .and_then(|host| host.delegate.clone())
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

    fn command_runner(cx: &App) -> Option<Rc<dyn CommandRunner>> {
        cx.try_global::<Host>()
            .and_then(|host| host.command_runner.clone())
    }

    /// Runs `spec` in the Script Runner, which shows its output. One command
    /// runs at a time: `Err(Busy)` while another is running. The task
    /// finishes when the run does; dropping it doesn't stop the run.
    pub fn run_command(
        spec: CommandSpec,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Task<CommandOutcome>, CommandError> {
        match Self::command_runner(cx) {
            Some(runner) => runner.run(spec, window, cx),
            None => Err(CommandError::NoRunner),
        }
    }

    /// Whether the Script Runner is running a command, so a panel can hold
    /// back the buttons that would start another.
    pub fn is_command_running(cx: &App) -> bool {
        Self::command_runner(cx).is_some_and(|runner| runner.is_running(cx))
    }
}
