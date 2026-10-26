//! The workspace backbone (design doc §12.6 phase 1.5, option C): just
//! enough of a project model for the Explorer, the Git panel and the
//! title-bar pickers to be real before Zed's `project`, `worktree`, `fs` and
//! `git` crates are vendored in phase 5, which replace this crate.
//!
//! - [`tree`]: list a folder's entries, respecting `.gitignore`.
//! - [`watch`]: report file changes under the workspace root.
//! - [`git`]: branches, worktrees, status, staging and commits through the
//!   `git` command line.
//!
//! No gpui: everything here is blocking and is run on a background thread
//! by `forge-host`'s workspace model.

pub mod git;
pub mod tree;
pub mod watch;
