//! The default theme: "Forge Dark", the Zed edition's theme translated to
//! gpui-component's format (`assets/themes/forge.json`). The title bar's
//! toggle switches to gpui-component's Default Light and back.
//!
//! Font sizes default to the Zed edition's (`ui_font_size: 13`,
//! `buffer_font_size: 15`). The mode and sizes are settings (`theme`,
//! `ui_font_size`, `buffer_font_size` in `settings.json`): read at startup,
//! saved whenever they change.

use forge_host::Settings;
use gpui::{App, px};
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

const FORGE_THEMES: &str = include_str!("../../assets/themes/forge.json");
const DEFAULT_DARK: &str = "Forge Dark";
const UI_FONT_SIZE: f32 = 13.;
const MONO_FONT_SIZE: f32 = 15.;

pub(super) fn init(cx: &mut App) {
    if let Err(err) = ThemeRegistry::global_mut(cx).load_themes_from_str(FORGE_THEMES) {
        eprintln!("failed to load the Forge themes: {err}");
    }
    if let Some(dark) = ThemeRegistry::global(cx).themes().get(DEFAULT_DARK).cloned() {
        Theme::global_mut(cx).dark_theme = dark;
    }
    let mode = match Settings::get::<String>("theme", cx).as_deref() {
        Some("light") => ThemeMode::Light,
        _ => ThemeMode::Dark,
    };
    Theme::change(mode, None, cx);

    let ui = Settings::get("ui_font_size", cx).unwrap_or(UI_FONT_SIZE);
    let mono = Settings::get("buffer_font_size", cx).unwrap_or(MONO_FONT_SIZE);
    let theme = Theme::global_mut(cx);
    theme.font_size = px(ui);
    theme.mono_font_size = px(mono);
    Theme::sync_base(cx);
}

/// Saves the theme mode and font sizes; unchanged values aren't rewritten.
pub(super) fn save(cx: &mut App) {
    let theme = Theme::global(cx);
    let mode = if theme.mode.is_dark() { "dark" } else { "light" };
    let (ui, mono) = (f32::from(theme.font_size), f32::from(theme.mono_font_size));
    let _ = Settings::set("theme", mode, cx);
    let _ = Settings::set("ui_font_size", ui, cx);
    let _ = Settings::set("buffer_font_size", mono, cx);
}
