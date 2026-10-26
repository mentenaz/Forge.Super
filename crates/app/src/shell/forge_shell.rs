//! Top-level shell layout, following Zed's dock model:
//!
//! ```text
//! ┌────────────────────────── title bar ──────────────────────────┐
//! │ left dock │ tab bar (primary | split)           │ right dock  │
//! │           │ tab content (primary | split)       │             │
//! │           ├─────────────────────────────────────┤             │
//! │           │ bottom dock                         │             │
//! └────────────────────────── status bar ─────────────────────────┘
//! ```
//!
//! Each dock holds any number of panels and shows one at a time (see
//! `docks.rs`). Panels come from `forge_host::PanelRegistry`: feature crates
//! register theirs, and `panels.rs` registers placeholders for the rest.
//!
//! Between runs the shell restores the workspace, recent projects, dock
//! layout, theme, font sizes and menu bar from `forge-host`'s settings and
//! state.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use forge_host::{DockPosition, Host, NoticeKind, Settings, State, Workspace};

use gpui::{
    Anchor, AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Pixels, Point, Render, ScrollHandle, Styled, Task, Window, actions,
    SharedString, div, px,
};
use gpui_component::{
    Root, Theme,
    input::{InputEvent, InputState},
    menu::AppMenuBar,
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
    v_flex,
};

use super::docks::{self, Dock, PanelEntry};
use super::dummy::{self, ShellStatus};
use super::host_delegate::ShellHost;
use super::notifications::NoticeEntry;
use super::tab_switcher::TabSwitcherState;
use super::tabs::{TabGroup, WorkspaceTab};

actions!(
    forge_shell,
    [
        SplitActiveTab,
        SwapPanes,
        CycleTabMru,
        JumpToLruTab,
        ToggleLeftDock,
        ToggleRightDock,
        ToggleBottomDock,
        GoBack,
        GoForward,
        ToggleZoom,
        DeploySearch
    ]
);

/// Call after `forge_host::init` and the feature crates' `init`, so their
/// panels are registered before the placeholders fill the gaps.
pub fn init(cx: &mut App) {
    super::theme::init(cx);
    super::app_menu::init(cx);
    super::panels::register_placeholders(cx);
    cx.on_app_quit(|cx| {
        State::flush(cx);
        async {}
    })
    .detach();

    // Toasts appear bottom-right, just above the status bar and its bell.
    let notification = &mut Theme::global_mut(cx).notification;
    notification.placement = Anchor::BottomRight;
    notification.margins.bottom = px(44.);

    // Ctrl+\ arrives as "ctrl-\" or "ctrl-|" depending on keyboard layout, so
    // both are bound. On Windows Shift is folded into the character, so
    // "ctrl-shift-|" would never match. Dock toggles use Zed's bindings.
    cx.bind_keys([
        KeyBinding::new("ctrl-\\", SplitActiveTab, None),
        KeyBinding::new("ctrl-|", SplitActiveTab, None),
        KeyBinding::new("ctrl-]", SwapPanes, None),
        KeyBinding::new("ctrl-tab", CycleTabMru, None),
        KeyBinding::new("ctrl-shift-tab", JumpToLruTab, None),
        KeyBinding::new("ctrl-b", ToggleLeftDock, None),
        KeyBinding::new("ctrl-alt-b", ToggleRightDock, None),
        KeyBinding::new("ctrl-j", ToggleBottomDock, None),
        KeyBinding::new("alt-left", GoBack, None),
        KeyBinding::new("alt-right", GoForward, None),
        KeyBinding::new("shift-escape", ToggleZoom, None),
        KeyBinding::new("ctrl-shift-f", DeploySearch, None),
    ]);
}

pub struct ForgeShell {
    /// Mirrored into `forge_host::Workspace` (see `set_workspace`).
    pub(super) workspace_root: Option<PathBuf>,
    /// Every registered panel with the dock it is in, in button order.
    pub(super) panels: Vec<PanelEntry>,
    /// Indexed by `DockPosition as usize`.
    pub(super) docks: [Dock; 3],
    pub(super) tabs: Vec<WorkspaceTab>,
    pub(super) active_tab: Option<usize>,
    /// Tabs in the split pane beside the primary group; empty when closed.
    pub(super) split_tabs: Vec<WorkspaceTab>,
    pub(super) split_active_tab: Option<usize>,
    /// The pane the user last worked in; new tabs open here.
    pub(super) active_group: TabGroup,
    pub(super) next_tab_id: usize,
    /// Back/forward history of tab ids (see `nav_history.rs`).
    pub(super) nav_back: Vec<usize>,
    pub(super) nav_forward: Vec<usize>,
    pub(super) last_seen_nav_tab: Option<usize>,
    pub(super) suppress_nav_record: bool,
    /// The tab area fills the window, docks hidden (Zed's zoom).
    pub(super) zoomed: bool,
    /// Tab ids, most recently active first (see `sync_tab_mru`).
    pub(super) tab_mru: VecDeque<usize>,
    pub(super) last_seen_active_tab_id: Option<usize>,
    pub(super) tab_switcher: Option<TabSwitcherState>,
    pub(super) tab_switcher_commit_task: Option<Task<()>>,
    /// Right-clicked tab's group, index and click position (`None` = closed).
    pub(super) tab_context_menu: Option<(TabGroup, usize, Point<Pixels>)>,
    /// Stand-in for git/LSP/editor state until the host API exists.
    pub(super) status: ShellStatus,
    /// Notification centre history, newest first.
    pub(super) notices: Vec<NoticeEntry>,
    pub(super) notifications_open: bool,
    /// Zed's `show_menus`: a menu bar row below the title bar instead of ☰.
    pub(super) show_menus: bool,
    pub(super) app_menu_bar: Entity<AppMenuBar>,
    /// The Recent Projects popover and its search box.
    pub(super) project_picker_open: bool,
    pub(super) project_search: Entity<InputState>,
    /// Index into the filtered list, moved with ↑/↓.
    pub(super) project_picker_selected: usize,
    pub(super) project_list_scroll: ScrollHandle,
    /// The worktree picker (see `worktree_picker.rs`).
    pub(super) worktree_picker_open: bool,
    pub(super) worktree_search: Entity<InputState>,
    pub(super) worktree_picker_selected: usize,
    pub(super) worktree_list_scroll: ScrollHandle,
    /// Focused from startup so the shell's key bindings work before the
    /// user clicks anything; menu actions are dispatched to it.
    pub(super) focus_handle: FocusHandle,
}

impl ForgeShell {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let shell = cx.new(|cx| Self::new(window, cx));
        shell.read(cx).focus_handle.clone().focus(window, cx);
        Host::set_delegate(Rc::new(ShellHost(shell.downgrade())), cx);

        // A settings or state file that couldn't be read is reported, not
        // fatal: the defaults are used and the bad file is kept aside.
        // Deferred: toasts need the window's `Root`, which wraps the shell
        // only after this returns.
        let errors = [Settings::load_error(cx), State::load_error(cx)];
        let weak = shell.downgrade();
        window.defer(cx, move |window, cx| {
            let _ = weak.update(cx, |shell, cx| {
                for error in errors.into_iter().flatten() {
                    shell.notify(NoticeKind::Warning, "Couldn't read a settings file", error, window, cx);
                }
            });
        });
        shell
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let panels = docks::build_panels(window, cx);
        let docks = [
            docks::load_dock(DockPosition::Left, &panels, true, Some("explorer"), cx),
            docks::load_dock(DockPosition::Right, &panels, false, Some("git"), cx),
            docks::load_dock(DockPosition::Bottom, &panels, false, Some("terminal"), cx),
        ];

        // Reopen the last workspace if it still exists, else the folder the
        // app was started in.
        let workspace_root = State::get::<PathBuf>("workspace_root", cx)
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok());
        let root = workspace_root.clone();
        Workspace::model(cx).update(cx, |workspace, cx| workspace.set_root(root, cx));

        let mut status = ShellStatus::sample();
        if let Some(recent) = State::get::<Vec<SharedString>>("recent_projects", cx) {
            status.recent_projects = recent;
        }

        // Redraw when the workspace's files or git state change.
        cx.observe(&Workspace::model(cx), |_, _, cx| cx.notify()).detach();

        // Theme, font sizes: save whatever changes them (title bar toggle,
        // menu, Appearance settings).
        cx.observe_global::<Theme>(|_, cx| super::theme::save(cx)).detach();

        let project_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search recent projects…"));
        cx.subscribe(&project_search, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.project_picker_selected = 0;
                cx.notify();
            }
        })
        .detach();

        // Redraw the status bar's pending-keystrokes indicator mid-chord.
        cx.observe_pending_input(window, |_, _, cx| cx.notify()).detach();

        let worktree_search = cx.new(|cx| InputState::new(window, cx).placeholder(Self::worktree_placeholder()));
        cx.subscribe(&worktree_search, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.worktree_picker_selected = 0;
                cx.notify();
            }
        })
        .detach();

        Self {
            workspace_root,
            panels,
            docks,
            tabs: Vec::new(),
            active_tab: None,
            split_tabs: Vec::new(),
            split_active_tab: None,
            active_group: TabGroup::Primary,
            next_tab_id: 0,
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            last_seen_nav_tab: None,
            suppress_nav_record: false,
            zoomed: false,
            tab_mru: VecDeque::new(),
            last_seen_active_tab_id: None,
            tab_switcher: None,
            tab_switcher_commit_task: None,
            tab_context_menu: None,
            status,
            notices: dummy::sample_notices(),
            notifications_open: false,
            show_menus: Settings::get("show_menus", cx).unwrap_or(false),
            app_menu_bar: AppMenuBar::new(cx),
            project_picker_open: false,
            project_search,
            project_picker_selected: 0,
            project_list_scroll: ScrollHandle::new(),
            worktree_picker_open: false,
            worktree_search,
            worktree_picker_selected: 0,
            worktree_list_scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub(super) fn workspace_label(&self) -> String {
        self.workspace_root
            .as_deref()
            .and_then(|root| root.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No folder open".to_string())
    }

    pub(super) fn toggle_zoom(&mut self, cx: &mut Context<Self>) {
        self.zoomed = !self.zoomed;
        cx.notify();
    }

    fn on_split_active_tab(&mut self, _: &SplitActiveTab, window: &mut Window, cx: &mut Context<Self>) {
        self.duplicate_active_tab_in_split(window, cx);
    }

    fn on_swap_panes(&mut self, _: &SwapPanes, _: &mut Window, cx: &mut Context<Self>) {
        self.swap_panes(cx);
    }

    fn on_toggle_left_dock(&mut self, _: &ToggleLeftDock, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_dock(DockPosition::Left, cx);
    }

    fn on_toggle_right_dock(&mut self, _: &ToggleRightDock, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_dock(DockPosition::Right, cx);
    }

    fn on_toggle_bottom_dock(&mut self, _: &ToggleBottomDock, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_dock(DockPosition::Bottom, cx);
    }

    fn on_go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.go_back(cx);
    }

    fn on_go_forward(&mut self, _: &GoForward, _: &mut Window, cx: &mut Context<Self>) {
        self.go_forward(cx);
    }

    fn on_toggle_zoom(&mut self, _: &ToggleZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_zoom(cx);
    }

    /// Tab area, then the bottom dock under it, then the left and right
    /// docks beside both. Each dock is its own resizable group (with its own
    /// state) so opening or closing one never shifts another's size.
    fn render_main_area(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let center = self.render_panes(window, cx);
        if self.zoomed {
            return center;
        }

        // Saves a dock's size when its divider is dragged; `index` is the
        // dock's panel within its resizable group.
        let on_resize = |position: DockPosition, index: usize, cx: &mut Context<Self>| {
            let shell = cx.weak_entity();
            move |state: &Entity<ResizableState>, _: &mut Window, cx: &mut App| {
                if let Some(size) = state.read(cx).sizes().get(index).copied() {
                    let _ = shell.update(cx, |shell, cx| shell.save_dock_size(position, size, cx));
                }
            }
        };

        let center: AnyElement = match self.render_dock(DockPosition::Bottom, cx) {
            Some(dock) => v_resizable("forge-bottom-dock")
                .with_state(&self.dock(DockPosition::Bottom).resize)
                .on_resize(on_resize(DockPosition::Bottom, 1, cx))
                .child(resizable_panel().child(center))
                .child(
                    resizable_panel()
                        .size(self.dock(DockPosition::Bottom).size)
                        .size_range(px(100.)..px(800.))
                        .child(dock),
                )
                .into_any_element(),
            None => center,
        };

        let center: AnyElement = match self.render_dock(DockPosition::Right, cx) {
            Some(dock) => h_resizable("forge-right-dock")
                .with_state(&self.dock(DockPosition::Right).resize)
                .on_resize(on_resize(DockPosition::Right, 1, cx))
                .child(resizable_panel().child(center))
                .child(
                    resizable_panel()
                        .size(self.dock(DockPosition::Right).size)
                        .size_range(px(180.)..px(640.))
                        .child(dock),
                )
                .into_any_element(),
            None => center,
        };

        match self.render_dock(DockPosition::Left, cx) {
            Some(dock) => h_resizable("forge-left-dock")
                .with_state(&self.dock(DockPosition::Left).resize)
                .on_resize(on_resize(DockPosition::Left, 0, cx))
                .child(
                    resizable_panel()
                        .size(self.dock(DockPosition::Left).size)
                        .size_range(px(180.)..px(640.))
                        .child(dock),
                )
                .child(resizable_panel().child(center))
                .into_any_element(),
            None => center,
        }
    }
}

impl Focusable for ForgeShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ForgeShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_tab_mru();
        self.sync_nav_history();

        let main_area = self.render_main_area(window, cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        let root = self.on_menu_actions(v_flex(), cx);
        root.size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_split_active_tab))
            .on_action(cx.listener(Self::on_swap_panes))
            .on_action(cx.listener(Self::on_action_cycle_tab_mru))
            .on_action(cx.listener(Self::on_action_jump_to_lru_tab))
            .on_action(cx.listener(Self::on_toggle_left_dock))
            .on_action(cx.listener(Self::on_toggle_right_dock))
            .on_action(cx.listener(Self::on_toggle_bottom_dock))
            .on_action(cx.listener(Self::on_go_back))
            .on_action(cx.listener(Self::on_go_forward))
            .on_action(cx.listener(Self::on_toggle_zoom))
            .on_action(cx.listener(|this, _: &DeploySearch, window, cx| this.open_project_search(window, cx)))
            .on_modifiers_changed(cx.listener(Self::on_modifiers_changed))
            .child(self.render_title_bar(window, cx))
            .children(self.render_menu_bar_row(cx))
            .child(div().flex_1().min_h_0().child(main_area))
            .child(self.render_status_bar(window, cx))
            .children(self.render_tab_context_menu(window, cx))
            .children(self.render_tab_switcher_overlay(window, cx))
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
