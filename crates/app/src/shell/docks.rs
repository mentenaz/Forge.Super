//! Zed's dock model: three docks (left, right, bottom), each holding any
//! number of panels and showing one at a time. A panel's status-bar button
//! opens its dock on that panel, or closes the dock if the panel is already
//! showing; right-clicking the button moves the panel to another dock.
//!
//! The panels come from `forge_host::PanelRegistry`. What the user chose is
//! kept between runs: a panel's dock in settings (`panels.<id>.dock`), and
//! each dock's open/active panel/size in state (`docks.<left|right|bottom>`).

use forge_host::{DockPosition, PanelInfo, PanelRegistry, Settings, State};
use gpui::{
    AnyElement, AnyView, App, AppContext, Context, Entity, IntoElement, ParentElement, Pixels,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, resizable::ResizableState};

use super::ForgeShell;
use super::panels::PanelId;

pub(super) struct PanelEntry {
    pub(super) info: PanelInfo,
    pub(super) position: DockPosition,
    pub(super) view: AnyView,
}

pub(super) struct Dock {
    pub(super) open: bool,
    pub(super) active: Option<PanelId>,
    /// The size the dock opens at (width for left/right, height for bottom).
    pub(super) size: Pixels,
    /// Kept as an entity so a dragged size survives the dock being closed
    /// and reopened.
    pub(super) resize: Entity<ResizableState>,
}

fn default_size(position: DockPosition) -> Pixels {
    match position {
        DockPosition::Left => px(260.),
        DockPosition::Right => px(280.),
        DockPosition::Bottom => px(260.),
    }
}

/// Builds every registered panel's view, in its saved (or default) dock,
/// ordered by `PanelInfo::order`.
pub(super) fn build_panels(window: &mut Window, cx: &mut App) -> Vec<PanelEntry> {
    let registrations: Vec<_> = cx
        .global::<PanelRegistry>()
        .panels()
        .iter()
        .map(|p| (p.info.clone(), p.build.clone()))
        .collect();
    let mut panels: Vec<PanelEntry> = registrations
        .into_iter()
        .map(|(info, build)| {
            let position = Settings::panel(info.id, "dock", cx).unwrap_or(info.default_dock);
            PanelEntry { view: build(window, cx), position, info }
        })
        .collect();
    panels.sort_by_key(|p| p.info.order);
    panels
}

/// A dock as it was left last run, or its default.
pub(super) fn load_dock(
    position: DockPosition,
    panels: &[PanelEntry],
    default_open: bool,
    default_active: Option<PanelId>,
    cx: &mut App,
) -> Dock {
    let key = position.key();
    let in_dock = |id: &str| panels.iter().find(|p| p.position == position && p.info.id == id).map(|p| p.info.id);
    let active = State::get::<String>(&format!("docks.{key}.active"), cx)
        .and_then(|id| in_dock(&id))
        .or_else(|| default_active.and_then(in_dock))
        .or_else(|| panels.iter().find(|p| p.position == position).map(|p| p.info.id));
    let open = State::get(&format!("docks.{key}.open"), cx).unwrap_or(default_open) && active.is_some();
    let size = State::get::<f32>(&format!("docks.{key}.size"), cx).map(px).unwrap_or(default_size(position));
    Dock { open, active, size, resize: cx.new(|_| ResizableState::default()) }
}

impl ForgeShell {
    pub(super) fn dock(&self, position: DockPosition) -> &Dock {
        &self.docks[position as usize]
    }

    fn dock_mut(&mut self, position: DockPosition) -> &mut Dock {
        &mut self.docks[position as usize]
    }

    pub(super) fn panel(&self, id: &str) -> Option<&PanelEntry> {
        self.panels.iter().find(|p| p.info.id == id)
    }

    pub(super) fn panel_position(&self, id: &str) -> Option<DockPosition> {
        self.panel(id).map(|p| p.position)
    }

    /// The panels in `position`, in button order.
    pub(super) fn panels_in(&self, position: DockPosition) -> impl Iterator<Item = &PanelEntry> + '_ {
        self.panels.iter().filter(move |p| p.position == position)
    }

    /// Whether `id` is the panel currently showing.
    pub(super) fn is_panel_visible(&self, id: &str) -> bool {
        self.panel_position(id).is_some_and(|position| {
            let dock = self.dock(position);
            dock.open && dock.active == Some(id)
        })
    }

    fn save_dock(&self, position: DockPosition, cx: &mut App) {
        let dock = self.dock(position);
        let key = position.key();
        State::set(&format!("docks.{key}.open"), dock.open, cx);
        State::set(&format!("docks.{key}.active"), dock.active, cx);
    }

    /// Records a dock's size after the user drags its divider.
    pub(super) fn save_dock_size(&mut self, position: DockPosition, size: Pixels, cx: &mut App) {
        self.dock_mut(position).size = size;
        State::set(&format!("docks.{}.size", position.key()), f32::from(size), cx);
    }

    pub(super) fn toggle_panel(&mut self, id: &str, cx: &mut Context<Self>) {
        let (Some(position), Some(id)) = (self.panel_position(id), self.panel(id).map(|p| p.info.id)) else {
            return;
        };
        let visible = self.is_panel_visible(id);
        let dock = self.dock_mut(position);
        if visible {
            dock.open = false;
        } else {
            dock.active = Some(id);
            dock.open = true;
        }
        self.save_dock(position, cx);
        cx.notify();
    }

    /// Opens the panel's dock on the panel (no-op if it's showing).
    pub(super) fn show_panel(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.is_panel_visible(id) {
            self.toggle_panel(id, cx);
        }
    }

    /// Open or close a dock, showing its last active panel (or its first).
    pub(super) fn toggle_dock(&mut self, position: DockPosition, cx: &mut Context<Self>) {
        let first = self.panels_in(position).next().map(|p| p.info.id);
        let dock = self.dock_mut(position);
        if dock.open {
            dock.open = false;
        } else if let Some(panel) = dock.active.or(first) {
            dock.active = Some(panel);
            dock.open = true;
        }
        self.save_dock(position, cx);
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
            self.save_dock(position, cx);
        }
        cx.notify();
    }

    /// Move `id` to another dock and show it there. Saved as the panel's
    /// `dock` setting.
    pub(super) fn move_panel(&mut self, id: &str, to: DockPosition, cx: &mut Context<Self>) {
        let (Some(from), Some(id)) = (self.panel_position(id), self.panel(id).map(|p| p.info.id)) else {
            return;
        };
        if from == to {
            return;
        }
        if let Some(entry) = self.panels.iter_mut().find(|p| p.info.id == id) {
            entry.position = to;
        }
        if self.dock(from).active == Some(id) {
            let next = self.panels_in(from).next().map(|p| p.info.id);
            let dock = self.dock_mut(from);
            dock.active = next;
            dock.open &= next.is_some();
        }
        let dock = self.dock_mut(to);
        dock.active = Some(id);
        dock.open = true;
        let _ = Settings::set_panel(id, "dock", to, cx);
        self.save_dock(from, cx);
        self.save_dock(to, cx);
        cx.notify();
    }

    /// The open dock's active panel, framed with a border on the side
    /// facing the tab area. `None` while the dock is closed.
    pub(super) fn render_dock(&self, position: DockPosition, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dock = self.dock(position);
        if !dock.open {
            return None;
        }
        let view = self.panel(dock.active?)?.view.clone();

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
