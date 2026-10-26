//! Stand-in data for the parts of the title bar and status bar that will
//! later come from real sources (git, language servers, the active editor).
//! Everything here is replaced as the host API and feature crates land.

use gpui::SharedString;

use super::notifications::{NoticeEntry, NoticeKind};

/// A language server's state, as Zed's language server menu shows it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LspState {
    Starting,
    Running,
    Warning,
    Error,
    Stopped,
}

impl LspState {
    pub(super) fn label(self) -> &'static str {
        match self {
            LspState::Starting => "Starting…",
            LspState::Running => "Running",
            LspState::Warning => "Warning",
            LspState::Error => "Error",
            LspState::Stopped => "Stopped",
        }
    }
}

pub(super) struct LspStatus {
    pub(super) name: SharedString,
    pub(super) state: LspState,
    /// The server's last message, shown with "View Message".
    pub(super) message: Option<SharedString>,
}

/// What the active editor reports about its buffer.
pub(super) struct EditorStatus {
    pub(super) line: u32,
    pub(super) column: u32,
    pub(super) language: SharedString,
    pub(super) line_ending: SharedString,
    pub(super) encoding: SharedString,
}

/// A git worktree of the workspace's repository.
pub(super) struct Worktree {
    /// Folder name, or `None` for the main worktree (the repository itself).
    pub(super) name: Option<SharedString>,
    pub(super) branch: SharedString,
    pub(super) commit: SharedString,
    /// `None` for the main worktree, which lives at the workspace root.
    pub(super) path: Option<SharedString>,
}

pub(super) struct ShellStatus {
    pub(super) worktrees: Vec<Worktree>,
    pub(super) current_worktree: usize,
    pub(super) recent_projects: Vec<SharedString>,
    pub(super) branch: SharedString,
    pub(super) branches: Vec<SharedString>,
    pub(super) errors: usize,
    pub(super) warnings: usize,
    pub(super) language_servers: Vec<LspStatus>,
    /// The activity indicator's message (progress from language servers,
    /// downloads, …), if anything is in progress.
    pub(super) activity: Option<SharedString>,
    /// What the language picker offers (the languages built into Zed).
    pub(super) languages: Vec<SharedString>,
    pub(super) editor: EditorStatus,
}

impl ShellStatus {
    pub(super) fn sample() -> Self {
        Self {
            recent_projects: vec![
                r"E:\mentenaz\Forge.Super".into(),
                r"E:\zed_Source".into(),
                r"E:\mentenaz\gpui".into(),
                r"E:\Forge_GPUI".into(),
            ],
            worktrees: vec![Worktree { name: None, branch: "main".into(), commit: "892cbe9".into(), path: None }],
            current_worktree: 0,
            branch: "main".into(),
            branches: vec![
                "main".into(),
                "feature/shell-title-bar".into(),
                "feature/host-api".into(),
                "fix/status-bar-overflow".into(),
            ],
            errors: 2,
            warnings: 5,
            language_servers: [
                ("rust-analyzer", LspState::Running, None),
                ("taplo", LspState::Running, None),
                ("tombi", LspState::Running, None),
                ("md-image-path-lsp", LspState::Running, None),
                ("markdown-oxide", LspState::Warning, Some("2 links in forge-super-design.md could not be resolved.")),
            ]
            .into_iter()
            .map(|(name, state, message)| LspStatus {
                name: name.into(),
                state,
                message: message.map(Into::into),
            })
            .collect(),
            activity: Some("rust-analyzer: Indexing… 42%".into()),
            languages: [
                "Bash", "C", "C++", "CSS", "Go", "JSON", "Markdown", "Plain Text", "Python", "Rust", "TOML",
                "TSX", "TypeScript", "YAML",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            editor: EditorStatus {
                line: 12,
                column: 8,
                language: "Rust".into(),
                line_ending: "LF".into(),
                encoding: "UTF-8".into(),
            },
        }
    }
}

/// A file the dummy Explorer can open, to exercise the tab bar.
pub(super) struct DummyFile {
    /// Workspace-relative, `/`-separated.
    pub(super) path: &'static str,
    /// The symbol under the cursor, for the breadcrumb.
    pub(super) symbol: Option<&'static str>,
    /// Has unsaved changes.
    pub(super) dirty: bool,
}

/// Includes several `Cargo.toml`s and `lib.rs`s so the tab bar's folder
/// hints show.
pub(super) const DUMMY_FILES: &[DummyFile] = &[
    DummyFile { path: "crates/app/src/main.rs", symbol: Some("fn main"), dirty: true },
    DummyFile { path: "crates/app/src/shell/forge_shell.rs", symbol: Some("impl ForgeShell"), dirty: false },
    DummyFile { path: "crates/app/Cargo.toml", symbol: None, dirty: false },
    DummyFile { path: "crates/features/node/ui/Cargo.toml", symbol: None, dirty: false },
    DummyFile { path: "crates/features/node/core/Cargo.toml", symbol: None, dirty: false },
    DummyFile { path: "crates/features/dotnet/ui/Cargo.toml", symbol: None, dirty: false },
    DummyFile { path: "crates/features/node/ui/src/lib.rs", symbol: Some("mod panel"), dirty: false },
    DummyFile { path: "crates/features/dotnet/ui/src/lib.rs", symbol: Some("mod panel"), dirty: true },
    DummyFile { path: "forge-super-design.md", symbol: Some("12.6 Phases"), dirty: false },
];

pub(super) fn sample_notices() -> Vec<NoticeEntry> {
    vec![
        NoticeEntry {
            id: 3,
            kind: NoticeKind::Error,
            title: "dotnet build failed".into(),
            message: "Api.csproj: 1 error, see the Script Runner output.".into(),
            time: "Just now".into(),
            read: false,
        },
        NoticeEntry {
            id: 0,
            kind: NoticeKind::Warning,
            title: "npm audit".into(),
            message: "3 vulnerable packages found in Backend.".into(),
            time: "2 min ago".into(),
            read: false,
        },
        NoticeEntry {
            id: 1,
            kind: NoticeKind::Success,
            title: "Script finished".into(),
            message: "`npm run build` completed in 14.2s.".into(),
            time: "8 min ago".into(),
            read: false,
        },
        NoticeEntry {
            id: 2,
            kind: NoticeKind::Info,
            title: "Update available".into(),
            message: "rust-analyzer 2026-10-20 is available.".into(),
            time: "1 hour ago".into(),
            read: true,
        },
    ]
}
