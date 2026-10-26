//! The Script Runner panel: runs one shell command at a time and streams
//! its output, with links and the SPFx debug query string picked out.
//!
//! It is also the host's command runner: other panels run commands with
//! `Host::run_command`, which shows them here. The running itself is in
//! `forge-script-runner`.

mod panel;
mod render;

pub use panel::ScriptRunnerPanel;

use forge_host::{
    CommandError, CommandOutcome, CommandRunner, CommandSpec, DockPosition, Host, PanelInfo,
    register_panel,
};
use gpui::{App, AppContext, Entity, Global, Task, WeakEntity, Window};
use gpui_component::Icon;

pub const PANEL_ID: &str = "script-runner";

/// The panel's entity, for the command runner.
struct Panel(WeakEntity<ScriptRunnerPanel>);

impl Global for Panel {}

fn panel(cx: &App) -> Option<Entity<ScriptRunnerPanel>> {
    cx.try_global::<Panel>().and_then(|panel| panel.0.upgrade())
}

/// Registers the Script Runner panel and makes it the host's command runner.
/// Call once at startup, after `forge_host::init`.
pub fn init(cx: &mut App) {
    let info = PanelInfo {
        id: PANEL_ID,
        title: "Script Runner",
        icon: Icon::empty().path("icons/zed_runner.svg"),
        tooltip: Some("Script Runner"),
        default_dock: DockPosition::Bottom,
        order: 30,
    };
    register_panel(cx, info, |window, cx| {
        let panel = cx.new(|cx| ScriptRunnerPanel::new(window, cx));
        cx.set_global(Panel(panel.downgrade()));
        panel.into()
    });
    Host::set_command_runner(Runner, cx);
}

struct Runner;

impl CommandRunner for Runner {
    fn run(
        &self,
        spec: CommandSpec,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Task<CommandOutcome>, CommandError> {
        let panel = panel(cx).ok_or(CommandError::NoRunner)?;
        let task = panel.update(cx, |panel, cx| panel.run_spec(spec, window, cx))?;
        // Deferred: the caller may be in the middle of updating the shell.
        window.defer(cx, |window, cx| Host::show_panel(PANEL_ID, window, cx));
        Ok(task)
    }

    fn is_running(&self, cx: &App) -> bool {
        panel(cx).is_some_and(|panel| panel.read(cx).is_running())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{TestAppContext, WindowHandle};

    /// The panel in a window of its own, registered as the command runner,
    /// with settings and state in a temporary folder.
    fn panel_window(
        cx: &mut TestAppContext,
    ) -> (WindowHandle<ScriptRunnerPanel>, tempfile::TempDir) {
        let folder = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            forge_host::init_at(
                folder.path().join("settings.json"),
                folder.path().join("state.json"),
                cx,
            );
            init(cx);
        });
        let window = cx.add_window(ScriptRunnerPanel::new);
        let panel = window.root(cx).unwrap();
        cx.update(|cx| cx.set_global(Panel(panel.downgrade())));
        (window, folder)
    }

    /// `Host::run_command` from inside the window, as another panel would
    /// call it: without holding the panel (the window's root view).
    fn run(
        window: &WindowHandle<ScriptRunnerPanel>,
        spec: CommandSpec,
        cx: &mut TestAppContext,
    ) -> Result<Task<CommandOutcome>, CommandError> {
        cx.update_window((*window).into(), |_, window, cx| {
            Host::run_command(spec, window, cx)
        })
        .unwrap()
    }

    /// Lets the command's own thread work until the panel is idle again.
    fn wait_until_idle(window: &WindowHandle<ScriptRunnerPanel>, cx: &mut TestAppContext) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            cx.run_until_parked();
            let running = window.read_with(cx, |panel, _| panel.is_running()).unwrap();
            if !running {
                return;
            }
            assert!(Instant::now() < deadline, "the command didn't finish");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[gpui::test]
    fn a_command_from_another_panel_runs_here_and_reports_how_it_ended(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let (window, _folder) = panel_window(cx);
        let outcome = Rc::new(RefCell::new(None));

        let spec = CommandSpec {
            label: "Greet".into(),
            command: "echo hello".into(),
            ..Default::default()
        };
        let task = run(&window, spec, cx).expect("the runner takes the command");
        assert!(cx.update(|cx| Host::is_command_running(cx)));

        // One run at a time.
        let second = CommandSpec {
            command: "echo again".into(),
            ..Default::default()
        };
        assert_eq!(run(&window, second, cx).err(), Some(CommandError::Busy));

        let slot = outcome.clone();
        cx.spawn(async move |_| *slot.borrow_mut() = Some(task.await))
            .detach();
        wait_until_idle(&window, cx);
        cx.run_until_parked();

        assert_eq!(*outcome.borrow(), Some(CommandOutcome::Exited(0)));
        let output = window
            .read_with(cx, |panel, _| panel.output.clone())
            .unwrap();
        assert_eq!(output.first().map(String::as_str), Some("$ echo hello"));
        assert!(output.iter().any(|line| line == "hello"), "{output:?}");
        assert!(!cx.update(|cx| Host::is_command_running(cx)));
    }

    #[gpui::test]
    fn links_in_the_output_are_collected(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let (window, _folder) = panel_window(cx);
        let spec = CommandSpec {
            command: "echo Local: http://localhost:5173/, docs https://example.test/x".into(),
            ..Default::default()
        };
        run(&window, spec, cx).unwrap().detach();
        wait_until_idle(&window, cx);
        let links = window
            .read_with(cx, |panel, _| panel.detected_links.clone())
            .unwrap();
        assert_eq!(links, ["http://localhost:5173/", "https://example.test/x"]);
    }
}
