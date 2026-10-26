mod assets;
mod shell;

use gpui::{App, AppContext, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, TitleBar};

use crate::assets::Assets;
use crate::shell::ForgeShell;

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx: &mut App| {
        gpui_component::init(cx);
        // The host first, then the feature crates register their panels,
        // then the shell fills the gaps with placeholders.
        forge_host::init(cx);
        explorer_ui::init(cx);
        git_ui::init(cx);
        node_ui::init(cx);
        dotnet_ui::init(cx);
        shell::init(cx);

        let bounds = WindowBounds::centered(size(px(1400.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                window_min_size: Some(size(px(480.0), px(320.0))),
                ..TitleBar::window_options()
            },
            |window, cx| {
                let shell = ForgeShell::view(window, cx);
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .expect("Failed to open Forge Super window");

        cx.activate(true);
    });
}
