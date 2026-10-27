//! A read-only view of a file's text, on gpui-component's code editor (line
//! numbers, scrolling, selection and copy). Loads in the background; files
//! that are too large or not UTF-8 text show a message instead.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use forge_host::{Host, TabRequest, TabRestore, Workspace};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div,
};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    input::{Editor, EditorState},
    v_flex,
};

/// Opens `path` (absolute) read-only in a tab. Registered as the host's file
/// opener, so the Git panel and others open files the same way.
pub fn open_file_tab(path: PathBuf, window: &mut Window, cx: &mut App) {
    Host::open_tab(file_tab(path, cx), window, cx);
}

/// The tab kind file tabs are saved and reopened as.
pub(crate) const FILE_TAB_KIND: &str = "file";

/// Reopens a saved file tab; `None` when the file is gone.
pub(crate) fn restore_file_tab(
    payload: &serde_json::Value,
    _: &mut Window,
    cx: &mut App,
) -> Option<TabRequest> {
    let path = PathBuf::from(payload.as_str()?);
    path.is_file().then(|| file_tab(path, cx))
}

fn file_tab(path: PathBuf, cx: &App) -> TabRequest {
    let title: SharedString = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .into();
    // Workspace-relative path for the breadcrumb and the tab's folder hint.
    let rel = Workspace::model(cx).read(cx).root().and_then(|root| {
        let rel = path.strip_prefix(root).ok()?;
        Some(
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        )
    });
    TabRequest {
        key: path.to_string_lossy().into_owned().into(),
        title,
        icon: Icon::new(IconName::File),
        path: Some(
            rel.unwrap_or_else(|| path.to_string_lossy().into_owned())
                .into(),
        ),
        symbol: None,
        dirty: false,
        restore: Some(TabRestore::new(FILE_TAB_KIND, &path)),
        build: Rc::new(move |window, cx| {
            let path = path.clone();
            cx.new(|cx| FileView::new(path, window, cx)).into()
        }),
    }
}

/// Larger files aren't opened (the view keeps the whole text in memory).
const MAX_SIZE: u64 = 4 * 1024 * 1024;

enum Content {
    Loading,
    Text(Entity<EditorState>),
    Unavailable(String),
}

pub struct FileView {
    content: Content,
}

/// The highlighter language for a path, by extension.
fn language(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "toml" => "toml",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "py" => "python",
        "cs" => "csharp",
        "yml" | "yaml" => "yaml",
        "html" | "htm" => "html",
        "css" => "css",
        "sh" | "bash" => "bash",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        other => other,
    }
    .to_string()
}

fn read(path: &Path) -> Result<String, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_SIZE {
        return Err(format!(
            "This file is {:.1} MB; files over 4 MB aren't opened yet.",
            size as f64 / 1_048_576.0
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes)
        .map_err(|_| "This file isn't UTF-8 text (it may be binary).".to_string())
}

impl FileView {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.spawn_in(window, async move |this, cx| {
            let language = language(&path);
            let result = cx
                .background_executor()
                .spawn(async move { read(&path) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.content = match result {
                    Ok(text) => {
                        let state = cx.new(|cx| {
                            EditorState::new(window, cx)
                                .line_number(true)
                                .default_value(text)
                        });
                        state.update(cx, |state, cx| state.set_highlighter(language, cx));
                        Content::Text(state)
                    }
                    Err(message) => Content::Unavailable(message),
                };
                cx.notify();
            });
        })
        .detach();
        Self {
            content: Content::Loading,
        }
    }
}

impl Render for FileView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message = |text: String| {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(text)
                .into_any_element()
        };
        match &self.content {
            Content::Loading => message("Loading…".into()),
            Content::Unavailable(text) => message(text.clone()),
            Content::Text(state) => div()
                .size_full()
                .bg(cx.theme().background)
                .child(
                    Editor::new(state)
                        .readonly(true)
                        .appearance(false)
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(cx.theme().mono_font_size)
                        .size_full(),
                )
                .into_any_element(),
        }
    }
}
