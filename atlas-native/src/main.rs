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

// ---------------------------------------------------------------------------
// Windows 11 rounded corners.
//
// gpui creates the frameless window before we can reach DWM, so a helper
// thread waits for it to appear and then asks DWM for rounded corners via
// DWMWA_WINDOW_CORNER_PREFERENCE (the same approach Electron/Tauri use).
// On older Windows the call is a harmless no-op. Result is logged.
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod rounded_corners {
    type Hwnd = *mut core::ffi::c_void;
    type Lparam = isize;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, Lparam) -> i32, lparam: Lparam) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: Hwnd,
            attr: u32,
            value: *const core::ffi::c_void,
            size: u32,
        ) -> i32;
    }

    /// First visible, unowned top-level window belonging to this process.
    fn find_main_window() -> Option<Hwnd> {
        struct Search {
            pid: u32,
            found: Hwnd,
        }

        unsafe extern "system" fn visit(hwnd: Hwnd, lparam: Lparam) -> i32 {
            // Safety: `lparam` is the `&mut Search` handed to `EnumWindows`.
            let search = unsafe { &mut *(lparam as *mut Search) };
            let mut pid = 0u32;
            // Safety: `hwnd` comes from the OS enumeration callback.
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            // GW_OWNER (4): skip owned popups such as tooltips and menus.
            if pid == search.pid
                && unsafe { GetWindow(hwnd, 4) }.is_null()
                && unsafe { IsWindowVisible(hwnd) } != 0
            {
                search.found = hwnd;
                return 0; // stop enumerating
            }
            1
        }

        let mut search = Search {
            pid: std::process::id(),
            found: std::ptr::null_mut(),
        };
        // Safety: `visit` reads exactly the `Search` we pass in.
        unsafe { EnumWindows(visit, &mut search as *mut Search as Lparam) };
        (!search.found.is_null()).then_some(search.found)
    }

    pub fn apply() {
        std::thread::spawn(|| {
            for _ in 0..400 {
                if let Some(hwnd) = find_main_window() {
                    let preference: i32 = 2; // DWMWCP_ROUND
                    // Safety: `hwnd` is a live top-level window of this process.
                    let hr = unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            33, // DWMWA_WINDOW_CORNER_PREFERENCE
                            &preference as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        )
                    };
                    crate::log_line(&format!("dwm corner preference set (hr={hr:#010x})"));
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            crate::log_line("dwm corner preference: own window never appeared");
        });
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
        // Nerd Font symbol glyphs (fastfetch icons, starship, powerlevel10k…)
        // ship inside the exe and are registered with the text system; icon
        // cells are shaped with this family (see `view::icon_font`). No user
        // install required.
        let symbols: std::borrow::Cow<'static, [u8]> = std::borrow::Cow::Borrowed(include_bytes!(
            "../assets/SymbolsNerdFontMono-Regular.ttf"
        ));
        match cx.text_system().add_fonts(vec![symbols]) {
            Ok(()) => log_line("nerd symbols font registered"),
            Err(err) => log_line(&format!("nerd symbols font failed: {err:#}")),
        }

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
        #[cfg(windows)]
        rounded_corners::apply();
        cx.activate(true);
    });
    log_line("event loop exited");
}
