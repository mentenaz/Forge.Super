//! Tabs that come back after a restart, in place of Zed's
//! `register_serializable_item`.
//!
//! A tab says what it is in a [`TabRestore`]: a `kind` and a JSON `payload`
//! (a file's path, a package manager's project). The shell saves those for
//! the open tabs. At the next start it asks the kind's restore function,
//! registered with [`register_tab_kind`], to turn the payload back into a
//! [`TabRequest`]. A tab without a `TabRestore`, or whose kind isn't
//! registered any more, is simply not reopened.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, Global, SharedString, Window};
use serde::Serialize;
use serde_json::Value;

use crate::host::TabRequest;

/// What a tab needs to be reopened.
#[derive(Clone, Debug, PartialEq)]
pub struct TabRestore {
    pub kind: SharedString,
    pub payload: Value,
}

impl TabRestore {
    pub fn new(kind: impl Into<SharedString>, payload: impl Serialize) -> Self {
        Self {
            kind: kind.into(),
            payload: serde_json::to_value(payload).unwrap_or(Value::Null),
        }
    }
}

/// Rebuilds a tab from its saved payload; `None` when it no longer applies
/// (the file is gone, the project was removed).
pub type RestoreTab = Rc<dyn Fn(&Value, &mut Window, &mut App) -> Option<TabRequest>>;

#[derive(Default)]
struct TabKinds(HashMap<SharedString, RestoreTab>);

impl Global for TabKinds {}

/// Registers how tabs of `kind` are rebuilt. Call from a feature's `init`;
/// registering a kind again replaces it.
pub fn register_tab_kind(
    cx: &mut App,
    kind: impl Into<SharedString>,
    restore: impl Fn(&Value, &mut Window, &mut App) -> Option<TabRequest> + 'static,
) {
    cx.default_global::<TabKinds>()
        .0
        .insert(kind.into(), Rc::new(restore));
}

/// The tab saved as `restore`, rebuilt by its kind's restore function.
pub fn restore_tab(restore: &TabRestore, window: &mut Window, cx: &mut App) -> Option<TabRequest> {
    let rebuild = cx.try_global::<TabKinds>()?.0.get(&restore.kind)?.clone();
    rebuild(&restore.payload, window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_component::Icon;

    fn request(key: &str) -> TabRequest {
        TabRequest {
            key: key.to_string().into(),
            title: key.to_string().into(),
            icon: Icon::empty(),
            path: None,
            symbol: None,
            dirty: false,
            restore: None,
            build: Rc::new(|_, _| unreachable!("not built in this test")),
        }
    }

    #[gpui::test]
    fn a_registered_kind_rebuilds_its_tab_and_an_unknown_one_does_not(cx: &mut TestAppContext) {
        cx.update(|cx| {
            register_tab_kind(cx, "demo", |payload, _, _| {
                // A payload that no longer applies gives no tab.
                payload.as_str().map(request)
            });
        });
        let window = cx.add_empty_window();
        let rebuilt = |kind: &str, payload: Value, cx: &mut gpui::VisualTestContext| {
            let restore = TabRestore {
                kind: kind.to_string().into(),
                payload,
            };
            cx.update(|window, cx| restore_tab(&restore, window, cx))
                .map(|tab| tab.key.to_string())
        };
        assert_eq!(
            rebuilt("demo", Value::from("a.txt"), window).as_deref(),
            Some("a.txt")
        );
        assert_eq!(rebuilt("demo", Value::Null, window), None);
        assert_eq!(rebuilt("other", Value::from("a.txt"), window), None);
    }

    #[test]
    fn restore_data_keeps_the_payload() {
        let restore = TabRestore::new("file", "C:/project/a.rs");
        assert_eq!(restore.kind, "file");
        assert_eq!(restore.payload, Value::from("C:/project/a.rs"));
    }
}
