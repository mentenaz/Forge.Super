//! Notifications: a toast for each new notice (bottom-right, above the status
//! bar) plus a notification centre behind the bell at the right end of the
//! status bar that keeps the history.

use gpui::{
    Anchor, App, Context, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, WindowExt,
    badge::Badge,
    button::{Button, ButtonVariants},
    h_flex,
    notification::Notification,
    popover::Popover,
    v_flex,
};

use super::ForgeShell;

pub(super) use forge_host::NoticeKind;

/// How each kind looks in the notification centre.
trait NoticeKindStyle {
    fn icon(self) -> IconName;
    fn color(self, cx: &App) -> Hsla;
}

impl NoticeKindStyle for NoticeKind {
    fn icon(self) -> IconName {
        match self {
            NoticeKind::Info => IconName::Info,
            NoticeKind::Success => IconName::CircleCheck,
            NoticeKind::Warning => IconName::TriangleAlert,
            NoticeKind::Error => IconName::CircleX,
        }
    }

    fn color(self, cx: &App) -> Hsla {
        match self {
            NoticeKind::Info => cx.theme().info,
            NoticeKind::Success => cx.theme().success,
            NoticeKind::Warning => cx.theme().warning,
            NoticeKind::Error => cx.theme().danger,
        }
    }
}

pub(super) struct NoticeEntry {
    pub(super) id: usize,
    pub(super) kind: NoticeKind,
    pub(super) title: gpui::SharedString,
    pub(super) message: gpui::SharedString,
    pub(super) time: gpui::SharedString,
    pub(super) read: bool,
}

impl ForgeShell {
    /// Show a toast and record it in the notification centre.
    pub(super) fn notify(
        &mut self,
        kind: NoticeKind,
        title: impl Into<gpui::SharedString>,
        message: impl Into<gpui::SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, message) = (title.into(), message.into());
        let toast = match kind {
            NoticeKind::Info => Notification::info(message.clone()),
            NoticeKind::Success => Notification::success(message.clone()),
            NoticeKind::Warning => Notification::warning(message.clone()),
            NoticeKind::Error => Notification::error(message.clone()),
        };
        window.push_notification(toast.title(title.clone()), cx);

        let id = self.notices.iter().map(|n| n.id + 1).max().unwrap_or(0);
        self.notices.insert(
            0,
            NoticeEntry {
                id,
                kind,
                title,
                message,
                time: "Just now".into(),
                read: self.notifications_open,
            },
        );
        cx.notify();
    }

    fn unread_notices(&self) -> usize {
        self.notices.iter().filter(|n| !n.read).count()
    }

    pub(super) fn render_notification_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let popover = Popover::new("notification-center")
            .anchor(Anchor::BottomRight)
            .open(self.notifications_open)
            .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                this.notifications_open = *open;
                if *open {
                    this.notices.iter_mut().for_each(|n| n.read = true);
                }
                cx.notify();
            }))
            .trigger(
                Button::new("notification-bell")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Bell)
                    .tooltip("Notifications"),
            )
            .w(px(360.))
            .p_0()
            .child(self.render_notification_center(cx));

        Badge::new()
            .count(self.unread_notices())
            .max(99)
            .child(popover)
    }

    fn render_notification_center(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .justify_between()
            .items_center()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().text_sm().font_semibold().child("Notifications"))
            .when(!self.notices.is_empty(), |el| {
                el.child(
                    Button::new("clear-notifications")
                        .ghost()
                        .xsmall()
                        .label("Clear all")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.notices.clear();
                            cx.notify();
                        })),
                )
            });

        let body = if self.notices.is_empty() {
            div()
                .px_3()
                .py_6()
                .text_sm()
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child("No notifications")
                .into_any_element()
        } else {
            let rows = self.notices.iter().map(|notice| {
                let id = notice.id;
                h_flex()
                    .id(("notice", id))
                    .items_start()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .hover(|el| el.bg(cx.theme().list_hover))
                    .child(
                        div()
                            .pt_0p5()
                            .text_color(notice.kind.color(cx))
                            .child(Icon::new(notice.kind.icon()).small()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_medium()
                                            .truncate()
                                            .child(notice.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(notice.time.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(notice.message.clone()),
                            ),
                    )
                    .child(
                        Button::new(("dismiss-notice", id))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.notices.retain(|n| n.id != id);
                                cx.notify();
                            })),
                    )
            });
            v_flex()
                .id("notice-list")
                .max_h(px(420.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        v_flex().child(header).child(body)
    }
}
