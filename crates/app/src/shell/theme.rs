//! The default theme: "Forge Dark", the Zed edition's theme translated to
//! gpui-component's format (`assets/themes/forge.json`). The title bar's
//! toggle switches to gpui-component's Default Light and back.
//!
//! Font sizes match the Zed edition (`ui_font_size: 13`,
//! `buffer_font_size: 15`). Neither theme sets a size, so they survive the
//! light/dark toggle.

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
    Theme::change(ThemeMode::Dark, None, cx);

    let theme = Theme::global_mut(cx);
    theme.font_size = px(UI_FONT_SIZE);
    theme.mono_font_size = px(MONO_FONT_SIZE);
    Theme::sync_base(cx);
}
