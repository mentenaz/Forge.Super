//! A pane's tab bar, styled like Zed's: back/forward buttons, then the tabs
//! (flush, divided by borders, the active one merging into the content),
//! then the pane's split/close and zoom buttons. Tabs can be dragged within
//! and between panes.

use std::path::MAIN_SEPARATOR_STR;

use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, h_flex,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::ForgeShell;
use super::tabs::{TabGroup, WorkspaceTab};

/// The payload of a tab drag, also rendered as the drag preview.
#[derive(Clone)]
pub(super) struct DraggedTab {
    id: usize,
    title: SharedString,
}

impl Render for DraggedTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .text_xs()
            .bg(cx.theme().tab_active)
            .text_color(cx.theme().tab_active_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .shadow_md()
            .child(self.title.clone())
    }
}

/// For each tab, the folder hint that tells same-named files apart, e.g.
/// `node\ui` for one of several `Cargo.toml`s. Same approach as Zed: add
/// parent folders, one at a time, only to the tabs that still collide.
fn folder_hints(tabs: &[WorkspaceTab]) -> Vec<Option<String>> {
    let parents: Vec<Option<Vec<&str>>> = tabs
        .iter()
        .map(|t| t.path.as_ref().map(|p| p.split('/').rev().skip(1).collect()))
        .collect();
    let hint = |idx: usize, depth: usize| -> Option<String> {
        let parents = parents[idx].as_ref()?;
        if depth == 0 || parents.is_empty() {
            return None;
        }
        let mut parts: Vec<&str> = parents.iter().take(depth).copied().collect();
        parts.reverse();
        Some(parts.join(MAIN_SEPARATOR_STR))
    };

    let mut depth = vec![0usize; tabs.len()];
    loop {
        // Compare against this pass's hints, not ones already deepened in it.
        let hints: Vec<Option<String>> = (0..tabs.len()).map(|i| hint(i, depth[i])).collect();
        let mut changed = false;
        for i in 0..tabs.len() {
            let max_depth = parents[i].as_ref().map_or(0, |p| p.len());
            let collides = tabs[i].path.is_some()
                && (0..tabs.len()).any(|j| {
                    j != i && tabs[j].path.is_some() && tabs[j].title == tabs[i].title && hints[j] == hints[i]
                });
            if collides && depth[i] < max_depth {
                depth[i] += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (0..tabs.len()).map(|i| hint(i, depth[i])).collect()
}

impl ForgeShell {
    pub(super) fn render_tab_bar(
        &self,
        group: TabGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = match group {
            TabGroup::Primary => "tab-bar-scroll-primary",
            TabGroup::Split => "tab-bar-scroll-split",
        };
        let scroll_handle = window.use_keyed_state(key, cx, |_, _| ScrollHandle::default()).read(cx).clone();

        let hints = folder_hints(self.group(group));
        let tab_count = self.group(group).len();
        let mut items = Vec::with_capacity(tab_count);
        for idx in 0..tab_count {
            items.push(self.render_tab(group, idx, hints[idx].clone(), cx));
        }

        h_flex()
            .id(match group {
                TabGroup::Primary => "tab-bar-primary",
                TabGroup::Split => "tab-bar-split",
            })
            .h(px(32.))
            .flex_shrink_0()
            .bg(cx.theme().tab_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(self.render_nav_buttons(group, cx))
            .child(
                h_flex()
                    .id(("tab-strip", group as usize))
                    .flex_1()
                    .h_full()
                    .overflow_x_scroll()
                    .track_scroll(&scroll_handle)
                    .horizontal_scrollbar(&scroll_handle)
                    .children(items)
                    // The empty space after the last tab: dropping a tab here
                    // moves it to the end of this pane.
                    .child(
                        div()
                            .id(("tab-strip-end", group as usize))
                            .flex_1()
                            .min_w(px(24.))
                            .h_full()
                            .drag_over::<DraggedTab>(|style, _, _, cx| style.bg(cx.theme().drop_target))
                            .on_drop(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
                                this.move_tab(dragged.id, group, tab_count, cx);
                            })),
                    ),
            )
            .child(self.render_pane_buttons(group, cx))
    }

    fn render_nav_buttons(&self, group: TabGroup, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h_full()
            .px_1()
            .gap_0p5()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                Button::new(("nav-back", group as usize))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Go Back")
                    .disabled(!self.can_go_back())
                    .on_click(cx.listener(|this, _, _, cx| this.go_back(cx))),
            )
            .child(
                Button::new(("nav-forward", group as usize))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowRight)
                    .tooltip("Go Forward")
                    .disabled(!self.can_go_forward())
                    .on_click(cx.listener(|this, _, _, cx| this.go_forward(cx))),
            )
    }

    fn render_pane_buttons(&self, group: TabGroup, cx: &mut Context<Self>) -> impl IntoElement {
        let split_or_close = match group {
            TabGroup::Primary => Button::new("split-right")
                .ghost()
                .xsmall()
                .icon(Icon::empty().path("icons/zed_split.svg"))
                .tooltip("Split Right")
                .disabled(self.active_tab.is_none())
                .on_click(cx.listener(|this, _, window, cx| this.duplicate_active_tab_in_split(window, cx))),
            TabGroup::Split => Button::new("close-split-pane")
                .ghost()
                .xsmall()
                .icon(IconName::Close)
                .tooltip("Close Pane")
                .on_click(cx.listener(|this, _, _, cx| this.close_split_pane(cx))),
        };
        let zoomed = self.zoomed;

        h_flex()
            .h_full()
            .px_1()
            .gap_0p5()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(split_or_close)
            .child(
                Button::new(("zoom", group as usize))
                    .ghost()
                    .xsmall()
                    .icon(if zoomed { IconName::Minimize } else { IconName::Maximize })
                    .tooltip(if zoomed { "Zoom Out" } else { "Zoom In" })
                    .selected(zoomed)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_zoom(cx))),
            )
    }

    fn render_tab(&self, group: TabGroup, idx: usize, hint: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let tab = &self.group(group)[idx];
        let is_active = self.active_index(group) == Some(idx);
        let hover_group: SharedString = format!("tab-{}", tab.id).into();
        let foreground = if is_active { cx.theme().tab_active_foreground } else { cx.theme().muted_foreground };
        let id = tab.id;

        // Close button, shown on hover. A dirty tab shows a dot in its place
        // until hovered; a pinned tab shows a pin that unpins.
        let close = div()
            .id(("tab-close", tab.id))
            .flex()
            .items_center()
            .justify_center()
            .size(px(16.))
            .rounded_sm()
            .text_color(cx.theme().muted_foreground)
            .hover(|d| d.bg(cx.theme().secondary).text_color(cx.theme().foreground))
            .invisible()
            .group_hover(hover_group.clone(), |s| s.visible())
            .child(Icon::new(IconName::Close).xsmall())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.close(group, idx, cx);
            }));
        let trailing: AnyElement = if tab.pinned {
            div()
                .id(("tab-unpin", tab.id))
                .flex()
                .items_center()
                .justify_center()
                .size(px(16.))
                .text_color(cx.theme().muted_foreground)
                .child(Icon::empty().path("icons/zed_pin.svg").xsmall())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_pin(group, idx, cx);
                }))
                .into_any_element()
        } else if tab.dirty {
            div()
                .relative()
                .size(px(16.))
                .child(
                    div()
                        .absolute()
                        .top(px(5.))
                        .left(px(5.))
                        .size(px(6.))
                        .rounded_full()
                        .bg(cx.theme().foreground)
                        .group_hover(hover_group.clone(), |s| s.invisible()),
                )
                .child(div().absolute().child(close))
                .into_any_element()
        } else {
            close.into_any_element()
        };

        h_flex()
            .id(("tab", tab.id))
            .group(hover_group)
            .h_full()
            .flex_shrink_0()
            .items_center()
            .gap_1p5()
            .pl_3()
            .pr_1p5()
            .cursor_pointer()
            .border_r_1()
            .border_color(cx.theme().border)
            // The active tab covers the bar's bottom border so it merges
            // into the content below.
            .when(is_active, |el| el.bg(cx.theme().tab_active).mb(px(-1.)).pb(px(1.)))
            .when(!is_active, |el| el.bg(cx.theme().tab))
            .text_sm()
            .text_color(foreground)
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.standard_click() {
                    this.activate(group, idx, cx);
                }
            }))
            .on_aux_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.is_middle_click() {
                    this.close(group, idx, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.tab_context_menu = Some((group, idx, event.position));
                    cx.notify();
                }),
            )
            .on_drag(DraggedTab { id, title: tab.title.clone() }, |dragged, _, _, cx: &mut App| {
                cx.new(|_| dragged.clone())
            })
            .drag_over::<DraggedTab>(|style, _, _, cx| style.bg(cx.theme().drop_target))
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
                this.move_tab(dragged.id, group, idx, cx);
            }))
            // Non-file tabs (panels opened as tabs) keep an icon, as in Zed.
            .when(tab.path.is_none(), |el| {
                el.child(div().flex_shrink_0().child(tab.icon.clone().xsmall()))
            })
            .child(div().max_w(px(200.)).truncate().child(tab.title.clone()))
            .when_some(hint, |el, hint| {
                el.child(div().text_color(cx.theme().muted_foreground).text_xs().child(hint))
            })
            .child(trailing)
            .into_any_element()
    }
}
