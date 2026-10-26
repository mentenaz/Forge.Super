//! Listing folders for the Explorer, one level at a time (folders are
//! expanded on demand, as in Zed's project panel).

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// The entries directly inside `dir`: folders first, then files, each
/// sorted case-insensitively. Respects `.gitignore` (including those of
/// parent folders) and always hides `.git`. Hidden files are shown.
pub fn list_dir(dir: &Path) -> io::Result<Vec<DirEntry>> {
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a folder", dir.display()),
        ));
    }
    let mut entries: Vec<DirEntry> = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        // Respect .gitignore even outside a repository, as Zed does.
        .require_git(false)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.depth() == 1)
        .filter(|entry| entry.file_name() != ".git")
        .map(|entry| DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: entry.file_type().is_some_and(|t| t.is_dir()),
            path: entry.into_path(),
        })
        .collect();
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn folders_first_sorted_and_gitignore_respected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        fs::write(root.join("b.rs"), "").unwrap();
        fs::write(root.join("A.toml"), "").unwrap();
        fs::write(root.join("debug.log"), "").unwrap();

        let names: Vec<_> = list_dir(root)
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.is_dir))
            .collect();
        assert_eq!(
            names,
            [
                ("src".to_string(), true),
                (".gitignore".to_string(), false),
                ("A.toml".to_string(), false),
                ("b.rs".to_string(), false),
            ]
        );
    }

    #[test]
    fn parent_gitignore_applies_to_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".gitignore"), "*.tmp\n").unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/keep.rs"), "").unwrap();
        fs::write(root.join("sub/drop.tmp"), "").unwrap();

        let names: Vec<_> = list_dir(&root.join("sub"))
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["keep.rs"]);
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_dir(&dir.path().join("nope")).is_err());
    }
}
