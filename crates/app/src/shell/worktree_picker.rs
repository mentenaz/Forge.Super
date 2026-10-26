//! The worktree button in the title bar and its picker, like Zed's: a
//! search box ("Select or type to create a worktree…") with a settings
//! button, a "Create new worktree" row, then the repository's worktrees
//! (current one ticked, `branch · commit · path` underneath).
//! ↑/↓ move the selection, Enter picks it, Escape closes the popover.
//!
//! Dummy data (`dummy::Worktree`) until git support exists.

use std::path::MAIN_SEPARATOR_STR;

use gpui::{
    Anchor, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, h_flex,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
    v_flex,
};

use super::ForgeShell;
use super::dummy::Worktree;
use super::notifications::NoticeKind;

/// What a picker row stands for.
enum WorktreeRow {
    Create,
    Existing(usize),
}

impl ForgeShell {
    fn worktree_label(&self, worktree: &Worktree) -> String {
        worktree.name.as_ref().map(|n| n.to_string()).unwrap_or_else(|| "main".to_string())
    }

    fn worktree_path(&self, worktree: &Worktree) -> String {
        match &worktree.path {
            Some(path) => path.to_string(),
            None => self
                .workspace_root
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    fn worktree_query(&self, cx: &Context<Self>) -> String {
        self.worktree_search.read(cx).value().trim().to_string()
    }

    /// The create row, then the worktrees matching the search box. The
    /// create row is left out when the search names an existing worktree.
    fn worktree_rows(&self, cx: &Context<Self>) -> Vec<WorktreeRow> {
        let query = self.worktree_query(cx).to_lowercase();
        let exists = self.status.worktrees.iter().any(|w| self.worktree_label(w).to_lowercase() == query);
        let matches = self.status.worktrees.iter().enumerate().filter(|(_, w)| {
            query.is_empty()
                || self.worktree_label(w).to_lowercase().contains(&query)
                || w.branch.to_lowercase().contains(&query)
                || self.worktree_path(w).to_lowercase().contains(&query)
        });
        (!exists)
            .then_some(WorktreeRow::Create)
            .into_iter()
            .chain(matches.map(|(ix, _)| WorktreeRow::Existing(ix)))
            .collect()
    }

    fn switch_worktree(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(worktree) = self.status.worktrees.get(ix) {
            self.status.branch = worktree.branch.clone();
            self.status.current_worktree = ix;
        }
        self.worktree_picker_open = false;
        cx.notify();
    }

    /// Creates a worktree on a new branch, based on the current branch, next
    /// to the workspace (`<root>-worktrees\<name>`), and switches to it.
    fn create_worktree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.worktree_query(cx);
        let name = if query.is_empty() {
            format!("worktree-{}", self.status.worktrees.len())
        } else {
            query.replace(char::is_whitespace, "-")
        };
        let base = &self.status.worktrees[self.status.current_worktree];
        let root = self.worktree_path(&self.status.worktrees[0]);
        let worktree = Worktree {
            name: Some(name.clone().into()),
            branch: name.clone().into(),
            commit: base.commit.clone(),
            path: Some(format!("{root}-worktrees{MAIN_SEPARATOR_STR}{name}").into()),
        };
        let based_on = base.branch.clone();
        self.status.worktrees.push(worktree);
        self.switch_worktree(self.status.worktrees.len() - 1, cx);
        self.notify(NoticeKind::Success, "Worktree created", format!("{name}, based on {based_on}."), window, cx);
    }

    fn on_worktree_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.worktree_rows(cx);
        let count = rows.len();
        match event.keystroke.key.as_str() {
            "up" | "down" if count == 0 => {}
            "up" => {
                self.worktree_picker_selected = (self.worktree_picker_selected + count - 1) % count;
                self.worktree_list_scroll.scroll_to_item(self.worktree_picker_selected);
            }
            "down" => {
                self.worktree_picker_selected = (self.worktree_picker_selected + 1) % count;
                self.worktree_list_scroll.scroll_to_item(self.worktree_picker_selected);
            }
            "enter" => match rows.get(self.worktree_picker_selected) {
                Some(WorktreeRow::Create) => self.create_worktree(window, cx),
                Some(WorktreeRow::Existing(ix)) => self.switch_worktree(*ix, cx),
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

    pub(super) fn render_worktree_picker(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = &self.status.worktrees[self.status.current_worktree];
        let label = self.worktree_label(current);

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
            .child(self.render_worktree_list(window, cx))
    }

    fn render_worktree_list(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.worktree_query(cx);
        let base = self.status.worktrees[self.status.current_worktree].branch.clone();

        let rows: Vec<_> = self
            .worktree_rows(cx)
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
                            .child(Icon::new(IconName::Plus).small().text_color(cx.theme().muted_foreground))
                            .child(text)
                            .on_click(cx.listener(|this, _, window, cx| this.create_worktree(window, cx)))
                            .into_any_element()
                    }
                    WorktreeRow::Existing(wt) => {
                        let worktree = &self.status.worktrees[wt];
                        let is_current = wt == self.status.current_worktree;
                        let title = match &worktree.name {
                            None => "main worktree".to_string(),
                            Some(name) => name.to_string(),
                        };
                        let detail = format!("{} · {} · {}", worktree.branch, worktree.commit, self.worktree_path(worktree));
                        el.child(
                            div().w(px(16.)).flex_shrink_0().when(is_current, |el| {
                                el.child(Icon::new(IconName::Check).small().text_color(cx.theme().foreground))
                            }),
                        )
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
                        .on_click(cx.listener(move |this, _, _, cx| this.switch_worktree(wt, cx)))
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
                                    "Where new worktrees are created will be configurable once git support lands.",
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
}
