//! The full-width bottom bar, laid out like the Zed edition's:
//! - left: the left dock's panel buttons, diagnostics, language servers;
//! - right: the active editor's cursor/language/line ending/encoding, the
//!   bottom dock's panel buttons, the right dock's panel buttons, and the
//!   notification bell.
//!
//! Right-clicking a panel button moves that panel to another dock.

use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use gpui_component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable, h_flex,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, PopupMenuItem},
    separator::Separator,
    spinner::Spinner,
    status_bar::StatusBar,
};

use super::ForgeShell;
use super::dummy::LspState;
use super::panels::{DockPosition, PanelId};

impl ForgeShell {
    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> StatusBar {
        let mut bar = StatusBar::new();

        for button in self.panel_buttons(DockPosition::Left, cx) {
            bar = bar.left(button);
        }
        bar = bar
            .left(Separator::vertical())
            .left(self.render_diagnostics(cx))
            .left(self.render_language_servers(cx));

        // Editor details only make sense while a tab is open.
        if self.active_tab.is_some() {
            let editor = &self.status.editor;
            bar = bar
                .right(self.status_text(format!("Ln {}, Col {}", editor.line, editor.column), cx))
                .right(self.status_text(editor.language.clone(), cx))
                .right(self.status_text(editor.line_ending.clone(), cx))
                .right(self.status_text(editor.encoding.clone(), cx));
        }

        for position in [DockPosition::Bottom, DockPosition::Right] {
            let buttons = self.panel_buttons(position, cx);
            if !buttons.is_empty() {
                bar = bar.right(Separator::vertical());
            }
            for button in buttons {
                bar = bar.right(button);
            }
        }

        bar.right(Separator::vertical()).right(self.render_notification_button(cx))
    }

    fn panel_buttons(&self, position: DockPosition, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let ids: Vec<PanelId> = self.panels_in(position).collect();
        let mut buttons = Vec::with_capacity(ids.len() + 1);
        for id in ids {
            // Demo is a dev aid, kept visually apart from the product panels.
            if id == PanelId::Demo {
                buttons.push(Separator::vertical().into_any_element());
            }
            buttons.push(self.panel_button(id, cx));
        }
        buttons
    }

    fn panel_button(&self, id: PanelId, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.weak_entity();
        let current = self.panel_position(id);

        div()
            .id(("panel-button", id as usize))
            .child(
                Button::new(("toggle-panel", id as usize))
                    .ghost()
                    .xsmall()
                    .icon(id.icon())
                    .tooltip(format!("{} (right-click to move)", id.title()))
                    .selected(self.is_panel_visible(id))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_panel(id, cx))),
            )
            .context_menu(move |menu, _, _| {
                let mut menu = menu.label(id.title());
                for position in DockPosition::ALL {
                    let this = this.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("Dock {}", position.title()))
                            .checked(position == current)
                            .on_click(move |_, _, cx| {
                                this.update(cx, |this, cx| this.move_panel(id, position, cx)).ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn status_text(&self, text: impl Into<gpui::SharedString>, cx: &mut Context<Self>) -> AnyElement {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
            .into_any_element()
    }

    fn render_diagnostics(&self, cx: &mut Context<Self>) -> AnyElement {
        let errors = self.status.errors;
        let warnings = self.status.warnings;
        let count = |icon: IconName, n: usize, color| {
            h_flex()
                .gap_0p5()
                .child(div().text_color(color).child(Icon::new(icon).xsmall()))
                .child(n.to_string())
        };

        Button::new("diagnostics")
            .ghost()
            .xsmall()
            .tooltip("Project diagnostics")
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .child(count(IconName::CircleX, errors, cx.theme().danger))
                    .child(count(IconName::TriangleAlert, warnings, cx.theme().warning)),
            )
            .into_any_element()
    }

    fn render_language_servers(&self, cx: &mut Context<Self>) -> AnyElement {
        let servers = self.status.language_servers.iter().map(|server| {
            let state = match server.state {
                LspState::Indexing { percent } => h_flex()
                    .gap_1()
                    .child(Spinner::new().xsmall().icon(IconName::Loader))
                    .child(format!("{percent}%")),
                LspState::Ready => h_flex().child(
                    div()
                        .text_color(cx.theme().success)
                        .child(Icon::new(IconName::CircleCheck).xsmall()),
                ),
            };
            h_flex()
                .gap_1()
                .child(div().text_color(cx.theme().muted_foreground).child(server.name.clone()))
                .child(state)
        });

        h_flex()
            .gap_3()
            .text_xs()
            .children(servers)
            .into_any_element()
    }
}
