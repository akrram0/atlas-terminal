//! Atlas Terminal — native shell (GPUI + alacritty_terminal). No webview, no CSS.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod keymap;
mod session;
mod theme;
mod view;

use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None, // frameless: the app draws its own titlebar
                window_min_size: Some(size(px(640.0), px(420.0))),
                is_resizable: true,
                is_movable: true,
                is_minimizable: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| view::AtlasView::new(window, cx)),
        )
        .expect("failed to open Atlas window");
        cx.activate(true);
    });
}
