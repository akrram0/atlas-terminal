//! Atlas Terminal — native shell (GPUI + alacritty_terminal). No webview, no CSS.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod cwd;
mod icons;
pub mod image_osc;
mod keymap;
mod session;
mod terminal;
mod theme;
mod ui;
pub mod updater;
pub mod url;
mod view;
mod workspace;
mod wsl;

use gpui::{
    App, AppContext as _, Application, Bounds, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, px, size,
};

pub(crate) fn user_data_dir() -> std::path::PathBuf {
    // 1. Portable mode: if portable.txt or settings.json exists right next to the executable,
    //    use the executable directory as the self-contained portable data root.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            if exe_dir.join("portable.txt").exists() || exe_dir.join("settings.json").exists() {
                return exe_dir.to_path_buf();
            }
        }
    }

    // 2. Custom override via environment variable (e.g. ATLAS_CONFIG_DIR)
    if let Some(custom) = std::env::var_os("ATLAS_CONFIG_DIR") {
        return std::path::PathBuf::from(custom);
    }

    // 3. Modern standard: ~/.config/atlas (cross-platform, dotfiles friendly)
    let modern_dir = if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
    {
        std::path::PathBuf::from(home).join(".config").join("atlas")
    } else {
        std::env::temp_dir().join(".config").join("atlas")
    };

    // 4. Backwards compatibility & automatic seamless migration from legacy %APPDATA%\atlas-terminal
    let legacy_dir = std::env::var_os("APPDATA")
        .map(|appdata| std::path::PathBuf::from(appdata).join("atlas-terminal"));

    if !modern_dir.exists() {
        if let Some(ref legacy) = legacy_dir {
            if legacy.join("settings.json").exists() {
                // Perform one-time migration to modern ~/.config/atlas
                let _ = std::fs::create_dir_all(&modern_dir);
                let _ = std::fs::copy(legacy.join("settings.json"), modern_dir.join("settings.json"));
                let _ = std::fs::create_dir_all(modern_dir.join("sessions"));
                if let Ok(entries) = std::fs::read_dir(legacy.join("sessions")) {
                    for entry in entries.flatten() {
                        let _ = std::fs::copy(entry.path(), modern_dir.join("sessions").join(entry.file_name()));
                    }
                }
            }
        }
    }

    modern_dir
}

fn cleanup_legacy_debris() {
    // 1. Remove obsolete Tauri bundle cache/state directories
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let tauri_local = std::path::PathBuf::from(&local).join("com.atlas.terminal");
        if tauri_local.exists() {
            let _ = std::fs::remove_dir_all(&tauri_local);
        }
        let legacy_updater_dir = std::path::PathBuf::from(&local).join("atlas-terminal");
        if legacy_updater_dir.exists() {
            let _ = std::fs::remove_dir_all(&legacy_updater_dir);
        }
        // Remove older installation directory if running from new "Atlas Terminal"
        let old_prog = std::path::PathBuf::from(&local).join("Programs").join("AtlasTerminal");
        let new_prog = std::path::PathBuf::from(&local).join("Programs").join("Atlas Terminal");
        if old_prog.exists() && new_prog.exists() {
            let _ = std::fs::remove_dir_all(&old_prog);
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let tauri_roaming = std::path::PathBuf::from(&appdata).join("com.atlas.terminal");
        if tauri_roaming.exists() {
            let _ = std::fs::remove_dir_all(&tauri_roaming);
        }
        let legacy_roaming = std::path::PathBuf::from(&appdata).join("atlas-terminal");
        if legacy_roaming.exists() {
            let _ = std::fs::remove_dir_all(&legacy_roaming);
        }
    }
}

pub(crate) fn init_user_data() {
    cleanup_legacy_debris();
    let base = user_data_dir();
    let logs_dir = base.join("logs");
    let sessions_dir = base.join("sessions");
    let settings_file = base.join("settings.json");

    let _ = std::fs::create_dir_all(&logs_dir);
    let _ = std::fs::create_dir_all(&sessions_dir);

    if !settings_file.exists() {
        let defaults = crate::config::AppSettingsFile::default();
        if let Ok(json) = serde_json::to_string_pretty(&defaults) {
            let _ = std::fs::write(&settings_file, json);
        }
    }
}

fn debug_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("atlas-native-debug.log")
}

pub(crate) fn log_line(msg: &str) {
    use std::io::Write as _;
    // Always mirror to TEMP debug log for CI smoke test
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(debug_log_path())
    {
        let _ = writeln!(f, "{msg}");
    }

    // Also persist into <user_data_dir>/logs/app.log
    let app_log = user_data_dir().join("logs").join("app.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(app_log)
    {
        let _ = writeln!(f, "{msg}");
    }
}


// ---------------------------------------------------------------------------
// Platform integration: Windows 11 DWM subclassing, acrylic blur & hit-testing
// ---------------------------------------------------------------------------
pub(crate) mod platform;
pub(crate) use platform::rounded_corners;

fn main() {
    // Hidden CI/test helper: apply a zstd delta and exit without starting GPUI.
    if let Some(code) = updater::run_patch_cli() {
        std::process::exit(code);
    }
    // Drop leftover staged/rollback update executables from a previous update.
    updater::cleanup_update_backup();

    #[cfg(windows)]
    unsafe {
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(AppID: *const u16) -> i32;
        }
        let aumid: Vec<u16> = "Atlas.Terminal\0".encode_utf16().collect();
        let _ = SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr());

        #[link(name = "winmm")]
        unsafe extern "system" {
            fn timeBeginPeriod(uPeriod: u32) -> u32;
        }
        let _ = timeBeginPeriod(1);
    }

    init_user_data();
    // Fresh log per launch. Release builds have no console
    // (windows_subsystem), so this file is how startup and panics stay visible.
    let _ = std::fs::write(debug_log_path(), "atlas-terminal starting\n");
    let app_log = user_data_dir().join("logs").join("app.log");
    let _ = std::fs::write(&app_log, "atlas-terminal starting\n");
    match wsl::default_distro() {
        Some(distro) => log_line(&format!(
            "wsl: default distro {} -> {} ({})",
            distro.name, distro.title, distro.icon
        )),
        None => log_line("wsl: default distro not detected"),
    }
    std::panic::set_hook(Box::new(|info| {
        let mut msg = format!("PANIC: {info}\n");
        if let Some(loc) = info.location() {
            msg.push_str(&format!("  at {loc}\n"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(debug_log_path())
        {
            let _ = std::io::Write::write_all(&mut f, msg.as_bytes());
        }
        let panic_log = user_data_dir().join("logs").join("panic.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(panic_log)
        {
            let _ = std::io::Write::write_all(&mut f, msg.as_bytes());
        }
    }));
    log_line("opening window");
    Application::new()
        .with_assets(icons::IconsAssets)
        .run(|cx: &mut App| {
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

        let initial_dir = std::env::args().nth(1).and_then(|arg| {
            let p = std::path::PathBuf::from(&arg);
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        });

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None, // frameless: the app draws its own titlebar
                // GPUI's native `Blurred` background maps to the acrylic
                // accent on Windows. `rounded_corners::apply_backdrop`
                // re-applies it on the UI thread with a nearly transparent
                // tint and toggles it with the opacity slider; the DWM system
                // backdrop is only the fallback when the accent API is
                // unavailable.
                window_background: WindowBackgroundAppearance::Blurred,
                window_decorations: Some(WindowDecorations::Client),
                window_min_size: Some(size(px(640.0), px(420.0))),
                is_resizable: true,
                is_movable: true,
                is_minimizable: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| view::AtlasView::new(window, initial_dir, cx)),
        )
        .expect("failed to open Atlas window");
        log_line("window opened");
        #[cfg(windows)]
        rounded_corners::apply();
        cx.activate(true);
    });
    log_line("event loop exited");
}
