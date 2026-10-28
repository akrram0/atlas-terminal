#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]
//! Windows 11 rounded corners, acrylic blur & full-bleed client area.
//! Subclasses the HWND to handle WM_NCCALCSIZE and WM_NCHITTEST with native DWM composition.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

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
const WM_SIZE: u32 = 0x0005;
const WM_GETMINMAXINFO: u32 = 0x0024;
const WM_NCCALCSIZE: u32 = 0x0083;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_NCLBUTTONDBLCLK: u32 = 0x00A3;
const WM_CLOSE: u32 = 0x0010;
/// Private message: apply the native backdrop on the window's UI thread.
const WM_APP_BACKDROP: u32 = 0x8000 + 1;
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
const MONITOR_DEFAULTTONEAREST: u32 = 2;

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
/// Border colour attribute (Windows 11 22000+). `DWMWA_COLOR_NONE` removes
/// the 1px DWM outline so the acrylic can run all the way to the edge.
const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
// Windows 11 22H2+ system backdrop (the supported replacement for the
// undocumented accent acrylic on build 22621 and newer).
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
const DWMSBT_NONE: i32 = 1;
const DWMSBT_TRANSIENTWINDOW: i32 = 3;
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
struct Point {
    x: i32,
    y: i32,
}

/// Filled by the system for `WM_GETMINMAXINFO`; `ptMaxPosition` is relative
/// to the monitor origin and `ptMaxSize` is the maximized window size.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MinMaxInfo {
    pt_reserved: Point,
    pt_max_size: Point,
    pt_max_position: Point,
    pt_max_track_size: Point,
    pt_min_track_size: Point,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MonitorInfo {
    cb_size: u32,
    rc_monitor: Rect,
    rc_work: Rect,
    dw_flags: u32,
}

/// gpui's original window procedure, called for every message we do not
/// replace ourselves.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Top-level window handle, published once `apply` finds it.
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static INITIAL_OPACITY: AtomicU32 = AtomicU32::new(f32::to_bits(0.15));
/// Whether the app theme is dark; picks the acrylic frost tint colour.
static DARK_MODE: AtomicBool = AtomicBool::new(true);
/// Last observed maximize state, so the backdrop is reasserted only on
/// restore <-> maximize transitions rather than on every WM_SIZE.
static LAST_ZOOMED: AtomicBool = AtomicBool::new(false);

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
    fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> Hwnd;
    fn GetMonitorInfoW(hwnd: Hwnd, info: *mut MonitorInfo) -> i32;
    fn ShowWindowAsync(hwnd: Hwnd, cmd: i32) -> i32;
    fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> i32;
    fn GetForegroundWindow() -> Hwnd;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(lp_module_name: *const u16) -> Hwnd;
    fn GetProcAddress(hmodule: Hwnd, proc_name: *const u8) -> *const ();
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

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: u32,
    data: *mut core::ffi::c_void,
    size_of_data: usize,
}

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_DISABLED: u32 = 0;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

unsafe fn apply_composition_accent(hwnd: Hwnd, enabled: bool) -> i32 {
    unsafe {
        let name: Vec<u16> = "user32.dll\0".encode_utf16().collect();
        let user32 = GetModuleHandleW(name.as_ptr());
        if user32.is_null() {
            return 0;
        }
        let p_fn = GetProcAddress(user32, b"SetWindowCompositionAttribute\0".as_ptr());
        if p_fn.is_null() {
            return 0;
        }
        type SetWindowCompositionAttributeFn =
            unsafe extern "system" fn(Hwnd, *mut WindowCompositionAttributeData) -> i32;
        let set_composition: SetWindowCompositionAttributeFn = core::mem::transmute(p_fn);

        let mut policy = if enabled {
            let zoomed = IsZoomed(hwnd) != 0;
            let alpha: u32 = if zoomed { 0x26 } else { 0x01 };
            let rgb: u32 = if DARK_MODE.load(Ordering::Relaxed) {
                0x000000
            } else {
                0xFFFFFF
            };
            AccentPolicy {
                accent_state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
                accent_flags: 0,
                gradient_color: (alpha << 24) | rgb,
                animation_id: 0,
            }
        } else {
            AccentPolicy {
                accent_state: ACCENT_DISABLED,
                accent_flags: 0,
                gradient_color: 0,
                animation_id: 0,
            }
        };

        let mut data = WindowCompositionAttributeData {
            attribute: WCA_ACCENT_POLICY,
            data: &mut policy as *mut AccentPolicy as *mut core::ffi::c_void,
            size_of_data: core::mem::size_of::<AccentPolicy>(),
        };

        let tint = policy.gradient_color;
        let res = set_composition(hwnd, &mut data);
        crate::log_line(&format!(
            "composition accent: enabled={enabled}, res={res}, tint={tint:#010x}"
        ));
        res
    }
}

/// Switch the native desktop backdrop on or off.
///
/// Prefer GPUI's native acrylic accent. On Windows,
/// `WindowBackgroundAppearance::Blurred` is exactly
/// `SetWindowCompositionAttribute` with `ACCENT_ENABLE_ACRYLICBLURBEHIND`
/// and a nearly transparent black tint. That thin tint keeps the wallpaper
/// and the windows behind visible, whereas the DWM system backdrop's fixed
/// gray acrylic tint washes them out into a flat gray sheet. While
/// maximized the tint is raised to a theme-matched veil: DWM's blur radius
/// is fixed, so a full-screen window otherwise reads as less frosted than a
/// small one.
///
/// The Windows 11 `DWMSBT_TRANSIENTWINDOW` system backdrop is only the
/// fallback if the accent API is unavailable. `DwmExtendFrameIntoClientArea`
/// is deliberately never called: with DirectComposition it turns the client
/// area into an opaque surface.
fn apply_backdrop(hwnd: Hwnd, translucent: bool) {
    unsafe {
        // Clear any previous system backdrop before changing the accent;
        // otherwise DWM paints its own tint on top of the acrylic.
        let none = DWMSBT_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &none as *const i32 as *const core::ffi::c_void,
            core::mem::size_of::<i32>() as u32,
        );

        if !translucent {
            let _ = apply_composition_accent(hwnd, false);
            return;
        }

        let accent_res = apply_composition_accent(hwnd, true);
        if accent_res != 0 {
            crate::log_line("backdrop: acrylic accent active (GPUI native)");
            return;
        }

        // The accent API is missing (or refused the policy): fall back to
        // the supported Win11 system backdrop. It has a stronger tint, but
        // it still blurs the desktop.
        let acrylic = DWMSBT_TRANSIENTWINDOW;
        let hr = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &acrylic as *const i32 as *const core::ffi::c_void,
            core::mem::size_of::<i32>() as u32,
        );
        if hr == 0 {
            crate::log_line("backdrop: DWMSBT_TRANSIENTWINDOW fallback active");
        } else {
            crate::log_line(&format!(
                "backdrop: accent unavailable (res={accent_res}), system backdrop failed (hr={hr:#010x})"
            ));
        }
    }
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
    let previous = DARK_MODE.swap(dark, Ordering::SeqCst);
    let raw = MAIN_HWND.load(Ordering::SeqCst);
    if raw == 0 {
        return;
    }
    if previous != dark {
        // The acrylic veil colour follows the theme; repaint the accent so
        // a light/dark switch does not leave the old tint behind.
        unsafe {
            PostMessageW(raw as Hwnd, WM_APP_BACKDROP, 0, 0);
        }
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

/// Update the native window backdrop to match the opacity slider.
///
/// Called from the view every frame; the atomic compare makes this a no-op
/// until the slider actually moves. The initial backdrop is queued by
/// `apply()` as `WM_APP_BACKDROP` and applied by `window_proc` on the UI
/// thread.
pub fn set_opacity(opacity: f32) {
    let bits = opacity.to_bits();
    if INITIAL_OPACITY.swap(bits, Ordering::SeqCst) == bits {
        return;
    }
    let raw = MAIN_HWND.load(Ordering::SeqCst);
    if raw != 0 {
        apply_backdrop(raw as Hwnd, opacity < 0.999);
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
        WM_GETMINMAXINFO => {
            // A frameless WS_THICKFRAME window is normally maximized to the
            // work area *plus* the resize frame, outdented past the screen.
            // Pinning the maximized rect to the work area lets the client
            // cover the whole window (see WM_NCCALCSIZE), so the acrylic
            // backdrop reaches the screen edges instead of leaving an
            // unblurred frame ring around a maximized window.
            let prev = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
            let result = if prev == 0 {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            } else {
                unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
            };
            let info = lparam as *mut MinMaxInfo;
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            if !info.is_null() && !monitor.is_null() {
                let mut mi = MonitorInfo {
                    cb_size: core::mem::size_of::<MonitorInfo>() as u32,
                    rc_monitor: Rect::default(),
                    rc_work: Rect::default(),
                    dw_flags: 0,
                };
                if unsafe { GetMonitorInfoW(monitor, &mut mi) } != 0 {
                    unsafe {
                        (*info).pt_max_position.x = mi.rc_work.left - mi.rc_monitor.left;
                        (*info).pt_max_position.y = mi.rc_work.top - mi.rc_monitor.top;
                        (*info).pt_max_size.x = mi.rc_work.right - mi.rc_work.left;
                        (*info).pt_max_size.y = mi.rc_work.bottom - mi.rc_work.top;
                        // DefWindowProc normally fills the tracking limits;
                        // keep a sane fallback if a custom wndproc did not.
                        if (*info).pt_max_track_size.x <= 0 {
                            (*info).pt_max_track_size.x =
                                mi.rc_monitor.right - mi.rc_monitor.left;
                        }
                        if (*info).pt_max_track_size.y <= 0 {
                            (*info).pt_max_track_size.y =
                                mi.rc_monitor.bottom - mi.rc_monitor.top;
                        }
                    }
                }
            }
            result
        }
        WM_NCCALCSIZE => {
            // The maximized rect is pinned to the work area by
            // WM_GETMINMAXINFO, so the client can cover the whole window at
            // every size. Leaving `rgrc[0]` untouched gives the acrylic a
            // full-bleed surface; the old maximized frame inset left a
            // non-blurred ring at the screen edges.
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
                // The client covers the whole window at every size (see
                // WM_NCCALCSIZE), so no frame offset is needed here.
                let client_x = (x - rect.left) as f32;
                let client_y = (y - rect.top) as f32;
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
        WM_SIZE => {
            // A maximize/restore transition can leave DWM with a stale or
            // dropped accent. Reassert it once per state change (never on
            // every resize: re-applying a visible accent briefly flashes).
            let zoomed = unsafe { IsZoomed(hwnd) } != 0;
            if LAST_ZOOMED.swap(zoomed, Ordering::SeqCst) != zoomed {
                let opacity = f32::from_bits(INITIAL_OPACITY.load(Ordering::SeqCst));
                if opacity < 0.999 {
                    crate::log_line(&format!("backdrop: zoomed={zoomed}, reasserting acrylic"));
                    if zoomed {
                        let mut wr = Rect::default();
                        let mut cr = Rect::default();
                        unsafe {
                            GetWindowRect(hwnd, &mut wr);
                            GetClientRect(hwnd, &mut cr);
                        }
                        crate::log_line(&format!(
                            "backdrop: maximized window {}x{} client {}x{}",
                            wr.right - wr.left,
                            wr.bottom - wr.top,
                            cr.right,
                            cr.bottom
                        ));
                    }
                    unsafe { PostMessageW(hwnd, WM_APP_BACKDROP, 0, 0) };
                }
            }
            let prev = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
            if prev == 0 {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            } else {
                unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
            }
        }
        WM_APP_BACKDROP => {
            // USER32/DWM composition calls run here, on the window's UI
            // thread, rather than on the helper thread that subclasses it.
            let opacity = f32::from_bits(INITIAL_OPACITY.load(Ordering::SeqCst));
            apply_backdrop(hwnd, opacity < 0.999);
            0
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
                    //    the window to retain standard desktop styles (WS_THICKFRAME).
                    //    We explicitly STRIP WS_CAPTION so DWM never draws native caption buttons
                    //    (minimize, maximize, close) inside our extended acrylic frame.
                    //    Our WM_NCCALCSIZE returns 0 so USER32 removes standard caption/border space,
                    //    giving our custom chrome 100% full-bleed coverage.
                    let style = GetWindowLongW(hwnd, GWL_STYLE);
                    let styled = (style & !WS_CAPTION) | WS_THICKFRAME;
                    SetWindowLongW(hwnd, GWL_STYLE, styled);

                    // 3. Force the frame change: reruns WM_NCCALCSIZE
                    //    through the subclass and makes DWM re-evaluate the
                    //    window frame (rounding + shadow) for the new style.
                    //
                    //    Do NOT call DwmExtendFrameIntoClientArea here.
                    //    With DirectComposition + WS_EX_NOREDIRECTIONBITMAP
                    //    it turns the client area into an opaque DWM
                    //    "sheet of glass" surface, which is exactly the
                    //    solid black background we are fixing.
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

                    // 4. Ask DWM for enabled non-client rendering policy and rounded corners.
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
                    // Remove the 1px DWM outline: at full size it reads as a
                    // sharp, unblurred rim around the acrylic.
                    let border: u32 = DWMWA_COLOR_NONE;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_BORDER_COLOR,
                        &border as *const u32 as *const core::ffi::c_void,
                        core::mem::size_of::<u32>() as u32,
                    );

                    // 5. Set initial immersive dark mode (1 for dark default theme).
                    LAST_ZOOMED.store(IsZoomed(hwnd) != 0, Ordering::SeqCst);
                    let dark_mode: i32 = 1;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_USE_IMMERSIVE_DARK_MODE,
                        &dark_mode as *const i32 as *const core::ffi::c_void,
                        core::mem::size_of::<i32>() as u32,
                    );

                    // 5b. Explicitly set window icon (big and small) from embedded resource #1
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

                    // 5c. Apply the initial native backdrop on the window's
                    //     UI thread. `apply_backdrop` touches USER32
                    //     composition state, so posting lets the GPUI
                    //     message loop dispatch it with the correct thread
                    //     affinity.
                    if PostMessageW(hwnd, WM_APP_BACKDROP, 0, 0) == 0 {
                        crate::log_line("backdrop: PostMessageW(WM_APP_BACKDROP) failed");
                    }

                    // 6. Diagnostics: with the ring gone the client should
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

/// Send a native Windows desktop notification asynchronously.
pub fn send_desktop_notification(title: &str, message: &str) {
    let t = title.replace('\'', "''");
    let m = message.replace('\'', "''");
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
         $nodes = $template.GetElementsByTagName('text'); \
         $nodes.Item(0).AppendChild($template.CreateTextNode('{t}')) | Out-Null; \
         $nodes.Item(1).AppendChild($template.CreateTextNode('{m}')) | Out-Null; \
         $toast = [Windows.UI.Notifications.ToastNotification]::new($template); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Atlas.Terminal').Show($toast)"
    );
    std::thread::spawn(move || {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&script)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    });
}
