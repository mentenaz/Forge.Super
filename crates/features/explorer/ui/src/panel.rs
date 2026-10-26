//! The Explorer panel: the workspace's file tree, expanded folder by folder
//! (each folder is listed when first opened), coloured by git status like
//! Zed's project panel. Clicking a file opens it read-only in a tab.

use forge_host::{FileState, Workspace};
use gpui::{
    AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, StyledExt, h_flex, v_flex};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Indent per tree level.
const INDENT: f32 = 14.;

pub struct ExplorerPanel {
    /// The root the tree state below belongs to.
    root: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
}

/// Git status of the workspace's files, keyed by path relative to the
/// workspace root (`/`-separated).
struct GitColors {
    files: HashMap<String, FileState>,
}

impl GitColors {
    fn file(&self, rel: &str) -> Option<FileState> {
        self.files.get(rel).copied()
    }

    /// The most severe state of anything inside the folder `rel`.
    fn folder(&self, rel: &str) -> Option<FileState> {
        let prefix = if rel.is_empty() {
            String::new()
        } else {
            format!("{rel}/")
        };
        let mut states = self
            .files
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(_, s)| *s);
        let first = states.next()?;
        Some(states.fold(first, |worst, state| {
            if severity(state) > severity(worst) {
                state
            } else {
                worst
            }
        }))
    }
}

fn severity(state: FileState) -> u8 {
    match state {
        FileState::Conflicted => 4,
        FileState::Deleted => 3,
        FileState::Modified | FileState::Renamed => 2,
        FileState::Added | FileState::Untracked => 1,
    }
}

/// `path` relative to `root`, `/`-separated.
fn relative(path: &Path, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

impl ExplorerPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&Workspace::model(cx), |_, _, cx| cx.notify())
            .detach();
        Self {
            root: None,
            expanded: HashSet::new(),
            selected: None,
        }
    }

    /// Git status of the files under the workspace root, if it's in a
    /// repository. The workspace root may be a subfolder of the repository,
    /// and Windows may spell the same folder differently (short names), so
    /// both are resolved before comparing.
    fn git_colors(&self, root: &Path, cx: &Context<Self>) -> Option<GitColors> {
        let workspace = Workspace::model(cx);
        let git = workspace.read(cx).git()?;
        let (Ok(root), Ok(repo)) = (
            std::fs::canonicalize(root),
            std::fs::canonicalize(&git.root),
        ) else {
            return None;
        };
        let within = relative(&root, &repo)?;
        let prefix = if within.is_empty() {
            String::new()
        } else {
            format!("{within}/")
        };
        let files = git
            .status
            .iter()
            .filter_map(|f| {
                f.path
                    .strip_prefix(&prefix)
                    .map(|rel| (rel.to_string(), f.state))
            })
            .collect();
        Some(GitColors { files })
    }

    fn toggle_folder(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if !self.expanded.remove(&dir) {
            self.expanded.insert(dir.clone());
            let workspace = Workspace::model(cx);
            if workspace.read(cx).entries(&dir).is_none() {
                workspace.update(cx, |workspace, cx| workspace.request_dir(dir, cx));
            }
        }
        cx.notify();
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(path.clone());
        crate::open_file_tab(path, window, cx);
        cx.notify();
    }

    fn state_color(state: FileState, cx: &Context<Self>) -> Hsla {
        match state {
            FileState::Modified | FileState::Renamed => cx.theme().warning,
            FileState::Added | FileState::Untracked => cx.theme().success,
            FileState::Deleted | FileState::Conflicted => cx.theme().danger,
        }
    }

    /// Appends `dir`'s rows (and those of its expanded subfolders).
    fn push_rows(
        &self,
        dir: &Path,
        depth: usize,
        root: &Path,
        colors: Option<&GitColors>,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let workspace = Workspace::model(cx);
        let Some(entries) = workspace.read(cx).entries(dir).map(<[_]>::to_vec) else {
            rows.push(
                div()
                    .pl(px(INDENT * depth as f32 + 26.))
                    .py_0p5()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Loading…")
                    .into_any_element(),
            );
            return;
        };
        for entry in entries {
            let rel = relative(&entry.path, root).unwrap_or_default();
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            let state = colors.and_then(|c| {
                if entry.is_dir {
                    c.folder(&rel)
                } else {
                    c.file(&rel)
                }
            });
            let color = state
                .map(|s| Self::state_color(s, cx))
                .unwrap_or(cx.theme().foreground);
            let selected = self.selected.as_ref() == Some(&entry.path);
            let (path, is_dir) = (entry.path.clone(), entry.is_dir);

            rows.push(
                h_flex()
                    .id(SharedString::from(format!(
                        "explorer:{}",
                        entry.path.display()
                    )))
                    .w_full()
                    .gap_1()
                    .pl(px(INDENT * depth as f32 + 6.))
                    .pr_2()
                    .py_0p5()
                    .text_sm()
                    .cursor_pointer()
                    .when(selected, |el| el.bg(cx.theme().list_active))
                    .when(!selected, |el| el.hover(|el| el.bg(cx.theme().list_hover)))
                    .child(
                        div()
                            .w(px(14.))
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .when(is_dir, |el| {
                                el.child(
                                    Icon::new(if expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .xsmall(),
                                )
                            }),
                    )
                    .child(
                        Icon::new(match (is_dir, expanded) {
                            (true, true) => IconName::FolderOpen,
                            (true, false) => IconName::Folder,
                            _ => IconName::File,
                        })
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().truncate().text_color(color).child(entry.name.clone()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if is_dir {
                            this.toggle_folder(path.clone(), cx);
                        } else {
                            this.open_file(path.clone(), window, cx);
                        }
                    }))
                    .into_any_element(),
            );
            if expanded {
                self.push_rows(&entry.path, depth + 1, root, colors, rows, cx);
            }
        }
    }
}

impl Render for ExplorerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = Workspace::model(cx).read(cx).root().map(Path::to_path_buf);
        // A new workspace starts with only its root expanded.
        if self.root != root {
            self.root = root.clone();
            self.expanded = root.iter().cloned().collect();
            self.selected = None;
        }
        let Some(root) = root else {
            return v_flex()
                .size_full()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No folder open. Open one from the project menu in the title bar.")
                .into_any_element();
        };

        let colors = self.git_colors(&root, cx);
        let root_expanded = self.expanded.contains(&root);
        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let root_color = colors
            .as_ref()
            .and_then(|c| c.folder(""))
            .map(|s| Self::state_color(s, cx));
        let root_for_click = root.clone();

        let mut rows =
            vec![
                h_flex()
                    .id("explorer:root")
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .text_sm()
                    .font_semibold()
                    .cursor_pointer()
                    .hover(|el| el.bg(cx.theme().list_hover))
                    .child(
                        Icon::new(if root_expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .truncate()
                            .when_some(root_color, |el, c| el.text_color(c))
                            .child(root_name),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_folder(root_for_click.clone(), cx)
                    }))
                    .into_any_element(),
            ];
        if root_expanded {
            self.push_rows(&root, 1, &root, colors.as_ref(), &mut rows, cx);
        }

        v_flex()
            .id("explorer-tree")
            .size_full()
            .py_1()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}
