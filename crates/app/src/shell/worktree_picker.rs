//! The worktree button in the title bar and its picker, like Zed's: a
//! search box ("Select or type to create a worktree…") with a settings
//! button, a "Create new worktree" row, then the repository's worktrees
//! (current one ticked, `branch · commit · path` underneath).
//! ↑/↓ move the selection, Enter picks it, Escape closes the popover.
//!
//! Real git data from `forge_host::Workspace`: picking a worktree opens its
//! folder as the workspace; creating one runs `git worktree add` next to the
//! repository (`<repo>-worktrees\<name>`) and opens it.

use std::path::{Path, PathBuf};

use forge_host::{GitState, NoticeKind, Workspace, Worktree};
use gpui::{
    Anchor, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    menu::DropdownMenu,
    popover::Popover,
    v_flex,
};

use super::ForgeShell;
use super::git_actions::report_git;

/// What a picker row stands for.
enum WorktreeRow {
    Create,
    Existing(usize),
}

/// Git reports paths with `/`; compare them independent of separators and
/// case (Windows paths are case-insensitive).
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn worktree_label(worktree: &Worktree) -> String {
    if worktree.is_main {
        "main".to_string()
    } else {
        worktree
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

fn current_worktree(git: &GitState) -> Option<usize> {
    git.worktrees
        .iter()
        .position(|w| same_path(&w.path, &git.root))
}

impl ForgeShell {
    fn worktree_query(&self, cx: &Context<Self>) -> String {
        self.worktree_search.read(cx).value().trim().to_string()
    }

    /// The create row, then the worktrees matching the search box. The
    /// create row is left out when the search names an existing worktree.
    fn worktree_rows(&self, git: &GitState, cx: &Context<Self>) -> Vec<WorktreeRow> {
        let query = self.worktree_query(cx).to_lowercase();
        let exists = git
            .worktrees
            .iter()
            .any(|w| worktree_label(w).to_lowercase() == query);
        let matches = git.worktrees.iter().enumerate().filter(|(_, w)| {
            query.is_empty()
                || worktree_label(w).to_lowercase().contains(&query)
                || w.branch
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
                || w.path.to_string_lossy().to_lowercase().contains(&query)
        });
        (!exists)
            .then_some(WorktreeRow::Create)
            .into_iter()
            .chain(matches.map(|(ix, _)| WorktreeRow::Existing(ix)))
            .collect()
    }

    fn git_state(&self, cx: &Context<Self>) -> Option<GitState> {
        Workspace::model(cx).read(cx).git().cloned()
    }

    /// Opens the worktree's folder as the workspace.
    fn switch_worktree(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.worktree_picker_open = false;
        self.set_workspace(path, cx);
    }

    /// Creates a worktree on a new branch, based on the current branch,
    /// next to the main worktree (`<repo>-worktrees\<name>`), then opens it.
    fn create_worktree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(git) = self.git_state(cx) else {
            return;
        };
        let Some(main) = git.worktrees.iter().find(|w| w.is_main) else {
            return;
        };
        let query = self.worktree_query(cx);
        let name = if query.is_empty() {
            format!("worktree-{}", git.worktrees.len())
        } else {
            query.replace(char::is_whitespace, "-")
        };
        let base = git.branch.clone().unwrap_or_else(|| git.head.clone());
        let folder = format!(
            "{}-worktrees",
            main.path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        let path = main
            .path
            .parent()
            .unwrap_or(&main.path)
            .join(folder)
            .join(&name);
        self.worktree_picker_open = false;

        let task = Workspace::model(cx).update(cx, |workspace, cx| {
            workspace.add_worktree(path.clone(), name.clone(), base.clone(), cx)
        });
        let this = cx.weak_entity();
        window
            .spawn(cx, async move |cx| match task.await {
                Ok(()) => {
                    let _ = cx.update(|window, cx| {
                        let _ = this.update(cx, |shell, cx| {
                            shell.notify(
                                NoticeKind::Success,
                                "Worktree created",
                                format!("{name}, based on {base}."),
                                window,
                                cx,
                            );
                            shell.set_workspace(path, cx);
                        });
                    });
                }
                Err(err) => {
                    let _ = cx.update(|window, cx| {
                        let _ = this.update(cx, |shell, cx| {
                            shell.notify(
                                NoticeKind::Warning,
                                "Couldn't create the worktree",
                                err.0,
                                window,
                                cx,
                            )
                        });
                    });
                }
            })
            .detach();
        cx.notify();
    }

    fn on_worktree_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(git) = self.git_state(cx) else {
            return;
        };
        let rows = self.worktree_rows(&git, cx);
        let count = rows.len();
        match event.keystroke.key.as_str() {
            "up" | "down" if count == 0 => {}
            "up" => {
                self.worktree_picker_selected = (self.worktree_picker_selected + count - 1) % count;
                self.worktree_list_scroll
                    .scroll_to_item(self.worktree_picker_selected);
            }
            "down" => {
                self.worktree_picker_selected = (self.worktree_picker_selected + 1) % count;
                self.worktree_list_scroll
                    .scroll_to_item(self.worktree_picker_selected);
            }
            "enter" => match rows.get(self.worktree_picker_selected) {
                Some(WorktreeRow::Create) => self.create_worktree(window, cx),
                Some(WorktreeRow::Existing(ix)) => {
                    self.switch_worktree(git.worktrees[*ix].path.clone(), cx)
                }
                None => {}
            },
            "escape" => self.worktree_picker_open = false,
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn open_worktree_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.worktree_picker_open = true;
        self.worktree_picker_selected = 0;
        self.worktree_search.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    /// The worktree button; `None` outside a git repository.
    pub(super) fn render_worktree_picker(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let git = self.git_state(cx)?;
        let label = current_worktree(&git)
            .map(|ix| worktree_label(&git.worktrees[ix]))
            .unwrap_or_else(|| "main".into());

        Some(
            Popover::new("worktree-picker")
                .anchor(Anchor::TopLeft)
                .open(self.worktree_picker_open)
                .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                    if *open {
                        this.open_worktree_picker(window, cx);
                    } else {
                        this.worktree_picker_open = false;
                        cx.notify();
                    }
                }))
                .trigger(
                    Button::new("worktree-picker-trigger")
                        .ghost()
                        .small()
                        .icon(Icon::empty().path("icons/zed_git_worktree.svg"))
                        .label(label.clone())
                        .text_color(cx.theme().muted_foreground)
                        .tooltip(format!("Worktree · Currently In Use: {label}")),
                )
                .w(px(460.))
                .p_0()
                .child(self.render_worktree_list(&git, window, cx)),
        )
    }

    fn render_worktree_list(
        &self,
        git: &GitState,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let query = self.worktree_query(cx);
        let base = git.branch.clone().unwrap_or_else(|| git.head.clone());
        let current = current_worktree(git);

        let rows: Vec<_> = self
            .worktree_rows(git, cx)
            .into_iter()
            .enumerate()
            .map(|(ix, row)| {
                let selected = ix == self.worktree_picker_selected;
                let el = h_flex()
                    .id(("worktree-row", ix))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .when(selected, |el| el.bg(cx.theme().list_active))
                    .when(!selected, |el| el.hover(|el| el.bg(cx.theme().list_hover)));
                match row {
                    WorktreeRow::Create => {
                        let text = if query.is_empty() {
                            format!("Create new worktree based on {base}")
                        } else {
                            format!("Create worktree \"{query}\" based on {base}")
                        };
                        el.text_sm()
                            .child(
                                Icon::new(IconName::Plus)
                                    .small()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(text)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.create_worktree(window, cx)),
                            )
                            .into_any_element()
                    }
                    WorktreeRow::Existing(wt) => {
                        let worktree = &git.worktrees[wt];
                        let is_current = current == Some(wt);
                        let title = if worktree.is_main {
                            "main worktree".to_string()
                        } else {
                            worktree_label(worktree)
                        };
                        let branch = worktree.branch.clone().unwrap_or_else(|| "detached".into());
                        let detail =
                            format!("{branch} · {} · {}", worktree.head, worktree.path.display());
                        let path = worktree.path.clone();
                        el.child(div().w(px(16.)).flex_shrink_0().when(is_current, |el| {
                            el.child(
                                Icon::new(IconName::Check)
                                    .small()
                                    .text_color(cx.theme().foreground),
                            )
                        }))
                        .child(
                            v_flex()
                                .min_w_0()
                                .child(div().text_sm().font_medium().truncate().child(title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(detail),
                                ),
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.switch_worktree(path.clone(), cx)
                            }),
                        )
                        .into_any_element()
                    }
                }
            })
            .collect();

        v_flex()
            // Ahead of the search box, which would otherwise take ↑/↓/Enter.
            .capture_key_down(cx.listener(Self::on_worktree_key))
            .child(
                h_flex()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().child(Input::new(&self.worktree_search).appearance(false).cleanable(true)))
                    .child(
                        Button::new("worktree-settings")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings)
                            .tooltip("Worktree Settings")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.notify(
                                    NoticeKind::Info,
                                    "Worktree settings",
                                    "New worktrees go next to the repository, in <repo>-worktrees. Choosing the location comes with the settings UI.",
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("worktree-list")
                    .p_1()
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .track_scroll(&self.worktree_list_scroll)
                    .children(rows),
            )
    }

    /// Placeholder text for the worktree search box.
    pub(super) fn worktree_placeholder() -> SharedString {
        "Select or type to create a worktree…".into()
    }

    /// The branch picker: switches with `git switch`; `None` outside a git
    /// repository.
    pub(super) fn render_branch_picker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let git = self.git_state(cx)?;
        let label: SharedString = git
            .branch
            .clone()
            .unwrap_or_else(|| format!("detached at {}", git.head))
            .into();
        let current = git.branch.clone();
        let branches = git.branches.clone();

        Some(
            Button::new("branch-picker")
                .ghost()
                .small()
                .icon(Icon::empty().path("icons/git-branch.svg"))
                .label(label.clone())
                .text_color(cx.theme().muted_foreground)
                .tooltip(format!("Branch · Currently Checked Out: {label}"))
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu.scrollable(true).max_h(px(400.)).label("Branches");
                    for branch in &branches {
                        let name = branch.clone();
                        let checked = current.as_deref() == Some(branch.as_str());
                        menu = menu.item(
                            gpui_component::menu::PopupMenuItem::new(branch.clone())
                                .checked(checked)
                                .on_click(move |_, window, cx| {
                                    let name = name.clone();
                                    let task = Workspace::model(cx).update(cx, |workspace, cx| {
                                        workspace.switch_branch(name, cx)
                                    });
                                    report_git(task, "Couldn't switch branch", None, window, cx);
                                }),
                        );
                    }
                    menu
                }),
        )
    }
}
