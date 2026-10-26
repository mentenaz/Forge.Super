//! Git asks the IDE's binary for github.com credentials and gets the
//! account set for the repository folder it runs in. Runs `git credential
//! fill`, which never contacts a remote.

use std::path::Path;
use std::process::{Command, Stdio};

use forge_ide_env::GitCredential;

/// The answer of a helper configured after the IDE's, which git reaches
/// only when the IDE's says nothing.
const FALLBACK: &str = "password=fallback";

/// `git credential fill` in `directory`, as a terminal of the IDE would run
/// it, with a fallback helper after the IDE's.
fn fill(directory: &Path) -> String {
    let mut env = std::collections::HashMap::new();
    let settings = forge_ide_env::git_credential_terminal_env(Some(directory));
    forge_ide_env::apply_git_credential_env(&mut env, settings);
    forge_ide_env::append_git_config(
        &mut env,
        &[(
            "credential.https://github.com.helper".to_string(),
            format!("!f() {{ echo username=fallback; echo {FALLBACK}; }}; f"),
        )],
    );
    let mut child = Command::new("git")
        .args(["credential", "fill"])
        .current_dir(directory)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("git runs");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"protocol=https\nhost=github.com\n\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn git_gets_the_repository_folders_credential_from_the_ide() {
    // Leave the machine's git config out, for this process (which reads the
    // user's helpers) and the git it starts, so a real helper never runs.
    // The only test in this binary, so nothing else reads the environment.
    unsafe {
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        std::env::set_var(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        );
        std::env::set_var("GIT_TERMINAL_PROMPT", "0");
    }
    let program = Path::new(env!("CARGO_BIN_EXE_mentenaz-forge-super"));
    forge_git_credential::start_server(program).unwrap();

    let repository = tempfile::tempdir().unwrap();
    let inside = repository.path().join("src");
    std::fs::create_dir(&inside).unwrap();

    forge_ide_env::set_git_credential(
        1,
        repository.path(),
        Some(GitCredential::Login {
            username: "x-access-token".to_string(),
            password: "secret".to_string(),
        }),
    );
    let output = fill(&inside);
    assert!(output.contains("username=x-access-token\n"), "{output}");
    assert!(output.contains("password=secret\n"), "{output}");

    // Signed out: git is told to stop, not to try another account.
    forge_ide_env::set_git_credential(1, repository.path(), Some(GitCredential::Refuse));
    let output = fill(&inside);
    assert!(!output.contains("password="), "{output}");
    assert!(!output.contains(FALLBACK), "{output}");
}
