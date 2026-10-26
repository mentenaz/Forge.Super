//! The Script Runner's view: header, command row, working folder, the
//! debug-script or links bar, and the output.

use forge_script_runner::output::url_ranges;
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, input::Input, spinner::Spinner, v_flex,
};

use crate::ScriptRunnerPanel;

/// The longest debug script shown before it is cut short (it is copied
/// whole).
const DEBUG_SCRIPT_SHOWN: usize = 72;

/// How many of the newest output lines are drawn.
const LINES_SHOWN: usize = 200;

/// One line of output, its first `http(s)://` URL (if any) a link that opens
/// in the browser, e.g. the dev-server URL a tool like Vite prints on start.
fn output_line(line: &str, muted: Hsla, link: Hsla, cx: &App) -> AnyElement {
    let font = cx.theme().mono_font_family.clone();
    let Some(&(start, end)) = url_ranges(line).first() else {
        return div()
            .font_family(font)
            .text_xs()
            .text_color(muted)
            .child(line.to_string())
            .into_any_element();
    };
    let url = line[start..end].to_string();
    h_flex()
        .font_family(font)
        .text_xs()
        .child(div().text_color(muted).child(line[..start].to_string()))
        .child(
            div()
                .id(SharedString::from(format!("runner-link-{url}")))
                .text_color(link)
                .cursor_pointer()
                .hover(|d| d.underline())
                .on_click({
                    let url = url.clone();
                    move |_, _, cx: &mut App| cx.open_url(&url)
                })
                .child(url),
        )
        .child(div().text_color(muted).child(line[end..].to_string()))
        .into_any_element()
}

/// A small text button in the panel's style.
fn text_button(
    id: &'static str,
    label: &'static str,
    background: Hsla,
    text: Hsla,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .bg(background)
        .text_color(text)
        .child(label)
}

impl ScriptRunnerPanel {
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        h_flex()
            .gap_2()
            .items_center()
            .px_3()
            .py_1p5()
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                Icon::empty()
                    .path("icons/zed_runner.svg")
                    .small()
                    .text_color(theme.foreground),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_xs()
                    .text_color(theme.foreground)
                    .child("Script Runner"),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .when(self.running, |row| row.child(Spinner::new().xsmall()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if self.running {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .child(if self.running { "Running…" } else { "Idle" }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("{} lines", self.output.len())),
            )
    }

    fn render_command_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let can_run = !self.running && !self.command_input.read(cx).value().trim().is_empty();
        let theme = cx.theme();
        let (run_background, run_text) = if can_run {
            (theme.success, theme.background)
        } else {
            (theme.secondary, theme.muted_foreground)
        };
        let (danger, background) = (theme.danger, theme.background);
        h_flex()
            .gap_2()
            .items_center()
            .px_3()
            .py_1()
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.command_input).xsmall()),
            )
            .child(
                text_button("runner-run-btn", "Run", run_background, run_text).on_click(
                    cx.listener(|this, _, _, cx| {
                        let command = this.command_input.read(cx).value().to_string();
                        this.run(command, Vec::new(), cx);
                    }),
                ),
            )
            .when(self.running, |row| {
                row.child(
                    text_button("runner-stop-btn", "Stop", danger, background)
                        .on_click(cx.listener(|this, _, _, _| this.stop())),
                )
            })
    }

    fn render_cwd_row(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let cwd = self
            .cwd
            .as_ref()
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_else(|| "No folder open: commands run in the IDE's own folder".to_string());
        h_flex()
            .gap_1()
            .items_center()
            .px_3()
            .py_0p5()
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("cwd"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(theme.mono_font_family.clone())
                    .text_xs()
                    .text_color(theme.foreground)
                    .child(cwd),
            )
    }

    /// The SPFx debug query string (`gulp serve` / `heft start`), when the
    /// run printed one.
    fn render_debug_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let script = self.debug_script.clone()?;
        let shown: String = if script.chars().count() > DEBUG_SCRIPT_SHOWN {
            script.chars().take(DEBUG_SCRIPT_SHOWN).collect::<String>() + "\u{2026}"
        } else {
            script.clone()
        };
        let copied = self.debug_copied;
        let theme = cx.theme();
        let copy_color = if copied {
            theme.success
        } else {
            theme.muted_foreground
        };
        let (hover_bg, hover_text) = (theme.list_hover, theme.foreground);
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .px_3()
                .py_0p5()
                .flex_shrink_0()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.primary)
                        .child("Debug script"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(theme.mono_font_family.clone())
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(shown),
                )
                .child(
                    h_flex()
                        .id("runner-debug-copy")
                        .items_center()
                        .gap_1()
                        .flex_shrink_0()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(copy_color)
                        .hover(move |d| d.bg(hover_bg).text_color(hover_text))
                        .child(
                            Icon::new(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .xsmall(),
                        )
                        .child(if copied { "Copied" } else { "Copy" })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.copy_debug_script(script.clone(), cx)
                        })),
                ),
        )
    }

    /// Every link the run printed, each with a copy button. Not shown when
    /// there is a debug script, which takes its place.
    fn render_links_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.debug_script.is_some() || self.detected_links.is_empty() {
            return None;
        }
        let theme = cx.theme();
        let (secondary, link, mono) = (theme.secondary, theme.link, theme.mono_font_family.clone());
        let (success, muted) = (theme.success, theme.muted_foreground);
        let links = self.detected_links.iter().cloned().map(|url| {
            let copied = self.link_copied.as_deref() == Some(url.as_str());
            let copy_color = if copied { success } else { muted };
            h_flex()
                .gap_1()
                .items_center()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(secondary)
                .child(
                    div()
                        .id(SharedString::from(format!("runner-open-link-{url}")))
                        .cursor_pointer()
                        .font_family(mono.clone())
                        .text_xs()
                        .text_color(link)
                        .hover(|d| d.underline())
                        .child(url.clone())
                        .on_click({
                            let url = url.clone();
                            move |_, _, cx: &mut App| cx.open_url(&url)
                        }),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("runner-copy-link-{url}")))
                        .cursor_pointer()
                        .text_color(copy_color)
                        .child(
                            Icon::new(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .xsmall(),
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.copy_link(url.clone(), cx)),
                        ),
                )
        });
        let theme = cx.theme();
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .flex_wrap()
                .px_3()
                .py_0p5()
                .flex_shrink_0()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Links"),
                )
                .children(links.collect::<Vec<_>>()),
        )
    }
}

impl Render for ScriptRunnerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.render_header(cx);
        let command_row = self.render_command_row(cx);
        let cwd_row = self.render_cwd_row(cx);
        let debug_bar = self.render_debug_bar(cx);
        let links_bar = self.render_links_bar(cx);

        let theme = cx.theme();
        let (muted, link) = (theme.muted_foreground, theme.link);
        let skip = self.output.len().saturating_sub(LINES_SHOWN);
        let lines: Vec<AnyElement> = self.output[skip..]
            .iter()
            .map(|line| output_line(line, muted, link, cx))
            .collect();

        let theme = cx.theme();
        v_flex()
            .id("script-runner-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .bg(theme.background)
            .child(header)
            .child(command_row)
            .child(cwd_row)
            .children(debug_bar)
            .children(links_bar)
            .child(
                div()
                    .id("runner-output-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .bg(theme.secondary)
                    .px_3()
                    .py_1()
                    .children(lines),
            )
    }
}
