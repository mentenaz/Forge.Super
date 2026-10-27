//! The workspace model, one per window, observable by panels:
//! `cx.observe(&Workspace::model(cx), …)` re-renders a panel when anything
//! here changes. Backed by `forge-workspace` (our thin backbone, phase 1.5);
//! in phase 5 Zed's `project` takes over behind the same API.
//!
//! - The root folder, set by the shell.
//! - Folder listings for the Explorer: [`Workspace::request_dir`] loads one
//!   in the background, [`Workspace::entries`] reads it once loaded. Loaded
//!   folders are refreshed automatically when files change.
//! - Git: [`Workspace::git`] (branch, branches, worktrees, status), kept up
//!   to date by the file watcher, and operations that return a [`Task`] with
//!   git's error message on failure.
//! - File events: [`WorkspaceEvent::PathsChanged`] emitted when files/folders change,
//!   allowing panels to rescan when `package.json`, `Cargo.toml`, etc. change.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use forge_workspace::git::{FileState, FileStatus, GitError, Worktree};
pub use forge_workspace::tree::DirEntry;
use forge_workspace::{git::Repository, tree, watch};
use futures::{StreamExt, channel::mpsc};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, Task};

/// Git state of the workspace's repository.
#[derive(Clone, Debug, Default)]
pub struct GitState {
    /// The repository (or worktree) root.
    pub root: PathBuf,
    /// The checked-out branch; `None` on a detached HEAD.
    pub branch: Option<String>,
    /// Short id of HEAD.
    pub head: String,
    pub branches: Vec<String>,
    pub worktrees: Vec<Worktree>,
    pub status: Vec<FileStatus>,
}

impl GitState {
    pub fn conflicts(&self) -> usize {
        self.status
            .iter()
            .filter(|f| f.state == FileState::Conflicted)
            .count()
    }
}

/// Events emitted by the workspace model for panels to subscribe to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// Files or folders in the workspace changed.
    PathsChanged(Arc<Vec<PathBuf>>),
}

impl WorkspaceEvent {
    /// Whether a changed path has one of these file names. A name starting
    /// with `*` matches by ending, e.g. `"*.csproj"`; any other name must
    /// match exactly (`"package.json"`). Names are compared without regard to
    /// case, as on Windows.
    pub fn touches(&self, names: &[&str]) -> bool {
        let WorkspaceEvent::PathsChanged(paths) = self;
        paths.iter().any(|path| {
            let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            let file = file.to_lowercase();
            names.iter().any(|name| {
                let name = name.to_lowercase();
                match name.strip_prefix('*') {
                    Some(ending) => file.ends_with(ending),
                    None => file == name,
                }
            })
        })
    }
}

pub struct Workspace {
    root: Option<PathBuf>,
    repo: Option<Repository>,
    git: Option<GitState>,
    dirs: HashMap<PathBuf, Vec<DirEntry>>,
    loading: HashSet<PathBuf>,
    _watcher: Option<watch::Watcher>,
    _watch_task: Option<Task<()>>,
    /// Bumped when the root changes, so results for an old root are dropped.
    generation: u64,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

struct GlobalWorkspace(Entity<Workspace>);

impl Global for GlobalWorkspace {}

pub(crate) fn init(cx: &mut App) {
    let model = cx.new(|_| Workspace::empty());
    cx.set_global(GlobalWorkspace(model));
}

/// Reads the repository containing `root` (blocking; run in the background).
fn load_git(root: &Path) -> (Option<Repository>, Option<GitState>) {
    let Some(repo) = Repository::discover(root) else {
        return (None, None);
    };
    let state = GitState {
        root: repo.root().to_path_buf(),
        branch: repo.current_branch().ok().flatten(),
        head: repo.head().unwrap_or_default(),
        branches: repo.branches().unwrap_or_default(),
        worktrees: repo.worktrees().unwrap_or_default(),
        status: repo.status().unwrap_or_default(),
    };
    (Some(repo), Some(state))
}

impl Workspace {
    fn empty() -> Self {
        Self {
            root: None,
            repo: None,
            git: None,
            dirs: HashMap::new(),
            loading: HashSet::new(),
            _watcher: None,
            _watch_task: None,
            generation: 0,
        }
    }

    /// The workspace model, to read or observe.
    pub fn model(cx: &App) -> Entity<Workspace> {
        cx.global::<GlobalWorkspace>().0.clone()
    }

    /// The workspace's root folder, if one is open.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Git state, once read; `None` outside a repository.
    pub fn git(&self) -> Option<&GitState> {
        self.git.as_ref()
    }

    /// A loaded folder's entries; `None` until [`Self::request_dir`] has
    /// loaded it.
    pub fn entries(&self, dir: &Path) -> Option<&[DirEntry]> {
        self.dirs.get(dir).map(Vec::as_slice)
    }

    /// Set by the shell when a folder is opened. Starts watching it and
    /// reads its git state and top-level entries.
    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.root == root {
            return;
        }
        self.generation += 1;
        self.root = root.clone();
        self.repo = None;
        self.git = None;
        self.dirs.clear();
        self.loading.clear();
        self._watcher = None;
        self._watch_task = None;

        if let Some(root) = root {
            let (tx, mut rx) = mpsc::unbounded();
            self._watcher = watch::watch(&root, move |changes| {
                let _ = tx.unbounded_send(changes);
            })
            .ok();
            self._watch_task = Some(cx.spawn(async move |this, cx| {
                while let Some(changes) = rx.next().await {
                    if this
                        .update(cx, |workspace, cx| workspace.on_changes(changes, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
            self.refresh_git(cx);
            self.request_dir(root, cx);
        }
        cx.notify();
    }

    /// Loads (or reloads) `dir`'s entries in the background.
    pub fn request_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if !self.loading.insert(dir.clone()) {
            return;
        }
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let path = dir.clone();
            let result = cx
                .background_executor()
                .spawn(async move { tree::list_dir(&path) })
                .await;
            let _ = this.update(cx, |workspace, cx| {
                if workspace.generation != generation {
                    return;
                }
                workspace.loading.remove(&dir);
                match result {
                    Ok(entries) => workspace.dirs.insert(dir, entries),
                    Err(_) => workspace.dirs.remove(&dir),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Re-reads git state in the background.
    pub fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let (repo, git) = cx
                .background_executor()
                .spawn(async move { load_git(&root) })
                .await;
            let _ = this.update(cx, |workspace, cx| {
                if workspace.generation == generation {
                    workspace.repo = repo;
                    workspace.git = git;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn on_changes(&mut self, changes: watch::Changes, cx: &mut Context<Self>) {
        let mut reload = HashSet::new();
        for path in &changes.paths {
            if self.dirs.contains_key(path) {
                reload.insert(path.clone());
            }
            if let Some(parent) = path.parent().filter(|p| self.dirs.contains_key(*p)) {
                reload.insert(parent.to_path_buf());
            }
        }
        for dir in reload {
            self.request_dir(dir, cx);
        }
        // File edits change git status; .git changes change branches/refs.
        self.refresh_git(cx);

        // Emit workspace file events so interested panels (Node, Rust, etc.) can rescan.
        if !changes.paths.is_empty() {
            cx.emit(WorkspaceEvent::PathsChanged(Arc::new(changes.paths)));
        }
    }

    /// Runs a git operation in the background, then re-reads git state.
    fn git_op(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Repository) -> Result<(), GitError> + Send + 'static,
    ) -> Task<Result<(), GitError>> {
        let Some(repo) = self.repo.clone() else {
            return Task::ready(Err(GitError("This folder isn't a git repository.".into())));
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { op(&repo) })
                .await;
            let _ = this.update(cx, |workspace, cx| workspace.refresh_git(cx));
            result
        })
    }

    pub fn switch_branch(
        &mut self,
        branch: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), GitError>> {
        self.git_op(cx, move |repo| repo.switch(&branch))
    }

    /// Adds a worktree at `path` on a new branch `branch` based on `base`.
    pub fn add_worktree(
        &mut self,
        path: PathBuf,
        branch: String,
        base: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), GitError>> {
        self.git_op(cx, move |repo| repo.add_worktree(&path, &branch, &base))
    }

    pub fn stage(
        &mut self,
        paths: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), GitError>> {
        self.git_op(cx, move |repo| {
            repo.stage(&paths.iter().map(String::as_str).collect::<Vec<_>>())
        })
    }

    pub fn unstage(
        &mut self,
        paths: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), GitError>> {
        self.git_op(cx, move |repo| {
            repo.unstage(&paths.iter().map(String::as_str).collect::<Vec<_>>())
        })
    }

    pub fn commit(
        &mut self,
        message: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), GitError>> {
        self.git_op(cx, move |repo| repo.commit(&message))
    }

    /// Reads a file's text in the background.
    pub fn read_file(
        &self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<std::io::Result<String>> {
        cx.background_executor()
            .spawn(async move { std::fs::read_to_string(path) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::TestAppContext;

    fn event(paths: &[&str]) -> WorkspaceEvent {
        WorkspaceEvent::PathsChanged(Arc::new(paths.iter().map(PathBuf::from).collect()))
    }

    #[test]
    fn touches_matches_file_names_and_endings_without_case() {
        let changed = event(&["app/Package.json", "app/src/App.CSProj", "notes.txt"]);
        assert!(changed.touches(&["package.json"]));
        assert!(changed.touches(&["Cargo.toml", "*.csproj"]));
        assert!(!changed.touches(&["Cargo.toml", "pyproject.toml"]));
        assert!(!changed.touches(&["*.sln"]));
        assert!(!event(&[]).touches(&["package.json"]));
    }

    #[gpui::test]
    fn a_changed_manifest_is_announced(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let model = cx.new(|_| Workspace::empty());
        let seen = Rc::new(RefCell::new(false));
        let _subscription = cx.update(|cx| {
            let seen = seen.clone();
            cx.subscribe(&model, move |_, event: &WorkspaceEvent, _| {
                if event.touches(&["package.json"]) {
                    *seen.borrow_mut() = true;
                }
            })
        });
        model.update(cx, |workspace, cx| {
            workspace.set_root(Some(root.clone()), cx)
        });
        cx.run_until_parked();

        std::fs::write(root.join("package.json"), "{}").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !*seen.borrow() {
            assert!(Instant::now() < deadline, "no event for package.json");
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    }
}
