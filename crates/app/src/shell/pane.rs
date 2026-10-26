//! A pane: its tab bar, the breadcrumb row for file tabs, and the active
//! tab's content. With the split open, two panes sit side by side.

use std::path::MAIN_SEPARATOR_STR;

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Window, div, img, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, h_flex,
    button::{Button, ButtonVariants},
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

use super::ForgeShell;
use super::tabs::TabGroup;

impl ForgeShell {
    pub(super) fn render_panes(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let primary = self.render_pane(TabGroup::Primary, window, cx);
        if !self.split_open() {
            return primary;
        }
        let split = self.render_pane(TabGroup::Split, window, cx);
        h_resizable("workspace-editor-split")
            .child(resizable_panel().child(primary))
            .child(resizable_panel().size(px(520.)).size_range(px(240.)..px(1400.)).child(split))
            .into_any_element()
    }

    fn render_pane(&self, group: TabGroup, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let active = self.active_tab_in(group);
        let content: AnyElement = match active {
            Some(tab) => tab.view.clone().into_any_element(),
            None => self.render_empty_pane(cx),
        };

        v_flex()
            .size_full()
            // Clicking anywhere in a pane makes it the active one.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.active_group != group {
                        this.active_group = group;
                        cx.notify();
                    }
                }),
            )
            .child(self.render_tab_bar(group, window, cx))
            .when_some(active.and_then(|t| t.path.clone().map(|p| (p, t.symbol.clone()))), |el, (path, symbol)| {
                el.child(self.render_breadcrumbs(group, &path, symbol, cx))
            })
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    /// `crates\app\src\main.rs › fn main`, with the editor's tool buttons on
    /// the right. The buttons are placeholders until Zed's editor is in.
    fn render_breadcrumbs(
        &self,
        group: TabGroup,
        path: &str,
        symbol: Option<gpui::SharedString>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = path.replace('/', MAIN_SEPARATOR_STR);

        h_flex()
            .h(px(28.))
            .flex_shrink_0()
            .px_3()
            .justify_between()
            .bg(cx.theme().tab_active)
            .text_sm()
            .child(
                h_flex()
                    .gap_1()
                    .min_w_0()
                    .child(div().truncate().text_color(cx.theme().muted_foreground).child(path))
                    .when_some(symbol, |el, symbol| {
                        el.child(div().text_color(cx.theme().muted_foreground).child("›"))
                            .child(div().text_color(cx.theme().foreground).child(symbol))
                    }),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .child(
                        Button::new(("buffer-search", group as usize))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Search)
                            .tooltip("Search in File"),
                    )
                    .child(
                        Button::new(("editor-controls", group as usize))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings2)
                            .tooltip("Editor Controls"),
                    ),
            )
    }

    fn render_empty_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(img("Graphics/Forge_Super_Watermark.png").w(px(320.)).h(px(320.)))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Open a file from the Explorer to get started"),
            )
            .into_any_element()
    }
}
