//! The machine's `PATH` as a terminal would see it.
//!
//! On Windows, GUI-launched apps inherit a stripped-down `PATH`, so
//! [`merged_path`] adds the registry `Environment` values (user, then
//! machine) and removes duplicates. Elsewhere there is nothing to add.
//! [`path_with`] puts folders in front of that, for the backends that start
//! package managers and toolchains.

use std::ffi::OsString;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use windows_registry::{CURRENT_USER, LOCAL_MACHINE};

/// Replaces `%NAME%` references with the process's environment variables.
/// Unknown names are left as they are.
#[cfg(target_os = "windows")]
fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    let mut start = 0;
    while let Some(open) = result[start..].find('%') {
        let open = start + open;
        let rest = &result[open + 1..];
        let Some(close_rel) = rest.find('%') else {
            break;
        };
        let close = open + 1 + close_rel;
        let name = &result[open + 1..close];
        if let Ok(value) = std::env::var(name) {
            result.replace_range(open..close + 1, &value);
            start = open + value.len();
        } else {
            start = close + 1;
        }
    }
    result
}

#[cfg(target_os = "windows")]
fn read_env_path(key: &windows_registry::Key) -> Option<String> {
    let val = key.get_string("Path").ok()?;
    if val.is_empty() {
        return None;
    }
    Some(expand_env_vars(&val))
}

/// Joins `;`-separated lists, dropping empty entries and case-insensitive
/// duplicates (the first one wins).
#[cfg(any(target_os = "windows", test))]
fn dedupe_path(path: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for part in path.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let key = part.to_lowercase();
        if !seen.contains(&key) {
            seen.push(key);
            out.push(part.to_string());
        }
    }
    out.join(";")
}

/// The process's `PATH` plus the user and machine `PATH` from the Windows
/// registry, without duplicates. `None` when there is nothing to add (other
/// platforms) or the result is empty.
#[cfg(target_os = "windows")]
pub fn merged_path() -> Option<String> {
    let mut parts = vec![std::env::var("PATH").unwrap_or_default()];

    match CURRENT_USER.open("Environment") {
        Ok(env_key) => match read_env_path(&env_key) {
            Some(val) => parts.push(val),
            None => log::debug!("merged_path: HKCU Path missing"),
        },
        Err(_) => log::debug!("merged_path: cannot open HKCU Environment"),
    }

    match LOCAL_MACHINE.open(r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment") {
        Ok(env_key) => match read_env_path(&env_key) {
            Some(val) => parts.push(val),
            None => log::debug!("merged_path: HKLM Path missing"),
        },
        Err(_) => log::debug!("merged_path: cannot open HKLM Environment"),
    }

    let merged = dedupe_path(&parts.join(";"));
    (!merged.is_empty()).then_some(merged)
}

#[cfg(not(target_os = "windows"))]
pub fn merged_path() -> Option<String> {
    None
}

/// The machine's `PATH`: [`merged_path`], or the process's own.
pub fn machine_path() -> OsString {
    merged_path()
        .map(OsString::from)
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default()
}

/// [`machine_path`] with `dirs` first, in the order given. Folders already
/// on the path are not repeated.
pub fn path_with(dirs: &[PathBuf]) -> OsString {
    let base = machine_path();
    crate::prepend(dirs, Some(&base)).unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_keeps_first_and_ignores_case_and_blanks() {
        assert_eq!(dedupe_path(r"C:\a;;c:\A; C:\b ;C:\a"), r"C:\a;C:\b");
    }

    #[test]
    fn path_with_puts_folders_first_before_the_machine_path() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("one");
        let second = dir.path().join("two");
        let path = path_with(&[first.clone(), second.clone()]);
        let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(entries[0], first);
        assert_eq!(entries[1], second);
        assert!(entries.len() > 2, "the machine's PATH follows");
    }

    #[test]
    fn no_folders_gives_the_machine_path() {
        assert_eq!(path_with(&[]), machine_path());
    }
}
