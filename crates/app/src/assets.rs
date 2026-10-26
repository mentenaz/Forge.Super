//! Combines gpui-component's bundled icons (everything `IconName` resolves
//! to) with the app's own `assets/` folder (custom icons, graphics), both
//! embedded in the binary.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
struct AppAssets;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = AppAssets::get(path) {
            return Ok(Some(file.data));
        }
        // gpui-component returns `Err` (not `Ok(None)`) for a path it doesn't
        // embed; treat that as "not found" too.
        Ok(gpui_component_assets::Assets.load(path).ok().flatten())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = gpui_component_assets::Assets.list(path).unwrap_or_default();
        out.extend(
            AppAssets::iter()
                .filter(|p| p.starts_with(path))
                .map(|p| SharedString::from(p.into_owned())),
        );
        Ok(out)
    }
}
