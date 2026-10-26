//! Watching the workspace for changes. Raw file events are collected for a
//! short quiet period and delivered as one [`Changes`] batch, so a build or
//! a branch switch touching thousands of files means one refresh, not
//! thousands.

use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long the workspace must be quiet before a batch is delivered.
const QUIET: Duration = Duration::from_millis(200);

/// One batch of changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Changed files and folders outside `.git`, each listed once.
    pub paths: Vec<PathBuf>,
    /// Something in `.git` changed that can affect branches, worktrees or
    /// status (refs, HEAD, the index), so git state should be re-read.
    pub git: bool,
}

/// Keeps watching while alive; drop it to stop.
pub struct Watcher {
    _watcher: RecommendedWatcher,
}

/// Whether a path inside `.git` matters: object and log writes don't change
/// what the UI shows, refs/HEAD/index do.
fn is_relevant_git_path(path: &Path) -> bool {
    let mut after_git = path.components().skip_while(|c| c != &Component::Normal(".git".as_ref())).skip(1);
    match after_git.next() {
        Some(Component::Normal(first)) => !matches!(first.to_str(), Some("objects" | "logs" | "lfs")),
        _ => true,
    }
}

fn in_git_dir(path: &Path) -> bool {
    path.components().any(|c| c == Component::Normal(".git".as_ref()))
}

/// Watches `root` recursively and calls `on_change` (on a background thread)
/// with each batch.
pub fn watch(root: &Path, on_change: impl Fn(Changes) + Send + 'static) -> notify::Result<Watcher> {
    let (tx, rx) = mpsc::channel::<notify::Event>();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;

    std::thread::Builder::new()
        .name("forge-workspace-watch".into())
        .spawn(move || {
            // Blocks for the first event, then gathers until it's quiet.
            while let Ok(first) = rx.recv() {
                let mut changes = Changes::default();
                let mut add = |event: notify::Event| {
                    for path in event.paths {
                        if in_git_dir(&path) {
                            changes.git |= is_relevant_git_path(&path);
                        } else if !changes.paths.contains(&path) {
                            changes.paths.push(path);
                        }
                    }
                };
                add(first);
                while let Ok(event) = rx.recv_timeout(QUIET) {
                    add(event);
                }
                if changes.git || !changes.paths.is_empty() {
                    on_change(changes);
                }
            }
        })
        .map_err(|err| notify::Error::generic(&err.to_string()))?;

    Ok(Watcher { _watcher: watcher })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[test]
    fn git_paths_that_matter() {
        assert!(is_relevant_git_path(Path::new("r/.git/HEAD")));
        assert!(is_relevant_git_path(Path::new("r/.git/refs/heads/main")));
        assert!(is_relevant_git_path(Path::new("r/.git/index")));
        assert!(!is_relevant_git_path(Path::new("r/.git/objects/ab/cdef")));
        assert!(!in_git_dir(Path::new("r/src/main.rs")));
    }

    #[test]
    fn delivers_a_batch_for_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let batches = Arc::new(Mutex::new(Vec::<Changes>::new()));
        let sink = batches.clone();
        let _watcher = watch(dir.path(), move |changes| sink.lock().unwrap().push(changes)).unwrap();

        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(dir.path().join("a.txt"), "1").unwrap();
        std::fs::write(dir.path().join("b.txt"), "2").unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while batches.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let batches = batches.lock().unwrap();
        let paths: Vec<_> = batches.iter().flat_map(|b| &b.paths).collect();
        assert!(paths.iter().any(|p| p.ends_with("a.txt")), "{batches:?}");
        assert!(paths.iter().any(|p| p.ends_with("b.txt")), "{batches:?}");
    }
}
