//! Hold-Ctrl-and-tap-Tab tab switcher (like VS Code's): Ctrl+Tab and
//! Ctrl+Shift+Tab step a highlighted preview through the tabs in
//! most-recently-used order, and releasing Ctrl switches to it.

use std::time::Duration;

use gpui::{
    AnchoredPositionMode, AnyElement, Context, InteractiveElement, IntoElement,
    ModifiersChangedEvent, MouseButton, ParentElement, StatefulInteractiveElement, Styled, Window,
    anchored, deferred, div, point, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use super::ForgeShell;
use super::forge_shell::{CycleTabMru, JumpToLruTab};

pub(super) struct TabSwitcherState {
    /// Tab ids, snapshotted when the switcher opened so repeated presses walk
    /// a stable list: MRU-known tabs first, then the rest in tab-bar order.
    order: Vec<usize>,
    /// Index into `order` currently highlighted.
    selected_ix: usize,
}

impl ForgeShell {
    /// Moves the active tab to the front of `tab_mru` whenever it changed.
    /// Called every render so no call site that changes `active_tab` has to
    /// remember to.
    pub(super) fn sync_tab_mru(&mut self) {
        let current_id = self.active_tab.and_then(|i| self.tabs.get(i)).map(|t| t.id);
        if current_id == self.last_seen_active_tab_id {
            return;
        }
        if let Some(id) = current_id {
            self.tab_mru.retain(|&x| x != id);
            self.tab_mru.push_front(id);
        }
        self.last_seen_active_tab_id = current_id;
    }

    fn advance_tab_switcher(&mut self, direction: isize, window: &mut Window, cx: &mut Context<Self>) {
        let switcher = self.tab_switcher.get_or_insert_with(|| {
            let mut order: Vec<usize> = self.tab_mru.iter().copied().collect();
            for tab in &self.tabs {
                if !order.contains(&tab.id) {
                    order.push(tab.id);
                }
            }
            TabSwitcherState { order, selected_ix: 0 }
        });
        if switcher.order.len() < 2 {
            self.tab_switcher = None;
            return;
        }
        let len = switcher.order.len() as isize;
        switcher.selected_ix = (switcher.selected_ix as isize + direction).rem_euclid(len) as usize;
        cx.notify();

        // Safety net in case the Ctrl release is never seen (e.g. the window
        // loses focus mid-hold). Replacing the task cancels the previous one.
        let this = cx.weak_entity();
        self.tab_switcher_commit_task = Some(cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| this.dismiss_tab_switcher(cx));
        }));
    }

    pub(super) fn on_action_cycle_tab_mru(&mut self, _: &CycleTabMru, window: &mut Window, cx: &mut Context<Self>) {
        self.advance_tab_switcher(1, window, cx);
    }

    pub(super) fn on_action_jump_to_lru_tab(&mut self, _: &JumpToLruTab, window: &mut Window, cx: &mut Context<Self>) {
        self.advance_tab_switcher(-1, window, cx);
    }

    /// GPUI key bindings only fire on key-down, so watching modifier changes
    /// is how the Ctrl release is detected.
    pub(super) fn on_modifiers_changed(&mut self, event: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.tab_switcher.is_some() && !event.control {
            self.commit_tab_switcher(cx);
        }
    }

    fn commit_tab_switcher(&mut self, cx: &mut Context<Self>) {
        let Some(switcher) = self.tab_switcher.take() else { return };
        self.tab_switcher_commit_task = None;
        if let Some(&target_id) = switcher.order.get(switcher.selected_ix)
            && let Some(idx) = self.tabs.iter().position(|t| t.id == target_id)
        {
            self.activate_tab(idx, cx);
        }
        cx.notify();
    }

    fn dismiss_tab_switcher(&mut self, cx: &mut Context<Self>) {
        self.tab_switcher = None;
        self.tab_switcher_commit_task = None;
        cx.notify();
    }

    pub(super) fn render_tab_switcher_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let switcher = self.tab_switcher.as_ref()?;
        let viewport = window.viewport_size();

        let rows: Vec<AnyElement> = switcher
            .order
            .iter()
            .enumerate()
            .filter_map(|(ix, &id)| {
                let tab = self.tabs.iter().find(|t| t.id == id)?;
                let selected = ix == switcher.selected_ix;
                Some(
                    h_flex()
                        .id(("tab-switcher-row", id))
                        .w_full()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .when(selected, |el| {
                            el.bg(cx.theme().tokens.accent).text_color(cx.theme().accent_foreground)
                        })
                        .when(!selected, |el| el.hover(|el| el.bg(cx.theme().list_hover)))
                        .child(div().flex_1().text_sm().truncate().child(tab.title.clone()))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(idx) = this.tabs.iter().position(|t| t.id == id) {
                                    this.activate_tab(idx, cx);
                                }
                                this.dismiss_tab_switcher(cx);
                            }),
                        )
                        .into_any_element(),
                )
            })
            .collect();

        Some(
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(px(0.), px(0.)))
                    .child(
                        div()
                            .id("tab-switcher-backdrop")
                            .w(viewport.width)
                            .h(viewport.height)
                            .occlude()
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.commit_tab_switcher(cx)),
                            )
                            .child(
                                v_flex()
                                    .id("tab-switcher-panel")
                                    .occlude()
                                    .w(px(360.))
                                    .max_h(px(420.))
                                    .overflow_y_scroll()
                                    .bg(cx.theme().popover)
                                    .text_color(cx.theme().popover_foreground)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded(cx.theme().radius)
                                    .shadow_lg()
                                    .p_1()
                                    .gap_0p5()
                                    .children(rows),
                            ),
                    ),
            )
            .with_priority(300)
            .into_any_element(),
        )
    }
}
