//! The project button in the title bar and its Recent Projects popover,
//! like Zed's: a search box, the recent projects (name over path, the open
//! one marked, ✕ on hover to forget one), and "Open Local Folder…".
//! ↑/↓ move the selection, Enter opens it, Escape closes the popover.

use std::path::{Path, PathBuf};

use gpui::{
    Anchor, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, PathPromptOptions, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, h_flex,
    button::{Button, ButtonVariants},
    input::Input,
    kbd::Kbd,
    popover::Popover,
    v_flex,
};

use forge_host::{State, Workspace};

use super::ForgeShell;
use super::app_menu::OpenFolder;

fn folder_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

impl ForgeShell {
    /// Make `path` the workspace and move it to the top of the recent list.
    /// Both are remembered for the next run, and panels observing
    /// `forge_host::Workspace` are told.
    pub(super) fn set_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let entry: SharedString = path.to_string_lossy().into_owned().into();
        self.status.recent_projects.retain(|p| *p != entry);
        self.status.recent_projects.insert(0, entry);
        State::set("recent_projects", &self.status.recent_projects, cx);
        State::set("workspace_root", &path, cx);
        let root = Some(path.clone());
        Workspace::model(cx).update(cx, |workspace, cx| workspace.set_root(root, cx));
        self.workspace_root = Some(path);
        self.project_picker_open = false;
        cx.notify();
    }

    pub(super) fn open_folder(&mut self, cx: &mut Context<Self>) {
        self.project_picker_open = false;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.set_workspace(path, cx));
            }
        })
        .detach();
        cx.notify();
    }

    /// The recent projects that match the search box, in list order.
    fn filtered_projects(&self, cx: &Context<Self>) -> Vec<SharedString> {
        let query = self.project_search.read(cx).value().to_lowercase();
        self.status
            .recent_projects
            .iter()
            .filter(|path| query.is_empty() || path.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    fn on_picker_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.filtered_projects(cx).len();
        match event.keystroke.key.as_str() {
            "up" if count > 0 => {
                self.project_picker_selected = (self.project_picker_selected + count - 1) % count;
                self.project_list_scroll.scroll_to_item(self.project_picker_selected);
            }
            "down" if count > 0 => {
                self.project_picker_selected = (self.project_picker_selected + 1) % count;
                self.project_list_scroll.scroll_to_item(self.project_picker_selected);
            }
            "enter" => {
                if let Some(path) = self.filtered_projects(cx).get(self.project_picker_selected) {
                    let path = PathBuf::from(path.as_ref());
                    self.set_workspace(path, cx);
                }
            }
            "escape" => self.project_picker_open = false,
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn open_project_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.project_picker_open = true;
        self.project_picker_selected = 0;
        self.project_search.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn render_project_picker(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_project = self.workspace_root.is_some();
        let label = if has_project { self.workspace_label() } else { "Open Recent Project".to_string() };

        Popover::new("recent-projects")
            .anchor(Anchor::TopLeft)
            .open(self.project_picker_open)
            .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                if *open {
                    this.open_project_picker(window, cx);
                } else {
                    this.project_picker_open = false;
                    cx.notify();
                }
            }))
            .trigger(
                Button::new("project-name")
                    .ghost()
                    .small()
                    .label(label)
                    .when(!has_project, |b| b.text_color(cx.theme().muted_foreground))
                    .tooltip("Recent Projects"),
            )
            .w(px(440.))
            .p_0()
            .child(self.render_recent_projects(window, cx))
    }

    fn render_recent_projects(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.workspace_root.as_ref().map(|p| p.to_string_lossy().into_owned());
        let projects = self.filtered_projects(cx);

        let rows: Vec<_> = projects
            .iter()
            .enumerate()
            .map(|(ix, path)| {
                let is_current = current.as_deref() == Some(path.as_ref());
                let is_selected = ix == self.project_picker_selected;
                let group: SharedString = format!("recent-{ix}").into();
                let open_path = PathBuf::from(path.as_ref());
                let forget = path.clone();

                h_flex()
                    .id(("recent-project", ix))
                    .group(group.clone())
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .when(is_selected, |el| el.bg(cx.theme().list_active))
                    .when(!is_selected, |el| el.hover(|el| el.bg(cx.theme().list_hover)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .text_sm()
                                    .child(div().font_medium().truncate().child(folder_name(path)))
                                    .when(is_current, |el| {
                                        el.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("(current)"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(path.clone()),
                            ),
                    )
                    .when(!is_current, |el| {
                        el.child(
                            div()
                                .id(("forget-project", ix))
                                .invisible()
                                .group_hover(group, |s| s.visible())
                                .p_0p5()
                                .rounded_sm()
                                .text_color(cx.theme().muted_foreground)
                                .hover(|d| d.bg(cx.theme().secondary).text_color(cx.theme().foreground))
                                .child(Icon::new(IconName::Close).xsmall())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.status.recent_projects.retain(|p| *p != forget);
                                    State::set("recent_projects", &this.status.recent_projects, cx);
                                    cx.notify();
                                })),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_workspace(open_path.clone(), cx)))
            })
            .collect();

        let list = if rows.is_empty() {
            div()
                .px_3()
                .py_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No matching projects")
                .into_any_element()
        } else {
            v_flex()
                .id("recent-project-list")
                .p_1()
                .max_h(px(360.))
                .overflow_y_scroll()
                .track_scroll(&self.project_list_scroll)
                .children(rows)
                .into_any_element()
        };

        v_flex()
            // Ahead of the search box, which would otherwise take ↑/↓/Enter.
            .capture_key_down(cx.listener(Self::on_picker_key))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.project_search).appearance(false).cleanable(true)),
            )
            .child(list)
            .child(
                h_flex()
                    .justify_end()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1p5()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("open-local-folder")
                            .ghost()
                            .small()
                            .label("Open Local Folder…")
                            .on_click(cx.listener(|this, _, _, cx| this.open_folder(cx))),
                    )
                    .children(Kbd::binding_for_action(&OpenFolder, None, window)),
            )
    }
}
