//! The PATH for spawned npm/yarn/pnpm/bun processes.
//!
//! A package manager run by the IDE should find the same tools a project's
//! own `npm run` would, under the same Node.js as the IDE's terminals and
//! scripts: the project's `node_modules/.bin` folders first, then the IDE's
//! own runtimes (the built-in Node.js the user picked, through
//! `forge_ide_env`), then the machine's PATH. [`path_with_node`] builds that.
//!
//! The machine's PATH on Windows is the registry-merged one, which
//! `forge_ide_env::machine_path` provides for every backend.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The PATH to give a package manager run in `project_root`, in the order
/// `npm run` uses: the project's `node_modules/.bin` folders, nearest first
/// (so a project's own tool wins over a global one), then the IDE's own
/// runtimes, then the machine's PATH.
///
/// The IDE's runtimes are what make the panels' npm calls run under the
/// Node.js the user picked in the Node panel, as the terminal and scripts
/// do, and work on a machine with no Node.js installed at all. This used to
/// put nvm-windows' current version first and the project's tools last.
pub fn path_with_node(project_root: &str) -> OsString {
    let root = Path::new(project_root);
    let mut dirs: Vec<PathBuf> = root
        .ancestors()
        .map(|dir| dir.join("node_modules").join(".bin"))
        .filter(|dir| dir.is_dir())
        .collect();
    dirs.extend(forge_ide_env::bin_dirs(Some(root)));

    let path = forge_ide_env::path_with(&dirs);
    log::debug!("npm_backend: PATH = {}", path.to_string_lossy());
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_tools_come_first_nearest_first() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("packages").join("app");
        for dir in [root.path(), app.as_path()] {
            std::fs::create_dir_all(dir.join("node_modules").join(".bin")).unwrap();
        }

        let path = path_with_node(&app.to_string_lossy());
        let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(entries[0], app.join("node_modules").join(".bin"));
        assert_eq!(entries[1], root.path().join("node_modules").join(".bin"));
        // The machine's PATH follows them, rather than being replaced.
        assert!(entries.len() > 2);
    }

    #[test]
    fn a_project_without_tools_gets_the_machines_path() {
        let root = tempfile::tempdir().unwrap();
        // Only meaningful where nothing above the temp folder is a Node
        // project of its own.
        if root
            .path()
            .ancestors()
            .any(|dir| dir.join("node_modules").join(".bin").is_dir())
        {
            return;
        }
        let path = path_with_node(&root.path().to_string_lossy());
        let base = forge_ide_env::machine_path();
        let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();
        let expected: Vec<PathBuf> = std::env::split_paths(&base).collect();
        assert_eq!(entries, expected);
    }
}
