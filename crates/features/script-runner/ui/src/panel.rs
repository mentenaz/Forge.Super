//! The Script Runner's state: the command input, the run in progress and
//! its output.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use forge_host::{CommandError, CommandOutcome, CommandSpec, Workspace};
use forge_script_runner::output::{debug_querystring_in, find_urls};
use forge_script_runner::{RunEvent, Stop};
use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use gpui::{
    App, AppContext as _, AsyncApp, ClipboardItem, Context, Entity, FocusHandle, Focusable,
    Subscription, Task, WeakEntity, Window,
};
use gpui_component::input::{InputEvent, InputState};

const MAX_OUTPUT_LINES: usize = 500;

/// How long a Copy button shows "Copied".
const COPIED_FOR: Duration = Duration::from_millis(1600);

pub struct ScriptRunnerPanel {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) command_input: Entity<InputState>,
    /// Where commands run: the workspace root, or the folder the last run
    /// from another panel asked for.
    pub(crate) cwd: Option<PathBuf>,
    /// The workspace root as last seen, to tell when it changes.
    workspace_root: Option<PathBuf>,
    pub(crate) output: Vec<String>,
    pub(crate) running: bool,
    stop: Stop,
    /// Told how the run in progress ends.
    waiters: Vec<oneshot::Sender<CommandOutcome>>,
    /// Latest SPFx debug query-string seen in this run, if any.
    pub(crate) debug_script: Option<String>,
    /// True for a moment after the debug script is copied.
    pub(crate) debug_copied: bool,
    /// Every distinct `http(s)://` URL seen in this run's output so far, in
    /// first-seen order, for the links bar.
    pub(crate) detected_links: Vec<String>,
    /// The link showing "copied" right now.
    pub(crate) link_copied: Option<String>,
    _run: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ScriptRunnerPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter a command…"));
        let workspace = Workspace::model(cx);
        let subscriptions = vec![
            cx.subscribe_in(
                &command_input,
                window,
                |this, input, event: &InputEvent, _, cx| match event {
                    InputEvent::PressEnter { .. } => {
                        let command = input.read(cx).value().to_string();
                        this.run(command, Vec::new(), cx);
                    }
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            ),
            // A newly opened workspace is where typed commands run from then on.
            cx.observe(&workspace, |this, workspace, cx| {
                let root = workspace.read(cx).root().map(PathBuf::from);
                if root != this.workspace_root {
                    if !this.running && root.is_some() {
                        this.cwd = root.clone();
                    }
                    this.workspace_root = root;
                    cx.notify();
                }
            }),
        ];
        let root = workspace.read(cx).root().map(PathBuf::from);
        Self {
            focus_handle: cx.focus_handle(),
            command_input,
            cwd: root.clone(),
            workspace_root: root,
            output: Vec::new(),
            running: false,
            stop: Stop::new(),
            waiters: Vec::new(),
            debug_script: None,
            debug_copied: false,
            detected_links: Vec::new(),
            link_copied: None,
            _run: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether a command is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Runs a command from another panel (`Host::run_command`): shows it in
    /// the input and runs it in `spec.cwd` (the workspace root when `None`),
    /// with `spec.env` for this run only.
    pub(crate) fn run_spec(
        &mut self,
        spec: CommandSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Task<CommandOutcome>, CommandError> {
        if self.running {
            return Err(CommandError::Busy);
        }
        let command = spec.command.trim().to_string();
        self.command_input
            .update(cx, |input, cx| input.set_value(command.clone(), window, cx));
        if let Some(cwd) = spec.cwd {
            self.cwd = Some(cwd);
        } else if self.workspace_root.is_some() {
            self.cwd = self.workspace_root.clone();
        }
        if !self.run(command, spec.env, cx) {
            return Ok(Task::ready(CommandOutcome::FailedToStart(
                "The command is empty.".to_string(),
            )));
        }
        let (sender, receiver) = oneshot::channel();
        self.waiters.push(sender);
        // A panel that goes away mid-run counts as stopping it.
        Ok(cx.background_spawn(async move { receiver.await.unwrap_or(CommandOutcome::Stopped) }))
    }

    /// Starts `command` in [`Self::cwd`]. False when it is empty or another
    /// command is running.
    pub(crate) fn run(
        &mut self,
        command: String,
        env: Vec<(OsString, OsString)>,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = command.trim().to_string();
        if self.running || command.is_empty() {
            return false;
        }

        self.output.clear();
        self.output.push(format!("$ {command}"));
        self.debug_script = None;
        self.debug_copied = false;
        self.detected_links.clear();
        self.link_copied = None;
        self.running = true;
        self.stop = Stop::new();
        cx.notify();

        let (sender, mut events) = mpsc::unbounded();
        let cwd = self.cwd.clone();
        let stop = self.stop.clone();
        std::thread::spawn(move || {
            forge_script_runner::run(&command, cwd.as_deref(), &env, &stop, move |event| {
                let _ = sender.unbounded_send(event);
            });
        });
        self._run = Some(
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                while let Some(event) = events.next().await {
                    // Take whatever else has arrived too, so a chatty command
                    // updates the panel once per batch, not once per line.
                    let mut batch = vec![event];
                    while let Ok(event) = events.try_recv() {
                        batch.push(event);
                    }
                    if this.update(cx, |this, cx| this.apply(batch, cx)).is_err() {
                        break;
                    }
                }
            }),
        );
        true
    }

    pub(crate) fn stop(&mut self) {
        self.stop.stop();
    }

    fn apply(&mut self, events: Vec<RunEvent>, cx: &mut Context<Self>) {
        for event in events {
            let outcome = match event {
                RunEvent::Line(line) => {
                    self.push_line(line);
                    continue;
                }
                RunEvent::Exited(code) => {
                    let verdict = if code == 0 { "passed" } else { "failed" };
                    self.output.push(format!("[exit {code} · {verdict}]"));
                    CommandOutcome::Exited(code)
                }
                RunEvent::Stopped => {
                    self.output.push("[stopped]".to_string());
                    CommandOutcome::Stopped
                }
                RunEvent::FailedToStart(message) => {
                    self.output.push(message.clone());
                    CommandOutcome::FailedToStart(message)
                }
            };
            self.running = false;
            for waiter in self.waiters.drain(..) {
                let _ = waiter.send(outcome.clone());
            }
        }
        cx.notify();
    }

    fn push_line(&mut self, line: String) {
        if self.output.len() >= MAX_OUTPUT_LINES {
            self.output
                .drain(0..(self.output.len() - MAX_OUTPUT_LINES + 1));
        }
        if let Some(query) = debug_querystring_in(&line) {
            self.debug_script = Some(query);
        }
        for url in find_urls(&line) {
            if !self.detected_links.contains(&url) {
                self.detected_links.push(url);
            }
        }
        self.output.push(line);
    }

    pub(crate) fn copy_debug_script(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.debug_copied = true;
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |this, cx| {
                this.debug_copied = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn copy_link(&mut self, url: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
        self.link_copied = Some(url.clone());
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |this, cx| {
                if this.link_copied.as_deref() == Some(url.as_str()) {
                    this.link_copied = None;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Focusable for ScriptRunnerPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
