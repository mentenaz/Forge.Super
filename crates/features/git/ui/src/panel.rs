//! The Git panel, laid out like Zed's: the branch, a commit message with a
//! Commit button, then Merge Conflicts, Staged Changes and Changes. Each
//! file shows a coloured status letter; hovering offers stage/unstage, and
//! clicking opens the file.

use forge_host::{FileState, FileStatus, GitError, Host, NoticeKind, Workspace};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};

pub struct GitPanel {
    message: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

fn letter(state: FileState) -> &'static str {
    match state {
        FileState::Modified => "M",
        FileState::Added => "A",
        FileState::Deleted => "D",
        FileState::Renamed => "R",
        FileState::Untracked => "U",
        FileState::Conflicted => "!",
    }
}

fn color(state: FileState, cx: &App) -> Hsla {
    match state {
        FileState::Modified | FileState::Renamed => cx.theme().warning,
        FileState::Added | FileState::Untracked => cx.theme().success,
        FileState::Deleted | FileState::Conflicted => cx.theme().danger,
    }
}

/// Reports a failed git operation with git's message.
fn report(
    task: Task<Result<(), GitError>>,
    failure: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    window
        .spawn(cx, async move |cx| {
            if let Err(err) = task.await {
                let _ = cx.update(|window, cx| {
                    Host::notify(NoticeKind::Warning, failure, err.0, window, cx)
                });
            }
        })
        .detach();
}

impl GitPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx
            .new(|cx| InputState::new(window, cx).placeholder("Commit message (Enter to commit)"));
        let subscriptions = vec![
            cx.observe(&Workspace::model(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &message,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => this.commit(window, cx),
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            ),
        ];
        Self {
            message,
            _subscriptions: subscriptions,
        }
    }

    fn stage(paths: Vec<String>, window: &mut Window, cx: &mut App) {
        let task = Workspace::model(cx).update(cx, |workspace, cx| workspace.stage(paths, cx));
        report(task, "Couldn't stage", window, cx);
    }

    fn unstage(paths: Vec<String>, window: &mut Window, cx: &mut App) {
        let task = Workspace::model(cx).update(cx, |workspace, cx| workspace.unstage(paths, cx));
        report(task, "Couldn't unstage", window, cx);
    }

    fn can_commit(&self, cx: &App) -> bool {
        let staged = Workspace::model(cx)
            .read(cx)
            .git()
            .is_some_and(|git| git.status.iter().any(|f| f.staged));
        staged && !self.message.read(cx).value().trim().is_empty()
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_commit(cx) {
            return;
        }
        let message = self.message.read(cx).value().trim().to_string();
        let task =
            Workspace::model(cx).update(cx, |workspace, cx| workspace.commit(message.clone(), cx));
        let input = self.message.clone();
        window
            .spawn(cx, async move |cx| {
                let result = task.await;
                let _ = cx.update(|window, cx| match result {
                    Ok(()) => {
                        input.update(cx, |input, cx| input.set_value("", window, cx));
                        let first_line = message.lines().next().unwrap_or_default().to_string();
                        Host::notify(NoticeKind::Success, "Committed", first_line, window, cx);
                    }
                    Err(err) => {
                        Host::notify(NoticeKind::Warning, "Couldn't commit", err.0, window, cx)
                    }
                });
            })
            .detach();
    }

    fn render_section(
        &self,
        title: &'static str,
        files: Vec<FileStatus>,
        staged: bool,
        root: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if files.is_empty() {
            return None;
        }
        let all: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        let (bulk_label, bulk_id) = if staged {
            ("Unstage All", "unstage-all")
        } else {
            ("Stage All", "stage-all")
        };
        let is_conflicts = title == "Merge Conflicts";

        let header = h_flex()
            .justify_between()
            .px_3()
            .pt_2()
            .pb_1()
            .text_xs()
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{} ({})", title.to_uppercase(), files.len()))
            .when(!is_conflicts, |el| {
                el.child(
                    Button::new(bulk_id)
                        .ghost()
                        .xsmall()
                        .label(bulk_label)
                        .on_click(move |_, window, cx| {
                            if staged {
                                Self::unstage(all.clone(), window, cx);
                            } else {
                                Self::stage(all.clone(), window, cx);
                            }
                        }),
                )
            });

        let rows = files.into_iter().map(|file| {
            let (dir, name) = match file.path.rsplit_once('/') {
                Some((dir, name)) => (dir.to_string(), name.to_string()),
                None => (String::new(), file.path.clone()),
            };
            let group: SharedString = format!("git-{title}-{}", file.path).into();
            let abs = root.join(&file.path);
            let path = file.path.clone();
            h_flex()
                .id(group.clone())
                .group(group.clone())
                .gap_2()
                .px_3()
                .py_0p5()
                .text_sm()
                .cursor_pointer()
                .hover(|el| el.bg(cx.theme().list_hover))
                .child(
                    div()
                        .w(px(12.))
                        .flex_shrink_0()
                        .font_semibold()
                        .text_xs()
                        .text_color(color(file.state, cx))
                        .child(letter(file.state)),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .child(div().truncate().child(name))
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(dir),
                        ),
                )
                .child(
                    div().invisible().group_hover(group, |s| s.visible()).child(
                        Button::new(SharedString::from(format!("toggle-{title}-{path}")))
                            .ghost()
                            .xsmall()
                            .icon(if staged {
                                IconName::Minus
                            } else {
                                IconName::Plus
                            })
                            .tooltip(if staged { "Unstage" } else { "Stage" })
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                if staged {
                                    Self::unstage(vec![path.clone()], window, cx);
                                } else {
                                    Self::stage(vec![path.clone()], window, cx);
                                }
                            }),
                    ),
                )
                .on_click(move |_, window, cx| Host::open_file(abs.clone(), window, cx))
        });

        Some(v_flex().child(header).children(rows).into_any_element())
    }
}

impl Render for GitPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(git) = Workspace::model(cx).read(cx).git().cloned() else {
            return v_flex()
                .size_full()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("This folder isn't a git repository.")
                .into_any_element();
        };

        let conflicts: Vec<_> = git
            .status
            .iter()
            .filter(|f| f.state == FileState::Conflicted)
            .cloned()
            .collect();
        let staged: Vec<_> = git.status.iter().filter(|f| f.staged).cloned().collect();
        let unstaged: Vec<_> = git
            .status
            .iter()
            .filter(|f| f.unstaged && f.state != FileState::Conflicted)
            .cloned()
            .collect();
        let branch = git
            .branch
            .clone()
            .unwrap_or_else(|| format!("detached at {}", git.head));
        let can_commit = self.can_commit(cx);
        let total = git.status.len();

        let sections: Vec<AnyElement> = [
            self.render_section("Merge Conflicts", conflicts, false, &git.root, cx),
            self.render_section("Staged Changes", staged, true, &git.root, cx),
            self.render_section("Changes", unstaged, false, &git.root, cx),
        ]
        .into_iter()
        .flatten()
        .collect();

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .gap_1p5()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Icon::empty()
                            .path("icons/git-branch.svg")
                            .small()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().font_semibold().truncate().child(branch))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if total == 0 {
                                "No changes".to_string()
                            } else {
                                format!("{total} changed")
                            }),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.message))
                    .child(
                        Button::new("commit")
                            .primary()
                            .small()
                            .w_full()
                            .label("Commit")
                            .disabled(!can_commit)
                            .tooltip("Commit the staged changes")
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            )
            .child(
                v_flex()
                    .id("git-changes")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(sections)
                    .when(total == 0, |el| {
                        el.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Working tree clean."),
                        )
                    }),
            )
            .into_any_element()
    }
}
