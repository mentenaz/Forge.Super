//! The full-width bottom bar, laid out like the Zed edition's:
//! - left: the left dock's panel buttons, then project search, the language
//!   server menu, diagnostics, the active file and the activity indicator;
//! - right: pending keystrokes and the editor's `line:column`, line ending,
//!   language and encoding (Zed's order), then the bottom and right docks'
//!   panel buttons and the notification bell.
//!
//! The items themselves are in `status_items.rs`. Right-clicking a panel
//! button moves that panel to another dock.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
};
use gpui_component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, PopupMenuItem},
    separator::Separator,
    status_bar::StatusBar,
};

use super::ForgeShell;
use forge_host::DockPosition;

impl ForgeShell {
    pub(super) fn render_status_bar(&self, window: &Window, cx: &mut Context<Self>) -> StatusBar {
        let mut bar = StatusBar::new();

        for button in self.panel_buttons(DockPosition::Left, cx) {
            bar = bar.left(button);
        }
        bar = bar
            .left(Separator::vertical())
            .left(self.render_search_button(cx))
            .left(self.render_lsp_button(cx))
            .left(self.render_diagnostics(cx));
        if let Some(conflicts) = self.render_merge_conflicts(cx) {
            bar = bar.left(conflicts);
        }
        if let Some(file) = self.render_active_file(cx) {
            bar = bar.left(file);
        }
        if let Some(activity) = self.render_activity(cx) {
            bar = bar.left(activity);
        }

        if let Some(keys) = self.render_pending_keys(window, cx) {
            bar = bar.right(keys);
        }
        // Editor details, in Zed's format: `line:column` and the language,
        // plus line ending and encoding only when they aren't the defaults
        // (LF, UTF-8). Only while a tab is open.
        if self.active_tab.is_some() {
            let editor = &self.status.editor;
            bar = bar.right(self.status_text(format!("{}:{}", editor.line, editor.column), cx));
            if editor.line_ending != "LF" {
                bar = bar.right(self.status_text(editor.line_ending.clone(), cx));
            }
            bar = bar.right(self.render_language_button(cx));
            if editor.encoding != "UTF-8" {
                bar = bar.right(self.status_text(editor.encoding.clone(), cx));
            }
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

        bar.right(Separator::vertical())
            .right(self.render_notification_button(cx))
    }

    fn panel_buttons(&self, position: DockPosition, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let ids: Vec<&'static str> = self.panels_in(position).map(|p| p.info.id).collect();
        let mut buttons = Vec::with_capacity(ids.len() + 1);
        for id in ids {
            // Demo is a dev aid, kept visually apart from the product panels.
            if id == "demo" {
                buttons.push(Separator::vertical().into_any_element());
            }
            buttons.push(self.panel_button(id, cx));
        }
        buttons
    }

    fn panel_button(&self, id: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.weak_entity();
        let Some(panel) = self.panel(id) else {
            return div().into_any_element();
        };
        let (current, title) = (panel.position, panel.info.title);
        let tooltip = panel.info.tooltip.unwrap_or(title);

        div()
            .id(gpui::SharedString::from(format!("panel-button-{id}")))
            .child(
                Button::new(gpui::SharedString::from(format!("toggle-panel-{id}")))
                    .ghost()
                    .xsmall()
                    .icon(panel.info.icon.clone())
                    .tooltip(format!("{tooltip} (right-click to move)"))
                    .selected(self.is_panel_visible(id))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_panel(id, cx))),
            )
            .context_menu(move |menu, _, _| {
                let mut menu = menu.label(title);
                for position in DockPosition::ALL {
                    let this = this.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("Dock {}", position.title()))
                            .checked(position == current)
                            .on_click(move |_, _, cx| {
                                this.update(cx, |this, cx| this.move_panel(id, position, cx))
                                    .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    pub(super) fn status_text(
        &self,
        text: impl Into<gpui::SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
            .into_any_element()
    }
}
