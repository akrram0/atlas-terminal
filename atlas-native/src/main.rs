//! Atlas Terminal — native shell (GPUI + alacritty_terminal). No webview, no CSS.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cwd;
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
mod rounded_corners {
    use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};

    type Hwnd = *mut core::ffi::c_void;
    type Lparam = isize;
    type Lresult = isize;
    type Wparam = usize;

    const GWL_STYLE: i32 = -16;
    const GWLP_WNDPROC: i32 = -4;
    const WS_CAPTION: i32 = 0x00C0_0000;
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
    const RESIZE_BORDER: i32 = 8;
    const RESIZE_CORNER: i32 = 16;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
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
    static CAPTION_RECTS: [AtomicI32; CAPTION_SLOTS * 4] =
        [const { AtomicI32::new(0) }; CAPTION_SLOTS * 4];

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

    /// Resize-border hit test for the frameless window (physical screen
    /// coords). `None` means the point is not on a resize edge.
    fn resize_hit_test(hwnd: Hwnd, x: i32, y: i32) -> Option<Lresult> {
        let mut rect = Rect::default();
        // Safety: `hwnd` is alive for the duration of the call.
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return None;
        }
        let left = x - rect.left;
        let right = rect.right - x;
        let top = y - rect.top;
        let bottom = rect.bottom - y;
        if left < 0 || top < 0 || right < 0 || bottom < 0 {
            return None;
        }
        Some(if top < RESIZE_BORDER {
            if left < RESIZE_CORNER {
                HTTOPLEFT
            } else if right < RESIZE_CORNER {
                HTTOPRIGHT
            } else {
                HTTOP
            }
        } else if bottom < RESIZE_BORDER {
            if left < RESIZE_CORNER {
                HTBOTTOMLEFT
            } else if right < RESIZE_CORNER {
                HTBOTTOMRIGHT
            } else {
                HTBOTTOM
            }
        } else if left < RESIZE_BORDER {
            HTLEFT
        } else if right < RESIZE_BORDER {
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
        CAPTION_RECTS[base].store(x.to_bits() as i32, Ordering::Relaxed);
        CAPTION_RECTS[base + 1].store(y.to_bits() as i32, Ordering::Relaxed);
        CAPTION_RECTS[base + 2].store((x + w).to_bits() as i32, Ordering::Relaxed);
        CAPTION_RECTS[base + 3].store((y + h).to_bits() as i32, Ordering::Relaxed);
    }

    /// True when a physical client point lies inside one of the reported drag
    /// bands.
    fn caption_hit(hwnd: Hwnd, client_x: f32, client_y: f32) -> bool {
        // Safety: `hwnd` is a live top-level window.
        let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
        for slot in 0..CAPTION_SLOTS {
            let base = slot * 4;
            let x0 = f32::from_bits(CAPTION_RECTS[base].load(Ordering::Relaxed) as u32);
            let y0 = f32::from_bits(CAPTION_RECTS[base + 1].load(Ordering::Relaxed) as u32);
            let x1 = f32::from_bits(CAPTION_RECTS[base + 2].load(Ordering::Relaxed) as u32);
            let y1 = f32::from_bits(CAPTION_RECTS[base + 3].load(Ordering::Relaxed) as u32);
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
            WM_NCCALCSIZE if wparam != 0 => {
                // On input `rgrc[0]` is the proposed *window* rect; leaving it
                // untouched makes the client cover the whole window.
                if unsafe { IsZoomed(hwnd) } != 0 {
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
            for _ in 0..400 {
                if let Some(hwnd) = find_main_window() {
                    MAIN_HWND.store(hwnd as isize, Ordering::SeqCst);
                    // Safety: `hwnd` is a live top-level window of this process.
                    unsafe {
                        // 1. Subclass first so the frame change below runs
                        //    through our WM_NCCALCSIZE.
                        let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
                        ORIGINAL_WNDPROC.store(original, Ordering::SeqCst);
                        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, window_proc as *const () as isize);

                        // 2. USER32 auto-adds WS_CAPTION to resizable windows;
                        //    DWM only rounds windows that draw their own chrome
                        //    without one (same as Chromium / VS Code).
                        let style = GetWindowLongW(hwnd, GWL_STYLE);
                        let stripped = style & !WS_CAPTION;
                        SetWindowLongW(hwnd, GWL_STYLE, stripped);

                        // 3. Force the frame change: reruns WM_NCCALCSIZE
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

                        // 3b. The client area just grew by the old non-client
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

                        // 4. Ask DWM for rounded corners.
                        let nc_policy: i32 = 1; // DWMNCRP_ENABLED
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            2, // DWMWA_NCRENDERING_POLICY
                            &nc_policy as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        );
                        let preference: i32 = 2; // DWMWCP_ROUND
                        let hr = DwmSetWindowAttribute(
                            hwnd,
                            33, // DWMWA_WINDOW_CORNER_PREFERENCE
                            &preference as *const i32 as *const core::ffi::c_void,
                            core::mem::size_of::<i32>() as u32,
                        );

                        // 5. Diagnostics: with the ring gone the client should
                        //    match the window size.
                        let mut client_rect = Rect::default();
                        GetClientRect(hwnd, &mut client_rect);
                        crate::log_line(&format!(
                            "dwm corners: style {style:#010x}->{stripped:#010x}, \
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
