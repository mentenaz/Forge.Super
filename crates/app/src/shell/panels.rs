//! The built-in panels, registered through `forge-host` like any feature
//! panel, and the placeholder views that stand in for them until the feature
//! crates provide real ones. A feature crate that registers the same id
//! first (e.g. `node-ui`) replaces the placeholder.
//!
//! The placeholders talk to the shell only through `forge_host::Host`, the
//! same way ported panels will.

use std::rc::Rc;

use forge_host::{DockPosition, Host, PanelInfo, TabRequest, register_panel};
use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::Button,
    h_flex, v_flex,
};

use super::dummy::DUMMY_FILES;

/// A registered panel's id (`PanelInfo::id`).
pub(super) type PanelId = &'static str;

/// The Zed edition's panels, with its icons and default docks.
fn builtin_panels() -> Vec<PanelInfo> {
    let (left, right, bottom) = (DockPosition::Left, DockPosition::Right, DockPosition::Bottom);
    let path = |p: &'static str| Icon::empty().path(p);
    let panel = |id, title, icon, default_dock, order| PanelInfo { id, title, icon, tooltip: None, default_dock, order };
    vec![
        panel("explorer", "Explorer", path("icons/zed_file_tree.svg"), left, 0),
        panel("cockpit", "Cockpit", path("icons/zed_cockpit.svg"), left, 10),
        panel("helm", "Helm", Icon::new(IconName::Github), left, 20),
        panel("python", "Python", path("icons/zed_python.svg"), left, 30),
        panel("node", "Node", path("icons/zed_node.svg"), left, 40),
        panel("dotnet", ".NET", path("icons/zed_dotnet.svg"), left, 50),
        panel("flows", "Flows", Icon::new(IconName::Network), left, 60),
        panel("rust", "Rust", path("icons/zed_file_rust.svg"), left, 70),
        panel("demo", "Demo", path("icons/Demo.svg"), left, 1000),
        panel("git", "Git", path("icons/zed_git_branch.svg"), right, 0),
        panel("outline", "Outline", path("icons/zed_list_tree.svg"), right, 10),
        panel("terminal", "Terminal", path("icons/zed_terminal_alt.svg"), bottom, 0),
        panel("debugger", "Debugger", path("icons/zed_debug.svg"), bottom, 10),
        panel("database", "Database", path("icons/zed_database_zap.svg"), bottom, 20),
        panel("script-runner", "Script Runner", path("icons/zed_runner.svg"), bottom, 30),
        panel("processes", "Processes", Icon::new(IconName::Cpu), bottom, 40),
    ]
}

/// Registers a placeholder for every built-in panel no feature crate has
/// registered yet. Call after the feature crates' `init`.
pub(super) fn register_placeholders(cx: &mut App) {
    for info in builtin_panels() {
        let (id, title, icon) = (info.id, info.title, info.icon.clone());
        register_panel(cx, info, move |_, cx| {
            let icon = icon.clone();
            cx.new(|_| PlaceholderPanel { id, title, icon }).into()
        });
    }
}

/// A placeholder tab for a panel's own content.
fn panel_tab(id: PanelId, title: &'static str, icon: Icon) -> TabRequest {
    TabRequest {
        key: id.into(),
        title: title.into(),
        icon,
        path: None,
        symbol: None,
        dirty: false,
        build: Rc::new(move |_, cx| {
            cx.new(|_| PlaceholderView {
                title: title.into(),
                description: format!("{title} content will live here.").into(),
            })
            .into()
        }),
    }
}

/// A tab for `DUMMY_FILES[index]`; the content stands in for the editor.
fn dummy_file_tab(index: usize) -> Option<TabRequest> {
    let file = DUMMY_FILES.get(index)?;
    let path = file.path;
    Some(TabRequest {
        key: path.into(),
        title: path.rsplit('/').next().unwrap_or(path).into(),
        icon: Icon::new(IconName::File),
        path: Some(path.into()),
        symbol: file.symbol.map(Into::into),
        dirty: file.dirty,
        build: Rc::new(move |_, cx| {
            cx.new(|_| PlaceholderView {
                title: path.into(),
                description: "The editor will be embedded here.".into(),
            })
            .into()
        }),
    })
}

/// Stand-in for a dock panel. The Explorer one lists dummy files so the tab
/// bar can be tried out.
pub(super) struct PlaceholderPanel {
    id: PanelId,
    title: &'static str,
    icon: Icon,
}

impl Render for PlaceholderPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title;

        let body = if self.id == "explorer" {
            let files = DUMMY_FILES.iter().enumerate().map(|(index, file)| {
                h_flex()
                    .id(("dummy-file", index))
                    .gap_1p5()
                    .px_2()
                    .py_0p5()
                    .rounded(cx.theme().radius)
                    .text_sm()
                    .cursor_pointer()
                    .hover(|el| el.bg(cx.theme().list_hover))
                    .child(Icon::new(IconName::File).xsmall().text_color(cx.theme().muted_foreground))
                    .child(div().truncate().child(file.path))
                    .on_click(move |_, window, cx| {
                        if let Some(tab) = dummy_file_tab(index) {
                            Host::open_tab(tab, window, cx);
                        }
                    })
            });
            v_flex().gap_0p5().children(files).into_any_element()
        } else {
            let (id, icon) = (self.id, self.icon.clone());
            v_flex()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("The {title} panel will live here.")),
                )
                .child(
                    Button::new("open-panel-tab")
                        .outline()
                        .label(format!("Open {title} tab"))
                        .on_click(move |_, window, cx| Host::open_tab(panel_tab(id, title, icon.clone()), window, cx)),
                )
                .into_any_element()
        };

        v_flex()
            .size_full()
            .p_3()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(title.to_uppercase()),
            )
            .child(body)
    }
}

/// Stand-in for tab content.
pub(super) struct PlaceholderView {
    pub(super) title: SharedString,
    pub(super) description: SharedString,
}

impl Render for PlaceholderView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_1()
            .bg(cx.theme().background)
            .child(div().text_lg().font_semibold().child(self.title.clone()))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.description.clone()),
            )
    }
}
