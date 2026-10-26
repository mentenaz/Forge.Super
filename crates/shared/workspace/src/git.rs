//! Git through the `git` command line: the repository root, branches,
//! worktrees, status, staging and commits. Each call runs `git -C <root>`
//! without a console window and never prompts (credentials, editors).

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What git said when a command failed (its stderr), or why it couldn't run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitError(pub String);

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GitError {}

pub type Result<T> = std::result::Result<T, GitError>;

/// Runs `git -C <dir> <args>` and returns its stdout.
fn run<I, S>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .map_err(|err| GitError(format!("couldn't run git: {err}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(GitError(if stderr.is_empty() {
            format!("git exited with {}", output.status)
        } else {
            stderr
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// `None` when the worktree is on a detached HEAD.
    pub branch: Option<String>,
    /// Short commit id of its HEAD.
    pub head: String,
    /// The repository's main worktree (the first `git worktree list` entry).
    pub is_main: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    /// A merge conflict.
    Conflicted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStatus {
    /// Path relative to the repository root, `/`-separated.
    pub path: String,
    pub state: FileState,
    /// Has changes in the index (`git add`ed).
    pub staged: bool,
    /// Has changes in the working tree not yet staged.
    pub unstaged: bool,
}

/// A git repository (or worktree) on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    /// The repository containing `path`, if any.
    pub fn discover(path: &Path) -> Option<Repository> {
        let root = run(path, ["rev-parse", "--show-toplevel"]).ok()?;
        Some(Repository {
            root: PathBuf::from(root.trim()),
        })
    }

    /// The top-level folder of this repository or worktree.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The checked-out branch, or `None` on a detached HEAD.
    pub fn current_branch(&self) -> Result<Option<String>> {
        let name = run(&self.root, ["branch", "--show-current"])?
            .trim()
            .to_string();
        Ok((!name.is_empty()).then_some(name))
    }

    /// Short id of HEAD.
    pub fn head(&self) -> Result<String> {
        Ok(run(&self.root, ["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_string())
    }

    /// Local branches, sorted by name.
    pub fn branches(&self) -> Result<Vec<String>> {
        Ok(run(
            &self.root,
            ["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        )?
        .lines()
        .map(str::to_string)
        .collect())
    }

    pub fn switch(&self, branch: &str) -> Result<()> {
        run(&self.root, ["switch", branch]).map(drop)
    }

    pub fn worktrees(&self) -> Result<Vec<Worktree>> {
        let output = run(&self.root, ["worktree", "list", "--porcelain"])?;
        Ok(parse_worktrees(&output))
    }

    /// Adds a worktree at `path` on a new branch `branch`, starting from
    /// `base` (a branch or commit).
    pub fn add_worktree(&self, path: &Path, branch: &str, base: &str) -> Result<()> {
        let args: [&OsStr; 6] = [
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            branch.as_ref(),
            path.as_os_str(),
            base.as_ref(),
        ];
        run(&self.root, args).map(drop)
    }

    /// Changed, untracked and conflicted files.
    pub fn status(&self) -> Result<Vec<FileStatus>> {
        let output = run(
            &self.root,
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        Ok(parse_status(&output))
    }

    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        run(
            &self.root,
            ["add", "--"].into_iter().chain(paths.iter().copied()),
        )
        .map(drop)
    }

    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        run(
            &self.root,
            ["restore", "--staged", "--"]
                .into_iter()
                .chain(paths.iter().copied()),
        )
        .map(drop)
    }

    /// Commits what is staged, with the user's git identity.
    pub fn commit(&self, message: &str) -> Result<()> {
        run(&self.root, ["commit", "-m", message]).map(drop)
    }
}

/// Parses `git worktree list --porcelain`: blocks of `worktree <path>`,
/// `HEAD <sha>`, `branch refs/heads/<name>` or `detached`.
fn parse_worktrees(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    for block in output.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let mut path = None;
        let mut head = String::new();
        let mut branch = None;
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                head = h.chars().take(7).collect();
            } else if let Some(b) = line.strip_prefix("branch ") {
                branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
            }
        }
        if let Some(path) = path {
            let is_main = worktrees.is_empty();
            worktrees.push(Worktree {
                path,
                branch,
                head,
                is_main,
            });
        }
    }
    worktrees
}

/// Parses `git status --porcelain=v1 -z`: `XY path\0`, with renames as
/// `XY new\0old\0`.
fn parse_status(output: &str) -> Vec<FileStatus> {
    let mut files = Vec::new();
    let mut fields = output.split('\0').filter(|f| !f.is_empty());
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let (code, path) = field.split_at(3);
        let mut chars = code.chars();
        let (x, y) = (chars.next().unwrap_or(' '), chars.next().unwrap_or(' '));
        if x == 'R' || x == 'C' {
            fields.next(); // the original path
        }
        let conflicted = matches!(
            (x, y),
            ('D', 'D')
                | ('A', 'U')
                | ('U', 'D')
                | ('U', 'A')
                | ('D', 'U')
                | ('A', 'A')
                | ('U', 'U')
        );
        let state = if conflicted {
            FileState::Conflicted
        } else if x == '?' {
            FileState::Untracked
        } else if x == 'R' || y == 'R' {
            FileState::Renamed
        } else if x == 'A' {
            FileState::Added
        } else if x == 'D' || y == 'D' {
            FileState::Deleted
        } else {
            FileState::Modified
        };
        let untracked = x == '?';
        files.push(FileStatus {
            path: path.to_string(),
            state,
            staged: !untracked && !conflicted && x != ' ',
            unstaged: untracked || conflicted || y != ' ',
        });
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A fresh repository with one commit on `main`.
    fn repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run(root, ["init", "-b", "main"]).unwrap();
        run(root, ["config", "user.name", "Test"]).unwrap();
        run(root, ["config", "user.email", "test@example.com"]).unwrap();
        run(root, ["config", "commit.gpgsign", "false"]).unwrap();
        fs::write(root.join("a.txt"), "one\n").unwrap();
        run(root, ["add", "."]).unwrap();
        run(root, ["commit", "-m", "first"]).unwrap();
        let repo = Repository::discover(root).unwrap();
        (dir, repo)
    }

    #[test]
    fn discover_outside_a_repository_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Repository::discover(dir.path()), None);
    }

    #[test]
    fn branches_and_switching() {
        let (_dir, repo) = repo();
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
        run(repo.root(), ["branch", "feature"]).unwrap();
        assert_eq!(repo.branches().unwrap(), ["feature", "main"]);
        repo.switch("feature").unwrap();
        assert_eq!(repo.current_branch().unwrap().as_deref(), Some("feature"));
        assert_eq!(repo.head().unwrap().len(), 7);
        assert!(repo.switch("missing").is_err());
    }

    #[test]
    fn worktrees_list_and_add() {
        let (dir, repo) = repo();
        let extra = dir
            .path()
            .join("..")
            .join(format!("wt-{}", std::process::id()));
        repo.add_worktree(&extra, "feature/wt", "main").unwrap();

        let worktrees = repo.worktrees().unwrap();
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature/wt"));
        assert!(!worktrees[1].is_main);
        let _ = run(
            repo.root(),
            ["worktree", "remove", "--force"]
                .into_iter()
                .chain([extra.to_str().unwrap()]),
        );
    }

    #[test]
    fn status_stage_unstage_commit() {
        let (_dir, repo) = repo();
        let root = repo.root().to_path_buf();
        fs::write(root.join("a.txt"), "two\n").unwrap();
        fs::write(root.join("new.txt"), "new\n").unwrap();

        let status = repo.status().unwrap();
        let find = |s: &[FileStatus], p: &str| s.iter().find(|f| f.path == p).cloned().unwrap();
        assert_eq!(find(&status, "a.txt").state, FileState::Modified);
        assert!(find(&status, "a.txt").unstaged && !find(&status, "a.txt").staged);
        assert_eq!(find(&status, "new.txt").state, FileState::Untracked);

        repo.stage(&["a.txt", "new.txt"]).unwrap();
        let status = repo.status().unwrap();
        assert!(find(&status, "a.txt").staged && !find(&status, "a.txt").unstaged);
        assert_eq!(find(&status, "new.txt").state, FileState::Added);

        repo.unstage(&["new.txt"]).unwrap();
        assert_eq!(
            find(&repo.status().unwrap(), "new.txt").state,
            FileState::Untracked
        );

        repo.commit("second").unwrap();
        let status = repo.status().unwrap();
        assert_eq!(
            status.len(),
            1,
            "only the unstaged file is left: {status:?}"
        );
        assert!(repo.commit("nothing staged").is_err());
    }

    #[test]
    fn parses_renames_and_conflicts() {
        let status = parse_status("R  new.rs\0old.rs\0UU both.rs\0?? scratch.md\0");
        assert_eq!(status.len(), 3);
        assert_eq!(
            (status[0].path.as_str(), status[0].state),
            ("new.rs", FileState::Renamed)
        );
        assert_eq!(status[1].state, FileState::Conflicted);
        assert_eq!(status[2].state, FileState::Untracked);
    }
}
