//! Atlas Terminal — native shell (GPUI + alacritty_terminal). No webview, no CSS.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod keymap;
mod session;
mod theme;
mod view;

use gpui::{App, AppContext as _, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn debug_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("atlas-native-debug.log")
}

pub(crate) fn log_line(msg: &str) {
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(debug_log_path())
    {
        let _ = writeln!(f, "{msg}");
    }
}

fn main() {
    // Fresh log per launch. Release builds have no console
    // (windows_subsystem), so this file is how startup and panics stay visible.
    let _ = std::fs::write(debug_log_path(), "atlas-native starting\n");
    std::panic::set_hook(Box::new(|info| {
        let mut msg = format!("PANIC: {info}\n");
        if let Some(loc) = info.location() {
            msg.push_str(&format!("  at {loc}\n"));
        }
        let _ = std::fs::write(std::env::temp_dir().join("atlas-native-debug.log"), msg);
    }));
    log_line("opening window");
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
        log_line("window opened");
        cx.activate(true);
    });
    log_line("event loop exited");
}
