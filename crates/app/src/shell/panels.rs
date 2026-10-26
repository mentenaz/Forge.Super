//! The built-in panels, registered through `forge-host` like any feature
//! panel, and the placeholder views that stand in for them until the feature
//! crates provide real ones. A feature crate that registers the same id
//! first (e.g. `node-ui`) replaces the placeholder.
//!
//! The Explorer and Git panels are real (`explorer-ui`, `git-ui`); their
//! entries here only matter if those crates aren't registered. The
//! placeholders talk to the shell only through `forge_host::Host`, the same
//! way ported panels will.

use std::rc::Rc;

use forge_host::{DockPosition, Host, PanelInfo, TabRequest, register_panel};
use gpui::{
    App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
};
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt, button::Button, v_flex};

/// A registered panel's id (`PanelInfo::id`).
pub(super) type PanelId = &'static str;

/// The Zed edition's panels, with its icons and default docks.
fn builtin_panels() -> Vec<PanelInfo> {
    let (left, right, bottom) = (
        DockPosition::Left,
        DockPosition::Right,
        DockPosition::Bottom,
    );
    let path = |p: &'static str| Icon::empty().path(p);
    let panel = |id, title, icon, default_dock, order| PanelInfo {
        id,
        title,
        icon,
        tooltip: None,
        default_dock,
        order,
    };
    vec![
        panel(
            "explorer",
            "Explorer",
            path("icons/zed_file_tree.svg"),
            left,
            0,
        ),
        panel(
            "cockpit",
            "Cockpit",
            path("icons/zed_cockpit.svg"),
            left,
            10,
        ),
        panel("helm", "Helm", Icon::new(IconName::Github), left, 20),
        panel("python", "Python", path("icons/zed_python.svg"), left, 30),
        panel("node", "Node", path("icons/zed_node.svg"), left, 40),
        panel("dotnet", ".NET", path("icons/zed_dotnet.svg"), left, 50),
        panel("flows", "Flows", Icon::new(IconName::Network), left, 60),
        panel("rust", "Rust", path("icons/zed_file_rust.svg"), left, 70),
        panel("demo", "Demo", path("icons/Demo.svg"), left, 1000),
        panel("git", "Git", path("icons/zed_git_branch.svg"), right, 0),
        panel(
            "outline",
            "Outline",
            path("icons/zed_list_tree.svg"),
            right,
            10,
        ),
        panel(
            "terminal",
            "Terminal",
            path("icons/zed_terminal_alt.svg"),
            bottom,
            0,
        ),
        panel(
            "debugger",
            "Debugger",
            path("icons/zed_debug.svg"),
            bottom,
            10,
        ),
        panel(
            "database",
            "Database",
            path("icons/zed_database_zap.svg"),
            bottom,
            20,
        ),
        panel(
            "script-runner",
            "Script Runner",
            path("icons/zed_runner.svg"),
            bottom,
            30,
        ),
        panel(
            "processes",
            "Processes",
            Icon::new(IconName::Cpu),
            bottom,
            40,
        ),
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

/// Stand-in for a dock panel.
pub(super) struct PlaceholderPanel {
    id: PanelId,
    title: &'static str,
    icon: Icon,
}

impl Render for PlaceholderPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, title, icon) = (self.id, self.title, self.icon.clone());

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
                    .on_click(move |_, window, cx| {
                        Host::open_tab(panel_tab(id, title, icon.clone()), window, cx)
                    }),
            )
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
