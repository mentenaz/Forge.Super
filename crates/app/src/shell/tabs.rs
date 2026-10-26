//! Workspace tab state and the operations on it. There are two tab groups
//! (panes): the primary one and an optional split beside it. Like Zed,
//! pinned tabs stay at the start of their group, "close others/all" skip
//! pinned tabs, and an emptied pane folds away.

use gpui::{AnyView, Context, SharedString, Window};
use gpui_component::Icon;

use super::ForgeShell;

/// Builds a fresh view for a tab — kept on the tab so "Duplicate in Split
/// View" can create a second, independent instance of the same content.
pub(super) use forge_host::BuildView as BuildTabView;
/// What `open_tab` needs to create a tab; panels send it through
/// `forge_host::Host::open_tab`.
pub(super) use forge_host::TabRequest as TabSpec;

#[derive(Clone)]
pub(super) struct WorkspaceTab {
    pub(super) id: usize,
    /// Identifies what the tab shows (e.g. a file path); opening the same
    /// key again focuses the existing tab instead of adding a duplicate.
    pub(super) key: SharedString,
    pub(super) title: SharedString,
    /// Shown for non-file tabs only; file tabs have no icon, as in Zed.
    pub(super) icon: Icon,
    /// Workspace-relative `/`-separated path, for file tabs.
    pub(super) path: Option<SharedString>,
    /// Breadcrumb symbol (dummy until the editor is embedded).
    pub(super) symbol: Option<SharedString>,
    pub(super) dirty: bool,
    pub(super) pinned: bool,
    pub(super) build: BuildTabView,
    pub(super) view: AnyView,
}

/// Which pane a tab (or its context menu) belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TabGroup {
    Primary,
    Split,
}

/// The index to keep active after removing `removed` from a group that now
/// has `len` tabs.
fn active_after_remove(active: Option<usize>, removed: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match active {
        Some(active) if active > removed => Some(active - 1),
        Some(active) if active == removed => Some(removed.saturating_sub(1).min(len - 1)),
        other => other,
    }
}

impl ForgeShell {
    pub(super) fn group(&self, group: TabGroup) -> &Vec<WorkspaceTab> {
        match group {
            TabGroup::Primary => &self.tabs,
            TabGroup::Split => &self.split_tabs,
        }
    }

    fn group_mut(&mut self, group: TabGroup) -> (&mut Vec<WorkspaceTab>, &mut Option<usize>) {
        match group {
            TabGroup::Primary => (&mut self.tabs, &mut self.active_tab),
            TabGroup::Split => (&mut self.split_tabs, &mut self.split_active_tab),
        }
    }

    pub(super) fn active_index(&self, group: TabGroup) -> Option<usize> {
        match group {
            TabGroup::Primary => self.active_tab,
            TabGroup::Split => self.split_active_tab,
        }
    }

    pub(super) fn active_tab_in(&self, group: TabGroup) -> Option<&WorkspaceTab> {
        self.active_index(group)
            .and_then(|idx| self.group(group).get(idx))
    }

    pub(super) fn split_open(&self) -> bool {
        !self.split_tabs.is_empty()
    }

    /// Where the tab with `id` is.
    pub(super) fn find_tab(&self, id: usize) -> Option<(TabGroup, usize)> {
        [TabGroup::Primary, TabGroup::Split]
            .into_iter()
            .find_map(|g| {
                self.group(g)
                    .iter()
                    .position(|t| t.id == id)
                    .map(|idx| (g, idx))
            })
    }

    fn next_id(&mut self) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }

    /// Keeps the groups in shape after any change: pinned tabs first, an
    /// empty primary pane takes over the split's tabs, and the active pane
    /// always exists.
    fn normalize_groups(&mut self) {
        for group in [TabGroup::Primary, TabGroup::Split] {
            let (tabs, active) = self.group_mut(group);
            let active_id = active.and_then(|idx| tabs.get(idx)).map(|t| t.id);
            tabs.sort_by_key(|t| !t.pinned);
            *active = active_id.and_then(|id| tabs.iter().position(|t| t.id == id));
            if active.is_none() && !tabs.is_empty() {
                *active = Some(0);
            }
        }
        if self.tabs.is_empty() && !self.split_tabs.is_empty() {
            self.tabs = std::mem::take(&mut self.split_tabs);
            self.active_tab = self.split_active_tab.take();
        }
        if !self.split_open() {
            self.split_active_tab = None;
            self.active_group = TabGroup::Primary;
        }
    }

    /// Open `spec` in the active pane, or activate it if it's already open
    /// there.
    pub(super) fn open_tab(&mut self, spec: TabSpec, window: &mut Window, cx: &mut Context<Self>) {
        let group = self.active_group;
        if let Some(idx) = self.group(group).iter().position(|t| t.key == spec.key) {
            self.activate(group, idx, cx);
            return;
        }
        let view = (spec.build)(window, cx);
        let tab = WorkspaceTab {
            id: self.next_id(),
            key: spec.key,
            title: spec.title,
            icon: spec.icon,
            path: spec.path,
            symbol: spec.symbol,
            dirty: spec.dirty,
            pinned: false,
            build: spec.build,
            view,
        };
        let (tabs, active) = self.group_mut(group);
        tabs.push(tab);
        *active = Some(tabs.len() - 1);
        cx.notify();
    }

    fn push_or_activate(&mut self, group: TabGroup, tab: WorkspaceTab) {
        let (tabs, active) = self.group_mut(group);
        match tabs.iter().position(|t| t.key == tab.key) {
            Some(idx) => *active = Some(idx),
            None => {
                tabs.push(tab);
                *active = Some(tabs.len() - 1);
            }
        }
    }

    pub(super) fn activate(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        let (tabs, active) = self.group_mut(group);
        if idx < tabs.len() {
            *active = Some(idx);
            self.active_group = group;
            cx.notify();
        }
    }

    /// Primary-pane shorthand, used by the tab switcher.
    pub(super) fn activate_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.activate(TabGroup::Primary, idx, cx);
    }

    pub(super) fn close(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        let (tabs, active) = self.group_mut(group);
        if idx >= tabs.len() {
            return;
        }
        let closed = tabs.remove(idx);
        *active = active_after_remove(*active, idx, tabs.len());
        self.tab_mru.retain(|&id| id != closed.id);
        self.normalize_groups();
        cx.notify();
    }

    /// Close the unpinned tabs in `group` that `keep` rejects.
    fn close_where(
        &mut self,
        group: TabGroup,
        cx: &mut Context<Self>,
        keep: impl Fn(usize, &WorkspaceTab) -> bool,
    ) {
        let (tabs, active) = self.group_mut(group);
        let active_id = active.and_then(|idx| tabs.get(idx)).map(|t| t.id);
        let mut idx = 0;
        tabs.retain(|tab| {
            let retain = tab.pinned || keep(idx, tab);
            idx += 1;
            retain
        });
        *active = active_id.and_then(|id| tabs.iter().position(|t| t.id == id));
        let open: Vec<usize> = self
            .tabs
            .iter()
            .chain(&self.split_tabs)
            .map(|t| t.id)
            .collect();
        self.tab_mru.retain(|id| open.contains(id));
        self.normalize_groups();
        cx.notify();
    }

    pub(super) fn close_others(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        self.close_where(group, cx, |i, _| i == idx);
    }

    pub(super) fn close_left(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        self.close_where(group, cx, |i, _| i >= idx);
    }

    pub(super) fn close_right(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        self.close_where(group, cx, |i, _| i <= idx);
    }

    pub(super) fn close_all(&mut self, group: TabGroup, cx: &mut Context<Self>) {
        self.close_where(group, cx, |_, _| false);
    }

    pub(super) fn close_split_pane(&mut self, cx: &mut Context<Self>) {
        self.split_tabs.clear();
        self.normalize_groups();
        cx.notify();
    }

    pub(super) fn toggle_pin(&mut self, group: TabGroup, idx: usize, cx: &mut Context<Self>) {
        let (tabs, _) = self.group_mut(group);
        if let Some(tab) = tabs.get_mut(idx) {
            tab.pinned = !tab.pinned;
        }
        self.normalize_groups();
        cx.notify();
    }

    /// Move tab `id` to position `to_idx` in `to` (a drag and drop). Dropping
    /// it among the pinned tabs pins it; dropping it after them unpins it.
    pub(super) fn move_tab(
        &mut self,
        id: usize,
        to: TabGroup,
        to_idx: usize,
        cx: &mut Context<Self>,
    ) {
        let Some((from, from_idx)) = self.find_tab(id) else {
            return;
        };
        let (tabs, active) = self.group_mut(from);
        let mut tab = tabs.remove(from_idx);
        *active = active_after_remove(*active, from_idx, tabs.len());

        let (tabs, active) = self.group_mut(to);
        let to_idx = to_idx.min(tabs.len());
        let pinned_count = tabs.iter().filter(|t| t.pinned).count();
        tab.pinned = to_idx < pinned_count || (tab.pinned && to_idx == pinned_count);
        tabs.insert(to_idx, tab);
        *active = Some(to_idx);
        self.active_group = to;
        self.normalize_groups();
        cx.notify();
    }

    /// Open a second, independent instance of the primary tab at `idx` in
    /// the split pane.
    pub(super) fn duplicate_tab_in_split(
        &mut self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get(idx).cloned() else {
            return;
        };
        if let Some(existing) = self.split_tabs.iter().position(|t| t.key == tab.key) {
            self.split_active_tab = Some(existing);
        } else {
            let view = (tab.build)(window, cx);
            let duplicate = WorkspaceTab {
                id: self.next_id(),
                view,
                pinned: false,
                ..tab
            };
            self.push_or_activate(TabGroup::Split, duplicate);
        }
        self.active_group = TabGroup::Split;
        cx.notify();
    }

    pub(super) fn duplicate_active_tab_in_split(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(idx) = self.active_tab {
            self.duplicate_tab_in_split(idx, window, cx);
        }
    }

    pub(super) fn move_tab_to_split(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(idx) {
            let id = tab.id;
            let len = self.split_tabs.len();
            self.move_tab(id, TabGroup::Split, len, cx);
        }
    }

    pub(super) fn move_split_tab_to_editor(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.split_tabs.get(idx) {
            let id = tab.id;
            let len = self.tabs.len();
            self.move_tab(id, TabGroup::Primary, len, cx);
        }
    }

    /// Swap the active primary tab with the active split tab.
    pub(super) fn swap_panes(&mut self, cx: &mut Context<Self>) {
        let (Some(p), Some(s)) = (self.active_tab, self.split_active_tab) else {
            return;
        };
        if p < self.tabs.len() && s < self.split_tabs.len() {
            std::mem::swap(&mut self.tabs[p], &mut self.split_tabs[s]);
            self.normalize_groups();
            cx.notify();
        }
    }
}
