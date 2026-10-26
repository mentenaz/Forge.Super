//! Git settings for the git commands that talk to a remote
//! (push, pull, fetch), per repository folder: whatever knows which account
//! a repository uses (the Helm panel) sets them with
//! [`set_git_remote_config`], and the code that starts those commands adds
//! them with [`with_git_remote_config`]. They go to git as `GIT_CONFIG_*`
//! environment variables, never on the command line.
//!
//! For git run in a terminal, the token is not put in the environment at
//! all. Instead the terminal's git asks the IDE's own credential helper
//! ([`git_credential_terminal_env`]), which looks the account up when git
//! asks ([`git_credential_for`]), so an open terminal follows an account
//! switch.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// `(key, value)` git settings, in the order git should read them.
pub type GitConfig = Vec<(String, String)>;

/// What [`git_credential_terminal_env`] gives a terminal: `GIT_CONFIG_*`
/// entries, and plain environment variables.
pub type TerminalGitEnv = (GitConfig, Vec<(String, String)>);

/// Git settings (`key`, `value`) for the remote commands of the repository
/// in each folder, with who set them: (folder, owner, entries). The same
/// folder can be open in two windows; each window's own settings are kept,
/// the latest apply, and when one window takes its settings back the
/// other's apply again.
static GIT_REMOTE_CONFIG: RwLock<Vec<(PathBuf, u64, GitConfig)>> = RwLock::new(Vec::new());

/// Sets the git settings push, pull and fetch get in the repository at
/// `repository`, for `owner` (any number unique to the caller, such as an
/// entity id), replacing any `owner` set before. Empty `entries` removes
/// `owner`'s.
pub fn set_git_remote_config(owner: u64, repository: &Path, entries: Vec<(String, String)>) {
    let Ok(mut config) = GIT_REMOTE_CONFIG.write() else {
        return;
    };
    config.retain(|(folder, set_by, _)| !(folder == repository && *set_by == owner));
    if !entries.is_empty() {
        config.push((repository.to_path_buf(), owner, entries));
    }
}

/// The git settings set for the repository at `repository`: the ones set
/// last, when more than one owner has set some.
pub fn git_remote_config(repository: &Path) -> Vec<(String, String)> {
    GIT_REMOTE_CONFIG
        .read()
        .ok()
        .and_then(|config| {
            config
                .iter()
                .rev()
                .find(|(folder, _, _)| folder == repository)
                .map(|(_, _, entries)| entries.clone())
        })
        .unwrap_or_default()
}

/// `env` with the git settings set for the repository at `repository`
/// added as `GIT_CONFIG_*` variables, after any `env` already has. `env`
/// itself when none are set.
pub fn with_git_remote_config<S: BuildHasher + Clone>(
    repository: &Path,
    env: Arc<HashMap<String, String, S>>,
) -> Arc<HashMap<String, String, S>> {
    let entries = git_remote_config(repository);
    if entries.is_empty() {
        return env;
    }
    let mut env = (*env).clone();
    append_git_config(&mut env, &entries);
    Arc::new(env)
}

/// Adds `entries` to the `GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_<n>` /
/// `GIT_CONFIG_VALUE_<n>` settings in `env`, numbering on from those already
/// there.
pub fn append_git_config<S: BuildHasher>(
    env: &mut HashMap<String, String, S>,
    entries: &[(String, String)],
) {
    let start = env
        .get("GIT_CONFIG_COUNT")
        .and_then(|count| count.parse::<usize>().ok())
        .unwrap_or(0);
    for (offset, (key, value)) in entries.iter().enumerate() {
        env.insert(format!("GIT_CONFIG_KEY_{}", start + offset), key.clone());
        env.insert(
            format!("GIT_CONFIG_VALUE_{}", start + offset),
            value.clone(),
        );
    }
    env.insert(
        "GIT_CONFIG_COUNT".to_string(),
        (start + entries.len()).to_string(),
    );
}

/// What git run in a repository is given when it asks to log in to
/// github.com.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitCredential {
    /// Log in with this username and password (a token).
    Login { username: String, password: String },
    /// The account the repository uses is signed out: git must not go on to
    /// another helper and log in as someone else.
    Refuse,
}

/// The github.com credential for the repository in each folder, with who
/// set it: (folder, owner, credential). As with [`GIT_REMOTE_CONFIG`], the
/// latest for a folder applies.
static GIT_CREDENTIALS: RwLock<Vec<(PathBuf, u64, GitCredential)>> = RwLock::new(Vec::new());

/// The IDE's own git credential helper, once it is answering: the helper
/// command for git's config, and the socket it talks to.
static GIT_CREDENTIAL_HELPER: RwLock<Option<(String, PathBuf)>> = RwLock::new(None);

/// The environment variable telling the IDE's credential helper which
/// socket to ask: `<env prefix>_GIT_CREDENTIAL_SOCKET`, so the dev and
/// release builds each reach their own.
pub fn git_credential_socket_var() -> String {
    format!("{}_GIT_CREDENTIAL_SOCKET", branding::PRODUCT.env_prefix)
}

/// A path compared the way the file system does: on Windows, without regard
/// to case or to which slash separates the parts.
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    } else {
        text.trim_end_matches('/').to_string()
    }
}

/// Sets, or with `None` removes, the github.com credential `owner` gives
/// git run in the repository at `repository` (or a folder below it).
pub fn set_git_credential(owner: u64, repository: &Path, credential: Option<GitCredential>) {
    let Ok(mut credentials) = GIT_CREDENTIALS.write() else {
        return;
    };
    let key = path_key(repository);
    credentials.retain(|(folder, set_by, _)| !(path_key(folder) == key && *set_by == owner));
    if let Some(credential) = credential {
        credentials.push((repository.to_path_buf(), owner, credential));
    }
}

/// The github.com credential for git run in `directory`: the one set for the
/// nearest repository folder that contains it.
pub fn git_credential_for(directory: &Path) -> Option<GitCredential> {
    let directory = path_key(directory);
    let separator = if cfg!(windows) { '\\' } else { '/' };
    let credentials = GIT_CREDENTIALS.read().ok()?;
    credentials
        .iter()
        .filter_map(|(folder, _, credential)| {
            let folder = path_key(folder);
            let contains = directory == folder
                || directory
                    .strip_prefix(&folder)
                    .is_some_and(|rest| rest.starts_with(separator));
            contains.then_some((folder.len(), credential))
        })
        // The nearest folder; for one folder set by several owners, the
        // last one set (`max_by_key` keeps the last of equals).
        .max_by_key(|(length, _)| *length)
        .map(|(_, credential)| credential.clone())
}

/// Records that the IDE's credential helper is answering: `command` is the
/// helper as written in git's config (`!'<program>' --git-credential`),
/// `socket` the socket it asks.
pub fn set_git_credential_helper(command: String, socket: PathBuf) {
    if let Ok(mut helper) = GIT_CREDENTIAL_HELPER.write() {
        *helper = Some((command, socket));
    }
}

/// The settings that make git run in a terminal opened in `directory` ask
/// the IDE's credential helper first for github.com, then the user's own
/// helpers in their usual order: `GIT_CONFIG_*` entries, and the socket
/// variable. Empty when the helper is not answering. Runs `git config`, so
/// call it from a background thread.
pub fn git_credential_terminal_env(directory: Option<&Path>) -> TerminalGitEnv {
    let Some((command, socket)) = GIT_CREDENTIAL_HELPER
        .read()
        .ok()
        .and_then(|helper| helper.clone())
    else {
        return (Vec::new(), Vec::new());
    };
    const KEY: &str = "credential.https://github.com.helper";
    let mut config = vec![
        // An empty value clears the helpers configured so far; they are put
        // back after the IDE's own.
        (KEY.to_string(), String::new()),
        (KEY.to_string(), command),
    ];
    config.extend(
        github_credential_helpers(directory)
            .into_iter()
            .map(|helper| (KEY.to_string(), helper)),
    );
    let variables = vec![(
        git_credential_socket_var(),
        socket.to_string_lossy().into_owned(),
    )];
    (config, variables)
}

/// Adds [`git_credential_terminal_env`]'s settings to `env`, a terminal's
/// environment that is still being put together.
pub fn apply_git_credential_env<S: BuildHasher>(
    env: &mut HashMap<String, String, S>,
    settings: TerminalGitEnv,
) {
    let (config, variables) = settings;
    if config.is_empty() {
        return;
    }
    append_git_config(env, &config);
    env.extend(variables);
}

/// The credential helpers git would use for `https://github.com` in
/// `directory`, in order, as the user's config sets them.
fn github_credential_helpers(directory: Option<&Path>) -> Vec<String> {
    let mut command = std::process::Command::new("git");
    command
        .args(["config", "--get-regexp", r"^credential\..*helper$"])
        .stdin(std::process::Stdio::null());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000);
    }
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    parse_github_credential_helpers(&String::from_utf8_lossy(&output.stdout))
}

/// The helpers for `https://github.com` out of `git config --get-regexp`
/// output (`key value` per line, in the order git reads them). An empty
/// value clears the ones before it, as it does for git. A helper set for a
/// path below github.com is left out.
pub fn parse_github_credential_helpers(output: &str) -> Vec<String> {
    let mut helpers = Vec::new();
    for line in output.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let Some(rest) = key.strip_prefix("credential.") else {
            continue;
        };
        let Some(url) = rest.strip_suffix("helper") else {
            continue;
        };
        let url = url.trim_end_matches('.').trim_end_matches('/');
        let applies = url.is_empty()
            || url.eq_ignore_ascii_case("https://github.com")
            || url.eq_ignore_ascii_case("github.com");
        if !applies {
            continue;
        }
        let value = value.trim();
        if value.is_empty() {
            helpers.clear();
        } else {
            helpers.push(value.to_string());
        }
    }
    helpers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helpers_for_github_follow_the_users_config_order_and_resets() {
        let output = "credential.helper manager\n\
                      credential.helper manager\n\
                      credential.https://github.com.helper\n\
                      credential.https://github.com.helper !'C:\\gh.exe' auth git-credential\n\
                      credential.https://dev.azure.com.usehttppath true\n\
                      credential.https://gitlab.com.helper other\n";
        assert_eq!(
            parse_github_credential_helpers(output),
            vec!["!'C:\\gh.exe' auth git-credential".to_string()]
        );
        assert_eq!(
            parse_github_credential_helpers("credential.helper manager\n"),
            vec!["manager".to_string()]
        );
    }

    #[test]
    fn a_credential_is_found_from_the_nearest_repository_folder() {
        let outer = PathBuf::from("ide-env-cred-test").join("outer");
        let inner = outer.join("inner");
        let login = |username: &str| GitCredential::Login {
            username: username.to_string(),
            password: "secret".to_string(),
        };
        set_git_credential(1, &outer, Some(login("a")));
        set_git_credential(1, &inner, Some(login("b")));

        let found = |path: &Path| match git_credential_for(path) {
            Some(GitCredential::Login { username, .. }) => Some(username),
            Some(GitCredential::Refuse) => Some("refused".to_string()),
            None => None,
        };
        assert_eq!(found(&outer), Some("a".to_string()));
        assert_eq!(found(&outer.join("src")), Some("a".to_string()));
        assert_eq!(found(&inner.join("deep")), Some("b".to_string()));
        // A sibling whose name merely starts the same is not inside.
        assert_eq!(
            found(&PathBuf::from("ide-env-cred-test").join("outer2")),
            None
        );

        set_git_credential(1, &inner, Some(GitCredential::Refuse));
        assert_eq!(found(&inner.join("deep")), Some("refused".to_string()));
        set_git_credential(1, &inner, None);
        assert_eq!(found(&inner), Some("a".to_string()));

        // The same folder in a second window: its credential applies while
        // it is set, and the first window's comes back when it goes.
        set_git_credential(2, &outer, Some(login("c")));
        assert_eq!(found(&outer), Some("c".to_string()));
        set_git_credential(2, &outer, None);
        assert_eq!(found(&outer), Some("a".to_string()));

        set_git_credential(1, &outer, None);
        assert_eq!(found(&outer), None);
    }

    #[test]
    fn git_config_entries_are_numbered_after_existing_ones() {
        let mut env = HashMap::new();
        env.insert("GIT_CONFIG_COUNT".to_string(), "1".to_string());
        env.insert("GIT_CONFIG_KEY_0".to_string(), "gpg.program".to_string());
        env.insert("GIT_CONFIG_VALUE_0".to_string(), "wrapper".to_string());
        append_git_config(
            &mut env,
            &[("credential.helper".to_string(), String::new())],
        );
        assert_eq!(env["GIT_CONFIG_COUNT"], "2");
        assert_eq!(env["GIT_CONFIG_KEY_0"], "gpg.program");
        assert_eq!(env["GIT_CONFIG_KEY_1"], "credential.helper");
        assert_eq!(env["GIT_CONFIG_VALUE_1"], "");
    }

    #[test]
    fn git_remote_config_is_per_repository_and_can_be_removed() {
        let repository = PathBuf::from("ide-env-test-repository");
        let other = PathBuf::from("ide-env-test-other");
        set_git_remote_config(1, &repository, vec![("a".to_string(), "1".to_string())]);

        let env = with_git_remote_config(&repository, Arc::new(HashMap::new()));
        assert_eq!(env["GIT_CONFIG_KEY_0"], "a");
        let untouched = Arc::new(HashMap::new());
        assert!(Arc::ptr_eq(
            &with_git_remote_config(&other, untouched.clone()),
            &untouched
        ));

        set_git_remote_config(1, &repository, Vec::new());
        assert!(git_remote_config(&repository).is_empty());
    }

    #[test]
    fn the_socket_variable_carries_the_products_prefix() {
        let name = git_credential_socket_var();
        assert!(name.starts_with(branding::PRODUCT.env_prefix), "{name}");
        assert!(name.ends_with("_GIT_CREDENTIAL_SOCKET"), "{name}");
    }
}
