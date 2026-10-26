//! Stand-in data for the parts of the shell that will later come from real
//! sources: language servers, diagnostics and the active editor's cursor.
//! Files and git are real since phase 1.5.
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

pub(super) struct ShellStatus {
    pub(super) recent_projects: Vec<SharedString>,
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
