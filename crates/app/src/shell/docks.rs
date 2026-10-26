//! Zed's dock model: three docks (left, right, bottom), each holding any
//! number of panels and showing one at a time. A panel's status-bar button
//! opens its dock on that panel, or closes the dock if the panel is already
//! showing; right-clicking the button moves the panel to another dock.

use gpui::{
    AnyElement, AnyView, AppContext, Context, Entity, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, resizable::ResizableState};

use super::ForgeShell;
use super::panels::{DockPosition, PanelId};

pub(super) struct PanelEntry {
    pub(super) id: PanelId,
    pub(super) position: DockPosition,
    pub(super) view: AnyView,
}

pub(super) struct Dock {
    pub(super) open: bool,
    pub(super) active: Option<PanelId>,
    /// Kept as an entity so the dock's size survives it being closed and
    /// reopened.
    pub(super) resize: Entity<ResizableState>,
}

impl Dock {
    pub(super) fn new(open: bool, active: Option<PanelId>, cx: &mut gpui::App) -> Self {
        Self { open, active, resize: cx.new(|_| ResizableState::default()) }
    }
}

impl ForgeShell {
    pub(super) fn dock(&self, position: DockPosition) -> &Dock {
        &self.docks[position as usize]
    }

    fn dock_mut(&mut self, position: DockPosition) -> &mut Dock {
        &mut self.docks[position as usize]
    }

    pub(super) fn panel_position(&self, id: PanelId) -> DockPosition {
        self.panels
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.position)
            .unwrap_or_else(|| id.default_position())
    }

    /// The panels in `position`, in button order.
    pub(super) fn panels_in(&self, position: DockPosition) -> impl Iterator<Item = PanelId> + '_ {
        self.panels.iter().filter(move |p| p.position == position).map(|p| p.id)
    }

    /// Whether `id` is the panel currently showing.
    pub(super) fn is_panel_visible(&self, id: PanelId) -> bool {
        let dock = self.dock(self.panel_position(id));
        dock.open && dock.active == Some(id)
    }

    pub(super) fn toggle_panel(&mut self, id: PanelId, cx: &mut Context<Self>) {
        let visible = self.is_panel_visible(id);
        let dock = self.dock_mut(self.panel_position(id));
        if visible {
            dock.open = false;
        } else {
            dock.active = Some(id);
            dock.open = true;
        }
        cx.notify();
    }

    /// Open or close a dock, showing its last active panel (or its first).
    pub(super) fn toggle_dock(&mut self, position: DockPosition, cx: &mut Context<Self>) {
        let first = self.panels_in(position).next();
        let dock = self.dock_mut(position);
        if dock.open {
            dock.open = false;
        } else if let Some(panel) = dock.active.or(first) {
            dock.active = Some(panel);
            dock.open = true;
        }
        cx.notify();
    }

    /// Zed's "Toggle All Docks": close every open dock, or reopen them all.
    pub(super) fn toggle_all_docks(&mut self, cx: &mut Context<Self>) {
        let any_open = self.docks.iter().any(|d| d.open);
        for position in DockPosition::ALL {
            if any_open {
                self.dock_mut(position).open = false;
            } else if self.dock(position).active.is_some() {
                self.dock_mut(position).open = true;
            }
        }
        cx.notify();
    }

    /// Move `id` to another dock and show it there.
    pub(super) fn move_panel(&mut self, id: PanelId, to: DockPosition, cx: &mut Context<Self>) {
        let from = self.panel_position(id);
        if from == to {
            return;
        }
        if let Some(entry) = self.panels.iter_mut().find(|p| p.id == id) {
            entry.position = to;
        }
        if self.dock(from).active == Some(id) {
            let next = self.panels_in(from).next();
            let dock = self.dock_mut(from);
            dock.active = next;
            dock.open &= next.is_some();
        }
        let dock = self.dock_mut(to);
        dock.active = Some(id);
        dock.open = true;
        cx.notify();
    }

    /// The open dock's active panel, framed with a border on the side
    /// facing the tab area. `None` while the dock is closed.
    pub(super) fn render_dock(&self, position: DockPosition, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dock = self.dock(position);
        if !dock.open {
            return None;
        }
        let active = dock.active?;
        let view = self.panels.iter().find(|p| p.id == active)?.view.clone();

        Some(
            div()
                .size_full()
                .bg(cx.theme().sidebar)
                .border_color(cx.theme().border)
                .when(position == DockPosition::Left, |el| el.border_r_1())
                .when(position == DockPosition::Right, |el| el.border_l_1())
                .when(position == DockPosition::Bottom, |el| el.border_t_1())
                .child(view)
                .into_any_element(),
        )
    }
}
