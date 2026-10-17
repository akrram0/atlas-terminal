//! AtlasView — the whole UI, drawn by Rust.
//!
//! Structure: frameless window -> compact pill top bar (workspace pill,
//! horizontal pill tabs, new-tab plus, native window controls) over a
//! floating rounded terminal sheet inset 8px, whose whisper header carries
//! the pane title and its split/close micro marks. Chrome marks that used to
//! be font glyphs are geometric quads now — exact 1px at any scale. The terminal
//! itself is a custom gpui `Element` that walks the alacritty grid and paints
//! cell backgrounds, text runs, procedural block glyphs and the cursor
//! directly on the GPU canvas.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClickEvent, ClipboardItem, Context, Corners, Element,
    ElementId, FocusHandle, Font, FontFallbacks, FontWeight, GlobalElementId, InspectorElementId,
    IntoElement, KeyDownEvent, Keystroke, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Render, Rgba, ScrollWheelEvent, SharedString, StrikethroughStyle, Style, TextRun, UnderlineStyle,
    Window, canvas, div, fill, point, px, relative, rgb, rgba,
};
use serde::{Deserialize, Serialize};

use crate::icons;
use crate::keymap;
use crate::session::{Profile, Session, SessionMsg};
use crate::theme::{Theme, ThemeRegistry};
use crate::updater::{self, ReleaseInfo, UpdaterSettings};

/// Preferred terminal monos, best first: Cascadia (Windows 11 inbox) then
/// Consolas. `terminal_font` picks the first family the text system resolves.
const FONT_CASCADE: &[&str] = &["Cascadia Code", "Cascadia Mono", "Consolas"];
const FONT_FALLBACK: &str = "Consolas";
const FONT_SIZE: f32 = 13.5;

/// Motion timings (ms) — Apple/macOS-style snappy, quiet, highly responsive.
const OVERLAY_IN_MS: u64 = 135;
const OVERLAY_OUT_MS: u64 = 85;
const BODY_IN_MS: u64 = 110;

pub const ACTION_NEW_TAB: &str = "new_tab";
pub const ACTION_CLOSE_TAB: &str = "close_tab";
pub const ACTION_SPLIT_HORIZONTAL: &str = "split_horizontal";
pub const ACTION_SPLIT_VERTICAL: &str = "split_vertical";
pub const ACTION_NEXT_TAB: &str = "next_tab";
pub const ACTION_PREV_TAB: &str = "prev_tab";
pub const ACTION_COPY: &str = "copy";
pub const ACTION_PASTE: &str = "paste";
pub const ACTION_OPEN_SETTINGS: &str = "open_settings";
pub const ACTION_ZOOM_IN: &str = "zoom_in";
pub const ACTION_ZOOM_OUT: &str = "zoom_out";
pub const ACTION_PALETTE: &str = "palette";
pub const ACTION_FULLSCREEN: &str = "fullscreen";
pub const ACTION_CLEAR_SCROLLBACK: &str = "clear_scrollback";
pub const ACTION_FOCUS_NEXT_PANE: &str = "focus_next_pane";
pub const ACTION_FOCUS_PREV_PANE: &str = "focus_prev_pane";

pub fn action_display_name(action: &str) -> &'static str {
    match action {
        ACTION_NEW_TAB => "New Tab",
        ACTION_CLOSE_TAB => "Close Tab / Pane",
        ACTION_SPLIT_HORIZONTAL => "Split Horizontal (Rows)",
        ACTION_SPLIT_VERTICAL => "Split Vertical (Columns)",
        ACTION_NEXT_TAB => "Next Tab",
        ACTION_PREV_TAB => "Previous Tab",
        ACTION_COPY => "Copy Selection",
        ACTION_PASTE => "Paste Clipboard",
        ACTION_OPEN_SETTINGS => "Open Settings",
        ACTION_ZOOM_IN => "Zoom In (Font Size)",
        ACTION_ZOOM_OUT => "Zoom Out (Font Size)",
        ACTION_PALETTE => "Command Palette",
        ACTION_FULLSCREEN => "Toggle Fullscreen / Maximize",
        ACTION_CLEAR_SCROLLBACK => "Clear Scrollback Buffer",
        ACTION_FOCUS_NEXT_PANE => "Focus Next Split Pane",
        ACTION_FOCUS_PREV_PANE => "Focus Previous Split Pane",
        _ => "Custom Action",
    }
}

pub fn default_shortcuts() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(ACTION_NEW_TAB.to_string(), "ctrl+shift+n".to_string());
    m.insert(ACTION_CLOSE_TAB.to_string(), "ctrl+shift+w".to_string());
    m.insert(ACTION_SPLIT_HORIZONTAL.to_string(), "ctrl+shift+d".to_string());
    m.insert(ACTION_SPLIT_VERTICAL.to_string(), "ctrl+shift+e".to_string());
    m.insert(ACTION_NEXT_TAB.to_string(), "ctrl+tab".to_string());
    m.insert(ACTION_PREV_TAB.to_string(), "ctrl+shift+tab".to_string());
    m.insert(ACTION_COPY.to_string(), "ctrl+shift+c".to_string());
    m.insert(ACTION_PASTE.to_string(), "ctrl+shift+v".to_string());
    m.insert(ACTION_OPEN_SETTINGS.to_string(), "ctrl+,".to_string());
    m.insert(ACTION_ZOOM_IN.to_string(), "ctrl+=".to_string());
    m.insert(ACTION_ZOOM_OUT.to_string(), "ctrl+-".to_string());
    m.insert(ACTION_PALETTE.to_string(), "ctrl+shift+p".to_string());
    m.insert(ACTION_FULLSCREEN.to_string(), "f11".to_string());
    m.insert(ACTION_CLEAR_SCROLLBACK.to_string(), "ctrl+shift+k".to_string());
    m.insert(ACTION_FOCUS_NEXT_PANE.to_string(), "alt+right".to_string());
    m.insert(ACTION_FOCUS_PREV_PANE.to_string(), "alt+left".to_string());
    m
}

pub fn format_keystroke_chord(ks: &Keystroke) -> Option<String> {
    let key = ks.key.to_lowercase();
    if matches!(
        key.as_str(),
        "control" | "shift" | "alt" | "super" | "meta" | "windows" | "menu"
    ) {
        return None;
    }
    let mut parts = Vec::new();
    if ks.modifiers.control {
        parts.push("ctrl");
    }
    if ks.modifiers.alt {
        parts.push("alt");
    }
    if ks.modifiers.shift {
        parts.push("shift");
    }
    let key_clean = match key.as_str() {
        "+" => "=",
        other => other,
    };
    parts.push(key_clean);
    Some(parts.join("+"))
}

pub fn pretty_shortcut(chord: &str) -> String {
    chord
        .split('+')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            "tab" => "Tab".to_string(),
            "enter" => "Enter".to_string(),
            "backspace" => "Backspace".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            "escape" => "Esc".to_string(),
            "," => ",".to_string(),
            "=" => "=".to_string(),
            "-" => "-".to_string(),
            other => {
                let mut c = other.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Shortcuts,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsFile {
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_blur")]
    pub blur_enabled: bool,
    #[serde(default = "default_cursor_shape")]
    pub cursor_shape: String,
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,

    #[serde(default = "default_shortcuts")]
    pub shortcuts: HashMap<String, String>,

    #[serde(default = "default_shell")]
    pub default_shell: String,
    #[serde(default = "default_start_dir")]
    pub start_directory: String,
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: usize,
    #[serde(default = "default_confirm_close")]
    pub confirm_close_process: bool,
    #[serde(default = "default_copy_on_select")]
    pub copy_on_select: bool,
    #[serde(default = "default_startup_mode")]
    pub startup_mode: String,

    #[serde(default = "default_auto_check")]
    pub auto_check_updates: bool,
    #[serde(default = "default_interval_mins")]
    pub update_check_interval_mins: u32,
    #[serde(default)]
    pub last_check_time: Option<u64>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_seen_version: Option<String>,
    #[serde(default)]
    pub skipped_version: Option<String>,
}

fn default_theme_name() -> String {
    "dark".to_string()
}
fn default_font_family() -> String {
    "Cascadia Code".to_string()
}
fn default_font_size() -> f32 {
    13.0
}
fn default_line_height() -> f32 {
    1.20
}
fn default_corner_radius() -> f32 {
    14.0
}
fn default_opacity() -> f32 {
    1.0
}
fn default_blur() -> bool {
    false
}
fn default_cursor_shape() -> String {
    "block".to_string()
}
fn default_cursor_blink() -> bool {
    true
}
fn default_shell() -> String {
    "PowerShell".to_string()
}
fn default_start_dir() -> String {
    "home".to_string()
}
fn default_scrollback() -> usize {
    10000
}
fn default_confirm_close() -> bool {
    true
}
fn default_copy_on_select() -> bool {
    true
}
fn default_startup_mode() -> String {
    "restore_session".to_string()
}
fn default_auto_check() -> bool {
    true
}
fn default_interval_mins() -> u32 {
    60
}

fn load_app_settings() -> AppSettingsFile {
    let settings_path = crate::user_data_dir().join("settings.json");
    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Ok(s) = serde_json::from_str::<AppSettingsFile>(&content) {
            return s;
        }
    }
    AppSettingsFile {
        theme: default_theme_name(),
        font_family: default_font_family(),
        font_size: default_font_size(),
        line_height: default_line_height(),
        corner_radius: default_corner_radius(),
        opacity: default_opacity(),
        blur_enabled: default_blur(),
        cursor_shape: default_cursor_shape(),
        cursor_blink: default_cursor_blink(),
        shortcuts: default_shortcuts(),
        default_shell: default_shell(),
        start_directory: default_start_dir(),
        scrollback_lines: default_scrollback(),
        confirm_close_process: default_confirm_close(),
        copy_on_select: default_copy_on_select(),
        startup_mode: default_startup_mode(),
        auto_check_updates: default_auto_check(),
        update_check_interval_mins: default_interval_mins(),
        last_check_time: None,
        etag: None,
        last_seen_version: None,
        skipped_version: None,
    }
}

fn save_app_settings(s: &AppSettingsFile) {
    let base = crate::user_data_dir();
    let _ = std::fs::create_dir_all(&base);
    let settings_path = base.join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(&settings_path, json);
    }
}

pub struct AtlasView {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    next_epoch: u64,
    focus: FocusHandle,
    theme: Theme,
    /// Smooth crossfade interpolation (from_theme, to_theme, progress 0.0..1.0).
    theme_transition: Option<(Theme, Theme, f32)>,
    /// Terminal font for the cell grid.
    font: Font,
    /// Active preset index into `registry` (settings pills + Ctrl+Shift+T).
    theme_id: usize,
    /// Named theme presets: neutral matte charcoal (default) and Warm Paper.
    registry: ThemeRegistry,
    /// Settings page visibility (replaces the terminal sheet).
    settings_open: bool,
    /// Workspace (new-tab profile) menu visibility.
    profile_menu: bool,
    /// Command palette (Ctrl+Shift+P / top-bar search icon).
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    /// Smoothly gliding floating pill index in the command palette.
    palette_anim_index: f32,
    /// Rename-tab dialog (palette).
    rename_open: bool,
    rename_value: String,
    /// Terminal font size in px (settings page).
    font_size: f32,
    /// Cursor override; `None` follows the shell's DECSCUSR.
    cursor_style: Option<CursorShape>,
    /// Blink phase of the cursor (palette/rename carets share it).
    cursor_on: bool,
    /// Continuous breathing phase (0.0 .. 2π) for organic cursor pulse.
    cursor_breath_phase: f32,
    /// Timestamp of last keystroke/output; keeps cursor 100% solid while typing.
    cursor_last_typing: std::time::Instant,
    /// Focus handles for the overlay surfaces.
    settings_focus: FocusHandle,
    palette_focus: FocusHandle,
    rename_focus: FocusHandle,
    /// Fade-out flags: an overlay stays mounted while it animates away and a
    /// timer task unmounts it (cleared whenever the open flag drops).
    palette_closing: bool,
    profile_closing: bool,
    /// Smooth scrolling: the pane being eased plus (position, target) display
    /// offsets in lines. Bound to a session so tab switches stay safe.
    scroll_anim: Option<(Rc<RefCell<Session>>, f32, f32)>,
    /// One easing task per wheel gesture burst; stops when the anim settles.
    scroll_task: bool,
    /// Floating auto-hiding scrollbar opacity and activity timer.
    scrollbar_opacity: f32,
    last_scroll_time: std::time::Instant,
    /// Tab pill currently under the pointer: its hover-only close mark.
    hover_tab: Option<usize>,
    /// Geometric chrome mark under the pointer (new-tab plus, pane actions).
    /// Painted quads can't inherit hover text colour, so the view tracks it.
    hover_mark: Option<u64>,
    /// Updater settings and runtime state.
    updater_settings: UpdaterSettings,
    is_checking_updates: bool,
    manual_status_message: Option<String>,
    last_manual_check: Option<std::time::Instant>,
    current_backoff_secs: u64,
    rate_limit_reset: Option<u64>,
    available_update: Option<ReleaseInfo>,
    update_panel_open: bool,
    update_panel_closing: bool,
    pub app_settings: AppSettingsFile,
    pub settings_tab: SettingsTab,
    pub recording_shortcut: Option<String>,
    pub font_family: String,
    pub line_height: f32,
    pub corner_radius: f32,
    pub opacity: f32,
    pub blur_enabled: bool,
    pub cursor_blink: bool,
    window_was_active: bool,
    pub mouse_down_pane: Option<usize>,
}

struct Tab {
    id: u64,
    /// User-set name (Rename dialog / palette); wins over the shell title.
    custom_title: Option<String>,
    /// Split panes; always at least one.
    panes: Vec<Pane>,
    /// Pane that owns the keyboard and the caret.
    active_pane: usize,
    /// Direction the panes are tiled in (the last split decides).
    axis: SplitAxis,
    /// While true only the active pane is shown (header maximize action).
    maximized: bool,
    /// Smooth width collapse on close.
    closing: bool,
    close_progress: f32,
}

/// How a tab's panes tile the sheet.
#[derive(Clone, Copy, PartialEq)]
enum SplitAxis {
    /// Side by side, divided by a vertical hairline.
    Columns,
    /// Stacked, divided by a horizontal hairline.
    Rows,
}

/// One shell inside a tab (a tab is one or more panes side by side).
struct Pane {
    /// Bumped on every (re)spawn; stale session messages are dropped.
    epoch: u64,
    /// Which profile this pane runs (Restart reuses it).
    profile: Profile,
    /// Latest shell-reported title (OSC 0/2), e.g. `vite` or
    /// `user@host: /mnt/c/Users/me`.
    title: String,
    session: Rc<RefCell<Session>>,
    /// Working directory of the shell, polled from the process (`crate::cwd`).
    cwd: Option<PathBuf>,
    /// Git branch of the working directory, polled alongside it.
    branch: Option<String>,
    exited: bool,
    /// Smooth cursor glide: (interpolated_col, interpolated_row) in cell units.
    cursor_pos: Option<(f32, f32)>,
    /// Target cursor position reported by the terminal grid.
    cursor_target: (f32, f32),
    /// Smooth split expansion progress (0.0 .. 1.0).
    pane_expansion: f32,
    /// Smooth pane collapse on close.
    closing: bool,
}

impl AtlasView {
    pub fn new(window: &mut Window, initial_dir: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let registry = ThemeRegistry::standard();
        let app_settings = load_app_settings();
        let theme_id = (0..registry.len())
            .find(|&i| registry.preset(i).name.eq_ignore_ascii_case(&app_settings.theme))
            .unwrap_or(0);
        let cursor_style = match app_settings.cursor_shape.as_str() {
            "block" => Some(CursorShape::Block),
            "bar" => Some(CursorShape::Beam),
            "underline" => Some(CursorShape::Underline),
            _ => None,
        };
        let font_family = app_settings.font_family.clone();
        let font_size = app_settings.font_size.clamp(10.0, 24.0);
        let line_height = app_settings.line_height.clamp(1.0, 1.8);
        let corner_radius = app_settings.corner_radius.clamp(0.0, 20.0);
        let opacity = app_settings.opacity.clamp(0.5, 1.0);
        let blur_enabled = app_settings.blur_enabled;
        let cursor_blink = app_settings.cursor_blink;

        let mut font = gpui::font(SharedString::from(font_family.clone()));
        font.fallbacks = Some(FontFallbacks::from_fonts(vec![
            "Segoe UI Emoji".to_string(),
            "Segoe UI Symbol".to_string(),
        ]));

        let mut view = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            next_epoch: 1,
            focus,
            theme: registry.preset(theme_id).theme.clone(),
            theme_transition: None,
            font,
            theme_id,
            registry,
            settings_open: false,
            profile_menu: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_anim_index: 0.0,
            rename_open: false,
            rename_value: String::new(),
            font_size,
            cursor_style,
            cursor_on: true,
            cursor_breath_phase: 0.0,
            cursor_last_typing: std::time::Instant::now(),
            settings_focus: cx.focus_handle(),
            palette_focus: cx.focus_handle(),
            rename_focus: cx.focus_handle(),
            palette_closing: false,
            profile_closing: false,
            scroll_anim: None,
            scroll_task: false,
            scrollbar_opacity: 0.0,
            last_scroll_time: std::time::Instant::now(),
            hover_tab: None,
            hover_mark: None,
            updater_settings: UpdaterSettings {
                auto_check_updates: app_settings.auto_check_updates,
                update_check_interval_mins: app_settings.update_check_interval_mins,
                last_check_time: app_settings.last_check_time,
                etag: app_settings.etag.clone(),
                last_seen_version: app_settings.last_seen_version.clone(),
                skipped_version: app_settings.skipped_version.clone(),
            },
            is_checking_updates: false,
            manual_status_message: None,
            last_manual_check: None,
            current_backoff_secs: 3600,
            rate_limit_reset: None,
            available_update: None,
            update_panel_open: false,
            update_panel_closing: false,
            app_settings: app_settings.clone(),
            settings_tab: SettingsTab::Appearance,
            recording_shortcut: None,
            font_family,
            line_height,
            corner_radius,
            opacity,
            blur_enabled,
            cursor_blink,
            window_was_active: true,
            mouse_down_pane: None,
        };
        crate::rounded_corners::set_dark_mode(theme_id == 0);
        crate::rounded_corners::set_blur(blur_enabled);
        let default_shell_prof = view.get_default_profile();
        view.new_tab_with_cwd(default_shell_prof, initial_dir, cx);

        // Motion & breathing ticker: drives continuous kinetic cursor gliding,
        // organic sinusoidal breathing, theme crossfading, palette gliding,
        // auto-hiding scrollbar transitions, and smooth tab/pane collapses.
        let motion_ticker = cx.spawn(async move |this, cx| {
            let mut last_tick = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let now = std::time::Instant::now();
                let dt = (now - last_tick).as_secs_f32().min(0.05);
                last_tick = now;

                let alive = this.update(cx, |view, cx| {
                    let mut needs_paint = false;

                    // 1. Organic cursor breathing (soft sinusoidal alpha when idle)
                    let typing_elapsed = view.cursor_last_typing.elapsed().as_millis();
                    if typing_elapsed > 450 {
                        view.cursor_breath_phase =
                            (view.cursor_breath_phase + dt * 4.5) % (std::f32::consts::PI * 2.0);
                        needs_paint = true;
                    } else if view.cursor_breath_phase != 0.0 {
                        view.cursor_breath_phase = 0.0;
                        needs_paint = true;
                    }

                    // 2. Theme crossfade interpolation (zero-flash smooth transition)
                    if let Some((from, to, ref mut t)) = view.theme_transition {
                        *t = (*t + dt * 6.5).min(1.0);
                        view.theme = Theme::interpolate(&from, &to, *t);
                        needs_paint = true;
                        if *t >= 1.0 {
                            view.theme = to;
                            view.theme_transition = None;
                        }
                    }

                    // 3. Command palette floating sliding selection pill
                    if view.palette_open {
                        let target = view.palette_index as f32;
                        let diff = target - view.palette_anim_index;
                        if diff.abs() > 0.005 {
                            let factor = (1.0 - (-26.0 * dt).exp()).clamp(0.0, 1.0);
                            view.palette_anim_index += diff * factor;
                            needs_paint = true;
                        } else {
                            view.palette_anim_index = target;
                        }
                    }

                    // 4. Floating auto-hiding scrollbar fade
                    let scroll_idle = view.last_scroll_time.elapsed().as_millis();
                    if scroll_idle < 650 {
                        if view.scrollbar_opacity < 1.0 {
                            view.scrollbar_opacity = (view.scrollbar_opacity + dt * 10.0).min(1.0);
                            needs_paint = true;
                        }
                    } else if view.scrollbar_opacity > 0.0 {
                        view.scrollbar_opacity = (view.scrollbar_opacity - dt * 3.5).max(0.0);
                        needs_paint = true;
                    }

                    // 5. Smooth tab collapse on close
                    let mut tabs_to_remove = Vec::new();
                    for (i, tab) in view.tabs.iter_mut().enumerate() {
                        if tab.closing {
                            tab.close_progress = (tab.close_progress + dt * 9.0).min(1.0);
                            needs_paint = true;
                            if tab.close_progress >= 1.0 {
                                tabs_to_remove.push(i);
                            }
                        }
                    }
                    for &i in tabs_to_remove.iter().rev() {
                        view.tabs.remove(i);
                        if i < view.active {
                            view.active = view.active.saturating_sub(1);
                        }
                        if view.active >= view.tabs.len() && !view.tabs.is_empty() {
                            view.active = view.tabs.len() - 1;
                        }
                    }

                    // 6. Spatial cursor gliding, split pane expansion and pane collapse
                    for tab in &mut view.tabs {
                        let mut panes_to_remove = Vec::new();
                        for (p_ix, pane) in tab.panes.iter_mut().enumerate() {
                            if let Some((cur_col, cur_row)) = pane.cursor_pos {
                                let (tgt_col, tgt_row) = pane.cursor_target;
                                let d_col = tgt_col - cur_col;
                                let d_row = tgt_row - cur_row;
                                if d_col.abs() > 0.01 || d_row.abs() > 0.01 {
                                    // Snappy Apple spring deceleration (~90ms)
                                    let factor = (1.0 - (-28.0 * dt).exp()).clamp(0.0, 1.0);
                                    let next_col = if d_col.abs() < 0.03 {
                                        tgt_col
                                    } else {
                                        cur_col + d_col * factor
                                    };
                                    let next_row = if d_row.abs() < 0.03 {
                                        tgt_row
                                    } else {
                                        cur_row + d_row * factor
                                    };
                                    pane.cursor_pos = Some((next_col, next_row));
                                    needs_paint = true;
                                } else {
                                    pane.cursor_pos = Some((tgt_col, tgt_row));
                                }
                            }
                            if pane.closing {
                                pane.pane_expansion = (pane.pane_expansion - dt * 10.0).max(0.0);
                                needs_paint = true;
                                if pane.pane_expansion <= 0.0 {
                                    panes_to_remove.push(p_ix);
                                }
                            } else if pane.pane_expansion < 1.0 {
                                pane.pane_expansion = (pane.pane_expansion + dt * 10.0).min(1.0);
                                needs_paint = true;
                            }
                        }
                        for &p_ix in panes_to_remove.iter().rev() {
                            tab.panes.remove(p_ix);
                            if p_ix < tab.active_pane {
                                tab.active_pane = tab.active_pane.saturating_sub(1);
                            }
                            if tab.active_pane >= tab.panes.len() && !tab.panes.is_empty() {
                                tab.active_pane = tab.panes.len() - 1;
                            }
                            if tab.panes.len() == 1 {
                                tab.maximized = false;
                            }
                        }
                    }

                    if needs_paint {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        motion_ticker.detach();

        // Working-directory poll: reads each live shell's PEB every 1.5 s so
        // the pane title can track `cd` like a real terminal. Repaints only
        // when a path actually changed.
        let cwd_poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                if this
                    .update(cx, |view, cx| {
                        let mut changed = false;
                        for tab in &mut view.tabs {
                            for pane in &mut tab.panes {
                                if pane.exited {
                                    continue;
                                }
                                let Some(cwd) =
                                    pane.session.borrow().pid.and_then(crate::cwd::process_cwd)
                                else {
                                    continue;
                                };
                                if pane.cwd.as_deref() != Some(cwd.as_path()) {
                                    pane.cwd = Some(cwd);
                                    changed = true;
                                }
                                let branch = git_cwd(pane).as_deref().and_then(git_branch);
                                if branch != pane.branch {
                                    pane.branch = branch;
                                    changed = true;
                                }
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        cwd_poll.detach();

        // Background updater scheduler: runs an initial check 4s after startup,
        // and checks periodically every 60s against configured interval and backoff.
        let update_scheduler = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(4))
                .await;

            let _ = this.update(cx, |view, cx| {
                view.maybe_auto_check(false, cx);
            });

            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;

                let alive = this.update(cx, |view, cx| {
                    view.maybe_auto_check(false, cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        update_scheduler.detach();

        view
    }

    /// Grid size to spawn a new PTY at: the active pane's current grid when
    /// one exists, so a fresh shell starts at (roughly) its final width
    /// instead of the classic 80×24 and reflowing under its first prompt.
    fn default_grid_size(&self) -> (usize, usize) {
        self.active_pane()
            .map(|pane| {
                let session = pane.session.borrow();
                (session.cols, session.rows)
            })
            .filter(|(cols, rows)| *cols > 0 && *rows > 0)
            .unwrap_or((80, 24))
    }

    pub fn new_tab(&mut self, profile: Profile, cx: &mut Context<Self>) {
        self.new_tab_with_cwd(profile, None, cx);
    }

    pub fn new_tab_with_cwd(
        &mut self,
        profile: Profile,
        cwd_override: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_id;
        let epoch = self.next_epoch;
        self.next_id += 1;
        self.next_epoch += 1;

        // A fresh terminal should be visible, not hidden behind settings.
        self.settings_open = false;

        let spawn_cwd = cwd_override.or_else(home_dir);
        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(init_cols, init_rows, spawn_cwd, profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd);
                self.tabs.push(Tab {
                    id,
                    custom_title: None,
                    panes: vec![Pane {
                        epoch,
                        profile,
                        title: profile_title(profile),
                        session: session.clone(),
                        cwd,
                        branch: None,
                        exited: false,
                        cursor_pos: None,
                        cursor_target: (0.0, 0.0),
                        pane_expansion: 1.0,
                        closing: false,
                    }],
                    active_pane: 0,
                    axis: SplitAxis::Columns,
                    maximized: false,
                    closing: false,
                    close_progress: 0.0,
                });
                self.active = self.tabs.len() - 1;
                Self::watch(epoch, session, rx, cx);
                crate::log_line(&format!("tab {id}: shell spawned"));
            }
            Err(err) => {
                eprintln!("atlas-native: failed to spawn shell: {err:#}");
                crate::log_line(&format!("tab {id}: spawn failed: {err:#}"));
            }
        }
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let open_count = self.tabs.iter().filter(|t| !t.closing).count();
        if open_count <= 1 {
            self.tabs.remove(index);
            self.hover_tab = None;
            if self.tabs.is_empty() {
                self.new_tab(Profile::PowerShell, cx);
                return;
            }
            if index < self.active {
                self.active = self.active.saturating_sub(1);
            }
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            }
            cx.notify();
            return;
        }

        // Animate smooth width and opacity collapse
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.closing = true;
            self.hover_tab = None;
        }
        if index == self.active {
            if let Some(next_active) = self.tabs.iter().enumerate().position(|(i, t)| i != index && !t.closing) {
                self.active = next_active;
            }
        }
        cx.notify();
    }

    /// Restart the active pane of a tab with a fresh session.
    fn restart_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let (id, profile, pane_ix) = match self.tabs.get(index) {
            Some(tab) => match tab.panes.get(tab.active_pane) {
                Some(pane) => (tab.id, pane.profile, tab.active_pane),
                None => return,
            },
            None => return,
        };
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(init_cols, init_rows, home_dir(), profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd);
                if let Some(tab) = self.tabs.get_mut(index) {
                    if let Some(pane) = tab.panes.get_mut(pane_ix) {
                        pane.epoch = epoch;
                        pane.session = session.clone();
                        pane.cwd = cwd;
                        pane.branch = None;
                        pane.title = profile_title(profile);
                        pane.exited = false;
                    }
                }
                Self::watch(epoch, session, rx, cx);
                crate::log_line(&format!("tab {id}: shell restarted"));
            }
            Err(err) => {
                eprintln!("atlas-native: failed to restart shell: {err:#}");
                crate::log_line(&format!("tab {id}: restart failed: {err:#}"));
            }
        }
        cx.notify();
    }

    /// Split the active tab: a new pane running the active pane's profile is
    /// spawned in `axis` and takes focus. The whole tab re-tiles to that axis.
    fn split_active(&mut self, axis: SplitAxis, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let profile = match tab.panes.get(tab.active_pane) {
            Some(pane) => pane.profile,
            None => return,
        };
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(init_cols, init_rows, home_dir(), profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd);
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.panes.push(Pane {
                        epoch,
                        profile,
                        title: profile_title(profile),
                        session: session.clone(),
                        cwd,
                        branch: None,
                        exited: false,
                        cursor_pos: None,
                        cursor_target: (0.0, 0.0),
                        pane_expansion: 0.05,
                        closing: false,
                    });
                    tab.active_pane = tab.panes.len() - 1;
                    tab.axis = axis;
                    tab.maximized = false;
                }
                Self::watch(epoch, session, rx, cx);
                crate::log_line(&format!("pane split: shell spawned (epoch {epoch})"));
            }
            Err(err) => {
                eprintln!("atlas-native: failed to split pane: {err:#}");
                crate::log_line(&format!("pane split failed: {err:#}"));
            }
        }
        cx.notify();
    }

    /// Close the active pane; the tab itself goes when its last pane does.
    fn close_pane(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let open_panes = tab.panes.iter().filter(|p| !p.closing).count();
        if open_panes <= 1 {
            let index = self.active;
            self.close_tab(index, cx);
            return;
        }
        if let Some(pane) = tab.panes.get_mut(tab.active_pane) {
            pane.closing = true;
        }
        if let Some(next_active) = tab.panes.iter().enumerate().position(|(i, p)| i != tab.active_pane && !p.closing) {
            tab.active_pane = next_active;
        }
        cx.notify();
    }

    /// Toggle zoom on the active pane: the sheet shows only it while the
    /// hidden panes keep running (the header's maximize action).
    fn toggle_maximize(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if tab.panes.len() > 1 {
                tab.maximized = !tab.maximized;
            }
        }
        cx.notify();
    }

    /// Drain one session's output/event stream into the UI.
    ///
    /// The task exits when the pane is gone or the session's channel closes.
    /// Stale messages from a previous session are ignored (epoch guard).
    fn watch(
        epoch: u64,
        session: Rc<RefCell<Session>>,
        rx: smol::channel::Receiver<SessionMsg>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(msg) = rx.recv().await {
                let keep = this.update(cx, |view, cx| {
                    let Some((tab_ix, pane_ix)) = view.tabs.iter().enumerate().find_map(|(ti, tab)| {
                        tab.panes
                            .iter()
                            .position(|pane| pane.epoch == epoch && Rc::ptr_eq(&pane.session, &session))
                            .map(|pi| (ti, pi))
                    }) else {
                        return false;
                    };
                    let pane = &mut view.tabs[tab_ix].panes[pane_ix];
                    match msg {
                        SessionMsg::Output(bytes) => {
                            view.cursor_last_typing = std::time::Instant::now();
                            pane.session.borrow_mut().feed(&bytes);
                        }
                        SessionMsg::Event(Event::Title(title)) => {
                            if !title.trim().is_empty() {
                                pane.title = title;
                            }
                        }
                        SessionMsg::Event(Event::Exit) => pane.exited = true,
                        SessionMsg::Event(Event::ColorRequest(index, formatter)) => {
                            // OSC 4/10/11/12 colour query (`\e]11;?`): answer
                            // with what is actually on screen — the current
                            // theme, or a program's own earlier override — so
                            // CLI agents can adapt to the live palette.
                            let over = *pane.session.borrow().term.colors();
                            let color = view.theme.color_at(index, &over);
                            let reply = formatter(alacritty_terminal::vte::ansi::Rgb {
                                r: (color.r * 255.0).round() as u8,
                                g: (color.g * 255.0).round() as u8,
                                b: (color.b * 255.0).round() as u8,
                            });
                            pane.session.borrow_mut().write_bytes(reply.as_bytes());
                        }
                        SessionMsg::Event(_) => {}
                    }
                    cx.notify();
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
            crate::log_line("watch: session channel closed");
        })
        .detach();
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// Pane that owns the keyboard and the status bar.
    fn active_pane(&self) -> Option<&Pane> {
        self.tabs
            .get(self.active)
            .and_then(|tab| tab.panes.get(tab.active_pane))
    }

    fn active_mode(&self) -> TermMode {
        self.active_pane()
            .map(|pane| *pane.session.borrow().term.mode())
            .unwrap_or_else(TermMode::empty)
    }

    fn write_to_shell(&self, bytes: &[u8]) {
        if let Some(pane) = self.active_pane() {
            pane.session.borrow_mut().write_bytes(bytes);
        }
    }

    /// Keyboard paging: instant, and it wins over any in-flight wheel easing.
    fn scroll(&mut self, lines: i32) {
        self.scroll_anim = None;
        if let Some(pane) = self.active_pane() {
            pane.session
                .borrow_mut()
                .term
                .scroll_display(Scroll::Delta(lines));
        }
    }

    /// Wheel scrolling: let the display offset glide toward the gesture target
    /// instead of stepping line-by-line (the trackpad "smooth" feel). Deltas
    /// accumulate while the gesture is hot; the easing task stops itself once
    /// the offset has settled.
    fn scroll_smooth(&mut self, lines: f32, cx: &mut Context<Self>) {
        if lines == 0.0 {
            return;
        }
        self.last_scroll_time = std::time::Instant::now();
        self.scrollbar_opacity = 1.0;
        let Some(pane) = self.active_pane() else {
            return;
        };
        let session = pane.session.clone();
        let current = session.borrow().term.renderable_content().display_offset as f32;
        let max = session.borrow().term.history_size() as f32;

        let same_pane = match &self.scroll_anim {
            Some((bound, _, _)) => Rc::ptr_eq(bound, &session),
            None => false,
        };
        if same_pane {
            if let Some((_, _, target)) = self.scroll_anim.as_mut() {
                *target = (*target + lines).clamp(0.0, max);
            }
        } else {
            self.scroll_anim = Some((session, current, (current + lines).clamp(0.0, max)));
        }

        if self.scroll_task {
            return;
        }
        self.scroll_task = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let tick = this.update(cx, |view, cx| {
                    let Some((session, pos, target)) = view.scroll_anim.clone() else {
                        view.scroll_task = false;
                        return true;
                    };
                    view.last_scroll_time = std::time::Instant::now();
                    // Exponential approach: quick at first, gentle at the end.
                    let next = if (target - pos).abs() < 0.04 {
                        target
                    } else {
                        pos + (target - pos) * 0.30
                    };
                    let settled = next == target;
                    view.scroll_anim = if settled {
                        None
                    } else {
                        Some((session.clone(), next, target))
                    };
                    if settled {
                        view.scroll_task = false;
                    }
                    let mut guard = session.borrow_mut();
                    let current = guard.term.renderable_content().display_offset as i32;
                    let target_int = next.floor() as i32;
                    let delta = target_int - current;
                    if delta != 0 {
                        guard.term.scroll_display(Scroll::Delta(delta));
                    }
                    cx.notify();
                    settled
                });
                if !matches!(tick, Ok(false)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn paste(&self, cx: &mut Context<Self>) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        let bracketed = pane
            .session
            .borrow()
            .term
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let mut bytes = Vec::with_capacity(text.len() + 12);
        if bracketed {
            bytes.extend_from_slice(b"\x1b[200~");
        }
        bytes.extend_from_slice(text.as_bytes());
        if bracketed {
            bytes.extend_from_slice(b"\x1b[201~");
        }
        pane.session.borrow_mut().write_bytes(&bytes);
        cx.notify();
    }

    fn cycle(&mut self, delta: i32) {
        if self.tabs.is_empty() {
            return;
        }
        let len = self.tabs.len() as i32;
        self.active = ((self.active as i32 + delta).rem_euclid(len)) as usize;
    }

    /// Cycle to the next theme preset (Ctrl+Shift+T).
    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = self.registry.next(self.theme_id);
        self.set_theme(next, cx);
    }

    /// Apply a registry preset and keep every mirror in sync (settings pills
    /// and the keyboard shortcut both land here).
    fn set_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_id = index.min(self.registry.len() - 1);
        let target = self.registry.preset(self.theme_id).theme.clone();
        self.theme_transition = Some((self.theme.clone(), target, 0.0));
        crate::rounded_corners::set_dark_mode(self.theme_id == 0);
        self.sync_and_save_settings();
        cx.notify();
    }

    /// Persist active settings and updater state to %APPDATA%/atlas-terminal/settings.json.
    fn sync_and_save_settings(&mut self) {
        let cursor_shape = match self.cursor_style {
            None => "auto".to_string(),
            Some(CursorShape::Block) => "block".to_string(),
            Some(CursorShape::Beam) => "bar".to_string(),
            Some(CursorShape::Underline) => "underline".to_string(),
            Some(CursorShape::HollowBlock) => "hollow".to_string(),
            Some(CursorShape::Hidden) => "hidden".to_string(),
        };
        self.app_settings.theme = self.registry.preset(self.theme_id).name.to_string();
        self.app_settings.font_family = self.font_family.clone();
        self.app_settings.font_size = self.font_size;
        self.app_settings.line_height = self.line_height;
        self.app_settings.corner_radius = self.corner_radius;
        self.app_settings.opacity = self.opacity;
        self.app_settings.blur_enabled = self.blur_enabled;
        self.app_settings.cursor_shape = cursor_shape;
        self.app_settings.cursor_blink = self.cursor_blink;
        self.app_settings.auto_check_updates = self.updater_settings.auto_check_updates;
        self.app_settings.update_check_interval_mins = self.updater_settings.update_check_interval_mins;
        self.app_settings.last_check_time = self.updater_settings.last_check_time;
        self.app_settings.etag = self.updater_settings.etag.clone();
        self.app_settings.last_seen_version = self.updater_settings.last_seen_version.clone();
        self.app_settings.skipped_version = self.updater_settings.skipped_version.clone();

        crate::rounded_corners::set_dark_mode(self.theme_id == 0);
        crate::rounded_corners::set_blur(self.blur_enabled);
        save_app_settings(&self.app_settings);
    }

    pub fn get_shortcut(&self, action: &str) -> String {
        self.app_settings
            .shortcuts
            .get(action)
            .cloned()
            .unwrap_or_else(|| {
                default_shortcuts()
                    .get(action)
                    .cloned()
                    .unwrap_or_default()
            })
    }

    pub fn get_default_profile(&self) -> Profile {
        match self.app_settings.default_shell.to_ascii_lowercase().as_str() {
            "cmd" | "command prompt" => Profile::Cmd,
            "wsl" | "ubuntu" => Profile::Wsl,
            "git bash" | "bash" => Profile::GitBash,
            _ => Profile::PowerShell,
        }
    }

    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        let session = pane.session.borrow();
        if let Some(text) = session.term.selection_to_string() {
            if !text.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
        }
    }

    pub fn point_from_mouse(&self, mouse_pos: gpui::Point<Pixels>, pane_ix: usize) -> Option<Point> {
        let tab = self.tabs.get(self.active)?;
        let pane = tab.panes.get(pane_ix)?;
        let session = pane.session.borrow();
        let (left, top, cell_width, cell_height) = session.grid_bounds?;
        let mx = f32::from(mouse_pos.x);
        let my = f32::from(mouse_pos.y);
        let col = if mx < left {
            0
        } else {
            ((mx - left) / cell_width).floor().max(0.0) as usize
        };
        let screen_row = if my < top {
            0
        } else {
            ((my - top) / cell_height).floor().max(0.0) as i32
        };
        let offset = session.term.renderable_content().display_offset as i32;
        let line = Line(screen_row - offset);
        Some(Point::new(line, Column(col)))
    }

    pub fn handle_terminal_right_click(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let copy_text = if let Some(pane) = self.active_pane() {
            let session = pane.session.borrow();
            session.term.selection_to_string()
        } else {
            None
        };

        if let Some(text) = copy_text.filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            if let Some(pane) = self.active_pane() {
                pane.session.borrow_mut().term.selection = None;
            }
            cx.notify();
        } else {
            // Paste from clipboard
            self.paste(cx);
            cx.notify();
        }
    }

    pub fn set_font_family(&mut self, family: &str, cx: &mut Context<Self>) {
        self.font_family = family.to_string();
        let mut font = gpui::font(SharedString::from(self.font_family.clone()));
        font.fallbacks = Some(FontFallbacks::from_fonts(vec![
            "Segoe UI Emoji".to_string(),
            "Segoe UI Symbol".to_string(),
        ]));
        self.font = font;
        self.sync_and_save_settings();
        cx.notify();
    }

    /// Periodic / focus-based automatic update check. Silently skips offline/errors.
    fn maybe_auto_check(&mut self, is_focus_event: bool, cx: &mut Context<Self>) {
        if !self.updater_settings.auto_check_updates || self.is_checking_updates {
            return;
        }

        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Rate limit pause: wait until GitHub quota resets
        if let Some(reset) = self.rate_limit_reset {
            if now_epoch < reset {
                return;
            }
        }

        if is_focus_event {
            // Window focus regain: only check if last check was > 1 hour ago
            if let Some(last) = self.updater_settings.last_check_time {
                if now_epoch.saturating_sub(last) < 3600 {
                    return;
                }
            }
        } else {
            // Background periodic check: check against configured interval or exponential backoff
            let interval_secs = (self.updater_settings.update_check_interval_mins as u64 * 60)
                .max(self.current_backoff_secs);
            if let Some(last) = self.updater_settings.last_check_time {
                if now_epoch.saturating_sub(last) < interval_secs {
                    return;
                }
            }
        }

        self.trigger_update_check(false, cx);
    }

    /// Manual update check triggered by button in top bar or Settings.
    fn check_updates_manual(&mut self, cx: &mut Context<Self>) {
        if self.is_checking_updates {
            return;
        }

        // 10 second cooldown on manual checks to prevent spam
        if let Some(last) = self.last_manual_check {
            if last.elapsed() < Duration::from_secs(10) {
                self.manual_status_message =
                    Some("Please wait a few seconds before checking again.".to_string());
                cx.notify();
                return;
            }
        }

        self.last_manual_check = Some(std::time::Instant::now());
        self.trigger_update_check(true, cx);
    }

    /// Spawn unblocked thread to run WinHTTP check against GitHub API.
    fn trigger_update_check(&mut self, is_manual: bool, cx: &mut Context<Self>) {
        if self.is_checking_updates {
            return;
        }
        self.is_checking_updates = true;
        if is_manual {
            self.manual_status_message = Some("Checking for updates...".to_string());
        }
        cx.notify();

        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let etag = self.updater_settings.etag.clone();

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                updater::check_for_updates(&current_version, etag.as_deref())
            })
            .await;

            let _ = this.update(cx, |view, cx| {
                view.handle_update_result(result, is_manual, cx);
            });
        })
        .detach();
    }

    /// Process the result of an update check (auto or manual).
    fn handle_update_result(
        &mut self,
        result: updater::UpdateCheckResult,
        is_manual: bool,
        cx: &mut Context<Self>,
    ) {
        self.is_checking_updates = false;
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match result {
            updater::UpdateCheckResult::UpdateAvailable(info) => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = info.reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(ref e) = info.etag {
                    self.updater_settings.etag = Some(e.clone());
                }

                let is_skipped = self
                    .updater_settings
                    .skipped_version
                    .as_deref()
                    == Some(&info.version);

                if is_skipped {
                    if is_manual {
                        self.manual_status_message =
                            Some(format!("v{} is available (skipped)", info.version));
                    }
                } else {
                    self.available_update = Some(info.clone());
                    self.updater_settings.last_seen_version = Some(info.version.clone());
                    if is_manual {
                        self.manual_status_message =
                            Some(format!("v{} is available!", info.version));
                        self.update_panel_open = true;
                        self.update_panel_closing = false;
                    }
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::NotModified { etag, reset_epoch } => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(e) = etag {
                    self.updater_settings.etag = Some(e);
                }
                if is_manual {
                    self.manual_status_message = Some("You're up to date".to_string());
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::UpToDate { etag, reset_epoch } => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(e) = etag {
                    self.updater_settings.etag = Some(e);
                }
                if is_manual {
                    self.manual_status_message = Some("You're up to date".to_string());
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::RateLimited { reset_epoch } => {
                self.rate_limit_reset = Some(reset_epoch);
                // Back off exponentially: 1h -> 2h -> 4h max
                self.current_backoff_secs = (self.current_backoff_secs * 2).min(14400);
                if is_manual {
                    let time_str = updater::format_epoch_time(reset_epoch);
                    self.manual_status_message =
                        Some(format!("Rate limited, try again at {time_str}"));
                }
            }
            updater::UpdateCheckResult::Offline => {
                // Silently skip when offline; manual check reports status
                if is_manual {
                    self.manual_status_message = Some("No internet connection".to_string());
                }
            }
            updater::UpdateCheckResult::Error(msg) => {
                // Auto check silently ignores errors; only manual check reports
                if is_manual {
                    self.manual_status_message = Some(format!("Check failed: {msg}"));
                }
            }
        }
        cx.notify();
    }

    fn toggle_update_panel(&mut self, cx: &mut Context<Self>) {
        if self.update_panel_closing {
            self.update_panel_closing = false;
        } else {
            self.update_panel_open = !self.update_panel_open;
            if self.update_panel_open {
                self.palette_open = false;
                self.profile_menu = false;
            }
        }
        cx.notify();
    }

    fn dismiss_update_panel(&mut self, cx: &mut Context<Self>) {
        if self.update_panel_closing || !self.update_panel_open {
            return;
        }
        self.update_panel_closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.update_panel_closing {
                    view.update_panel_closing = false;
                    view.update_panel_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn skip_current_update(&mut self, cx: &mut Context<Self>) {
        if let Some(update) = self.available_update.take() {
            self.updater_settings.skipped_version = Some(update.version);
            self.sync_and_save_settings();
        }
        self.dismiss_update_panel(cx);
    }

    /// Focus whatever is on top: the settings page when it is open, the
    /// terminal otherwise. Overlay dismissal returns keys here.
    fn focus_main(&self, window: &mut Window) {
        if self.settings_open {
            window.focus(&self.settings_focus);
        } else {
            window.focus(&self.focus);
        }
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_closing = false;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_anim_index = 0.0;
        self.profile_menu = false;
        window.focus(&self.palette_focus);
        cx.notify();
    }

    /// Fade the palette out, then unmount it once the timer task fires; a
    /// fresh open cancels the pending dismissal.
    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_closing {
            return;
        }
        self.palette_closing = true;
        self.focus_main(window);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.palette_closing {
                    view.palette_closing = false;
                    view.palette_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Commands matching the current query, in declaration order.
    fn palette_matches(&self) -> Vec<(Command, &'static str, &'static str, &'static str)> {
        let query = self.palette_query.trim().to_ascii_lowercase();
        COMMANDS
            .iter()
            .copied()
            .filter(|(_, _, label, _)| {
                query.is_empty() || label.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// Palette key routing: type to filter, ↑/↓ to move, Enter to run, Esc to
    /// dismiss. Key events arrive here only while the palette holds focus, so
    /// nothing leaks into the shell.
    fn handle_palette_key(
        &mut self,
        ks: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ks.key.as_str() {
            "escape" => self.close_palette(window, cx),
            "enter" => {
                let matches = self.palette_matches();
                let index = self.palette_index.min(matches.len().saturating_sub(1));
                match matches.get(index).copied() {
                    Some((command, _, _, _)) => self.run_command(command, window, cx),
                    None => self.close_palette(window, cx),
                }
            }
            "up" => {
                self.palette_index = self.palette_index.saturating_sub(1);
                cx.notify();
            }
            "down" => {
                let last = self.palette_matches().len().saturating_sub(1);
                self.palette_index = (self.palette_index + 1).min(last);
                cx.notify();
            }
            "backspace" => {
                if self.palette_query.pop().is_some() {
                    self.palette_index = 0;
                }
                cx.notify();
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    // The space bar arrives without `key_char` (gpui treats
                    // it as an immutable key), so type it by hand.
                    let typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else {
                        ks.key_char
                            .as_deref()
                            .map(|text| {
                                text.chars().filter(|c| !c.is_control()).collect()
                            })
                            .unwrap_or_default()
                    };
                    if !typed.is_empty() {
                        self.palette_query.push_str(&typed);
                        self.palette_index = 0;
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Run a palette command; the palette fades out on the way.
    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        match command {
            Command::NewTab => {
                self.new_tab(Profile::PowerShell, cx);
                window.focus(&self.focus);
            }
            Command::NewTabWsl => {
                self.new_tab(Profile::Wsl, cx);
                window.focus(&self.focus);
            }
            Command::SplitPane => {
                self.settings_open = false;
                self.split_active(SplitAxis::Columns, cx);
                window.focus(&self.focus);
            }
            Command::SplitPaneDown => {
                self.settings_open = false;
                self.split_active(SplitAxis::Rows, cx);
                window.focus(&self.focus);
            }
            Command::MaximizePane => {
                self.toggle_maximize(cx);
                window.focus(&self.focus);
            }
            Command::ClosePane => {
                self.close_pane(cx);
                self.focus_main(window);
            }
            Command::CloseTab => {
                let index = self.active;
                self.close_tab(index, cx);
                self.focus_main(window);
            }
            Command::NextTab => {
                self.cycle(1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::PrevTab => {
                self.cycle(-1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::RestartTab => {
                let index = self.active;
                self.restart_tab(index, cx);
                window.focus(&self.focus);
            }
            Command::RenameTab => self.open_rename(window, cx),
            Command::ToggleTheme => {
                self.toggle_theme(cx);
                self.focus_main(window);
            }
            Command::OpenSettings => {
                self.settings_open = true;
                self.profile_menu = false;
                window.focus(&self.settings_focus);
                cx.notify();
            }
            Command::FontInc => {
                self.font_size = (self.font_size + 1.0).min(24.0);
                self.focus_main(window);
                cx.notify();
            }
            Command::FontDec => {
                self.font_size = (self.font_size - 1.0).max(10.0);
                self.focus_main(window);
                cx.notify();
            }
            Command::CheckUpdates => {
                self.check_updates_manual(cx);
            }
        }
    }

    fn toggle_profile_menu(&mut self, cx: &mut Context<Self>) {
        if self.profile_closing {
            // Clicking the caret again while the menu fades keeps it open.
            self.profile_closing = false;
        } else {
            self.profile_menu = !self.profile_menu;
            if self.profile_menu {
                self.settings_open = false;
            }
        }
        cx.notify();
    }

    /// Fade the profile (new-tab) menu out, then unmount it.
    fn dismiss_profile(&mut self, cx: &mut Context<Self>) {
        if self.profile_closing {
            return;
        }
        self.profile_closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.profile_closing {
                    view.profile_closing = false;
                    view.profile_menu = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Open the rename dialog prefilled with the tab's current name.
    fn open_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(pane) = tab.panes.get(tab.active_pane) else {
            return;
        };
        self.rename_value = tab
            .custom_title
            .clone()
            .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)));
        self.rename_open = true;
        if self.palette_open {
            self.close_palette(window, cx);
        }
        window.focus(&self.rename_focus);
        cx.notify();
    }

    fn close_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rename_open = false;
        self.focus_main(window);
        cx.notify();
    }

    /// Commit the rename; an empty name falls back to the shell title.
    fn apply_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            let name = self.rename_value.trim().to_string();
            tab.custom_title = if name.is_empty() { None } else { Some(name) };
        }
        self.close_rename(window, cx);
    }

    fn handle_rename_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        match ks.key.as_str() {
            "escape" => self.close_rename(window, cx),
            "enter" => self.apply_rename(window, cx),
            "backspace" => {
                if self.rename_value.pop().is_some() {
                    cx.notify();
                }
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    // Space arrives without `key_char` (gpui quirk).
                    let typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else {
                        ks.key_char
                            .as_deref()
                            .map(|text| {
                                text.chars().filter(|c| !c.is_control()).collect()
                            })
                            .unwrap_or_default()
                    };
                    if !typed.is_empty() {
                        self.rename_value.push_str(&typed);
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Command palette overlay, centred under the titlebar.
    fn palette(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let matches = self.palette_matches();
        let closing = self.palette_closing;

        let mut text_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .flex_1()
            .text_size(px(13.0));
        if self.palette_query.is_empty() {
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(1.5)).h(px(17.0)).rounded_full().bg(theme.accent),
                );
            }
            text_row = text_row.child(div().text_color(theme.faint_fg).child("Type a command…"));
        } else {
            text_row = text_row.child(
                div()
                    .text_color(theme.chrome_fg_active)
                    .child(self.palette_query.clone()),
            );
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(1.5)).h(px(17.0)).rounded_full().bg(theme.accent),
                );
            }
        }

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .h(px(46.0))
            .px(px(16.0))
            .text_color(theme.chrome_fg)
            .child(icons::icon(icons::SEARCH).size(px(14.0)).text_color(theme.chrome_fg))
            .child(text_row);

        let mut list = div().flex().flex_col().p(px(6.0)).relative();
        if matches.is_empty() {
            list = list.child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .text_size(px(12.0))
                    .text_color(theme.faint_fg)
                    .child("No matching commands"),
            );
        } else {
            // Floating sliding selection pill (Raycast/Spotlight smooth glide)
            let pill_top = 6.0 + self.palette_anim_index * 36.0;
            list = list.child(
                div()
                    .absolute()
                    .left(px(6.0))
                    .right(px(6.0))
                    .top(px(pill_top))
                    .h(px(36.0))
                    .rounded(px(7.0))
                    .bg(theme.card_active),
            );

            for (index, (command, icon, label, shortcut)) in matches.into_iter().enumerate() {
                let hover = theme.card_hover;
                let row = div()
                    .id(ElementId::named_usize("palette-command", index))
                    .h(px(36.0))
                    .px(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(7.0))
                    .text_color(theme.chrome_fg)
                    .hover(move |style| style.bg(hover))
                    .child(icons::icon(icon).size(px(13.0)).text_color(theme.chrome_fg))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint_fg)
                            .child(shortcut),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.run_command(command, window, cx);
                    }));
                list = list.child(row);
            }
        }

        div()
            .absolute()
            .top(px(54.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("palette-panel", 0))
                    .w(px(560.0))
                    .flex()
                    .flex_col()
                    .rounded(px(12.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .track_focus(&self.palette_focus)
                    .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                        let keystroke = event.keystroke.clone();
                        view.handle_palette_key(&keystroke, window, cx);
                    }))
                    .child(input_row)
                    .child(div().h(px(1.0)).bg(theme.hairline))
                    .child(list),
            )
            .with_animation(
                ElementId::named_usize("palette-anim", closing as usize),
                Animation::new(Duration::from_millis(if closing {
                    OVERLAY_OUT_MS
                } else {
                    OVERLAY_IN_MS
                }))
                .with_easing(ease_apple_snappy),
                move |el, delta| {
                    let t = if closing { 1.0 - delta } else { delta };
                    el.opacity(t).top(px(54.0 - 8.0 * (1.0 - t)))
                },
            )
            .into_any_element()
    }

    /// Rename-tab dialog overlay (pane pen icon).
    fn rename_dialog(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut field = div()
            .h(px(34.0))
            .px(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .rounded(px(8.0))
            .bg(theme.card_hover)
            .border_1()
            .border_color(theme.elevated_border)
            .text_size(px(13.0))
            .text_color(theme.chrome_fg_active)
            .child(self.rename_value.clone());
        if self.cursor_on {
            field = field.child(div().w(px(1.5)).h(px(17.0)).rounded_full().bg(theme.accent));
        }

        div()
            .absolute()
            .top(px(120.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("rename-panel", 0))
                    .w(px(430.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(16.0))
                    .rounded(px(12.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .track_focus(&self.rename_focus)
                    .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                        let keystroke = event.keystroke.clone();
                        view.handle_rename_key(&keystroke, window, cx);
                    }))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme.chrome_fg_active)
                            .child("Rename tab"),
                    )
                    .child(field)
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint_fg)
                            .child("Enter to apply · Esc to cancel · empty restores the shell name"),
                    ),
            )
            .with_animation(
                ElementId::named_usize("rename-anim", 0),
                Animation::new(Duration::from_millis(OVERLAY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta).top(px(120.0 - 8.0 * (1.0 - delta))),
            )
            .into_any_element()
    }

    /// Full settings page shown in place of the terminal sheet.
    fn settings_page(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let card = || {
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(16.0))
                .rounded(px(10.0))
                .bg(theme.elevated_bg)
                .border_1()
                .border_color(theme.elevated_border)
                .w(relative(1.0))
                .min_w(px(0.0))
        };
        let section = |label: &'static str| -> AnyElement {
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.faint_fg)
                .child(label)
                .into_any_element()
        };
        let row_label = |label: &'static str| -> AnyElement {
            div()
                .w(px(110.0))
                .text_size(px(12.5))
                .text_color(theme.chrome_fg)
                .child(label)
                .into_any_element()
        };

        // --- Left Sidebar (220px) ---
        let nav_items = [
            (SettingsTab::Appearance, "Appearance", icons::SLIDERS),
            (SettingsTab::Shortcuts, "Shortcuts", icons::KEYBOARD),
            (SettingsTab::General, "General", icons::TERMINAL),
        ];

        let mut sidebar = div()
            .w(px(220.0))
            .flex_shrink_0()
            .h(relative(1.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.hairline)
            .p(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .mb(px(10.0))
                    .child(icons::icon(icons::SETTINGS).size(px(16.0)).text_color(theme.accent))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.chrome_fg_active)
                            .child("Settings"),
                    ),
            );

        for (nav_idx, &(tab, label, icon)) in nav_items.iter().enumerate() {
            let is_active = self.settings_tab == tab;
            let item_fg = if is_active {
                theme.chrome_fg_active
            } else {
                theme.chrome_fg
            };
            let item_bg = if is_active {
                Rgba {
                    a: 0.14,
                    ..theme.chrome_fg_active
                }
            } else {
                rgba(0x00000000)
            };
            let hover_bg = theme.card_hover;
            sidebar = sidebar.child(
                div()
                    .id(ElementId::named_usize("settings-nav-item", nav_idx))
                    .h(px(32.0))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .rounded(px(6.0))
                    .bg(item_bg)
                    .text_color(item_fg)
                    .hover(move |s| s.bg(hover_bg))
                    .child(icons::icon(icon).size(px(14.0)).text_color(item_fg))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(if is_active {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(label),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.settings_tab = tab;
                        cx.notify();
                    })),
            );
        }


        // --- Appearance Tab ---
        let fonts = [
            "Cascadia Code",
            "Consolas",
            "Fira Code",
            "JetBrains Mono",
        ];
        let mut font_family_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Font family"));
        for (idx, &family) in fonts.iter().enumerate() {
            let is_active = self.font_family.eq_ignore_ascii_case(family);
            let fam_str = family.to_string();
            font_family_row = font_family_row.child(pill_button(
                fam_str.clone(),
                100 + idx,
                is_active,
                theme,
                move |view, cx| {
                    view.set_font_family(&fam_str, cx);
                },
                cx,
            ));
        }

        let font_size_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Font size"))
            .child(stepper(
                110,
                format!("{} pt", self.font_size as i32),
                theme,
                |view, cx| {
                    view.font_size = (view.font_size - 1.0).max(10.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.font_size = (view.font_size + 1.0).min(24.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let line_height_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Line height"))
            .child(stepper(
                111,
                format!("{:.2}", self.line_height),
                theme,
                |view, cx| {
                    view.line_height =
                        ((view.line_height - 0.05) * 100.0).round() / 100.0;
                    view.line_height = view.line_height.max(1.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.line_height =
                        ((view.line_height + 0.05) * 100.0).round() / 100.0;
                    view.line_height = view.line_height.min(1.8);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let corner_radius_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Corner radius"))
            .child(stepper(
                112,
                format!("{} px", self.corner_radius as i32),
                theme,
                |view, cx| {
                    view.corner_radius = (view.corner_radius - 1.0).max(0.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.corner_radius = (view.corner_radius + 1.0).min(20.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let opacity_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Window opacity"))
            .child(stepper(
                113,
                format!("{}%", (self.opacity * 100.0).round() as i32),
                theme,
                |view, cx| {
                    view.opacity =
                        ((view.opacity - 0.05) * 100.0).round() / 100.0;
                    view.opacity = view.opacity.max(0.5);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.opacity =
                        ((view.opacity + 0.05) * 100.0).round() / 100.0;
                    view.opacity = view.opacity.min(1.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let blur_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Windows 11 Acrylic blur"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint_fg)
                            .child("System backdrop acrylic blur (Windows 11 DWM)"),
                    ),
            )
            .child(toggle_pill(
                114,
                self.blur_enabled,
                theme,
                |view, cx| {
                    view.blur_enabled = !view.blur_enabled;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let mut theme_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Theme preset"));
        for index in 0..self.registry.len() {
            let is_sel = self.theme_id == index;
            theme_row = theme_row.child(pill_button(
                self.registry.preset(index).name.to_string(),
                120 + index,
                is_sel,
                theme,
                move |view, cx| view.set_theme(index, cx),
                cx,
            ));
        }

        let cursor_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Cursor style"))
            .child(pill_button(
                "Auto".to_string(),
                130,
                self.cursor_style.is_none(),
                theme,
                |view, cx| {
                    view.cursor_style = None;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Block".to_string(),
                131,
                self.cursor_style == Some(CursorShape::Block),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Block);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Bar".to_string(),
                132,
                self.cursor_style == Some(CursorShape::Beam),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Beam);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Underline".to_string(),
                133,
                self.cursor_style == Some(CursorShape::Underline),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Underline);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let cursor_blink_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Cursor blinking"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint_fg)
                            .child("Animate cursor breathing and blinking when idle"),
                    ),
            )
            .child(toggle_pill(
                134,
                self.cursor_blink,
                theme,
                |view, cx| {
                    view.cursor_blink = !view.cursor_blink;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let appearance_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.fg)
                            .child("Appearance"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.faint_fg)
                            .child("Font typography, window geometry, backdrop blur, and color palette"),
                    ),
            )
            .child(
                card()
                    .child(section("FONT & TYPOGRAPHY"))
                    .child(font_family_row)
                    .child(font_size_row)
                    .child(line_height_row),
            )
            .child(
                card()
                    .child(section("WINDOW & BACKDROP"))
                    .child(corner_radius_row)
                    .child(opacity_row)
                    .child(blur_row),
            )
            .child(
                card()
                    .child(section("THEME & CURSOR"))
                    .child(theme_row)
                    .child(cursor_row)
                    .child(cursor_blink_row),
            );

        // --- Shortcuts Tab ---
        let actions = [
            ACTION_NEW_TAB,
            ACTION_CLOSE_TAB,
            ACTION_SPLIT_HORIZONTAL,
            ACTION_SPLIT_VERTICAL,
            ACTION_FOCUS_NEXT_PANE,
            ACTION_FOCUS_PREV_PANE,
            ACTION_NEXT_TAB,
            ACTION_PREV_TAB,
            ACTION_COPY,
            ACTION_PASTE,
            ACTION_PALETTE,
            ACTION_FULLSCREEN,
            ACTION_CLEAR_SCROLLBACK,
            ACTION_OPEN_SETTINGS,
            ACTION_ZOOM_IN,
            ACTION_ZOOM_OUT,
        ];

        let mut shortcuts_list = card().child(section("KEYBOARD SHORTCUTS"));
        let def_map = default_shortcuts();

        for (idx, &action) in actions.iter().enumerate() {
            let current_chord = self.get_shortcut(action);
            let default_chord = def_map.get(action).cloned().unwrap_or_default();
            let is_recording = self.recording_shortcut.as_deref() == Some(action);
            let is_custom = current_chord != default_chord;

            let conflict = self
                .app_settings
                .shortcuts
                .iter()
                .find(|(act, ch)| {
                    act.as_str() != action && ch.as_str() == current_chord.as_str()
                })
                .map(|(act, _)| action_display_name(act));

            let action_str = action.to_string();
            let def_str = default_chord.clone();

            let mut row = div()
                .id(ElementId::named_usize("shortcut-row", idx))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .py(px(4.0));

            let mut left_col = div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.chrome_fg_active)
                        .child(action_display_name(action)),
                );

            if let Some(other) = conflict {
                left_col = left_col.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0xE6A700))
                        .child(format!("Conflict with {other}")),
                );
            }
            row = row.child(left_col);

            let keycap_text = if is_recording {
                "Press key combo…".to_string()
            } else {
                pretty_shortcut(&current_chord)
            };

            let keycap_bg = if is_recording {
                Rgba {
                    a: 0.2,
                    ..theme.accent
                }
            } else {
                theme.soft_bg
            };
            let keycap_border = if is_recording {
                theme.accent
            } else {
                theme.hairline
            };
            let keycap_fg = if is_recording {
                theme.accent
            } else {
                theme.chrome_fg_active
            };

            let mut right_col = div().flex().flex_row().items_center().gap(px(6.0));

            let act_clone = action_str.clone();
            let keycap_btn = div()
                .id(ElementId::named_usize("shortcut-keycap", idx))
                .h(px(26.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(keycap_bg)
                .border_1()
                .border_color(keycap_border)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(keycap_fg)
                .hover(move |s| s.opacity(0.85))
                .child(keycap_text)
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.recording_shortcut = Some(act_clone.clone());
                    cx.notify();
                }));
            right_col = right_col.child(keycap_btn);

            if is_custom {
                let reset_btn = div()
                    .id(ElementId::named_usize("shortcut-reset", idx))
                    .h(px(26.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_size(px(11.0))
                    .text_color(theme.faint_fg)
                    .hover(move |s| {
                        s.bg(theme.card_hover).text_color(theme.chrome_fg_active)
                    })
                    .child("Reset")
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.app_settings
                            .shortcuts
                            .insert(action_str.clone(), def_str.clone());
                        view.sync_and_save_settings();
                        cx.notify();
                    }));
                right_col = right_col.child(reset_btn);
            }

            row = row.child(right_col);
            shortcuts_list = shortcuts_list.child(row);
        }

        let shortcuts_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.fg)
                                    .child("Shortcuts"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.faint_fg)
                                    .child("Click a binding to rebind. Press Esc to cancel."),
                            ),
                    )
                    .child(pill_button(
                        "Reset All".to_string(),
                        299,
                        false,
                        theme,
                        |view, cx| {
                            view.app_settings.shortcuts = default_shortcuts();
                            view.sync_and_save_settings();
                            cx.notify();
                        },
                        cx,
                    )),
            )
            .child(shortcuts_list);

        // --- General Tab ---
        let shells = ["PowerShell", "Command Prompt", "WSL", "Git Bash"];
        let mut default_shell_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Default shell"));
        for (idx, &shell_name) in shells.iter().enumerate() {
            let is_sel = self.app_settings.default_shell.eq_ignore_ascii_case(shell_name);
            let s_str = shell_name.to_string();
            default_shell_row = default_shell_row.child(pill_button(
                s_str.clone(),
                300 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.default_shell = s_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let start_dirs = [("home", "User home (~)"), ("current", "Current directory")];
        let mut start_dir_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Start directory"));
        for (idx, &(dir_key, dir_label)) in start_dirs.iter().enumerate() {
            let is_sel = self.app_settings.start_directory == dir_key;
            let k_str = dir_key.to_string();
            start_dir_row = start_dir_row.child(pill_button(
                dir_label.to_string(),
                310 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.start_directory = k_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let scroll_opts = [
            (5000, "5,000"),
            (10000, "10,000"),
            (20000, "20,000"),
            (50000, "50,000"),
        ];
        let mut scroll_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Scrollback"));
        for (idx, &(lines, lbl)) in scroll_opts.iter().enumerate() {
            let is_sel = self.app_settings.scrollback_lines == lines;
            scroll_row = scroll_row.child(pill_button(
                lbl.to_string(),
                320 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.scrollback_lines = lines;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let startup_opts = [
            ("restore_session", "Restore last session"),
            ("new_tab", "Open new tab"),
        ];
        let mut startup_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Startup mode"));
        for (idx, &(mode_key, mode_lbl)) in startup_opts.iter().enumerate() {
            let is_sel = self.app_settings.startup_mode == mode_key;
            let m_str = mode_key.to_string();
            startup_row = startup_row.child(pill_button(
                mode_lbl.to_string(),
                330 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.startup_mode = m_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let confirm_close_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Confirm before closing active process"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint_fg)
                            .child("Ask before closing tabs with running commands"),
                    ),
            )
            .child(toggle_pill(
                340,
                self.app_settings.confirm_close_process,
                theme,
                |view, cx| {
                    view.app_settings.confirm_close_process =
                        !view.app_settings.confirm_close_process;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let copy_on_select_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Copy on select"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint_fg)
                            .child("Automatically copy selected terminal text to clipboard"),
                    ),
            )
            .child(toggle_pill(
                341,
                self.app_settings.copy_on_select,
                theme,
                |view, cx| {
                    view.app_settings.copy_on_select = !view.app_settings.copy_on_select;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let is_auto_check = self.updater_settings.auto_check_updates;
        let auto_check_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Check for updates automatically"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.faint_fg)
                            .child("Background check every interval and on focus; silent when offline"),
                    ),
            )
            .child(toggle_pill(
                350,
                is_auto_check,
                theme,
                |view, cx| {
                    view.updater_settings.auto_check_updates =
                        !view.updater_settings.auto_check_updates;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let mut interval_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Interval"));
        for (idx, &(mins, label)) in updater::INTERVAL_OPTIONS.iter().enumerate() {
            let is_sel = self.updater_settings.update_check_interval_mins == mins;
            interval_row = interval_row.child(pill_button(
                label.to_string(),
                360 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.updater_settings.update_check_interval_mins = mins;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let last_checked_text = self
            .updater_settings
            .last_check_time
            .map(updater::format_time_ago)
            .unwrap_or_else(|| "Never".to_string());

        let is_checking = self.is_checking_updates;
        let status_line = if is_checking {
            "Checking for updates…".to_string()
        } else if let Some(ref msg) = self.manual_status_message {
            format!("{msg} · Last checked: {last_checked_text}")
        } else {
            format!("Last checked: {last_checked_text}")
        };

        let check_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pt(px(2.0))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.chrome_fg)
                    .child(status_line),
            )
            .child(pill_button(
                if is_checking {
                    "Checking…".to_string()
                } else {
                    "Check now".to_string()
                },
                370,
                false,
                theme,
                |view, cx| {
                    view.check_updates_manual(cx);
                },
                cx,
            ));

        let link_hot = theme.chrome_fg_active;
        let pkg_version = env!("CARGO_PKG_VERSION");
        let about = card()
            .child(section("ABOUT"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                icons::icon(icons::ATLAS_MARK)
                                    .size(px(18.0))
                                    .text_color(theme.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(theme.chrome_fg_active)
                                    .child(format!("Atlas Terminal {pkg_version}")),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.chrome_fg)
                            .child("Native GPUI renderer · alacritty_terminal · no webview, no HTML"),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("settings-link", 0))
                            .text_size(px(11.5))
                            .text_color(theme.accent)
                            .hover(move |style| style.text_color(link_hot))
                            .child("github.com/akrram0/atlas-terminal")
                            .on_click(cx.listener(|_view, _: &ClickEvent, _window, cx| {
                                cx.open_url("https://github.com/akrram0/atlas-terminal");
                            })),
                    ),
            );

        let general_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.fg)
                            .child("General"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.faint_fg)
                            .child("Shell profiles, session startup, and application updates"),
                    ),
            )
            .child(
                card()
                    .child(section("SHELL & ENVIRONMENT"))
                    .child(default_shell_row)
                    .child(start_dir_row)
                    .child(scroll_row),
            )
            .child(
                card()
                    .child(section("SESSION & BEHAVIOR"))
                    .child(startup_row)
                    .child(confirm_close_row)
                    .child(copy_on_select_row),
            )
            .child(
                card()
                    .child(section("UPDATES"))
                    .child(auto_check_row)
                    .child(interval_row)
                    .child(check_row),
            )
            .child(about);

        let content_body = div()
            .flex()
            .flex_col()
            .child(match self.settings_tab {
                SettingsTab::Appearance => appearance_view.into_any_element(),
                SettingsTab::Shortcuts => shortcuts_view.into_any_element(),
                SettingsTab::General => general_view.into_any_element(),
            })
            .with_animation(
                ElementId::named_usize("settings-tab-pane", self.settings_tab as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta),
            )
            .into_any_element();

        let content_header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .mb(px(18.0))
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.chrome_fg_active)
                    .child(match self.settings_tab {
                        SettingsTab::Appearance => "Appearance",
                        SettingsTab::Shortcuts => "Keyboard Shortcuts",
                        SettingsTab::General => "General",
                    }),
            )
            .child(
                div()
                    .id(ElementId::named_usize("settings-top-back-btn", 0))
                    .h(px(28.0))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .rounded(px(6.0))
                    .bg(Rgba { a: 0.08, ..theme.chrome_fg })
                    .hover(move |s| s.bg(theme.card_hover))
                    .text_color(theme.chrome_fg)
                    .child(icons::icon(icons::CHEVRON_LEFT).size(px(13.0)))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Back to Terminal (Esc)"),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    })),
            );

        let right_pane = div()
            .id(ElementId::named_usize("settings-content-scroll", 0))
            .flex_1()
            .min_w(px(0.0))
            .h(relative(1.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .p(px(24.0))
            .child(
                div()
                    .w(relative(1.0))
                    .max_w(px(720.0))
                    .min_w(px(0.0))
                    .child(content_header)
                    .child(content_body),
            );

        div()
            .id(ElementId::named_usize("settings-root", 0))
            .flex()
            .flex_row()
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .track_focus(&self.settings_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if let Some(ref action) = view.recording_shortcut.clone() {
                    if event.keystroke.key == "escape" {
                        view.recording_shortcut = None;
                        cx.notify();
                        return;
                    }
                    if let Some(chord) = format_keystroke_chord(&event.keystroke) {
                        view.app_settings.shortcuts.insert(action.clone(), chord);
                        view.recording_shortcut = None;
                        view.sync_and_save_settings();
                        cx.notify();
                        return;
                    }
                }
                if event.keystroke.key == "escape" {
                    view.settings_open = false;
                    window.focus(&view.focus);
                    cx.notify();
                    return;
                }
                if let Some(chord) = format_keystroke_chord(&event.keystroke) {
                    if chord == view.get_shortcut(ACTION_OPEN_SETTINGS) {
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                        return;
                    }
                }
            }))
            .child(sidebar)
            .child(right_pane)
            .into_any_element()
    }

    fn handle_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // Solid cursor while typing; the motion ticker flips it back to breathing on its own.
        self.cursor_on = true;
        self.cursor_last_typing = std::time::Instant::now();

        if key == "escape"
            && (self.settings_open || self.profile_menu || self.update_panel_open)
        {
            if self.recording_shortcut.is_some() {
                self.recording_shortcut = None;
                cx.notify();
                return;
            }
            if self.settings_open {
                self.settings_open = false;
                window.focus(&self.focus);
                cx.notify();
                return;
            }
            if self.profile_menu {
                self.dismiss_profile(cx);
            }
            if self.update_panel_open {
                self.dismiss_update_panel(cx);
            }
            cx.notify();
            return;
        }

        // Dynamic shortcuts matching
        if let Some(chord) = format_keystroke_chord(ks) {
            if chord == self.get_shortcut(ACTION_OPEN_SETTINGS) {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    window.focus(&self.settings_focus);
                } else {
                    window.focus(&self.focus);
                }
                cx.notify();
                return;
            }

            if !self.settings_open {
                if chord == self.get_shortcut(ACTION_NEW_TAB) {
                    let prof = self.get_default_profile();
                    self.new_tab(prof, cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_CLOSE_TAB) {
                    self.close_pane(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_SPLIT_HORIZONTAL) {
                    self.split_active(SplitAxis::Rows, cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_SPLIT_VERTICAL) {
                    self.split_active(SplitAxis::Columns, cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEXT_TAB) {
                    self.cycle(1);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_PREV_TAB) {
                    self.cycle(-1);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_FOCUS_NEXT_PANE) {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == self.get_shortcut(ACTION_FOCUS_PREV_PANE) {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = if tab.active_pane == 0 {
                                tab.panes.len() - 1
                            } else {
                                tab.active_pane - 1
                            };
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == self.get_shortcut(ACTION_PALETTE) {
                    self.open_palette(window, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_FULLSCREEN) {
                    crate::rounded_corners::window_button(1);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_CLEAR_SCROLLBACK) {
                    self.write_to_shell(b"\x1b[3J\x1b[H\x1b[2J");
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_COPY) {
                    self.copy_selection(cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_PASTE) {
                    self.paste(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_IN) {
                    self.font_size = (self.font_size + 1.0).min(24.0);
                    self.sync_and_save_settings();
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_OUT) {
                    self.font_size = (self.font_size - 1.0).max(10.0);
                    self.sync_and_save_settings();
                    cx.notify();
                    return;
                }
            }
        }

        // Mouseless shortcut: Ctrl+1 through Ctrl+9 to switch tabs directly
        if modifiers.control && !modifiers.shift && !modifiers.alt {
            if let Ok(num) = key.parse::<usize>() {
                if (1..=9).contains(&num) {
                    let target = num - 1;
                    if target < self.tabs.len() {
                        self.active = target;
                        self.settings_open = false;
                        self.focus_main(window);
                        cx.notify();
                        return;
                    }
                }
            }
        }

        // Mouseless shortcut: Alt+Arrows to cycle focus between split panes
        if modifiers.alt && !modifiers.control && !modifiers.shift {
            match key {
                "arrowright" | "arrowdown" => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                            cx.notify();
                            return;
                        }
                    }
                }
                "arrowleft" | "arrowup" => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = if tab.active_pane == 0 {
                                tab.panes.len() - 1
                            } else {
                                tab.active_pane - 1
                            };
                            cx.notify();
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        // Standard terminal behavior: Ctrl+C copies if there is an active text selection,
        // otherwise sends \x03 (SIGINT) to running process.
        if modifiers.control && !modifiers.shift && !modifiers.alt && key == "c" {
            let has_selection = self.active_pane().map(|p| {
                let s = p.session.borrow();
                s.term.selection.is_some()
                    && s.term.selection_to_string().map(|t| !t.is_empty()).unwrap_or(false)
            }).unwrap_or(false);
            if has_selection {
                self.copy_selection(cx);
                if let Some(p) = self.active_pane() {
                    p.session.borrow_mut().term.selection = None;
                }
                cx.notify();
                return;
            }
        }

        // Escape: deselect text if active
        if key == "escape" && !modifiers.control && !modifiers.alt && !modifiers.shift {
            if let Some(pane) = self.active_pane() {
                if pane.session.borrow().term.selection.is_some() {
                    pane.session.borrow_mut().term.selection = None;
                    cx.notify();
                    return;
                }
            }
        }

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.toggle_theme(cx),
                "m" => {
                    self.toggle_maximize(cx);
                    self.focus_main(window);
                }
                "p" => self.open_palette(window, cx),
                _ => {}
            }
            return;
        }

        if modifiers.shift && !modifiers.control && !modifiers.alt {
            match key {
                "pageup" => {
                    self.scroll(12);
                    cx.notify();
                    return;
                }
                "pagedown" => {
                    self.scroll(-12);
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        // Dead pane: only R restarts it.
        if self.active_pane().map(|pane| pane.exited).unwrap_or(false) {
            if !modifiers.control && !modifiers.alt && key == "r" {
                let index = self.active;
                self.restart_tab(index, cx);
            }
            return;
        }

        let mode = self.active_mode();
        if let Some(bytes) = keymap::to_bytes(ks, mode) {
            self.write_to_shell(&bytes);
            cx.notify();
        }
    }
}

impl Render for AtlasView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = crate::rounded_corners::is_active();
        if is_active && !self.window_was_active {
            self.maybe_auto_check(true, cx);
        }
        self.window_was_active = is_active;

        // A dismissal that raced another state change must not stay armed.
        if !self.palette_open {
            self.palette_closing = false;
        }
        if !self.profile_menu {
            self.profile_closing = false;
        }
        if !self.update_panel_open {
            self.update_panel_closing = false;
        }
        let theme = self.theme.clone();
        let zoomed = crate::rounded_corners::is_zoomed();
        let outer_radius = if zoomed { 0.0 } else { self.corner_radius };
        let inner_radius = if zoomed { 0.0 } else { (self.corner_radius - 1.0).max(0.0) };

        // ---- Pill top bar --------------------------------------------
        // Workspace pill · horizontal pill tabs · drag gap · settings /
        // palette / new-tab icons · native window controls. The gap reports
        // itself as caption slot 0 so presses there start the native move.
        let tb = theme.clone();
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .w(relative(1.0))
            .h(px(36.0))
            .pl(px(8.0))
            .bg(tb.panel_bg)
            .border_b_1()
            .border_color(tb.hairline);
        if !zoomed {
            bar = bar.rounded_t(px(inner_radius));
        }

        // Workspace pill (26×20): a 6%-ink translucent fill with the quiet
        // sidebar mark from the reference chrome. Opens the new-tab profile
        // menu (this shell has no sidebar).
        let ws_bg = tb.soft_bg;
        let ws_idle = tb.chrome_fg;
        let ws_hot = tb.chrome_fg_active;
        bar = bar.child(
            div()
                .id(ElementId::named_usize("workspace-pill", 0))
                .w(px(26.0))
                .h(px(22.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(ws_bg)
                .text_color(ws_idle)
                .hover(move |style| style.text_color(ws_hot))
                .child(icons::icon(icons::ATLAS_MARK).size(px(14.0)).text_color(ws_idle))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.toggle_profile_menu(cx);
                    window.focus(&view.focus);
                })),
        );

        // Horizontal tabs: active tab is a clean rounded rect (elevated bg + subtle border)
        // carrying the running process's own mark; inactive tabs are quiet with folder icon.
        // Tab close is hover-only.
        for ix in 0..self.tabs.len() {
            let active = ix == self.active;
            let tab = &self.tabs[ix];
            let pane = tab.panes.get(tab.active_pane);
            let label = match pane {
                Some(pane) => tab.custom_title.clone().unwrap_or_else(|| {
                    format_tab_title(pane)
                }),
                None => "Terminal".to_string(),
            };
            let icon = match pane {
                Some(pane) => session_icon(&pane.title, pane.profile),
                None => icons::TERMINAL,
            };
            let (pill_bg, pill_border, pill_fg, pill_icon) = if active {
                (
                    tb.elevated_bg,
                    tb.elevated_border,
                    tb.chrome_fg_active,
                    tb.accent,
                )
            } else {
                (
                    rgba(0x00000000),
                    rgba(0x00000000),
                    tb.chrome_fg,
                    tb.chrome_fg,
                )
            };
            let pill_hover = tb.card_hover;
            let pill_hot = tb.chrome_fg_active;
            let pill_close = active && self.hover_tab == Some(ix) && !tab.closing;

            let (max_w, min_w, px_pad, opacity) = if tab.closing {
                let scale = (1.0 - tab.close_progress).max(0.0);
                (220.0 * scale, 0.0, 8.0 * scale, scale)
            } else {
                (220.0, 40.0, 8.0, 1.0)
            };

            let mut pill = div()
                .id(ElementId::named_usize("tab-pill", ix))
                .h(px(24.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(px_pad))
                .max_w(px(max_w))
                .min_w(px(min_w))
                .opacity(opacity)
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(6.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            if !tab.closing {
                pill = pill
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                        if !*hovered && view.hover_tab == Some(ix) {
                            view.hover_tab = None;
                            cx.notify();
                        } else if *hovered && view.hover_tab != Some(ix) {
                            view.hover_tab = Some(ix);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(move |view, _: &MouseDownEvent, window, cx| {
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        }),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.active = ix;
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    }));
            }

            pill = pill
                .child(
                    div()
                        .text_color(pill_icon)
                        .child(icons::icon(icon).size(px(14.0)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child(label),
                );
            if pill_close {
                let close_hot = tb.chrome_fg_active;
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", ix))
                        .w(px(16.0))
                        .h(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .text_color(tb.faint_fg)
                        .hover(move |style| style.text_color(close_hot).bg(tb.card_hover))
                        .child(icons::icon(icons::CLOSE).size(px(10.5)).text_color(tb.faint_fg))
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        })),
                );
            }
            bar = bar.child(pill.with_animation(
                ElementId::named_usize("tab-pill-in", tab.id as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta),
            ));
        }

        // New tab: clean vector plus button
        let plus_hover = tb.card_hover;
        let plus_ink = if self.hover_mark == Some(HOVER_MARK_NEW_TAB) {
            tb.chrome_fg_active
        } else {
            tb.chrome_fg
        };
        bar = bar.child(
            div()
                .id(ElementId::named_usize("topbar-new", 0))
                .w(px(26.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .hover(move |style| style.bg(plus_hover))
                .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                    set_hover_mark(view, HOVER_MARK_NEW_TAB, *hovered, cx);
                }))
                .child(icons::icon(icons::PLUS).size(px(14.0)).text_color(plus_ink))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.new_tab(Profile::PowerShell, cx);
                    window.focus(&view.focus);
                })),
        );

        bar = bar.child(caption_spacer(0));

        let top_btn_hover = tb.card_hover;

        if let Some(ref update) = self.available_update {
            let badge_ink = tb.accent;
            let badge_bg = Rgba {
                a: 0.16,
                ..tb.accent
            };
            let badge_border = Rgba {
                a: 0.28,
                ..tb.accent
            };
            let ver = update.version.clone();
            bar = bar.child(
                div()
                    .id(ElementId::named_usize("topbar-update-badge", 0))
                    .h(px(24.0))
                    .px(px(9.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .rounded(px(6.0))
                    .bg(badge_bg)
                    .border_1()
                    .border_color(badge_border)
                    .text_color(badge_ink)
                    .hover(move |s| s.opacity(0.85))
                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(badge_ink))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("v{ver}")),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.toggle_update_panel(cx);
                    })),
            );
        } else if self.is_checking_updates {
            bar = bar.child(
                div()
                    .h(px(26.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .rounded(px(6.0))
                    .text_color(tb.faint_fg)
                    .child(icons::icon(icons::REFRESH).size(px(13.0)).text_color(tb.faint_fg))
                    .child(div().text_size(px(11.5)).child("Checking…")),
            );
        } else {
            bar = bar.child(
                div()
                    .id(ElementId::named_usize("topbar-update-btn", 0))
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .hover(move |s| s.bg(top_btn_hover))
                    .child(icons::icon(icons::REFRESH).size(px(15.0)).text_color(tb.chrome_fg))
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.toggle_update_panel(cx);
                    })),
            );
        }

        bar = bar.child(
            div()
                .id(ElementId::named_usize("topbar-settings-btn", 0))
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .hover(move |s| s.bg(top_btn_hover))
                .child(icons::icon(icons::SETTINGS).size(px(15.0)).text_color(tb.chrome_fg))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.settings_open = !view.settings_open;
                    if view.settings_open {
                        window.focus(&view.settings_focus);
                    } else {
                        window.focus(&view.focus);
                    }
                    cx.notify();
                })),
        );

        bar = bar.child(div().w(px(4.0)));

        // Window controls: clean 44px wide native vector controls filling top bar height
        let controls = [
            (icons::WINDOW_MINIMIZE, false),
            (
                if zoomed {
                    icons::WINDOW_RESTORE
                } else {
                    icons::WINDOW_MAXIMIZE
                },
                false,
            ),
            (icons::CLOSE, true),
        ];
        for (index, (icon_path, is_close)) in controls.into_iter().enumerate() {
            let button_theme = theme.clone();
            let hover_id = HOVER_MARK_WINDOW + index as u64;
            let is_hovered = self.hover_mark == Some(hover_id);
            let hover_bg = if is_close {
                rgb(0xC42B1C)
            } else {
                button_theme.card_hover
            };
            let ink = if is_hovered {
                if is_close {
                    rgb(0xFFFFFF)
                } else {
                    button_theme.chrome_fg_active
                }
            } else {
                button_theme.chrome_fg
            };
            let mut btn = div()
                .id(ElementId::named_usize("window-control", index))
                .w(px(46.0))
                .h(relative(1.0))
                .flex()
                .items_center()
                .justify_center()
                .hover(move |style| style.bg(hover_bg))
                .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                    set_hover_mark(view, hover_id, *hovered, cx);
                }))
                .child(
                    icons::icon(icon_path)
                        .size(px(15.0))
                        .text_color(ink),
                )
                .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                    crate::rounded_corners::window_button(index);
                    cx.notify();
                }));
            if is_close && !zoomed {
                btn = btn.rounded_tr(px(inner_radius));
            }
            bar = bar.child(btn);
        }

        // Active terminal color overrides (OSC 11 etc.) define the sheet surface.
        let _over = self
            .active_pane()
            .map(|pane| *pane.session.borrow().term.colors())
            .unwrap_or_default();

        // ---- Terminal area -------------------------------------------
        let index = self.active.min(self.tabs.len().saturating_sub(1));
        let body: AnyElement = if self.settings_open {
            self.settings_page(&theme, cx)
        } else if self.tabs.is_empty() {
            div().flex_1().into_any_element()
        } else {
            // Snapshot cursor targets and initialize cursor position for active panes
            for pane in &mut self.tabs[index].panes {
                let (tgt_col, tgt_row) = {
                    let session = pane.session.borrow();
                    let content = session.term.renderable_content();
                    let offset = content.display_offset as i32;
                    (
                        content.cursor.point.column.0 as f32,
                        (content.cursor.point.line.0 + offset) as f32,
                    )
                };
                pane.cursor_target = (tgt_col, tgt_row);
                if pane.cursor_pos.is_none() {
                    pane.cursor_pos = Some((tgt_col, tgt_row));
                }
            }

            let tab = &self.tabs[index];
            let axis = tab.axis;

            let mut area = div()
                .id(ElementId::named_usize("terminal-area", 0))
                .flex()
                .flex_1()
                .w(relative(1.0))
                .h(relative(1.0))
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    let keystroke = event.keystroke.clone();
                    view.handle_key(&keystroke, window, cx);
                }))
                .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                    let lines = match event.delta {
                        gpui::ScrollDelta::Lines(delta) => delta.y as f32,
                        gpui::ScrollDelta::Pixels(delta) => f32::from(delta.y) / 18.0,
                    };
                    view.scroll_smooth(lines, cx);
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _: &MouseDownEvent, window, _cx| {
                        window.focus(&view.focus);
                    }),
                );
            area = match axis {
                SplitAxis::Columns => area.flex_row(),
                SplitAxis::Rows => area.flex_col(),
            };

            // One padded cell per visible pane with a hairline divider
            // between; maximizing hides the inactive panes without killing
            // their shells.
            let visible: Vec<usize> = if tab.maximized && tab.panes.len() > 1 {
                vec![tab.active_pane]
            } else {
                (0..tab.panes.len()).collect()
            };
            for (slot, &pane_ix) in visible.iter().enumerate() {
                let pane = &tab.panes[pane_ix];
                if slot > 0 {
                    area = area.child(match axis {
                        SplitAxis::Columns => div()
                            .w(px(1.0))
                            .h(relative(1.0))
                            .flex_shrink_0()
                            .bg(theme.hairline),
                        SplitAxis::Rows => div()
                            .h(px(1.0))
                            .w(relative(1.0))
                            .flex_shrink_0()
                            .bg(theme.hairline),
                    });
                }
                let container_bg = if self.opacity < 1.0 {
                    Rgba {
                        a: self.opacity,
                        ..theme.panel_bg
                    }
                } else {
                    theme.panel_bg
                };

                let pane_down = pane_ix;
                let pane_move = pane_ix;
                let pane_up = pane_ix;
                let pane_rc = pane_ix;

                let is_focused_pane = pane_ix == tab.active_pane && tab.panes.len() > 1;
                let mut cell = div()
                    .id(ElementId::named_usize("pane-cell", pane_ix))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .opacity(pane.pane_expansion)
                    .overflow_hidden()
                    .bg(container_bg)
                    .px(px(10.0))
                    .py(px(10.0));
                if tab.panes.len() > 1 {
                    cell = cell
                        .border_1()
                        .border_color(if is_focused_pane {
                            Rgba { a: 0.28, ..theme.accent }
                        } else {
                            rgba(0x00000000)
                        });
                }
                cell = cell
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            view.mouse_down_pane = Some(pane_down);
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if tab.active_pane != pane_down {
                                    tab.active_pane = pane_down;
                                    cx.notify();
                                }
                            }
                            window.focus(&view.focus);
                            if let Some(pt) = view.point_from_mouse(event.position, pane_down) {
                                if let Some(tab) = view.tabs.get_mut(view.active) {
                                    if let Some(p) = tab.panes.get_mut(pane_down) {
                                        p.session.borrow_mut().term.selection =
                                            Some(Selection::new(SelectionType::Simple, pt, Side::Left));
                                        cx.notify();
                                    }
                                }
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                        if view.mouse_down_pane == Some(pane_move) {
                            if let Some(pt) = view.point_from_mouse(event.position, pane_move) {
                                if let Some(tab) = view.tabs.get_mut(view.active) {
                                    if let Some(p) = tab.panes.get_mut(pane_move) {
                                        let mut sess = p.session.borrow_mut();
                                        if let Some(ref mut sel) = sess.term.selection {
                                            sel.update(pt, Side::Right);
                                            cx.notify();
                                        }
                                    }
                                }
                            }
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _event: &MouseUpEvent, window, cx| {
                            view.mouse_down_pane = None;
                            if let Some(tab) = view.tabs.get(view.active) {
                                if let Some(p) = tab.panes.get(pane_up) {
                                    let selected_text = {
                                        let sess = p.session.borrow();
                                        sess.term.selection_to_string()
                                    };
                                    if let Some(text) = selected_text {
                                        if !text.is_empty() && view.app_settings.copy_on_select {
                                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                                        }
                                    }
                                }
                            }
                            window.focus(&view.focus);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, _event: &MouseDownEvent, window, cx| {
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                tab.active_pane = pane_rc;
                            }
                            window.focus(&view.focus);
                            view.handle_terminal_right_click(window, cx);
                        }),
                    );
                if !zoomed {
                    cell = cell.rounded_b(px(inner_radius));
                }
                cell = match axis {
                    SplitAxis::Columns => cell.h(relative(1.0)),
                    SplitAxis::Rows => cell.w(relative(1.0)),
                };
                if pane.exited {
                    cell = cell
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(theme.fg)
                                .child("Process exited"),
                        )
                        .child(
                            div()
                                .mt(px(6.0))
                                .text_size(px(11.5))
                                .text_color(theme.chrome_fg)
                                .child("Press R to restart this pane"),
                        );
                } else {
                    let cell_height = (self.font_size * self.line_height).round().max(1.0);
                    let subpixel_y = if let Some((bound, pos, _)) = &self.scroll_anim {
                        if Rc::ptr_eq(bound, &pane.session) {
                            (pos - pos.floor()) * cell_height
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    let typing_recent = self.cursor_last_typing.elapsed().as_millis() < 450;
                    let cursor_alpha = if !self.cursor_blink || typing_recent {
                        1.0
                    } else {
                        0.25 + 0.75 * (self.cursor_breath_phase.sin() * 0.5 + 0.5)
                    };

                    cell = cell.child(TerminalElement {
                        session: pane.session.clone(),
                        theme: theme.clone(),
                        font: self.font.clone(),
                        font_size: self.font_size,
                        line_height: self.line_height,
                        cursor_on: self.cursor_on,
                        cursor_override: self.cursor_style,
                        cursor_pos: pane.cursor_pos,
                        cursor_alpha,
                        subpixel_y,
                        scrollbar_opacity: self.scrollbar_opacity,
                    });
                }
                area = area.child(cell.with_animation(
                    ElementId::named_usize("pane-in", pane.epoch as usize),
                    Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                    |el, delta| el.opacity(delta),
                ));
            }

            area.into_any_element()
        };

        // Swap the body through a quick fade: the settings page slides in over
        // the terminal, and the terminal fades back when settings closes (its
        // element remounts, which restarts the animation).
        let body = div()
            .flex()
            .flex_col()
            .flex_1()
            .child(body)
            .with_animation(
                ElementId::named_usize("body-anim", self.settings_open as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta),
            );

        let container_bg = if self.opacity < 1.0 {
            Rgba {
                a: self.opacity,
                ..theme.panel_bg
            }
        } else {
            theme.panel_bg
        };

        // The floating sheet holds the active terminal body (or settings page).
        // It stretches edge-to-edge beneath the top tab bar.
        let mut sheet = div()
            .id(ElementId::named_usize("terminal-sheet", 0))
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(container_bg)
            .overflow_hidden()
            .child(body);

        if !zoomed {
            sheet = sheet.rounded_b(px(inner_radius));
        }

        let mut container = div()
            .id(ElementId::named_usize("terminal-container", 0))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(container_bg)
            .overflow_hidden()
            .shadow_lg();

        if !zoomed {
            container = container
                .rounded(px(outer_radius))
                .border_1()
                .border_color(rgba(0xFFFFFF1A));
        }

        container = container
            .child(bar)
            .child(sheet);

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .font_family("Segoe UI")
            .text_color(theme.fg)
            .child(container);
        // Overlays paint last: each is a dim click-catcher plus its panel, in
        // stacking order (profile menu, palette, rename dialog). The
        // `*_closing` flags drive fade-outs; dismissal tasks unmount them.
        if self.profile_menu {
            let closing = self.profile_closing;
            root = root.child(overlay_backdrop(
                0,
                closing,
                |view, window, cx| {
                    window.focus(&view.focus);
                    view.dismiss_profile(cx);
                },
                cx,
            ));
            root = root.child(profile_menu(&theme, closing, cx));
        }
        if self.palette_open {
            root = root.child(overlay_backdrop(
                2,
                self.palette_closing,
                |view, window, cx| {
                    view.close_palette(window, cx);
                },
                cx,
            ));
            root = root.child(self.palette(&theme, cx));
        }
        if self.rename_open {
            root = root.child(overlay_backdrop(
                3,
                false,
                |view, window, cx| {
                    view.close_rename(window, cx);
                },
                cx,
            ));
            root = root.child(self.rename_dialog(&theme, cx));
        }
        if self.update_panel_open {
            let closing = self.update_panel_closing;
            root = root.child(overlay_backdrop(
                4,
                closing,
                |view, window, cx| {
                    window.focus(&view.focus);
                    view.dismiss_update_panel(cx);
                },
                cx,
            ));
            root = root.child(self.render_update_panel(&theme, closing, cx));
        }

        root
    }
}

// ----------------------------------------------------------------------------
// Motion
// ----------------------------------------------------------------------------

/// Apple macOS snappy curve: rapid acceleration, cushioned deceleration.
fn ease_apple_snappy(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(4)
}

/// Cubic ease-out for entrances: quick takeoff, soft landing.
#[allow(dead_code)]
fn ease_out_cubic(d: f32) -> f32 {
    ease_apple_snappy(d)
}

// ----------------------------------------------------------------------------
// Terminal renderer
// ----------------------------------------------------------------------------

struct TerminalElement {
    session: Rc<RefCell<Session>>,
    theme: Theme,
    font: Font,
    font_size: f32,
    line_height: f32,
    cursor_on: bool,
    cursor_override: Option<CursorShape>,
    cursor_pos: Option<(f32, f32)>,
    cursor_alpha: f32,
    subpixel_y: f32,
    scrollbar_opacity: f32,
}

/// How a cell's glyph is drawn: shaped with the terminal font, isolated from
/// its neighbours (emoji, CJK…), with the bundled Nerd symbols font, or
/// painted procedurally as pixel-exact quads (block elements, box drawing).
#[derive(Clone, Copy, PartialEq)]
enum CharClass {
    Text,
    Other,
    Icon,
    Block,
}

fn char_class(c: char) -> CharClass {
    let u = c as u32;
    match u {
        // Private use areas: Nerd Font icons, Powerline separators, …
        0xE000..=0xF8FF | 0xF0000..=0x10FFFD => CharClass::Icon,
        // Block elements and the box-drawing pieces this renderer draws
        // itself: font glyphs there leave hairline seams between cells.
        0x2500..=0x259F if is_procedural(c) => CharClass::Block,
        // Latin-ish ranges the terminal font covers: keep runs long.
        0x20..=0x24FF => CharClass::Text,
        // Emoji, CJK, exotic scripts, and non-procedural symbols: shape in
        // isolation so their glyph advances can never shift the rest of the row off the cell grid.
        _ => CharClass::Other,
    }
}

/// True for the codepoints drawn procedurally (block elements + the
/// box-drawing pieces this renderer knows); everything else falls back to
/// the font, so a glyph can never go missing.
fn is_procedural(c: char) -> bool {
    matches!(
        c as u32,
        0x2580..=0x259F
            | 0x2500
            | 0x2501
            | 0x2502
            | 0x2503
            | 0x250C
            | 0x250F
            | 0x2510
            | 0x2513
            | 0x2514
            | 0x2517
            | 0x2518
            | 0x251B
            | 0x251C
            | 0x2523
            | 0x2524
            | 0x252B
            | 0x252C
            | 0x2533
            | 0x2534
            | 0x253B
            | 0x253C
            | 0x254B
            | 0x256D..=0x2570
    )
}

/// Cell-local rects `(x0, y0, x1, y1)` + paint colour for a procedurally
/// drawn character: block elements fill exact fractions of the cell, box
/// drawing turns into centred 1px/2px strokes, shades become translucent
/// full-cell fills — all seam-free by construction.
fn procedural_cell(
    ch: char,
    cw: f32,
    lh: f32,
    fg: Rgba,
) -> Option<(Vec<(f32, f32, f32, f32)>, Rgba)> {
    let u = ch as u32;
    let mut rects: Vec<(f32, f32, f32, f32)> = Vec::new();
    let cx = (cw / 2.0).floor();
    let cy = (lh / 2.0).floor();
    let half_w = (cw / 2.0).floor();
    let half_h = (lh / 2.0).floor();
    // Light strokes are 1px, heavy 2px, both pinned to the cell centre.
    let hline = |rects: &mut Vec<(f32, f32, f32, f32)>, x0: f32, x1: f32, th: f32| {
        rects.push((x0, cy + 1.0 - th, x1, cy + 1.0));
    };
    let vline = |rects: &mut Vec<(f32, f32, f32, f32)>, y0: f32, y1: f32, th: f32| {
        rects.push((cx + 1.0 - th, y0, cx + 1.0, y1));
    };
    let alpha = match u {
        // Linear fractional blocks: upper/lower eighths, left eighths,
        // right half, upper / right one-eighths, full block.
        0x2580 => {
            rects.push((0.0, 0.0, cw, half_h));
            1.0
        }
        0x2581..=0x2588 => {
            let k = (u - 0x2580) as f32;
            let h = (lh * k / 8.0).floor();
            rects.push((0.0, lh - h, cw, lh));
            1.0
        }
        0x2589..=0x258F => {
            let k = (0x2590 - u) as f32;
            let w = (cw * k / 8.0).floor();
            rects.push((0.0, 0.0, w, lh));
            1.0
        }
        0x2590 => {
            rects.push((half_w, 0.0, cw, lh));
            1.0
        }
        // Shades: ░ ▒ ▓ = translucent full-cell fills.
        0x2591..=0x2593 => {
            rects.push((0.0, 0.0, cw, lh));
            [0.25, 0.5, 0.75][(u - 0x2591) as usize]
        }
        0x2594 => {
            let h = (lh / 8.0).floor().max(1.0);
            rects.push((0.0, 0.0, cw, h));
            1.0
        }
        0x2595 => {
            let w = (cw / 8.0).floor().max(1.0);
            rects.push((cw - w, 0.0, cw, lh));
            1.0
        }
        // Quadrants: every combination of upper/lower × left/right halves.
        0x2596..=0x259F => {
            match u {
                0x2596 => rects.push((0.0, half_h, half_w, lh)),
                0x2597 => rects.push((half_w, half_h, cw, lh)),
                0x2598 => rects.push((0.0, 0.0, half_w, half_h)),
                0x2599 => {
                    rects.push((0.0, 0.0, half_w, half_h));
                    rects.push((0.0, half_h, cw, lh));
                }
                0x259A => {
                    rects.push((0.0, 0.0, half_w, half_h));
                    rects.push((half_w, half_h, cw, lh));
                }
                0x259B => {
                    rects.push((0.0, 0.0, cw, half_h));
                    rects.push((0.0, half_h, half_w, lh));
                }
                0x259C => {
                    rects.push((0.0, 0.0, cw, half_h));
                    rects.push((half_w, half_h, cw, lh));
                }
                0x259D => rects.push((half_w, 0.0, cw, half_h)),
                0x259E => {
                    rects.push((half_w, 0.0, cw, half_h));
                    rects.push((0.0, half_h, half_w, lh));
                }
                _ => {
                    rects.push((half_w, 0.0, cw, half_h));
                    rects.push((0.0, half_h, cw, lh));
                }
            }
            1.0
        }
        // Box drawing: light (1px) and heavy (2px) lines, corners, tees and
        // crosses; rounded corners reuse the square geometry at this size.
        0x2500 => {
            hline(&mut rects, 0.0, cw, 1.0);
            1.0
        }
        0x2501 => {
            hline(&mut rects, 0.0, cw, 2.0);
            1.0
        }
        0x2502 => {
            vline(&mut rects, 0.0, lh, 1.0);
            1.0
        }
        0x2503 => {
            vline(&mut rects, 0.0, lh, 2.0);
            1.0
        }
        0x250C | 0x256D => {
            hline(&mut rects, cx, cw, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2510 | 0x256E => {
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2514 | 0x256F => {
            hline(&mut rects, cx, cw, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x2518 | 0x2570 => {
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x250F => {
            hline(&mut rects, cx - 1.0, cw, 2.0);
            vline(&mut rects, cy - 1.0, lh, 2.0);
            1.0
        }
        0x2513 => {
            hline(&mut rects, 0.0, cx + 1.0, 2.0);
            vline(&mut rects, cy - 1.0, lh, 2.0);
            1.0
        }
        0x2517 => {
            hline(&mut rects, cx - 1.0, cw, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 2.0);
            1.0
        }
        0x251B => {
            hline(&mut rects, 0.0, cx + 1.0, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 2.0);
            1.0
        }
        0x251C => {
            vline(&mut rects, 0.0, lh, 1.0);
            hline(&mut rects, cx, cw, 1.0);
            1.0
        }
        0x2524 => {
            vline(&mut rects, 0.0, lh, 1.0);
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            1.0
        }
        0x252C => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2534 => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x253C => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, 0.0, lh, 1.0);
            1.0
        }
        0x2523 => {
            vline(&mut rects, 0.0, lh, 2.0);
            hline(&mut rects, cx, cw, 1.0);
            1.0
        }
        0x252B => {
            vline(&mut rects, 0.0, lh, 2.0);
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            1.0
        }
        0x2533 => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x253B => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x254B => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, 0.0, lh, 2.0);
            1.0
        }
        _ => return None,
    };
    let color = Rgba {
        a: fg.a * alpha,
        ..fg
    };
    Some((rects, color))
}

#[derive(Clone, Copy, PartialEq)]
struct CellStyle {
    fg: Rgba,
    bg: Rgba,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    class: CharClass,
}

struct Run {
    bytes: usize,
    cells: usize,
    style: CellStyle,
}

#[derive(Default)]
struct Row {
    text: String,
    runs: Vec<Run>,
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.0;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&self.font);
        let font_size = px(self.font_size);
        let test_run = TextRun {
            len: 10,
            font: self.font.clone(),
            color: rgba(0xFFFFFFFF).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let test_line = text_system.shape_line("0123456789".into(), font_size, &[test_run], None);
        let measured_advance = f32::from(test_line.width) / 10.0;
        let cell_width = if measured_advance > 2.0 {
            measured_advance.round()
        } else {
            (text_system
                .em_advance(font_id, font_size)
                .map(f32::from)
                .unwrap_or(7.8)
                .max(1.0))
            .round()
        };
        let cell_height = (f32::from(font_size) * self.line_height).round().max(1.0);

        let cols = (f32::from(bounds.size.width) / cell_width).floor().max(1.0) as usize;
        let rows = (f32::from(bounds.size.height) / cell_height).floor().max(1.0) as usize;
        self.session.borrow_mut().resize(cols, rows);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = self.theme.clone();
        // OSC overrides (OSC 4 palette writes, OSC 10/11/12 dynamic colors)
        // live in the terminal, not the theme: pull them once per paint so a
        // program's `\e]11;…` re-themes the cells like it would in xterm —
        // base sheet included.
        let over = *self.session.borrow().term.colors();
        let (default_bg, _default_fg, default_cursor) = theme.defaults(&over);

        let text_system = window.text_system().clone();
        let font_size = px(self.font_size);
        let font_id = text_system.resolve_font(&self.font);
        let test_run = TextRun {
            len: 10,
            font: self.font.clone(),
            color: default_bg.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let test_line = text_system.shape_line("0123456789".into(), font_size, &[test_run], None);
        let measured_advance = f32::from(test_line.width) / 10.0;
        let cell_width = if measured_advance > 2.0 {
            measured_advance.round()
        } else {
            (text_system
                .em_advance(font_id, font_size)
                .map(f32::from)
                .unwrap_or(7.8)
                .max(1.0))
            .round()
        };
        let cell_height = (f32::from(font_size) * self.line_height).round().max(1.0);
        let cols = (f32::from(bounds.size.width) / cell_width).floor().max(1.0) as usize;

        // Integer pixel snapping: rounded cell sizes + whole-pixel origin
        // keep every quad boundary crisp.
        let left = f32::from(bounds.left()).round();
        let top = f32::from(bounds.top()).round();

        // Store active grid dimensions for mouse selection hit testing
        self.session.borrow_mut().grid_bounds = Some((left, top, cell_width, cell_height));

        // Snapshot the grid into paint-ready rows (no borrows outlive this block).
        let (rows, cursor_cell, cursor_shape) = {
            let session = self.session.borrow();
            let content = session.term.renderable_content();
            let offset = content.display_offset as i32;
            let cursor_shape = self.cursor_override.unwrap_or(content.cursor.shape);
            let cursor_row = content.cursor.point.line.0 + offset;
            let cursor_col = content.cursor.point.column.0;
            let cursor_cell = if cursor_row >= 0 {
                Some((cursor_row as usize, cursor_col))
            } else {
                None
            };
            let cursor_inverts = matches!(
                cursor_shape,
                CursorShape::Block | CursorShape::HollowBlock
            );

            let selection_range = session.term.selection.as_ref().and_then(|s| s.to_range(&session.term));
            let mut rows: Vec<Row> = Vec::new();
            for indexed in content.display_iter {
                let point = indexed.point;
                let cell = indexed.cell;

                let virtual_row = point.line.0 + offset;
                if virtual_row < 0 {
                    continue;
                }
                let row_index = virtual_row as usize;
                if row_index >= 4096 {
                    continue;
                }
                let col = point.column.0;

                while rows.len() <= row_index {
                    rows.push(Row::default());
                }
                let row = &mut rows[row_index];
                let flags = cell.flags;

                // Second half of a wide char: belongs to the previous run.
                if flags.contains(Flags::WIDE_CHAR_SPACER) {
                    if let Some(last) = row.runs.last_mut() {
                        last.cells += 1;
                    }
                    continue;
                }

                let is_selected = if let Some(ref range) = selection_range {
                    if range.is_block {
                        let (min_col, max_col) = if range.start.column <= range.end.column {
                            (range.start.column.0, range.end.column.0)
                        } else {
                            (range.end.column.0, range.start.column.0)
                        };
                        let (min_line, max_line) = if range.start.line <= range.end.line {
                            (range.start.line.0, range.end.line.0)
                        } else {
                            (range.end.line.0, range.start.line.0)
                        };
                        point.line.0 >= min_line && point.line.0 <= max_line && col >= min_col && col <= max_col
                    } else {
                        let (start, end) = if range.start <= range.end {
                            (range.start, range.end)
                        } else {
                            (range.end, range.start)
                        };
                        if point.line < start.line || point.line > end.line {
                            false
                        } else if point.line == start.line && point.line == end.line {
                            point.column >= start.column && point.column <= end.column
                        } else if point.line == start.line {
                            point.column >= start.column
                        } else if point.line == end.line {
                            point.column <= end.column
                        } else {
                            true
                        }
                    }
                } else {
                    false
                };

                let mut fg = theme.resolve(cell.fg, flags.contains(Flags::DIM), &over);
                let mut bg = theme.resolve(cell.bg, false, &over);
                if flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if is_selected {
                    bg = Rgba {
                        r: theme.accent.r,
                        g: theme.accent.g,
                        b: theme.accent.b,
                        a: 0.38,
                    };
                    fg = theme.chrome_fg_active;
                }
                if cursor_inverts && self.cursor_on && cursor_cell == Some((row_index, col)) {
                    std::mem::swap(&mut fg, &mut bg);
                }

                let ch = if flags.contains(Flags::HIDDEN)
                    || (cell.c as u32) < 32
                    || cell.c == '\u{7f}'
                {
                    ' '
                } else {
                    cell.c
                };
                let mut text = String::new();
                text.push(ch);
                if let Some(zerowidth) = cell.zerowidth() {
                    for &z in zerowidth {
                        text.push(z);
                    }
                }

                let is_icon = char_class(ch) == CharClass::Icon;
                let style = CellStyle {
                    fg,
                    bg,
                    bold: flags.contains(Flags::BOLD) && !is_icon,
                    italic: flags.contains(Flags::ITALIC) && !is_icon,
                    underline: flags.intersects(Flags::ALL_UNDERLINES),
                    strike: flags.contains(Flags::STRIKEOUT),
                    class: char_class(ch),
                };

                if let Some(last) = row.runs.last_mut() {
                    if last.style == style {
                        last.bytes += text.len();
                        last.cells += 1;
                        row.text.push_str(&text);
                        continue;
                    }
                }
                row.runs.push(Run {
                    bytes: text.len(),
                    cells: 1,
                    style,
                });
                row.text.push_str(&text);
            }
            (rows, cursor_cell, cursor_shape)
        };

        let total_rows = rows.len();
        for (row_index, row) in rows.iter().enumerate() {
            let y0 = top + cell_height * row_index as f32 + self.subpixel_y;
            let y1 = y0 + cell_height;
            let x = |col: usize| left + cell_width * col as f32;

            // Cell backgrounds: one quad per run of equal colour, so
            // highlight strips (prompt rows, selections) paint as a single
            // seamless rectangle instead of per-cell quads.
            let mut col = 0usize;
            let mut span_start = 0usize;
            let mut span_bg = default_bg;
            let flush = |col: usize, span_start: usize, span_bg: Rgba, window: &mut Window| {
                if span_bg != default_bg && col > span_start {
                    let mut quad = fill(
                        Bounds::from_corners(
                            point(px(x(span_start)), px(y0)),
                            point(px(x(col)), px(y1)),
                        ),
                        span_bg,
                    );
                    let is_bottom = row_index + 1 >= total_rows;
                    let is_left = span_start == 0;
                    let is_right = col >= cols;
                    if is_bottom && (is_left || is_right) {
                        quad.corner_radii = Corners {
                            top_left: px(0.0),
                            top_right: px(0.0),
                            bottom_left: if is_left { px(13.0) } else { px(0.0) },
                            bottom_right: if is_right { px(13.0) } else { px(0.0) },
                        };
                    }
                    window.paint_quad(quad);
                }
            };
            for run in &row.runs {
                if run.style.bg != span_bg {
                    flush(col, span_start, span_bg, window);
                    span_start = col;
                    span_bg = run.style.bg;
                }
                col += run.cells;
            }
            flush(col, span_start, span_bg, window);
        }

        // Smooth kinetic cursor with organic breathing alpha
        if let Some((target_row, target_col)) = cursor_cell.filter(|_| self.cursor_on) {
            let (cur_col, cur_row) = self
                .cursor_pos
                .unwrap_or((target_col as f32, target_row as f32));
            let cx0 = left + cell_width * cur_col;
            let cy0 = top + cell_height * cur_row + self.subpixel_y;
            let cx1 = cx0 + cell_width;
            let cy1 = cy0 + cell_height;
            let cursor_color = Rgba {
                r: default_cursor.r,
                g: default_cursor.g,
                b: default_cursor.b,
                a: (default_cursor.a * self.cursor_alpha).clamp(0.0, 1.0),
            };
            let quad = |rx0: f32, ry0: f32, rx1: f32, ry1: f32, window: &mut Window| {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(px(rx0), px(ry0)),
                        point(px(rx1), px(ry1)),
                    ),
                    cursor_color,
                ));
            };
            match cursor_shape {
                CursorShape::Block => quad(cx0, cy0, cx1, cy1, window),
                CursorShape::HollowBlock => {
                    let t = 1.0;
                    quad(cx0, cy0, cx1, cy0 + t, window);
                    quad(cx0, cy1 - t, cx1, cy1, window);
                    quad(cx0, cy0, cx0 + t, cy1, window);
                    quad(cx1 - t, cy0, cx1, cy1, window);
                }
                CursorShape::Beam => quad(cx0, cy0, cx0 + 2.0, cy1, window),
                CursorShape::Underline => quad(cx0, cy1 - 2.0, cx1, cy1, window),
                CursorShape::Hidden => {}
            }
        }

        for (row_index, row) in rows.iter().enumerate() {
            let y0 = top + cell_height * row_index as f32 + self.subpixel_y;
            let x = |col: usize| left + cell_width * col as f32;

            // Text: one shaped line per segment, each pinned to its starting
            // grid column, so glyph advances (emoji, CJK, Nerd icons) can
            // never drag text — or the cursor — off the cell grid. Icon runs
            // are split per glyph so every icon lands on its own cell.
            struct Segment<'a> {
                text: &'a str,
                x0: f32,
                /// Grid cells this segment covers (wide glyphs span two).
                cells: usize,
                font: Font,
                style: &'a CellStyle,
            }
            let mut segments: Vec<Segment> = Vec::new();
            let mut byte = 0usize;
            let mut col = 0usize;
            for run in &row.runs {
                let run_text = &row.text[byte..byte + run.bytes];
                byte += run.bytes;
                let mut font = match run.style.class {
                    CharClass::Icon => icon_font(),
                    _ => self.font.clone(),
                };
                if run.style.bold && run.style.class != CharClass::Icon {
                    font = font.bold();
                }
                if run.style.italic && run.style.class != CharClass::Icon {
                    font = font.italic();
                }
                if matches!(run.style.class, CharClass::Icon | CharClass::Block) {
                    let mut cell = col;
                    let mut offset = 0usize;
                    for c in run_text.chars() {
                        let end = offset + c.len_utf8();
                        segments.push(Segment {
                            text: &run_text[offset..end],
                            x0: x(cell),
                            cells: 1,
                            font: font.clone(),
                            style: &run.style,
                        });
                        offset = end;
                        cell += 1;
                    }
                } else {
                    segments.push(Segment {
                        text: run_text,
                        x0: x(col),
                        cells: run.cells,
                        font,
                        style: &run.style,
                    });
                }
                col += run.cells;
            }
            for segment in &segments {
                if segment.text.trim().is_empty() {
                    continue;
                }
                // Block elements and box drawing: painted as quads on exact
                // cell boundaries, never through a font glyph, so pixel-art
                // logos and separator runs are 100% solid.
                if segment.style.class == CharClass::Block {
                    if let Some(ch) = segment.text.chars().next() {
                        if let Some((rects, color)) =
                            procedural_cell(ch, cell_width, cell_height, segment.style.fg)
                        {
                            for (rx0, ry0, rx1, ry1) in rects {
                                window.paint_quad(fill(
                                    Bounds::from_corners(
                                        point(px(segment.x0 + rx0), px(y0 + ry0)),
                                        point(px(segment.x0 + rx1), px(y0 + ry1)),
                                    ),
                                    color,
                                ));
                            }
                        }
                    }
                    continue;
                }
                let text_run = TextRun {
                    len: segment.text.len(),
                    font: segment.font.clone(),
                    color: segment.style.fg.into(),
                    background_color: None,
                    underline: segment.style.underline.then(|| UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(segment.style.fg.into()),
                        wavy: false,
                    }),
                    strikethrough: segment.style.strike.then(|| StrikethroughStyle {
                        thickness: px(1.0),
                        color: Some(segment.style.fg.into()),
                    }),
                };
                // Strict cell-grid snapping: when a segment's glyphs map
                // one-to-one onto cells, shaping with `force_width =
                // cell_width` makes gpui pin glyph i to
                // `start + i * cell_width` (it moves any glyph sitting more
                // than 1px off its snapped spot), so long runs can never
                // accumulate drift off the grid. Segments with wide glyphs
                // (CJK, emoji) or combining marks flow naturally — their
                // start is already pinned to the exact column.
                let glyphs = segment.text.chars().count();
                let force = if glyphs == segment.cells {
                    Some(px(cell_width))
                } else {
                    None
                };
                // Nerd Font icons shape at the exact text size — PUA glyphs
                // are never downscaled, so fastfetch marks stay crisp and sit
                // on the baseline. Per-glyph splitting plus `force_width`
                // pins each icon's advance to its own cell, so oversized ink
                // may fill the cell but can never drag the grid.
                let line = text_system.shape_line(
                    segment.text.to_string().into(),
                    font_size,
                    &[text_run.clone()],
                    force,
                );
                let _ = line.paint(
                    point(px(segment.x0), px(y0)),
                    px(cell_height),
                    window,
                    cx,
                );
            }
        }

        // --------------------------------------------------------------------
        // Floating auto-hiding scrollbar indicator (Apple/macOS style)
        // --------------------------------------------------------------------
        if self.scrollbar_opacity > 0.01 {
            let (history, offset) = {
                let session = self.session.borrow();
                (session.term.history_size(), session.term.renderable_content().display_offset)
            };
            if history > 0 {
                let display_rows = rows.len().max(1);
                let total_lines = history + display_rows;
                let view_height = f32::from(bounds.size.height);
                let thumb_height = ((display_rows as f32 / total_lines as f32) * view_height)
                    .max(24.0)
                    .min(view_height - 8.0);
                let scroll_ratio = (history.saturating_sub(offset)) as f32 / history.max(1) as f32;
                let travel_space = (view_height - thumb_height - 8.0).max(0.0);
                let thumb_top = top + 4.0 + travel_space * scroll_ratio;
                let thumb_right = left + f32::from(bounds.size.width) - 4.0;
                let thumb_left = thumb_right - 4.0;

                let mut thumb_quad = fill(
                    Bounds::from_corners(
                        point(px(thumb_left), px(thumb_top)),
                        point(px(thumb_right), px(thumb_top + thumb_height)),
                    ),
                    Rgba {
                        r: theme.chrome_fg.r,
                        g: theme.chrome_fg.g,
                        b: theme.chrome_fg.b,
                        a: (theme.chrome_fg.a * 0.45 * self.scrollbar_opacity).clamp(0.0, 1.0),
                    },
                );
                thumb_quad.corner_radii = Corners {
                    top_left: px(2.0),
                    top_right: px(2.0),
                    bottom_left: px(2.0),
                    bottom_right: px(2.0),
                };
                window.paint_quad(thumb_quad);
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

// ----------------------------------------------------------------------------
// Chrome icons are stroke-style VS Code codicons from the bundled Symbols
// Nerd Font, so the whole UI reads thin like the mock. Session tiles use
// filled Font Awesome marks (terminal prompt, Ubuntu brand, cloud, desktop)
// because that is what the mock's colored tiles show. Codepoints verified
// against the Nerd Fonts glyph table and the bundled TTF's cmap. Colors are
// inherited: the parent button sets idle text color, its hover the hot one.
// ----------------------------------------------------------------------------

/// Commands offered by the palette.
#[derive(Clone, Copy)]
enum Command {
    NewTab,
    NewTabWsl,
    SplitPane,
    SplitPaneDown,
    MaximizePane,
    ClosePane,
    CloseTab,
    NextTab,
    PrevTab,
    RestartTab,
    RenameTab,
    ToggleTheme,
    OpenSettings,
    FontInc,
    FontDec,
    CheckUpdates,
}

/// Palette entries: command, vector icon path, label, shortcut hint.
const COMMANDS: [(Command, &str, &str, &str); 16] = [
    (Command::NewTab, icons::PLUS, "New tab", "Ctrl Shift N"),
    (Command::NewTabWsl, icons::TERMINAL, "New Ubuntu (WSL) tab", ""),
    (Command::SplitPane, icons::SPLIT_COLUMNS, "Split pane right", "Ctrl Shift D"),
    (Command::SplitPaneDown, icons::SPLIT_ROWS, "Split pane down", "Ctrl Shift E"),
    (
        Command::MaximizePane,
        icons::WINDOW_MAXIMIZE,
        "Toggle maximize pane",
        "Ctrl Shift M",
    ),
    (Command::ClosePane, icons::CLOSE, "Close pane", "Ctrl Shift W"),
    (Command::CloseTab, icons::CLOSE, "Close tab", "Ctrl Shift W"),
    (Command::NextTab, icons::CHEVRON_RIGHT, "Next tab", "Ctrl Tab"),
    (
        Command::PrevTab,
        icons::CHEVRON_LEFT,
        "Previous tab",
        "Ctrl Shift Tab",
    ),
    (Command::RestartTab, icons::REFRESH, "Restart pane", ""),
    (Command::RenameTab, icons::PEN, "Rename tab", ""),
    (
        Command::ToggleTheme,
        icons::SUN,
        "Switch theme",
        "Ctrl Shift T",
    ),
    (Command::OpenSettings, icons::SETTINGS, "Open settings", ""),
    (Command::FontInc, icons::SETTINGS, "Increase font size", ""),
    (Command::FontDec, icons::SETTINGS, "Decrease font size", ""),
    (Command::CheckUpdates, icons::REFRESH, "Check for updates", ""),
];

/// A flexible spacer in the titlebar that reports its painted bounds to the
/// window subclass as drag-band slot `slot` (see `rounded_corners`).
fn caption_spacer(slot: usize) -> impl IntoElement {
    div()
        .id(ElementId::named_usize("caption-spacer", slot))
        .flex_1()
        .h(px(36.0))
        .relative()
        .child(
            canvas(
                move |_bounds, _window, _cx| (),
                move |bounds, _, _window, _cx| {
                    crate::rounded_corners::set_caption_rect(
                        slot,
                        f32::from(bounds.origin.x),
                        f32::from(bounds.origin.y),
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    );
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// Profile picker behind the workspace pill, anchored under the top bar.
fn profile_menu(theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
    let entries: [(&'static str, Profile); 4] = [
        ("PowerShell", Profile::PowerShell),
        ("Command Prompt", Profile::Cmd),
        ("Ubuntu (WSL)", Profile::Wsl),
        ("Git Bash", Profile::GitBash),
    ];
    let item_hover = theme.card_hover;
    let mut menu = div()
        .absolute()
        .top(px(40.0))
        .left(px(8.0))
        .w(px(220.0))
        .flex()
        .flex_col()
        .p(px(6.0))
        .rounded(px(8.0))
        .bg(theme.elevated_bg)
        .border_1()
        .border_color(theme.elevated_border)
        .shadow_lg();
    for (index, (label, profile)) in entries.into_iter().enumerate() {
        let icon = session_icon(&profile_title(profile), profile);
        menu = menu.child(
            div()
                .id(ElementId::named_usize("profile-item", index))
                .h(px(30.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .rounded(px(6.0))
                .text_size(px(12.5))
                .text_color(theme.chrome_fg_active)
                .hover(move |style| style.bg(item_hover))
                .child(icons::icon(icon).size(px(13.0)).text_color(theme.chrome_fg))
                .child(label)
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.new_tab(profile, cx);
                    view.dismiss_profile(cx);
                    window.focus(&view.focus);
                })),
        );
    }
    menu.with_animation(
        ElementId::named_usize("profile-anim", closing as usize),
        Animation::new(Duration::from_millis(if closing {
            OVERLAY_OUT_MS
        } else {
            OVERLAY_IN_MS
        }))
        .with_easing(ease_apple_snappy),
        move |el, delta| {
            let t = if closing { 1.0 - delta } else { delta };
            el.opacity(t).top(px(40.0 - 6.0 * (1.0 - t)))
        },
    )
    .into_any_element()
}

/// Software update popover anchored under the top bar on the right side.
impl AtlasView {
    fn render_update_panel(&self, theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
        let item_hover = theme.card_hover;
        let mut panel = div()
            .id(ElementId::named_usize("update-panel", 0))
            .absolute()
            .top(px(40.0))
            .right(px(14.0))
            .w(px(330.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .rounded(px(10.0))
            .bg(theme.elevated_bg)
            .border_1()
            .border_color(theme.elevated_border)
            .shadow_lg();

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        icons::icon(icons::ATLAS_MARK)
                            .size(px(14.0))
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.chrome_fg_active)
                            .child("Software Update"),
                    ),
            )
            .child(
                div()
                    .id(ElementId::named_usize("update-close-btn", 0))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_color(theme.faint_fg)
                    .hover(move |s| s.bg(item_hover).text_color(theme.chrome_fg_active))
                    .child(icons::icon(icons::CLOSE).size(px(11.0)).text_color(theme.faint_fg))
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.dismiss_update_panel(cx);
                    })),
            );

        panel = panel.child(header);

        if let Some(ref update) = self.available_update {
            let ver_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.fg)
                        .child(format!("v{}", update.version)),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.5))
                        .rounded(px(4.0))
                        .bg(Rgba { a: 0.16, ..theme.accent })
                        .text_size(px(10.5))
                        .text_color(theme.accent)
                        .child("Available"),
                );

            let title_line = div()
                .text_size(px(12.0))
                .text_color(theme.chrome_fg)
                .child(update.title.clone());

            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(ver_row)
                .child(title_line);

            if !update.notes.trim().is_empty() {
                body = body.child(
                    div()
                        .max_h(px(100.0))
                        .overflow_hidden()
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .bg(theme.panel_bg)
                        .border_1()
                        .border_color(theme.hairline)
                        .text_size(px(11.0))
                        .text_color(theme.chrome_fg)
                        .child(update.notes.trim().to_string()),
                );
            }

            let download_target = update
                .installer_url
                .clone()
                .unwrap_or_else(|| update.html_url.clone());

            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pt(px(4.0))
                .child(
                    div()
                        .id(ElementId::named_usize("update-skip-action", 0))
                        .h(px(26.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |s| s.bg(item_hover))
                        .text_size(px(11.5))
                        .text_color(theme.faint_fg)
                        .child("Skip this version")
                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                            view.skip_current_update(cx);
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id(ElementId::named_usize("update-later-action", 0))
                                .h(px(26.0))
                                .px(px(10.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.0))
                                .hover(move |s| s.bg(item_hover))
                                .text_size(px(11.5))
                                .text_color(theme.chrome_fg)
                                .child("Later")
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.dismiss_update_panel(cx);
                                })),
                        )
                        .child(
                            div()
                                .id(ElementId::named_usize("update-install-action", 0))
                                .h(px(26.0))
                                .px(px(12.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.0))
                                .bg(theme.accent)
                                .hover(move |s| s.opacity(0.88))
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xFFFFFF))
                                .child("Install update")
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    cx.open_url(&download_target);
                                    view.dismiss_update_panel(cx);
                                })),
                        ),
                );

            panel = panel.child(body).child(actions);
        } else {
            let last_checked_text = self
                .updater_settings
                .last_check_time
                .map(updater::format_time_ago)
                .unwrap_or_else(|| "Never".to_string());

            let is_checking = self.is_checking_updates;
            let status_line = if is_checking {
                "Checking for updates…".to_string()
            } else if let Some(ref msg) = self.manual_status_message {
                format!("{msg} · Last checked: {last_checked_text}")
            } else {
                format!(
                    "Atlas Terminal is up to date (v{}) · Last checked: {last_checked_text}",
                    env!("CARGO_PKG_VERSION")
                )
            };

            let body = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.chrome_fg)
                        .child(status_line),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .pt(px(2.0))
                        .child(pill_button(
                            if is_checking {
                                "Checking…".to_string()
                            } else {
                                "Check now".to_string()
                            },
                            70,
                            false,
                            theme,
                            |view, cx| {
                                view.check_updates_manual(cx);
                            },
                            cx,
                        )),
                );

            panel = panel.child(body);
        }

        panel.with_animation(
            ElementId::named_usize("update-anim", closing as usize),
            Animation::new(Duration::from_millis(if closing {
                OVERLAY_OUT_MS
            } else {
                OVERLAY_IN_MS
            }))
            .with_easing(ease_apple_snappy),
            move |el, delta| {
                let t = if closing { 1.0 - delta } else { delta };
                el.opacity(t).top(px(40.0 - 6.0 * (1.0 - t)))
            },
        )
        .into_any_element()
    }
}

/// Hover-state ids for chrome marks.
const HOVER_MARK_NEW_TAB: u64 = 0;
const HOVER_MARK_WINDOW: u64 = 10; // + window control index

fn set_hover_mark(view: &mut AtlasView, mark: u64, hovered: bool, cx: &mut Context<AtlasView>) {
    if hovered {
        if view.hover_mark != Some(mark) {
            view.hover_mark = Some(mark);
            cx.notify();
        }
    } else if view.hover_mark == Some(mark) {
        view.hover_mark = None;
        cx.notify();
    }
}

/// Full-screen click-catcher painted under an overlay panel; fades in with
/// the panel, dims the chrome a touch and dismisses the overlay on any click.
fn overlay_backdrop(
    id: usize,
    closing: bool,
    on_click: impl Fn(&mut AtlasView, &mut Window, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    div()
        .id(ElementId::named_usize("overlay-backdrop", id))
        .absolute()
        .inset_0()
        .bg(rgba(0x00000059))
        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
            on_click(view, window, cx);
        }))
        .with_animation(
            ElementId::named_usize("backdrop-anim", id * 2 + closing as usize),
            Animation::new(Duration::from_millis(if closing {
                OVERLAY_OUT_MS
            } else {
                OVERLAY_IN_MS
            }))
            .with_easing(ease_apple_snappy),
            move |el, delta| el.opacity(if closing { 1.0 - delta } else { delta }),
        )
        .into_any_element()
}

/// Absolute path carried by a shell title (`user@host: /mnt/c/Users/me` →
/// `/mnt/c/Users/me`). "Windows PowerShell"-style titles — including the
/// legacy `…\powershell.exe` startup title — return `None`.
fn title_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let candidate = match trimmed.rsplit_once(": ") {
        Some((_, path)) => path.trim(),
        None => trimmed,
    };
    let looks_like_path = candidate.starts_with('/')
        || candidate.starts_with('~')
        || candidate.contains(":\\")
        || candidate.starts_with("\\\\");
    let is_executable = candidate.to_ascii_lowercase().ends_with(".exe");
    if looks_like_path && !candidate.contains('@') && !is_executable {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Directory label for a quiet tab pill: the basename of the shell's working
/// directory — `~` while it sits in the home folder — so the folder mark and
/// the words agree (never the raw shell name like "PowerShell").
fn dir_label(pane: &Pane) -> Option<String> {
    let raw = title_path(&pane.title)
        .or_else(|| pane.cwd.as_ref().map(|dir| dir.display().to_string()))?;
    let trimmed = raw.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() || trimmed == "~" {
        return Some("~".to_string());
    }
    if let Some(home) = home_dir() {
        let home = home.display().to_string();
        if trimmed.eq_ignore_ascii_case(home.trim_end_matches(['\\', '/'])) {
            return Some("~".to_string());
        }
    }
    trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|base| !base.is_empty())
        .map(str::to_string)
}

/// Working directory to run git against: the title path mapped back to
/// Windows (`/mnt/c/…` → `C:\…`), `None` for Linux-only paths, the PEB poll
/// otherwise.
fn git_cwd(pane: &Pane) -> Option<PathBuf> {
    if let Some(path) = title_path(&pane.title) {
        if let Some(rest) = path.strip_prefix("/mnt/") {
            let bytes = rest.as_bytes();
            if bytes.len() > 2 && bytes[1] == b'/' && bytes[0].is_ascii_alphabetic() {
                return Some(PathBuf::from(format!(
                    "{}:\\{}",
                    (bytes[0] as char).to_ascii_uppercase(),
                    &rest[2..]
                )));
            }
            return None;
        }
        if path.starts_with('/') || path.starts_with('~') {
            return None;
        }
        return Some(PathBuf::from(path));
    }
    pane.cwd.clone()
}

fn format_path_condensed(raw_path: &str) -> String {
    let normalized = raw_path.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        return "~".to_string();
    }
    let with_home = if let Some(home) = home_dir() {
        let home_norm = home.display().to_string().replace('\\', "/");
        let home_trimmed = home_norm.trim_end_matches('/');
        if trimmed.eq_ignore_ascii_case(home_trimmed) {
            "~".to_string()
        } else if let Some(stripped) = trimmed.strip_prefix(home_trimmed) {
            format!("~{}", stripped)
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    if with_home == "~" {
        return "~".to_string();
    }

    if with_home.chars().count() > 18 {
        let parts: Vec<&str> = with_home.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            format!("../{}", parts.last().unwrap())
        } else if let Some(last) = parts.last() {
            format!("../{}", last)
        } else {
            with_home
        }
    } else {
        with_home
    }
}

fn format_tab_title(pane: &Pane) -> String {
    let p_name = profile_title(pane.profile);
    let cmd = tab_name(&pane.title, "");
    let path = title_path(&pane.title)
        .or_else(|| pane.cwd.as_ref().map(|d| d.display().to_string()));

    if !cmd.is_empty() && cmd != p_name {
        return cmd;
    }

    if let Some(p) = path {
        let condensed = format_path_condensed(&p);
        if condensed == "~" {
            p_name
        } else {
            format!("{} · {}", p_name, condensed)
        }
    } else {
        p_name
    }
}

/// Process mark for a tab, keyed off the shell-reported title (OSC 0/2) and profile:
/// Matches PowerShell, Ubuntu/WSL, CMD, Git Bash, OpenCode, Claude, Cursor, Antigravity.
/// Never falls back to a folder icon.
fn session_icon(title: &str, profile: Profile) -> &'static str {
    let lower = title.to_ascii_lowercase();
    if lower.contains("claude") {
        icons::CLAUDE_MARK
    } else if lower.contains("cursor") {
        icons::CURSOR_MARK
    } else if lower.contains("antigravity") || lower.contains("agy") {
        icons::ANTIGRAVITY_MARK
    } else if lower.contains("opencode") || lower.contains("open-code") {
        icons::OPENCODE_MARK
    } else if matches!(profile, Profile::Wsl) || lower.contains("ubuntu") || lower.contains("wsl") {
        icons::UBUNTU
    } else if matches!(profile, Profile::Cmd) || lower.contains("cmd.exe") || lower == "cmd" {
        icons::CMD
    } else if matches!(profile, Profile::GitBash) || lower.contains("bash") || lower.contains("git") {
        icons::GIT
    } else if matches!(profile, Profile::PowerShell) || lower.contains("powershell") || lower.contains("pwsh") {
        icons::POWERSHELL
    } else {
        icons::TERMINAL
    }
}

/// Default title for a freshly spawned profile.
fn profile_title(profile: Profile) -> String {
    match profile {
        Profile::PowerShell => "PowerShell".to_string(),
        Profile::Cmd => "Command Prompt".to_string(),
        Profile::Wsl => "Ubuntu".to_string(),
        Profile::GitBash => "Git Bash".to_string(),
    }
}

/// Branch name for a working directory, read from `<cwd>/.git/HEAD`
/// (`ref: refs/heads/main` -> `main`). `None` outside a repository and on a
/// detached HEAD.
fn git_branch(cwd: &Path) -> Option<String> {
    let head = cwd.join(".git").join("HEAD");
    let text = std::fs::read_to_string(head).ok()?;
    Some(
        text.trim()
            .strip_prefix("ref: refs/heads/")?
            .trim()
            .to_string(),
    )
}

/// Terminal font for the cell grid: the first family from `FONT_CASCADE` the
/// text system actually resolves (Cascadia on Windows 11, Consolas elsewhere),
/// with emoji shaping fallbacks. Nerd icon cells are drawn with the dedicated
/// bundled font instead (see `icon_font`), because fallback families are
/// looked up in the system font collection, where an in-memory font does not
/// appear.
fn terminal_font(window: &Window) -> Font {
    let available = window.text_system().all_font_names();
    let family = FONT_CASCADE
        .iter()
        .find(|family| {
            available
                .iter()
                .any(|name| name.eq_ignore_ascii_case(**family))
        })
        .copied()
        .unwrap_or(FONT_FALLBACK);
    let mut font = gpui::font(family);
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "Segoe UI Emoji".to_string(),
        "Segoe UI Symbol".to_string(),
    ]));
    font
}

/// Bundled Nerd Font symbols (embedded in the exe, registered at startup).
/// Resolved from gpui's custom in-memory font collection.
fn icon_font() -> Font {
    gpui::font("Symbols Nerd Font Mono")
}

/// Numerical stepper with native SVG minus and plus icons (zero unicode icons).
fn stepper(
    id: usize,
    val_text: String,
    theme: &Theme,
    on_dec: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    on_inc: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let hover_bg = theme.card_hover;
    let fg = theme.chrome_fg_active;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .id(ElementId::named_usize("stepper-dec", id))
                .w(px(26.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .border_1()
                .border_color(theme.hairline)
                .hover(move |s| s.bg(hover_bg))
                .child(icons::icon(icons::MINUS).size(px(10.0)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_dec(view, cx);
                })),
        )
        .child(
            div()
                .min_w(px(60.0))
                .h(px(26.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(theme.soft_bg)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.chrome_fg_active)
                .child(val_text),
        )
        .child(
            div()
                .id(ElementId::named_usize("stepper-inc", id))
                .w(px(26.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .border_1()
                .border_color(theme.hairline)
                .hover(move |s| s.bg(hover_bg))
                .child(icons::icon(icons::PLUS).size(px(10.0)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_inc(view, cx);
                })),
        )
        .into_any_element()
}

/// Settings toggle pill with native SVG checkmark when active (zero unicode icons).
fn toggle_pill(
    id: usize,
    enabled: bool,
    theme: &Theme,
    on_toggle: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let active_bg = Rgba {
        a: 0.16,
        ..theme.accent
    };
    let active_border = Rgba {
        a: 0.35,
        ..theme.accent
    };
    let hover_bg = theme.card_hover;
    let mut btn = div()
        .id(ElementId::named_usize("settings-toggle", id))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(if enabled { active_border } else { theme.hairline })
        .hover(move |s| s.bg(hover_bg))
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_toggle(view, cx);
        }));

    if enabled {
        btn = btn
            .bg(active_bg)
            .text_color(theme.accent)
            .child(icons::icon(icons::CHECK).size(px(11.0)).text_color(theme.accent))
            .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Enabled"));
    } else {
        btn = btn
            .text_color(theme.chrome_fg)
            .child(div().text_size(px(12.0)).child("Disabled"));
    }
    btn.into_any_element()
}

/// One settings option pill; `active` paints the selected state.
fn pill_button(
    label: String,
    id: usize,
    active: bool,
    theme: &Theme,
    on_click: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let active_bg = Rgba {
        a: 0.16,
        ..theme.chrome_fg_active
    };
    let hover_bg = theme.card_hover;
    let fg = if active {
        theme.chrome_fg_active
    } else {
        theme.chrome_fg
    };
    let mut button = div()
        .id(ElementId::named_usize("settings-pill", id))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .text_size(px(12.0))
        .text_color(fg)
        .hover(move |style| style.bg(hover_bg))
        .child(label)
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_click(view, cx);
        }));
    if active {
        button = button.bg(active_bg).font_weight(FontWeight::MEDIUM);
    }
    button.into_any_element()
}

/// Sidebar/pane label for a tab: the shell-reported title when it reads as a
/// real name (e.g. "vite"), the profile label when it is shell-ish, a prompt
/// string (`user@host: /path`) or a bare path (the path itself is shown in
/// the subtitle and status bar instead).
fn tab_name(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let shellish = matches!(
        lower.as_str(),
        "powershell" | "pwsh" | "windows powershell" | "powershell 7" | "ubuntu" | "wsl"
    ) || lower.contains("windows powershell");
    let promptish = trimmed.contains('@') && trimmed.contains(": ");
    if trimmed.is_empty() || shellish || promptish || trimmed.contains(['\\', '/']) {
        return fallback.to_string();
    }
    let mut name: String = trimmed.chars().take(24).collect();
    if trimmed.chars().count() > 24 {
        name.push('…');
    }
    name
}
