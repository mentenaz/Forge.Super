//! The custom window title bar:
//! - left, as in Zed: the application menu (☰ or the menu bar, see
//!   `app_menu.rs`), the project button (`recent_projects.rs`), and the
//!   worktree picker (`worktree_picker.rs`) / branch picker;
//! - middle: empty, so it can be used to drag the window;
//! - right: theme toggle and settings, then the window controls drawn by
//!   gpui-component's `TitleBar`.

use gpui::{
    Anchor, App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollbarMode,
};

use super::ForgeShell;
use super::notifications::NoticeKind;

fn set_font_size(size: f32, window: &mut Window, cx: &mut App) {
    Theme::global_mut(cx).font_size = px(size);
    Theme::sync_base(cx);
    window.refresh();
}

fn set_radius(radius: f32, window: &mut Window, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.radius = px(radius);
    theme.radius_lg = if theme.radius > px(0.) { theme.radius + px(2.) } else { px(0.) };
    Theme::sync_base(cx);
    window.refresh();
}

fn set_scrollbar_mode(mode: ScrollbarMode, window: &mut Window, cx: &mut App) {
    Theme::set_scrollbar_mode(mode, cx);
    window.refresh();
}

impl ForgeShell {
    fn render_branch_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        let branches = self.status.branches.clone();
        let current = self.status.branch.clone();

        Button::new("branch-picker")
            .ghost()
            .small()
            .icon(Icon::empty().path("icons/git-branch.svg"))
            .label(self.status.branch.clone())
            .text_color(cx.theme().muted_foreground)
            .tooltip(format!("Branch · Currently Checked Out: {}", self.status.branch))
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.label("Branches");
                for branch in &branches {
                    let this = this.clone();
                    let name = branch.clone();
                    menu = menu.item(PopupMenuItem::new(branch.clone()).checked(*branch == current).on_click(move |_, _, cx| {
                        let name = name.clone();
                        this.update(cx, |this, cx| {
                            let current = this.status.current_worktree;
                            if let Some(worktree) = this.status.worktrees.get_mut(current) {
                                worktree.branch = name.clone();
                            }
                            this.status.branch = name;
                            cx.notify();
                        })
                        .ok();
                    }));
                }
                menu
            })
    }

    fn render_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        let show_menus = self.show_menus;

        Button::new("forge-settings")
            .small()
            .ghost()
            .icon(IconName::Settings2)
            .tooltip("Appearance")
            .dropdown_menu(move |menu, _, cx| {
                let font_size = cx.theme().font_size.as_f32() as i32;
                let radius = cx.theme().radius.as_f32() as i32;
                let scrollbar = cx.theme().scrollbar_mode;
                let this = this.clone();

                menu.scrollable(true)
                    .max_h(px(520.))
                    .label("Font Size")
                    .item(PopupMenuItem::new("Large (16px)").checked(font_size == 16).on_click(|_, w, cx| set_font_size(16., w, cx)))
                    .item(PopupMenuItem::new("Medium (14px)").checked(font_size == 14).on_click(|_, w, cx| set_font_size(14., w, cx)))
                    .item(PopupMenuItem::new("Default (13px)").checked(font_size == 13).on_click(|_, w, cx| set_font_size(13., w, cx)))
                    .item(PopupMenuItem::new("Small (12px)").checked(font_size == 12).on_click(|_, w, cx| set_font_size(12., w, cx)))
                    .separator()
                    .label("Border Radius")
                    .item(PopupMenuItem::new("8px").checked(radius == 8).on_click(|_, w, cx| set_radius(8., w, cx)))
                    .item(PopupMenuItem::new("6px (default)").checked(radius == 6).on_click(|_, w, cx| set_radius(6., w, cx)))
                    .item(PopupMenuItem::new("4px").checked(radius == 4).on_click(|_, w, cx| set_radius(4., w, cx)))
                    .item(PopupMenuItem::new("0px").checked(radius == 0).on_click(|_, w, cx| set_radius(0., w, cx)))
                    .separator()
                    .label("Scrollbar")
                    .item(
                        PopupMenuItem::new("Scrolling to show")
                            .checked(scrollbar == ScrollbarMode::Scrolling)
                            .on_click(|_, w, cx| set_scrollbar_mode(ScrollbarMode::Scrolling, w, cx)),
                    )
                    .item(
                        PopupMenuItem::new("Hover to show")
                            .checked(scrollbar == ScrollbarMode::Hover)
                            .on_click(|_, w, cx| set_scrollbar_mode(ScrollbarMode::Hover, w, cx)),
                    )
                    .item(
                        PopupMenuItem::new("Always show")
                            .checked(scrollbar == ScrollbarMode::Always)
                            .on_click(|_, w, cx| set_scrollbar_mode(ScrollbarMode::Always, w, cx)),
                    )
                    .separator()
                    .label("Title Bar")
                    .item(PopupMenuItem::new("Show Menu Bar").checked(show_menus).on_click({
                        let this = this.clone();
                        move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                this.show_menus = !this.show_menus;
                                let _ = forge_host::Settings::set("show_menus", this.show_menus, cx);
                                cx.notify();
                            })
                            .ok();
                        }
                    }))
                    .separator()
                    .label("Developer")
                    .item(PopupMenuItem::new("Send test notification").on_click(move |_, window, cx| {
                        this.update(cx, |this, cx| {
                            this.notify(NoticeKind::Info, "Test notification", "Notifications show here and in the bell's list.", window, cx);
                        })
                        .ok();
                    }))
            })
            .anchor(Anchor::TopRight)
    }

    pub(super) fn render_title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_dark = cx.theme().mode.is_dark();
        let theme_button = Button::new("forge-theme")
            .small()
            .ghost()
            .icon(if is_dark { IconName::Sun } else { IconName::Moon })
            .tooltip(if is_dark { "Switch to light theme" } else { "Switch to dark theme" })
            .on_click(move |_, window, cx| {
                let mode = if is_dark { ThemeMode::Light } else { ThemeMode::Dark };
                Theme::change(mode, Some(window), cx);
            });

        TitleBar::new()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .pl_1()
                    .gap_1()
                    // Keep button clicks from starting a window drag.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(self.render_app_menu())
                    .child(self.render_project_picker(window, cx))
                    .child(self.render_worktree_picker(window, cx))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).opacity(0.25).child("/"))
                    .child(self.render_branch_picker(cx)),
            )
            .child(div().flex_1().h_full())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .h_full()
                    .px_2()
                    .gap_1()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(theme_button)
                    .child(self.render_settings_button(cx)),
            )
    }
}
