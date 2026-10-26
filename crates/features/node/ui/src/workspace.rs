//! The Node.js workspace, shown in the tab area.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, StyledExt, v_flex};

pub struct NodeWorkspace;

impl NodeWorkspace {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NodeWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for NodeWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_3()
            .gap_2()
            .child(div().text_sm().font_semibold().child("Node.js"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Ported from the Zed edition in phase 4."),
            )
    }
}
