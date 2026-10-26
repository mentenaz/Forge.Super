//! Right-click menu for a tab, with Zed's items: the close variants (which
//! skip pinned tabs), pin/unpin, copy path, and the split actions.

use gpui::{
    AnchoredPositionMode, AnyElement, ClickEvent, ClipboardItem, Context, InteractiveElement,
    IntoElement, MouseButton, ParentElement, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, point, prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

use super::ForgeShell;
use super::tabs::TabGroup;

impl ForgeShell {
    pub(super) fn render_tab_context_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (group, idx, position) = self.tab_context_menu?;
        let tab = self.group(group).get(idx)?;
        let is_primary = group == TabGroup::Primary;
        let pinned = tab.pinned;
        let path = tab.path.clone();
        let tab_count = self.group(group).len();
        let viewport = window.viewport_size();

        let menu = div()
            .id("tab-ctx-menu")
            .occlude()
            .absolute()
            .left(position.x)
            .top(position.y)
            .min_w(px(200.))
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .shadow_md()
            .py_1()
            .child(
                self.menu_item("tab-ctx-close", "Close", true, cx, move |this, _, cx| {
                    this.close(group, idx, cx)
                }),
            )
            .child(self.menu_item(
                "tab-ctx-close-others",
                "Close Others",
                tab_count > 1,
                cx,
                move |this, _, cx| this.close_others(group, idx, cx),
            ))
            .child(self.menu_item(
                "tab-ctx-close-left",
                "Close to the Left",
                idx > 0,
                cx,
                move |this, _, cx| this.close_left(group, idx, cx),
            ))
            .child(self.menu_item(
                "tab-ctx-close-right",
                "Close to the Right",
                idx + 1 < tab_count,
                cx,
                move |this, _, cx| this.close_right(group, idx, cx),
            ))
            .child(self.menu_item(
                "tab-ctx-close-all",
                "Close All",
                true,
                cx,
                move |this, _, cx| this.close_all(group, cx),
            ))
            .child(self.menu_separator(cx))
            .child(self.menu_item(
                "tab-ctx-pin",
                if pinned { "Unpin Tab" } else { "Pin Tab" },
                true,
                cx,
                move |this, _, cx| this.toggle_pin(group, idx, cx),
            ))
            .when_some(path, |el, path| {
                el.child(self.menu_item(
                    "tab-ctx-copy-path",
                    "Copy Relative Path",
                    true,
                    cx,
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path.to_string()));
                    },
                ))
            })
            .child(self.menu_separator(cx))
            .when(is_primary, |el| {
                el.child(self.menu_item(
                    "tab-ctx-split",
                    "Split Right",
                    true,
                    cx,
                    move |this, window, cx| this.duplicate_tab_in_split(idx, window, cx),
                ))
                .child(self.menu_item(
                    "tab-ctx-move-split",
                    "Move to Split Pane",
                    true,
                    cx,
                    move |this, _, cx| this.move_tab_to_split(idx, cx),
                ))
            })
            .when(!is_primary, |el| {
                el.child(self.menu_item(
                    "tab-ctx-move-primary",
                    "Move to Main Pane",
                    true,
                    cx,
                    move |this, _, cx| this.move_split_tab_to_editor(idx, cx),
                ))
            })
            .when(self.split_open(), |el| {
                el.child(self.menu_item(
                    "tab-ctx-swap-panes",
                    "Swap Panes",
                    true,
                    cx,
                    |this, _, cx| this.swap_panes(cx),
                ))
            });

        // Full-window backdrop: clicking anywhere outside the menu closes it.
        let backdrop = div()
            .id("tab-ctx-backdrop")
            .w(viewport.width)
            .h(viewport.height)
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.tab_context_menu = None;
                    cx.notify();
                }),
            )
            .child(menu);

        Some(
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(px(0.), px(0.)))
                    .child(backdrop),
            )
            .with_priority(200)
            .into_any_element(),
        )
    }

    fn menu_separator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().my_1().h(px(1.)).bg(cx.theme().border)
    }

    fn menu_item(
        &self,
        id: &'static str,
        label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_1()
            .text_sm()
            .when(!enabled, |el| el.text_color(cx.theme().muted_foreground))
            .when(enabled, |el| {
                el.cursor_pointer()
                    .hover(|d| d.bg(cx.theme().list_hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.tab_context_menu = None;
                        action(this, window, cx);
                        cx.notify();
                    }))
            })
            .child(label)
    }
}
