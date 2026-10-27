//! Starting one command through the platform's shell and reading its output
//! until it exits or is stopped.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::output::strip_ansi;

/// What a run reports, in order: lines of output, then exactly one of the
/// others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEvent {
    /// A line of output (stdout or stderr), escape codes removed.
    Line(String),
    /// The command finished with this exit code (-1 when it had none).
    Exited(i32),
    /// The command was stopped with [`Stop::stop`].
    Stopped,
    /// The shell couldn't be started.
    FailedToStart(String),
}

/// Stops a run from another thread: everything the command started goes
/// with it.
#[derive(Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Runs `command` (one line, through `pwsh` with a `cmd` fallback on
/// Windows, `sh -c` elsewhere) in `cwd`, with `env` added to the IDE's own
/// environment, and reports through `on_event` until it is done. A `PATH`
/// with the IDE's own runtimes first is added unless `env` sets one.
///
/// Blocks until the command exits or `stop` stops it: call it from a
/// thread of its own. `on_event` is called from that thread and from the
/// one reading stderr.
///
/// Two things this specifically works around:
/// - Python (and other interpreters) fully block-buffer stdout when it's not
///   a real console, so a piped `python app.py` can run for a long time
///   without producing any lines. `PYTHONUNBUFFERED=1` makes lines show up as
///   they're printed.
/// - Reading a line blocks until one arrives, so checking `stop` only
///   between lines never fires for a process that isn't printing anything
///   (a running server the user wants to stop). A watcher thread checks
///   `stop` on its own and kills the whole process tree (`taskkill /T`, since
///   the direct child is the shell, not the program it runs); that closes
///   the output pipe, which ends the read loop.
///
/// stderr is read on its own thread and interleaved with stdout: on Windows a
/// child process's native stderr reaches `pwsh`'s stderr rather than its
/// `*>&1` merge, so without it a Python traceback (or a "not recognized"
/// error) would be lost.
pub fn run(
    command: &str,
    cwd: Option<&Path>,
    env: &[(OsString, OsString)],
    stop: &Stop,
    on_event: impl Fn(RunEvent) + Send + Sync + 'static,
) {
    let on_event = Arc::new(on_event);
    let env = with_ide_runtimes(env.to_vec(), cwd);
    let mut child = match spawn_shell(command, cwd, &env) {
        Ok(child) => child,
        Err(error) => {
            on_event(RunEvent::FailedToStart(format!(
                "Failed to start shell: {error}"
            )));
            return;
        }
    };

    let pid = child.id();
    let finished = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let finished = finished.clone();
        std::thread::spawn(move || {
            while !finished.load(Ordering::SeqCst) {
                if stop.is_stopped() {
                    kill_process_tree(pid);
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    let stderr_thread = child.stderr.take().map(|stderr| {
        let on_event = on_event.clone();
        let stop = stop.clone();
        std::thread::spawn(move || forward_lines(stderr, &stop, &*on_event))
    });
    if let Some(stdout) = child.stdout.take() {
        forward_lines(stdout, stop, &*on_event);
    }

    let stopped = stop.is_stopped();
    if stopped {
        kill_process_tree(pid);
    }
    let exit_code = child
        .wait()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(-1);
    finished.store(true, Ordering::SeqCst);
    if let Some(thread) = stderr_thread {
        let _ = thread.join();
    }
    on_event(if stopped {
        RunEvent::Stopped
    } else {
        RunEvent::Exited(exit_code)
    });
}

/// Sends each line read from `pipe` until it closes or the run is stopped.
fn forward_lines(pipe: impl Read, stop: &Stop, on_event: &dyn Fn(RunEvent)) {
    for line in BufReader::new(pipe).lines() {
        if stop.is_stopped() {
            break;
        }
        match line {
            Ok(line) => on_event(RunEvent::Line(strip_ansi(&line))),
            Err(_) => break,
        }
    }
}

/// `env` with a `PATH` that has the IDE's own runtimes (the Node.js the Node
/// panel chose, …) first, so a command run from `cwd` finds them however it
/// was started. A caller that passed its own `PATH` keeps it. Reads the disk.
fn with_ide_runtimes(
    mut env: Vec<(OsString, OsString)>,
    cwd: Option<&Path>,
) -> Vec<(OsString, OsString)> {
    let has_path = env
        .iter()
        .any(|(name, _)| name.to_str().is_some_and(forge_ide_env::is_path_variable));
    if !has_path && let Some(path) = forge_ide_env::path_env(cwd) {
        env.push((OsString::from("PATH"), path));
    }
    env
}

/// Starts the shell for `command`, with its output piped.
fn spawn_shell(
    command: &str,
    cwd: Option<&Path>,
    env: &[(OsString, OsString)],
) -> std::io::Result<std::process::Child> {
    let prepare = |shell: &mut Command| {
        if let Some(cwd) = cwd {
            shell.current_dir(cwd);
        }
        shell
            .envs(env.iter().map(|(name, value)| (name, value)))
            .env("PYTHONUNBUFFERED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    };

    // `*>&1` merges PowerShell's own streams into stdout, but not a child
    // process's native stderr, which the stderr thread reads.
    // `; exit $LASTEXITCODE` passes on the command's real exit code: a bare
    // `-Command` always exits 0.
    #[cfg(windows)]
    {
        let merged = format!("& {{ {command} }} *>&1; exit $LASTEXITCODE");
        let mut pwsh = forge_process::new_command("pwsh.exe");
        pwsh.args(["-NoProfile", "-NonInteractive", "-Command", &merged]);
        prepare(&mut pwsh);
        // Without PowerShell 7, `cmd`: its `2>&1` merges stderr itself, and
        // it passes on the exit code.
        pwsh.spawn().or_else(|_| {
            let mut cmd = forge_process::new_command("cmd.exe");
            cmd.args(["/C", &format!("{command} 2>&1")]);
            prepare(&mut cmd);
            cmd.spawn()
        })
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt as _;
        let mut sh = forge_process::new_command("sh");
        sh.args(["-c", &format!("{{ {command}\n}} 2>&1")]);
        prepare(&mut sh);
        // Its own process group, so stopping kills everything the command
        // started (`kill_process_tree`), as `taskkill /T` does on Windows.
        sh.stderr(Stdio::null()).process_group(0);
        sh.spawn()
    }
}

/// Force-kills the process `pid` and everything it started.
#[cfg(windows)]
fn kill_process_tree(pid: u32) {
    let _ = forge_process::new_command("taskkill.exe")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}

/// Off Windows the shell leads its own process group (see `spawn_shell`),
/// so `pid` is also the group's id: killing the group takes everything the
/// command started with it.
#[cfg(not(windows))]
fn kill_process_tree(pid: u32) {
    let group = format!("-{pid}");
    let killed_group = forge_process::new_command("kill")
        .args(["-9", "--", &group])
        .output()
        .is_ok_and(|output| output.status.success());
    // A shell that already exited leaves no group to signal; whatever is
    // left is then at most the process itself.
    if !killed_group {
        let _ = forge_process::new_command("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    /// Runs `command` and collects what it reports.
    fn collect(command: &str, cwd: Option<&Path>, stop: &Stop) -> Vec<RunEvent> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        run(command, cwd, &[], stop, move |event| {
            sink.lock().unwrap().push(event)
        });
        Arc::try_unwrap(events).unwrap().into_inner().unwrap()
    }

    #[test]
    fn output_and_the_exit_code_come_back() {
        let events = collect("echo hello", None, &Stop::new());
        assert!(
            events.contains(&RunEvent::Line("hello".to_string())),
            "{events:?}"
        );
        assert_eq!(events.last(), Some(&RunEvent::Exited(0)));

        let events = collect("exit 3", None, &Stop::new());
        assert_eq!(events.last(), Some(&RunEvent::Exited(3)));
    }

    #[test]
    fn the_command_runs_in_its_folder() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("marker.txt"), "found").unwrap();
        let read = if cfg!(windows) {
            "Get-Content marker.txt"
        } else {
            "cat marker.txt"
        };
        let events = collect(read, Some(folder.path()), &Stop::new());
        assert!(
            events.contains(&RunEvent::Line("found".to_string())),
            "{events:?}"
        );
    }

    #[test]
    fn a_silent_long_command_can_be_stopped() {
        let stop = Stop::new();
        let stopper = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            stopper.stop();
        });
        let wait = if cfg!(windows) {
            "Start-Sleep -Seconds 30"
        } else {
            "sleep 30"
        };
        let started = Instant::now();
        let events = collect(wait, None, &stop);
        assert_eq!(events.last(), Some(&RunEvent::Stopped));
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_path_given_by_the_caller_is_kept() {
        let env = vec![(OsString::from("PATH"), OsString::from("mine"))];
        assert_eq!(with_ide_runtimes(env.clone(), None), env);
    }
}
