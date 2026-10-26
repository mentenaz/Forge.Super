//! Back/forward navigation through previously active tabs (the arrows at
//! the start of each tab bar), browser-style: going somewhere new clears
//! the forward stack.

use gpui::Context;

use super::ForgeShell;

impl ForgeShell {
    /// The tab the user is looking at: the active tab of the active pane.
    fn current_tab_id(&self) -> Option<usize> {
        self.active_tab_in(self.active_group).map(|t| t.id)
    }

    /// Records a history entry whenever the current tab changed. Called
    /// every render, so no call site that changes tabs has to remember to.
    pub(super) fn sync_nav_history(&mut self) {
        let current = self.current_tab_id();
        if current == self.last_seen_nav_tab {
            return;
        }
        if self.suppress_nav_record {
            self.suppress_nav_record = false;
        } else if let Some(previous) = self.last_seen_nav_tab {
            self.nav_back.push(previous);
            self.nav_forward.clear();
        }
        self.last_seen_nav_tab = current;
    }

    pub(super) fn can_go_back(&self) -> bool {
        self.nav_back.iter().any(|&id| self.find_tab(id).is_some())
    }

    pub(super) fn can_go_forward(&self) -> bool {
        self.nav_forward.iter().any(|&id| self.find_tab(id).is_some())
    }

    pub(super) fn go_back(&mut self, cx: &mut Context<Self>) {
        self.navigate(true, cx);
    }

    pub(super) fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.navigate(false, cx);
    }

    fn navigate(&mut self, back: bool, cx: &mut Context<Self>) {
        let current = self.current_tab_id();
        loop {
            let entry = if back { self.nav_back.pop() } else { self.nav_forward.pop() };
            let Some(id) = entry else { return };
            // Entries for tabs closed since are skipped.
            let Some((group, idx)) = self.find_tab(id) else { continue };
            if let Some(current) = current {
                if back { self.nav_forward.push(current) } else { self.nav_back.push(current) }
            }
            self.suppress_nav_record = true;
            self.activate(group, idx, cx);
            return;
        }
    }
}
