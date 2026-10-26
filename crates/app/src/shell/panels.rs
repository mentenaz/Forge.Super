//! The panels the shell's docks can hold, and the placeholder views that
//! stand in for them until the feature crates (`node-ui`, `dotnet-ui`, ...)
//! provide real ones.

use gpui::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::Button,
    h_flex, v_flex,
};

use super::dummy::DUMMY_FILES;

/// The three docks around the tab area, as in Zed: left and right run the
/// full height between the title bar and status bar, bottom sits under the
/// tab area only.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DockPosition {
    Left,
    Right,
    Bottom,
}

impl DockPosition {
    pub(super) const ALL: [DockPosition; 3] =
        [DockPosition::Left, DockPosition::Right, DockPosition::Bottom];

    pub(super) fn title(self) -> &'static str {
        match self {
            DockPosition::Left => "Left",
            DockPosition::Right => "Right",
            DockPosition::Bottom => "Bottom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelId {
    Explorer,
    Cockpit,
    Helm,
    Python,
    Node,
    DotNet,
    Flows,
    Rust,
    Demo,
    Git,
    Outline,
    Terminal,
    Debugger,
    Database,
    ScriptRunner,
    Processes,
}

impl PanelId {
    /// Every panel, in status-bar button order within each dock.
    pub(super) const ALL: [PanelId; 16] = [
        PanelId::Explorer,
        PanelId::Cockpit,
        PanelId::Helm,
        PanelId::Python,
        PanelId::Node,
        PanelId::DotNet,
        PanelId::Flows,
        PanelId::Rust,
        PanelId::Demo,
        PanelId::Git,
        PanelId::Outline,
        PanelId::Terminal,
        PanelId::Debugger,
        PanelId::Database,
        PanelId::ScriptRunner,
        PanelId::Processes,
    ];

    /// Where the panel starts out; the user can move it to another dock.
    /// Will come from settings.
    pub(super) fn default_position(self) -> DockPosition {
        match self {
            PanelId::Git | PanelId::Outline => DockPosition::Right,
            PanelId::Terminal
            | PanelId::Debugger
            | PanelId::Database
            | PanelId::ScriptRunner
            | PanelId::Processes => DockPosition::Bottom,
            _ => DockPosition::Left,
        }
    }

    /// Stable string id, used for tab keys.
    pub(super) fn slug(self) -> &'static str {
        match self {
            PanelId::Explorer => "explorer",
            PanelId::Cockpit => "cockpit",
            PanelId::Helm => "helm",
            PanelId::Python => "python",
            PanelId::Node => "node",
            PanelId::DotNet => "dotnet",
            PanelId::Flows => "flows",
            PanelId::Rust => "rust",
            PanelId::Demo => "demo",
            PanelId::Git => "git",
            PanelId::Outline => "outline",
            PanelId::Terminal => "terminal",
            PanelId::Debugger => "debugger",
            PanelId::Database => "database",
            PanelId::ScriptRunner => "script-runner",
            PanelId::Processes => "processes",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            PanelId::Explorer => "Explorer",
            PanelId::Cockpit => "Cockpit",
            PanelId::Helm => "Helm",
            PanelId::Python => "Python",
            PanelId::Node => "Node",
            PanelId::DotNet => ".NET",
            PanelId::Flows => "Flows",
            PanelId::Rust => "Rust",
            PanelId::Demo => "Demo",
            PanelId::Git => "Git",
            PanelId::Outline => "Outline",
            PanelId::Terminal => "Terminal",
            PanelId::Debugger => "Debugger",
            PanelId::Database => "Database",
            PanelId::ScriptRunner => "Script Runner",
            PanelId::Processes => "Processes",
        }
    }

    /// The Zed edition's icon for each panel.
    pub(super) fn icon(self) -> Icon {
        let path = |p: &'static str| Icon::empty().path(p);
        match self {
            PanelId::Explorer => path("icons/zed_file_tree.svg"),
            PanelId::Cockpit => path("icons/zed_cockpit.svg"),
            PanelId::Helm => Icon::new(IconName::Github),
            PanelId::Python => path("icons/zed_python.svg"),
            PanelId::Node => path("icons/zed_node.svg"),
            PanelId::DotNet => path("icons/zed_dotnet.svg"),
            PanelId::Flows => Icon::new(IconName::Network),
            PanelId::Rust => path("icons/zed_file_rust.svg"),
            PanelId::Demo => path("icons/Demo.svg"),
            PanelId::Git => path("icons/zed_git_branch.svg"),
            PanelId::Outline => path("icons/zed_list_tree.svg"),
            PanelId::Terminal => path("icons/zed_terminal_alt.svg"),
            PanelId::Debugger => path("icons/zed_debug.svg"),
            PanelId::Database => path("icons/zed_database_zap.svg"),
            PanelId::ScriptRunner => path("icons/zed_runner.svg"),
            PanelId::Processes => Icon::new(IconName::Cpu),
        }
    }
}

/// What a panel asks the shell to do.
pub(super) enum PanelEvent {
    /// Open (or focus) this panel's own tab.
    OpenTab,
    /// Open `DUMMY_FILES[index]` (dummy Explorer only).
    OpenFile(usize),
}

/// Stand-in for a dock panel. The Explorer one lists dummy files so the tab
/// bar can be tried out.
pub(super) struct PlaceholderPanel {
    pub(super) panel: PanelId,
}

impl EventEmitter<PanelEvent> for PlaceholderPanel {}

impl Render for PlaceholderPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.panel.title();

        let body = if self.panel == PanelId::Explorer {
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
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(PanelEvent::OpenFile(index))))
            });
            v_flex().gap_0p5().children(files).into_any_element()
        } else {
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
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PanelEvent::OpenTab))),
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
