//! The status bar's individual items, as in the Zed edition: project search,
//! the language server menu, diagnostics, the active file, the activity
//! indicator, pending keystrokes and the language picker.

use std::path::MAIN_SEPARATOR_STR;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext, Context, Hsla, IntoElement, ParentElement, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    kbd::Kbd,
    menu::{DropdownMenu, PopupMenuItem},
    spinner::Spinner,
};

use super::ForgeShell;
use super::dummy::LspState;
use super::notifications::NoticeKind;
use super::panels::PlaceholderView;
use super::tabs::{BuildTabView, TabSpec};

fn lsp_color(state: LspState, cx: &App) -> Hsla {
    match state {
        LspState::Running => cx.theme().success,
        LspState::Starting => cx.theme().warning,
        LspState::Warning => cx.theme().warning,
        LspState::Error => cx.theme().danger,
        LspState::Stopped => cx.theme().muted_foreground,
    }
}

impl ForgeShell {
    /// Opens (or focuses) a tab showing a placeholder, for views that come
    /// with later phases (project search, diagnostics, server logs).
    pub(super) fn open_placeholder_tab(
        &mut self,
        key: &str,
        title: &str,
        icon: Icon,
        description: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view_title: SharedString = title.to_string().into();
        let build: BuildTabView = Rc::new(move |_, cx| {
            let title = view_title.clone();
            cx.new(|_| PlaceholderView {
                title,
                description: description.into(),
            })
            .into()
        });
        let spec = TabSpec {
            key: key.to_string().into(),
            title: title.to_string().into(),
            icon,
            path: None,
            symbol: None,
            dirty: false,
            build,
        };
        self.open_tab(spec, window, cx);
    }

    pub(super) fn open_project_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = "Project-wide search arrives with the editor (phase 5).";
        self.open_placeholder_tab(
            "project-search",
            "Project Search",
            Icon::new(IconName::Search),
            note,
            window,
            cx,
        );
    }

    pub(super) fn render_search_button(&self, cx: &mut Context<Self>) -> AnyElement {
        Button::new("project-search")
            .ghost()
            .xsmall()
            .icon(IconName::Search)
            .tooltip("Project Search")
            .on_click(cx.listener(|this, _, window, cx| this.open_project_search(window, cx)))
            .into_any_element()
    }

    fn set_server_state(&mut self, name: &str, state: LspState, cx: &mut Context<Self>) {
        for server in self
            .status
            .language_servers
            .iter_mut()
            .filter(|s| name.is_empty() || s.name == name)
        {
            server.state = state;
            if state != LspState::Warning {
                server.message = None;
            }
        }
        cx.notify();
    }

    /// Restart one server (or all, with an empty name): "Starting…", then
    /// "Running" a moment later.
    fn restart_servers(&mut self, name: SharedString, cx: &mut Context<Self>) {
        self.set_server_state(&name, LspState::Starting, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.set_server_state(&name, LspState::Running, cx)
            });
        })
        .detach();
    }

    /// The ⚡ button and Zed's language server menu: the project, each server
    /// with its status and a submenu, then restart/stop all.
    pub(super) fn render_lsp_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let servers = &self.status.language_servers;
        let worst = [LspState::Error, LspState::Warning]
            .into_iter()
            .find(|state| servers.iter().any(|s| s.state == *state));
        let summary = match worst {
            Some(LspState::Error) => "Server with errors",
            Some(_) => "Server with warnings",
            None => "All Servers Operational",
        };
        let this = cx.weak_entity();

        let button = Button::new("language-servers")
            .ghost()
            .xsmall()
            .icon(Icon::empty().path("icons/zed_bolt_outlined.svg"))
            .tooltip(format!("Language Servers · {summary}"))
            .dropdown_menu(move |menu, window, cx| {
                let Some(shell) = this.upgrade() else { return menu };
                let shell_ref = shell.read(cx);
                let project = shell_ref.workspace_label();
                let servers: Vec<_> = shell_ref
                    .status
                    .language_servers
                    .iter()
                    .map(|s| (s.name.clone(), s.state, s.message.clone()))
                    .collect();

                let mut menu = menu.label(project);
                for (name, state, message) in servers {
                    let dot = Icon::empty().path("icons/zed_circle.svg").text_color(lsp_color(state, cx));
                    let this = this.clone();
                    menu = menu.submenu_with_icon(Some(dot), name.clone(), window, cx, move |sub, _, _| {
                        let mut sub = sub.label(state.label());
                        if let Some(message) = message.clone() {
                            let (this, name) = (this.clone(), name.clone());
                            sub = sub.item(PopupMenuItem::new("View Message").on_click(move |_, window, cx| {
                                let message = message.clone();
                                let name = name.clone();
                                this.update(cx, |this, cx| {
                                    this.notify(NoticeKind::Warning, name, message, window, cx)
                                })
                                .ok();
                            }));
                        }
                        let (logs, restart, stop) = (this.clone(), this.clone(), this.clone());
                        let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
                        sub.item(PopupMenuItem::new("View Logs").on_click(move |_, window, cx| {
                            let title = format!("{n1} logs");
                            logs.update(cx, |this, cx| {
                                let note = "Server logs arrive with the language server support (phase 5).";
                                this.open_placeholder_tab(&format!("lsp-logs-{n1}"), &title, Icon::new(IconName::File), note, window, cx)
                            })
                            .ok();
                        }))
                        .separator()
                        .item(PopupMenuItem::new("Restart Server").on_click(move |_, _, cx| {
                            restart.update(cx, |this, cx| this.restart_servers(n2.clone(), cx)).ok();
                        }))
                        .item(
                            PopupMenuItem::new("Stop Server")
                                .disabled(state == LspState::Stopped)
                                .on_click(move |_, _, cx| {
                                    stop.update(cx, |this, cx| this.set_server_state(&n3, LspState::Stopped, cx)).ok();
                                }),
                        )
                    });
                }
                let (restart, stop) = (this.clone(), this.clone());
                menu.separator()
                    .item(PopupMenuItem::new("Restart All Servers").on_click(move |_, _, cx| {
                        restart.update(cx, |this, cx| this.restart_servers("".into(), cx)).ok();
                    }))
                    .item(PopupMenuItem::new("Stop All Servers").on_click(move |_, _, cx| {
                        stop.update(cx, |this, cx| this.set_server_state("", LspState::Stopped, cx)).ok();
                    }))
            });

        // A dot on the button while any server has a warning or an error.
        div()
            .relative()
            .child(button)
            .when_some(worst, |el, state| {
                el.child(
                    div()
                        .absolute()
                        .top(px(2.))
                        .right(px(2.))
                        .size(px(5.))
                        .rounded_full()
                        .bg(lsp_color(state, cx)),
                )
            })
            .into_any_element()
    }

    /// ✓ when the project is clean, otherwise error and warning counts.
    /// Opens the Diagnostics tab.
    pub(super) fn render_diagnostics(&self, cx: &mut Context<Self>) -> AnyElement {
        let (errors, warnings) = (self.status.errors, self.status.warnings);
        let count = |icon: IconName, n: usize, color| {
            h_flex()
                .gap_0p5()
                .child(div().text_color(color).child(Icon::new(icon).xsmall()))
                .child(n.to_string())
        };
        let content = if errors == 0 && warnings == 0 {
            h_flex().child(
                Icon::new(IconName::Check)
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            )
        } else {
            h_flex()
                .gap_2()
                .when(errors > 0, |el| {
                    el.child(count(IconName::CircleX, errors, cx.theme().danger))
                })
                .when(warnings > 0, |el| {
                    el.child(count(IconName::TriangleAlert, warnings, cx.theme().warning))
                })
        };

        Button::new("diagnostics")
            .ghost()
            .xsmall()
            .tooltip("Project Diagnostics")
            .child(content.text_xs())
            .on_click(cx.listener(|this, _, window, cx| {
                let note =
                    "The diagnostics list arrives with the language server support (phase 5).";
                this.open_placeholder_tab(
                    "diagnostics",
                    "Diagnostics",
                    Icon::new(IconName::TriangleAlert),
                    note,
                    window,
                    cx,
                );
            }))
            .into_any_element()
    }

    /// Zed's merge-conflict indicator: shown while the repository has
    /// conflicts; opens the Git panel.
    pub(super) fn render_merge_conflicts(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let conflicts = forge_host::Workspace::model(cx).read(cx).git()?.conflicts();
        if conflicts == 0 {
            return None;
        }
        let label = if conflicts == 1 {
            "1 merge conflict".to_string()
        } else {
            format!("{conflicts} merge conflicts")
        };
        Some(
            Button::new("merge-conflicts")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning))
                .label(label)
                .tooltip("Resolve in the Git panel")
                .on_click(cx.listener(|this, _, _, cx| this.show_panel("git", cx)))
                .into_any_element(),
        )
    }

    /// The active file's path, when a file tab is active.
    pub(super) fn render_active_file(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let path = self.active_tab_in(self.active_group)?.path.clone()?;
        Some(self.status_text(path.replace('/', MAIN_SEPARATOR_STR), cx))
    }

    /// Progress of background work (language servers indexing, downloads).
    pub(super) fn render_activity(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let message = self.status.activity.clone()?;
        Some(
            h_flex()
                .gap_1()
                .child(Spinner::new().xsmall().icon(IconName::Loader))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(message),
                )
                .into_any_element(),
        )
    }

    /// The first half of a chord while it waits for the rest, e.g. `Ctrl+K`.
    pub(super) fn render_pending_keys(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let keys = window.pending_input_keystrokes()?;
        let text = keys.iter().map(Kbd::format).collect::<Vec<_>>().join(" ");
        Some(self.status_text(format!("{text} …"), cx))
    }

    /// The active file's language; opens a picker, as in Zed.
    pub(super) fn render_language_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.weak_entity();
        let current = self.status.editor.language.clone();
        let languages = self.status.languages.clone();

        Button::new("language-picker")
            .ghost()
            .xsmall()
            .label(current.clone())
            .text_color(cx.theme().muted_foreground)
            .tooltip("Select Language")
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.scrollable(true).max_h(px(360.));
                for language in &languages {
                    let (this, name) = (this.clone(), language.clone());
                    menu = menu.item(
                        PopupMenuItem::new(language.clone())
                            .checked(*language == current)
                            .on_click(move |_, _, cx| {
                                let name = name.clone();
                                this.update(cx, |this, cx| {
                                    this.status.editor.language = name;
                                    cx.notify();
                                })
                                .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }
}
