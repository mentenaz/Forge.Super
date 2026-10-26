//! The application menu, laid out like the Zed edition's: by default a ☰
//! button in the title bar that opens the first (brand) menu with its
//! submenus flattened in, or, with "Show Menu Bar" on (Zed's `show_menus`),
//! the full menu bar on its own row below the title bar.
//!
//! Items that need parts not built yet (settings, the editor) are left out
//! rather than shown dead; they come back as those parts land.

use gpui::{
    App, Context, Div, InteractiveElement, IntoElement, KeyBinding, Menu, MenuItem, ParentElement,
    Styled, Window, actions, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex, input,
    menu::{DropdownMenu, PopupMenu},
};

use super::ForgeShell;
use super::forge_shell::{
    GoBack, GoForward, SplitActiveTab, ToggleBottomDock, ToggleLeftDock, ToggleRightDock,
    ToggleZoom,
};
use super::notifications::NoticeKind;

const BRAND: &str = "Forge Super";

actions!(
    forge_menu,
    [
        About,
        Quit,
        OpenFolder,
        OpenRecent,
        CloseActiveTab,
        CloseAllTabs,
        ToggleAllDocks,
        ToggleTheme,
        ToggleExplorer,
        ToggleOutline,
        ToggleGit,
        ToggleTerminal,
        ToggleDebugger
    ]
);

pub(super) fn init(cx: &mut App) {
    // Zed's Windows bindings.
    cx.bind_keys([
        KeyBinding::new("ctrl-k ctrl-o", OpenFolder, None),
        KeyBinding::new("ctrl-alt-o", OpenRecent, None),
        KeyBinding::new("ctrl-w", CloseActiveTab, None),
        KeyBinding::new("ctrl-k w", CloseAllTabs, None),
        KeyBinding::new("ctrl-alt-y", ToggleAllDocks, None),
        KeyBinding::new("ctrl-shift-e", ToggleExplorer, None),
        KeyBinding::new("ctrl-shift-g", ToggleGit, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);

    let owned: Vec<_> = menus().into_iter().map(|m| m.owned()).collect();
    gpui_component::GlobalState::global_mut(cx).set_app_menus(owned);
    cx.set_menus(menus());
}

fn menus() -> Vec<Menu> {
    vec![
        Menu::new(BRAND).items([
            MenuItem::action(format!("About {BRAND}"), About),
            MenuItem::separator(),
            MenuItem::submenu(
                Menu::new("Settings")
                    .items([MenuItem::action("Toggle Light/Dark Theme", ToggleTheme)]),
            ),
            MenuItem::separator(),
            MenuItem::action(format!("Quit {BRAND}"), Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open Folder…", OpenFolder),
            MenuItem::action("Open Recent…", OpenRecent),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseActiveTab),
            MenuItem::action("Close All Tabs", CloseAllTabs),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo", input::Undo),
            MenuItem::action("Redo", input::Redo),
            MenuItem::separator(),
            MenuItem::action("Cut", input::Cut),
            MenuItem::action("Copy", input::Copy),
            MenuItem::action("Paste", input::Paste),
            MenuItem::separator(),
            MenuItem::action("Find", input::Search),
            MenuItem::separator(),
            MenuItem::action("Select All", input::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Left Dock", ToggleLeftDock),
            MenuItem::action("Toggle Right Dock", ToggleRightDock),
            MenuItem::action("Toggle Bottom Dock", ToggleBottomDock),
            MenuItem::action("Toggle All Docks", ToggleAllDocks),
            MenuItem::submenu(Menu::new("Editor Layout").items([
                MenuItem::action("Split Right", SplitActiveTab),
                MenuItem::action("Zoom", ToggleZoom),
            ])),
            MenuItem::separator(),
            MenuItem::action("Project Panel", ToggleExplorer),
            MenuItem::action("Outline Panel", ToggleOutline),
            MenuItem::action("Terminal Panel", ToggleTerminal),
            MenuItem::action("Debugger Panel", ToggleDebugger),
            MenuItem::action("Git Panel", ToggleGit),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
        ]),
        Menu::new("Help").items([MenuItem::action(format!("About {BRAND}"), About)]),
    ]
}

/// Adds `items` to `menu`, flattening submenus in place (as Zed's ☰ does).
fn add_items(mut menu: PopupMenu, items: Vec<MenuItem>) -> PopupMenu {
    for item in items {
        menu = match item {
            MenuItem::Separator => menu.separator(),
            MenuItem::Action { name, action, .. } => menu.menu(name, action),
            MenuItem::Submenu(submenu) => add_items(menu, submenu.items),
            MenuItem::SystemMenu(_) => menu,
        };
    }
    menu
}

impl ForgeShell {
    /// The title bar's ☰. Not shown while the menu bar row is.
    pub(super) fn render_app_menu(&self) -> Option<gpui::AnyElement> {
        if self.show_menus {
            return None;
        }
        let focus_handle = self.focus_handle.clone();
        Button::new("application-menu")
            .ghost()
            .small()
            .icon(IconName::Menu)
            .tooltip("Open Application Menu")
            .dropdown_menu(move |menu, _, _| {
                let first = menus()
                    .into_iter()
                    .next()
                    .map(|m| m.items)
                    .unwrap_or_default();
                // Actions go to the shell, where their handlers are.
                add_items(menu.action_context(focus_handle.clone()), first)
            })
            .into_any_element()
            .into()
    }

    /// The menu bar row under the title bar, when "Show Menu Bar" is on.
    pub(super) fn render_menu_bar_row(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.show_menus {
            return None;
        }
        Some(
            h_flex()
                .h(px(28.))
                .flex_shrink_0()
                .px_1()
                .bg(cx.theme().title_bar)
                .border_b_1()
                .border_color(cx.theme().border)
                .child(self.app_menu_bar.clone())
                .into_any_element(),
        )
    }

    /// Hooks up the menu's actions on the shell's root element.
    pub(super) fn on_menu_actions(&self, root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &About, window, cx| {
            let message = format!(
                "Version {}. Built on gpui and gpui-component.",
                env!("CARGO_PKG_VERSION")
            );
            this.notify(
                NoticeKind::Info,
                format!("Mentenaz {BRAND}"),
                message,
                window,
                cx,
            );
        }))
        .on_action(|_: &Quit, _, cx: &mut App| cx.quit())
        .on_action(cx.listener(|this, _: &OpenFolder, _, cx| this.open_folder(cx)))
        .on_action(
            cx.listener(|this, _: &OpenRecent, window, cx| this.open_project_picker(window, cx)),
        )
        .on_action(cx.listener(|this, _: &CloseActiveTab, _, cx| {
            if let Some(idx) = this.active_index(this.active_group) {
                this.close(this.active_group, idx, cx);
            }
        }))
        .on_action(
            cx.listener(|this, _: &CloseAllTabs, _, cx| this.close_all(this.active_group, cx)),
        )
        .on_action(cx.listener(|this, _: &ToggleAllDocks, _, cx| this.toggle_all_docks(cx)))
        .on_action(|_: &ToggleTheme, window: &mut Window, cx: &mut App| {
            let mode = if Theme::global(cx).mode.is_dark() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            Theme::change(mode, Some(window), cx);
        })
        .on_action(cx.listener(|this, _: &ToggleExplorer, _, cx| this.toggle_panel("explorer", cx)))
        .on_action(cx.listener(|this, _: &ToggleOutline, _, cx| this.toggle_panel("outline", cx)))
        .on_action(cx.listener(|this, _: &ToggleGit, _, cx| this.toggle_panel("git", cx)))
        .on_action(cx.listener(|this, _: &ToggleTerminal, _, cx| this.toggle_panel("terminal", cx)))
        .on_action(cx.listener(|this, _: &ToggleDebugger, _, cx| this.toggle_panel("debugger", cx)))
    }
}
