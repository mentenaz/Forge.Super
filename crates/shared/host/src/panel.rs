//! Describing and registering panels, in place of Zed's `impl Panel`.
//!
//! A panel registers once at startup with its [`PanelInfo`] and a
//! constructor. The shell builds its docks and status-bar buttons from the
//! [`PanelRegistry`].

use std::rc::Rc;

use gpui::{AnyView, App, Global, Window};
use gpui_component::Icon;
use serde::{Deserialize, Serialize};

/// The three docks around the tab area, as in Zed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Left,
    Right,
    Bottom,
}

impl DockPosition {
    pub const ALL: [DockPosition; 3] = [DockPosition::Left, DockPosition::Right, DockPosition::Bottom];

    pub fn title(self) -> &'static str {
        match self {
            DockPosition::Left => "Left",
            DockPosition::Right => "Right",
            DockPosition::Bottom => "Bottom",
        }
    }

    /// The key used in settings and state (`"left"`, …).
    pub fn key(self) -> &'static str {
        match self {
            DockPosition::Left => "left",
            DockPosition::Right => "right",
            DockPosition::Bottom => "bottom",
        }
    }
}

/// Builds a view. Used for panels and for tab contents.
pub type BuildView = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

#[derive(Clone)]
pub struct PanelInfo {
    /// Stable id, used as the key in settings (`panels.<id>`) and state.
    pub id: &'static str,
    pub title: &'static str,
    pub icon: Icon,
    /// The status-bar button's tooltip; the title when `None`.
    pub tooltip: Option<&'static str>,
    /// Where the panel starts out; the user can move it.
    pub default_dock: DockPosition,
    /// Order of the panel's button within its dock.
    pub order: u32,
}

pub struct RegisteredPanel {
    pub info: PanelInfo,
    pub build: BuildView,
}

#[derive(Default)]
pub struct PanelRegistry {
    panels: Vec<RegisteredPanel>,
}

impl Global for PanelRegistry {}

impl PanelRegistry {
    pub fn panels(&self) -> &[RegisteredPanel] {
        &self.panels
    }

    pub fn contains(&self, id: &str) -> bool {
        self.panels.iter().any(|p| p.info.id == id)
    }
}

/// Registers a panel. The first registration of an id wins, so a feature
/// crate's panel replaces the shell's placeholder of the same id as long as
/// it registers first.
pub fn register_panel(
    cx: &mut App,
    info: PanelInfo,
    build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
) {
    let registry = cx.global_mut::<PanelRegistry>();
    if !registry.contains(info.id) {
        registry.panels.push(RegisteredPanel { info, build: Rc::new(build) });
    }
}
