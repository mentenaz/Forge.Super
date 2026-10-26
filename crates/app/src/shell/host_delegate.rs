//! The shell's side of `forge_host::Host`: panels' requests land here and
//! are forwarded to the `ForgeShell` entity.

use forge_host::{HostDelegate, NoticeKind, TabRequest};
use gpui::{App, SharedString, WeakEntity, Window};

use super::ForgeShell;

pub(super) struct ShellHost(pub(super) WeakEntity<ForgeShell>);

impl HostDelegate for ShellHost {
    fn open_tab(&self, tab: TabRequest, window: &mut Window, cx: &mut App) {
        let _ = self
            .0
            .update(cx, |shell, cx| shell.open_tab(tab, window, cx));
    }

    fn show_panel(&self, id: &str, _: &mut Window, cx: &mut App) {
        let _ = self.0.update(cx, |shell, cx| shell.show_panel(id, cx));
    }

    fn toggle_panel(&self, id: &str, _: &mut Window, cx: &mut App) {
        let _ = self.0.update(cx, |shell, cx| shell.toggle_panel(id, cx));
    }

    fn notify(
        &self,
        kind: NoticeKind,
        title: SharedString,
        message: SharedString,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = self.0.update(cx, |shell, cx| {
            shell.notify(kind, title, message, window, cx)
        });
    }
}
