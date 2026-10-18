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
mod view;
mod workspace;

use gpui::{
    App, AppContext as _, Application, Bounds, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, px, size,
};

pub(crate) fn user_data_dir() -> std::path::PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        std::path::PathBuf::from(appdata).join("atlas-terminal")
    } else if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        std::path::PathBuf::from(userprofile).join("AppData").join("Roaming").join("atlas-terminal")
    } else {
        std::env::temp_dir().join("atlas-terminal")
    }
}

pub(crate) fn init_user_data() {
    let base = user_data_dir();
    let logs_dir = base.join("logs");
    let sessions_dir = base.join("sessions");
    let settings_file = base.join("settings.json");

    let _ = std::fs::create_dir_all(&logs_dir);
    let _ = std::fs::create_dir_all(&sessions_dir);

    if !settings_file.exists() {
        let default_settings = r#"{
  "theme": "dark",
  "font_family": "Cascadia Code",
  "font_size": 13.0,
  "line_height": 1.2,
  "corner_radius": 14.0,
  "opacity": 1.0,
  "blur_enabled": false,
  "cursor_shape": "block",
  "cursor_blink": true,
  "shortcuts": {
    "new_tab": "ctrl+shift+n",
    "close_tab": "ctrl+shift+w",
    "split_horizontal": "ctrl+shift+d",
    "split_vertical": "ctrl+shift+e",
    "next_tab": "ctrl+tab",
    "prev_tab": "ctrl+shift+tab",
    "copy": "ctrl+shift+c",
    "paste": "ctrl+shift+v",
    "open_settings": "ctrl+,",
    "zoom_in": "ctrl+=",
    "zoom_out": "ctrl+-"
  },
  "default_shell": "PowerShell",
  "start_directory": "home",
  "scrollback_lines": 10000,
  "confirm_close_process": true,
  "copy_on_select": false,
  "startup_mode": "restore_session",
  "auto_check_updates": true,
  "update_check_interval_mins": 60
}
"#;
        let _ = std::fs::write(&settings_file, default_settings);
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

    // Also persist into %APPDATA%\atlas-terminal\logs\app.log
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
// Windows 11 rounded corners + full-bleed client area.
//
// DWM derives corner rounding from the window's frame, and two gpui defaults
// defeat it: USER32 auto-adds WS_CAPTION to resizable windows, and gpui's
// WM_NCCALCSIZE keeps an ~8 px non-client resize ring, so app content never
// touches the window edge. Chromium solves this for custom-titlebar windows
// (VS Code etc.) by removing WS_CAPTION, giving the client the whole window,
// then asking DWM to round - this module applies the same recipe: a helper
// thread waits for the window to appear, subclasses its wndproc, strips
// WS_CAPTION, forces a frame change and sets DWMWA_WINDOW_CORNER_PREFERENCE.
// The subclass also owns the window skeleton: WM_NCCALCSIZE gives the client
// the whole window (frame insets kept only while maximized), WM_NCHITTEST
// serves resize borders plus the titlebar drag band the view reports, and
// presses on those run DefWindowProc's native move/size loops. No-ops
// harmlessly on older Windows; the result is logged for the CI smoke test.
// ---------------------------------------------------------------------------
#[cfg(windows)]
pub(crate) mod rounded_corners {
    use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

    type Hwnd = *mut core::ffi::c_void;
    type Lparam = isize;
    type Lresult = isize;
    type Wparam = usize;

    const GWL_STYLE: i32 = -16;
    const GWLP_WNDPROC: i32 = -4;
    const WS_CAPTION: i32 = 0x00C0_0000;
    const WS_THICKFRAME: i32 = 0x0004_0000;
    // SetWindowPos flags.
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    // Window messages, hit-test codes and metrics.
    const WM_NCCALCSIZE: u32 = 0x0083;
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_NCLBUTTONDOWN: u32 = 0x00A1;
    const WM_NCLBUTTONDBLCLK: u32 = 0x00A3;
    const WM_CLOSE: u32 = 0x0010;
    const HTCLIENT: Lresult = 1;
    const HTCAPTION: Lresult = 2;
    // ShowWindow commands.
    const SW_MAXIMIZE: i32 = 3;
    const SW_MINIMIZE: i32 = 6;
    const SW_RESTORE: i32 = 9;
    const HTLEFT: Lresult = 10;
    const HTRIGHT: Lresult = 11;
    const HTTOP: Lresult = 12;
    const HTTOPLEFT: Lresult = 13;
    const HTTOPRIGHT: Lresult = 14;
    const HTBOTTOM: Lresult = 15;
    const HTBOTTOMLEFT: Lresult = 16;
    const HTBOTTOMRIGHT: Lresult = 17;
    const SM_CXSIZEFRAME: i32 = 32;
    const SM_CXPADDEDBORDER: i32 = 92;

    const WM_SETICON: u32 = 0x0080;
    const ICON_SMALL: usize = 0;
    const ICON_BIG: usize = 1;
    const IMAGE_ICON: u32 = 1;
    const LR_SHARED: u32 = 0x8000;
    const LR_DEFAULTSIZE: u32 = 0x0040;

    // DWM window attributes and policies.
    const DWMWA_NCRENDERING_POLICY: u32 = 2;
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMNCRP_ENABLED: i32 = 2;
    const DWMWCP_ROUND: i32 = 2;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Margins {
        cx_left_width: i32,
        cx_right_width: i32,
        cy_top_height: i32,
        cy_bottom_height: i32,
    }

    #[repr(C)]
    struct NcCalcSizeParams {
        rgrc: [Rect; 3],
        _lppos: *mut core::ffi::c_void,
    }

    /// gpui's original window procedure, called for every message we do not
    /// replace ourselves.
    static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

    /// Top-level window handle, published once `apply` finds it.
    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

    /// Titlebar drag bands reported by the view on every paint, in logical
    /// client pixels: 4 slots of two f32 bit patterns (top-left, bottom-right).
    /// The bands are disjoint — the titlebar is split by its centred search
    /// pill, so each side reports its own rect.
    const CAPTION_SLOTS: usize = 4;
    static CAPTION_RECTS: [AtomicU32; CAPTION_SLOTS * 4] =
        [const { AtomicU32::new(0) }; CAPTION_SLOTS * 4];

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, Lparam) -> i32, lparam: Lparam) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
        fn SetWindowLongW(hwnd: Hwnd, index: i32, value: i32) -> i32;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn CallWindowProcW(
            prev: isize,
            hwnd: Hwnd,
            msg: u32,
            wparam: Wparam,
            lparam: Lparam,
        ) -> Lresult;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
        fn SendMessageW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
        fn LoadImageW(
            hinst: Hwnd,
            name: *const u16,
            type_: u32,
            cx: i32,
            cy: i32,
            fu_load: u32,
        ) -> Hwnd;
        fn SetWindowPos(
            hwnd: Hwnd,
            insert_after: Hwnd,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn IsZoomed(hwnd: Hwnd) -> i32;
        fn GetDpiForWindow(hwnd: Hwnd) -> u32;
        fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
        fn ShowWindowAsync(hwnd: Hwnd, cmd: i32) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> i32;
        fn GetForegroundWindow() -> Hwnd;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(lp_module_name: *const u16) -> Hwnd;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: Hwnd,
            attr: u32,
            value: *const core::ffi::c_void,
            size: u32,
        ) -> i32;
        fn DwmExtendFrameIntoClientArea(hwnd: Hwnd, margins: *const Margins) -> i32;
    }

    /// Resize-border hit test for the frameless window (physical screen
    /// coords). `None` means the point is not on a resize edge.
    /// Scaled by window DPI so fractional scaling (1.15x, 1.25x, 1.5x) preserves
    /// comfortable mouse grab targets.
    fn resize_hit_test(hwnd: Hwnd, x: i32, y: i32) -> Option<Lresult> {
        let mut rect = Rect::default();
        // Safety: `hwnd` is alive for the duration of the call.
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return None;
        }
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = if dpi == 0 { 1.0 } else { (dpi as f32 / 96.0).max(1.0) };
        let border = (8.0 * scale).round() as i32;
        let corner = (16.0 * scale).round() as i32;

        let left = x - rect.left;
        let right = rect.right - x;
        let top = y - rect.top;
        let bottom = rect.bottom - y;
        if left < 0 || top < 0 || right < 0 || bottom < 0 {
            return None;
        }
        Some(if top < border {
            if left < corner {
                HTTOPLEFT
            } else if right < corner {
                HTTOPRIGHT
            } else {
                HTTOP
            }
        } else if bottom < border {
            if left < corner {
                HTBOTTOMLEFT
            } else if right < corner {
                HTBOTTOMRIGHT
            } else {
                HTBOTTOM
            }
        } else if left < border {
            HTLEFT
        } else if right < border {
            HTRIGHT
        } else {
            return None;
        })
    }

    /// The view reports its titlebar drag bands here on every paint (logical
    /// client pixels); `slot` selects one of `CAPTION_SLOTS` disjoint rects.
    /// A degenerate rect (zero size) marks the slot as unused.
    pub fn set_caption_rect(slot: usize, x: f32, y: f32, w: f32, h: f32) {
        if slot >= CAPTION_SLOTS {
            return;
        }
        let base = slot * 4;
        CAPTION_RECTS[base].store(x.to_bits(), Ordering::Relaxed);
        CAPTION_RECTS[base + 1].store(y.to_bits(), Ordering::Relaxed);
        CAPTION_RECTS[base + 2].store((x + w).to_bits(), Ordering::Relaxed);
        CAPTION_RECTS[base + 3].store((y + h).to_bits(), Ordering::Relaxed);
    }

    /// True when a physical client point lies inside one of the reported drag
    /// bands.
    fn caption_hit(hwnd: Hwnd, client_x: f32, client_y: f32) -> bool {
        // Safety: `hwnd` is a live top-level window.
        let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
        for slot in 0..CAPTION_SLOTS {
            let base = slot * 4;
            let x0 = f32::from_bits(CAPTION_RECTS[base].load(Ordering::Relaxed));
            let y0 = f32::from_bits(CAPTION_RECTS[base + 1].load(Ordering::Relaxed));
            let x1 = f32::from_bits(CAPTION_RECTS[base + 2].load(Ordering::Relaxed));
            let y1 = f32::from_bits(CAPTION_RECTS[base + 3].load(Ordering::Relaxed));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            if client_x >= x0 * scale
                && client_x < x1 * scale
                && client_y >= y0 * scale
                && client_y < y1 * scale
            {
                return true;
            }
        }
        false
    }

    /// Window controls from the titlebar buttons: 0 minimize, 1 toggle
    /// maximize, 2 close. The buttons are ordinary client clicks (the OS
    /// non-client path is reserved for the drag band and resize borders), so
    /// the view wires them here.
    pub fn window_button(index: usize) {
        let raw = MAIN_HWND.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        let hwnd = raw as Hwnd;
        // Safety: `hwnd` is the process's main window, checked non-null above.
        unsafe {
            match index {
                0 => {
                    ShowWindowAsync(hwnd, SW_MINIMIZE);
                }
                1 => {
                    if IsZoomed(hwnd) != 0 {
                        ShowWindowAsync(hwnd, SW_RESTORE);
                    } else {
                        ShowWindowAsync(hwnd, SW_MAXIMIZE);
                    }
                }
                _ => {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
        }
    }

    /// Maximized state of the main window: the maximize caption button shows
    /// the restore glyph while this is true.
    pub fn is_zoomed() -> bool {
        let raw = MAIN_HWND.load(Ordering::SeqCst);
        if raw == 0 {
            return false;
        }
        // Safety: `hwnd` is the process's main window, stored once found.
        unsafe { IsZoomed(raw as Hwnd) != 0 }
    }

    /// Whether the Atlas main window is currently the active foreground window.
    pub fn is_active() -> bool {
        let raw = MAIN_HWND.load(Ordering::SeqCst);
        if raw == 0 {
            return false;
        }
        unsafe { GetForegroundWindow() == (raw as Hwnd) }
    }

    /// Update Windows 11 DWM immersive dark mode to match theme luminance.
    pub fn set_dark_mode(dark: bool) {
        let raw = MAIN_HWND.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        let hwnd = raw as Hwnd;
        unsafe {
            let value: i32 = if dark { 1 } else { 0 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const i32 as *const core::ffi::c_void,
                core::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// Toggle Windows 11 DWM acrylic / blur background.
    pub fn set_blur(enabled: bool) {
        let raw = MAIN_HWND.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        let hwnd = raw as Hwnd;
        unsafe {
            let value: i32 = if enabled { 2 } else { 1 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &value as *const i32 as *const core::ffi::c_void,
                core::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// Replacement window procedure: the client area covers the entire window
    /// (so DWM can round the real content); WM_NCHITTEST serves the resize
    /// borders and the view-reported drag band; presses on those non-client
    /// codes bypass gpui and run DefWindowProc's native move/size loops (snap,
    /// double-click maximize). Every other message goes to gpui's procedure.
    ///
    /// # Safety
    /// Installed with `SetWindowLongPtrW`; called by USER32 on the window
    /// thread.
    unsafe extern "system" fn window_proc(
        hwnd: Hwnd,
        msg: u32,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult {
        match msg {
            WM_NCCALCSIZE => {
                // On input `rgrc[0]` is the proposed *window* rect; leaving it
                // untouched makes the client cover the whole window.
                if wparam != 0 && unsafe { IsZoomed(hwnd) } != 0 {
                    // Maximized windows are outdented past the screen edges;
                    // inset by the frame so content stays on-screen.
                    // Safety: `lparam` points to NCCALCSIZE_PARAMS here.
                    unsafe {
                        let dpi = GetDpiForWindow(hwnd);
                        let frame = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi)
                            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                        let params = lparam as *mut NcCalcSizeParams;
                        let rect = &mut (*params).rgrc[0];
                        rect.left += frame;
                        rect.top += frame;
                        rect.right -= frame;
                        rect.bottom -= frame;
                    }
                }
                0
            }
            WM_NCHITTEST => {
                let x = (lparam as usize & 0xFFFF) as u16 as i16 as i32;
                let y = ((lparam as usize >> 16) & 0xFFFF) as u16 as i16 as i32;
                let mut rect = Rect::default();
                // Safety: `hwnd` is live for the duration of the call.
                if unsafe { GetWindowRect(hwnd, &mut rect) } != 0 {
                    let zoomed = unsafe { IsZoomed(hwnd) } != 0;
                    if !zoomed {
                        if let Some(hit) = resize_hit_test(hwnd, x, y) {
                            return hit;
                        }
                    }
                    // While maximized the client is inset by the frame the
                    // WM_NCCALCSIZE arm above keeps.
                    let frame = if zoomed {
                        // Safety: pure metrics lookups on a live hwnd.
                        unsafe {
                            let dpi = GetDpiForWindow(hwnd);
                            GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi)
                                + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
                        }
                    } else {
                        0
                    };
                    let client_x = (x - rect.left - frame) as f32;
                    let client_y = (y - rect.top - frame) as f32;
                    if caption_hit(hwnd, client_x, client_y) {
                        return HTCAPTION;
                    }
                }
                HTCLIENT
            }
            // Presses on the drag band or a resize border run USER32's native
            // move/size loop. Sending them straight to DefWindowProc (never
            // through gpui) keeps the two event paths from fighting and gets
            // Win11 snap and double-click-to-maximize for free.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                let code = wparam as Lresult;
                if matches!(
                    code,
                    HTCAPTION
                        | HTLEFT
                        | HTRIGHT
                        | HTTOP
                        | HTTOPLEFT
                        | HTTOPRIGHT
                        | HTBOTTOM
                        | HTBOTTOMLEFT
                        | HTBOTTOMRIGHT
                ) {
                    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                } else {
                    let prev = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
                    if prev == 0 {
                        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                    } else {
                        unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
                    }
                }
            }
            _ => {
                let prev = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
                if prev == 0 {
                    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                } else {
                    unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
                }
            }
        }
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
            for _ in 0..40 {
                if let Some(hwnd) = find_main_window() {
                    MAIN_HWND.store(hwnd as isize, Ordering::SeqCst);
                    // Safety: `hwnd` is a live top-level window of this process.
                    unsafe {
                        // 1. Subclass first so the frame change below runs
                        //    through our WM_NCCALCSIZE.
                        let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
                        ORIGINAL_WNDPROC.store(original, Ordering::SeqCst);
                        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, window_proc as *const () as isize);

                        // 2. Windows 11 DWM rounded corners and compositor shadows require
                        //    the window to retain standard desktop styles (WS_THICKFRAME | WS_CAPTION).
                        //    Our WM_NCCALCSIZE returns 0 so USER32 removes standard caption/border space,
                        //    giving our custom chrome 100% full-bleed coverage.
                        let style = GetWindowLongW(hwnd, GWL_STYLE);
                        let styled = style | WS_THICKFRAME | WS_CAPTION;
                        SetWindowLongW(hwnd, GWL_STYLE, styled);

                        // 3. Extend the DWM frame 1px into the client area.
                        //    This signals DWM to render its native compositor shadow and rounded corner mask.
                        let margins = Margins {
                            cx_left_width: 1,
                            cx_right_width: 1,
                            cy_top_height: 1,
                            cy_bottom_height: 1,
                        };
                        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

                        // 4. Force the frame change: reruns WM_NCCALCSIZE
                        //    through the subclass and makes DWM re-evaluate the
                        //    window frame (rounding + shadow) for the new style.
                        SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            0,
                            0,
                            0,
                            0,
                            SWP_FRAMECHANGED
                                | SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOZORDER
                                | SWP_NOACTIVATE,
                        );

                        // 4b. The client area just grew by the old non-client
                        //     ring; nudge the window size by one pixel so gpui
                        //     is guaranteed to see a WM_SIZE for it.
                        let mut window_rect = Rect::default();
                        if GetWindowRect(hwnd, &mut window_rect) != 0 {
                            let width = window_rect.right - window_rect.left;
                            let height = window_rect.bottom - window_rect.top;
                            SetWindowPos(
                                hwnd,
                                std::ptr::null_mut(),
                                0,
                                0,
                                width + 1,
                                height,
                                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                            SetWindowPos(
                                hwnd,
                                std::ptr::null_mut(),
                                0,
                                0,
                                width,
                                height,
                                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }

                        // 5. Ask DWM for enabled non-client rendering policy and rounded corners.
                        //    CRITICAL: DWMNCRP_ENABLED is 2 (1 is DWMNCRP_DISABLED!).
                        let nc_policy: i32 = DWMNCRP_ENABLED;
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_NCRENDERING_POLICY,
                            &nc_policy as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        );
                        let preference: i32 = DWMWCP_ROUND;
                        let hr = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_WINDOW_CORNER_PREFERENCE,
                            &preference as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        );

                        // 6. Set initial immersive dark mode (1 for dark default theme).
                        let dark_mode: i32 = 1;
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_USE_IMMERSIVE_DARK_MODE,
                            &dark_mode as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        );

                        // 6b. Explicitly set window icon (big and small) from embedded resource #1
                        let hmodule = GetModuleHandleW(std::ptr::null());
                        let icon_res = 1usize as *const u16;
                        let hicon_big = LoadImageW(hmodule, icon_res, IMAGE_ICON, 0, 0, LR_DEFAULTSIZE | LR_SHARED);
                        let hicon_small = LoadImageW(hmodule, icon_res, IMAGE_ICON, 16, 16, LR_SHARED);
                        if !hicon_big.is_null() {
                            let _ = SendMessageW(hwnd, WM_SETICON, ICON_BIG, hicon_big as isize);
                        }
                        if !hicon_small.is_null() {
                            let _ = SendMessageW(hwnd, WM_SETICON, ICON_SMALL, hicon_small as isize);
                        }

                        // 7. Diagnostics: with the ring gone the client should
                        //    match the window size.
                        let mut client_rect = Rect::default();
                        GetClientRect(hwnd, &mut client_rect);
                        crate::log_line(&format!(
                            "dwm corners: style {style:#010x}->{styled:#010x}, \
                             client {}x{} / window {}x{}, round hr={hr:#010x}",
                            client_rect.right - client_rect.left,
                            client_rect.bottom - client_rect.top,
                            window_rect.right - window_rect.left,
                            window_rect.bottom - window_rect.top,
                        ));
                    }
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            crate::log_line("dwm corners: own window never appeared");
        });
    }
}

#[cfg(not(windows))]
mod rounded_corners {
    pub fn apply() {}
    pub fn is_zoomed() -> bool {
        false
    }
    pub fn is_active() -> bool {
        true
    }
    pub fn set_dark_mode(_dark: bool) {}
    pub fn set_blur(_enabled: bool) {}
    pub fn window_button(_index: usize) {}
    pub fn set_caption_rect(_slot: usize, _x: f32, _y: f32, _w: f32, _h: f32) {}
}

fn main() {
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
                window_background: WindowBackgroundAppearance::Transparent,
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
