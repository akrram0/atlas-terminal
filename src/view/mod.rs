//! AtlasView - core view coordinator, window lifecycle, and primary rendering orchestration.

pub mod actions;
pub mod input;
pub mod scroll;
pub mod sidebar;
pub mod tab_bar;
pub mod tiling;
pub mod update;
pub mod window_frame;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Point;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context,
    ElementId, FocusHandle, Font, FontFallbacks, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, Rgba, ScrollHandle,
    SharedString, Window, div, px, relative, rgb, rgba,
};

use crate::config::{load_app_settings, AppSettingsFile, SettingsTab};
use crate::icons;
use crate::motion::Motion;
use crate::session::Session;
use crate::theme::{Theme, ThemeRegistry};
use crate::ui::animation::{ease_apple_snappy, BODY_IN_MS, EASE_OUT};
use crate::ui::overlay::{
    caption_spacer, overlay_backdrop, profile_menu, set_hover_mark,
    HOVER_MARK_WINDOW,
};
use crate::updater::{self, ReleaseInfo, UpdaterSettings};
use crate::workspace::{
    git_branch, git_cwd, home_dir,
    Tab,
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UpdateDownloadState {
    Idle,
    Downloading {
        version: String,
        received_bytes: u64,
        total_bytes: u64,
        percent: f32,
    },
    Verifying {
        version: String,
    },
    ReadyToRestart {
        version: String,
        prepared: updater::PreparedUpdate,
    },
    Failed {
        version: String,
        error: String,
    },
}

pub struct AtlasView {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) next_id: u64,
    pub(crate) next_epoch: u64,
    pub(crate) focus: FocusHandle,
    pub(crate) theme: Theme,
    /// Smooth crossfade interpolation (from_theme, to_theme, progress 0.0..1.0).
    pub(crate) theme_transition: Option<(Theme, Theme, f32)>,
    /// Terminal font for the cell grid.
    pub(crate) font: Font,
    /// Active preset index into `registry` (settings pills + Ctrl+Shift+T).
    pub(crate) theme_id: usize,
    /// Active dark theme preset index into `registry`.
    pub(crate) theme_dark_id: usize,
    /// Active light theme preset index into `registry`.
    pub(crate) theme_light_id: usize,
    /// Color mode: "auto" (system sync), "dark", "light"
    pub(crate) theme_mode: String,
    pub(crate) last_theme_check: std::time::Instant,
    /// Named theme presets: neutral matte charcoal (default) and Warm Paper.
    pub(crate) registry: ThemeRegistry,
    /// Settings page visibility (replaces the terminal sheet).
    pub(crate) settings_open: bool,
    /// Settings tab mounted on top bar (stays until closed).
    pub(crate) settings_tab_open: bool,
    /// Workspace (new-tab profile) menu visibility.
    pub(crate) profile_menu: bool,
    /// Command palette (Ctrl+K / top-bar search icon).
    pub(crate) palette_open: bool,
    pub(crate) palette_category: crate::ui::palette::PaletteCategory,
    pub(crate) palette_category_menu_open: bool,
    pub(crate) palette_query: String,
    pub(crate) palette_index: usize,
    /// Smoothly gliding floating pill index in the command palette.
    pub(crate) palette_anim_index: f32,
    /// Rename-tab dialog (palette).
    pub(crate) rename_open: bool,
    pub(crate) rename_value: String,
    /// Pending "confirm before closing" state: the tab to close after the user
    /// confirms, or a whole-window close. Cleared by the dialog actions.
    pub(crate) pending_close_tab: Option<usize>,
    pub(crate) pending_close_window: bool,
    /// Terminal font size in px (settings page).
    pub(crate) font_size: f32,
    /// Cursor override; `None` follows the shell's DECSCUSR.
    pub(crate) cursor_style: Option<CursorShape>,
    /// Blink phase of the cursor (palette/rename carets share it).
    pub(crate) cursor_on: bool,
    /// Continuous breathing phase (0.0 .. 2Ï€) for organic cursor pulse.
    pub(crate) cursor_breath_phase: f32,
    /// Timestamp of last keystroke/output; keeps cursor 100% solid while typing.
    pub(crate) cursor_last_typing: std::time::Instant,
    /// Focus handles for the overlay surfaces.
    pub(crate) settings_focus: FocusHandle,
    pub(crate) palette_focus: FocusHandle,
    pub(crate) rename_focus: FocusHandle,
    pub(crate) search_focus: FocusHandle,
    /// Find in Terminal search state.
    pub(crate) search_open: bool,
    pub(crate) search_query: String,
    pub(crate) search_matches: Vec<(alacritty_terminal::index::Point, alacritty_terminal::index::Point)>,
    pub(crate) search_index: usize,
    /// Fade-out flags: an overlay stays mounted while it animates away and a
    /// timer task unmounts it (cleared whenever the open flag drops).
    pub(crate) palette_closing: bool,
    pub(crate) profile_closing: bool,
    /// Smooth scrolling: the pane being eased plus (position, target) display
    /// offsets in lines. Bound to a session so tab switches stay safe.
    pub(crate) scroll_anim: Option<(Rc<RefCell<Session>>, f32, f32)>,
    /// Floating auto-hiding scrollbar opacity and activity timer.
    pub(crate) scrollbar_opacity: f32,
    pub(crate) last_scroll_time: std::time::Instant,
    /// Dynamic Island HUD opacity (0.0 to 1.0) and hover state
    pub(crate) island_hud_opacity: f32,
    pub(crate) island_hud_hovered: bool,
    pub(crate) last_island_hud_time: std::time::Instant,
    pub(crate) last_background_appearance: Option<gpui::WindowBackgroundAppearance>,
    /// Tab pill currently under the pointer: its hover-only close mark.
    pub(crate) hover_tab: Option<usize>,
    /// Tab being dragged with the mouse to reorder slots.
    pub(crate) tab_drag_origin: Option<usize>,
    /// Mouse-down candidate for tab drag: (tab_index, mouse_x, mouse_y).
    pub(crate) tab_drag_candidate: Option<(usize, f32, f32)>,
    /// Whether mouse has exceeded the drag threshold distance (8px).
    pub(crate) tab_is_dragging: bool,
    /// Geometric chrome mark under the pointer (new-tab plus, pane actions).
    /// Painted quads can't inherit hover text colour, so the view tracks it.
    pub(crate) hover_mark: Option<u64>,
    /// Updater settings and runtime state.
    pub(crate) updater_settings: UpdaterSettings,
    pub(crate) is_checking_updates: bool,
    pub(crate) manual_status_message: Option<String>,
    pub(crate) last_manual_check: Option<std::time::Instant>,
    pub(crate) current_backoff_secs: u64,
    pub(crate) rate_limit_reset: Option<u64>,
    pub(crate) available_update: Option<ReleaseInfo>,
    pub(crate) update_panel_open: bool,
    pub(crate) update_panel_closing: bool,
    pub(crate) update_toast: Option<(String, bool, std::time::Instant)>,
    pub(crate) update_download_state: UpdateDownloadState,
    pub(crate) update_cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub(crate) app_settings: AppSettingsFile,
    pub(crate) settings_tab: SettingsTab,
    pub(crate) settings_search_query: String,
    pub(crate) settings_search_active: bool,
    pub(crate) recording_shortcut: Option<String>,
    pub(crate) font_family: String,
    pub(crate) line_height: f32,
    pub(crate) corner_radius: f32,
    pub(crate) opacity: f32,
    pub(crate) cursor_blink: bool,
    pub(crate) window_was_active: bool,
    pub(crate) mouse_down_pane: Option<usize>,
    pub(crate) mouse_down_point: Option<Point>,
    pub(crate) mouse_down_sel_type: SelectionType,
    pub(crate) last_mouse_btn: Option<u8>,
    pub(crate) last_mouse_cell: Option<(usize, usize)>,
    pub(crate) settings_scroll_handle: ScrollHandle,
    pub(crate) palette_scroll_handle: ScrollHandle,
    pub(crate) settings_scroll_pos: f32,
    pub(crate) settings_scroll_target: f32,
    pub(crate) settings_nav_scroll: f32,
    pub(crate) hovered_link: Option<(usize, crate::url::DetectedUrl)>,
    pub(crate) sidebar_open: bool,
    pub(crate) sidebar_expansion: f32,
    pub(crate) sidebar_mode: sidebar::SidebarMode,
    pub(crate) sidebar_width: f32,
    pub(crate) sidebar_current_width: f32,
    pub(crate) sidebar_is_resizing: bool,
    pub(crate) active_split_resize: Option<(u64, crate::tiling::SplitAxis, f32, f32)>,
    pub(crate) last_settings_mtime: Option<std::time::SystemTime>,
    pub(crate) user_profile: crate::discovery::UserProfile,
}


pub(crate) fn resolve_terminal_font(family: &str) -> gpui::Font {
    let mut font = gpui::font(SharedString::from(family.to_string()));
    let mut fallbacks = vec![
        "Cascadia Code".to_string(),
        "Consolas".to_string(),
        "JetBrainsMono NF".to_string(),
        "Courier New".to_string(),
        "Segoe UI Emoji".to_string(),
        "Segoe UI Symbol".to_string(),
    ];
    fallbacks.retain(|f| !f.eq_ignore_ascii_case(family));
    font.fallbacks = Some(FontFallbacks::from_fonts(fallbacks));
    font
}

/// Normalize opacity from settings.json: supports percentage notation (e.g. 66 or 67 -> 0.67)
/// as well as standard 0.0..1.0 float values (e.g. 0.0, 0.67, 1.0).
pub(crate) fn normalize_opacity(saved: f32) -> f32 {
    if saved > 1.0 {
        (saved / 100.0).clamp(0.0, 1.0)
    } else {
        saved.clamp(0.0, 1.0)
    }
}

impl AtlasView {
    pub fn new(window: &mut Window, initial_dir: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let registry = ThemeRegistry::standard();
        let app_settings = load_app_settings();
        let mut user_profile = crate::discovery::UserProfile::load_or_init();
        if !app_settings.default_shell.is_empty() && app_settings.default_shell != "PowerShell" {
            user_profile.default_shell = app_settings.default_shell.clone();
        }
        let theme_mode = app_settings.theme_mode.clone();
        let theme_dark_id = registry
            .find_by_name_or_id(&app_settings.theme_dark)
            .unwrap_or_else(|| registry.find_by_name_or_id(&app_settings.theme).unwrap_or(0));
        let theme_light_id = registry
            .find_by_name_or_id(&app_settings.theme_light)
            .unwrap_or_else(|| registry.find_by_name_or_id("solarized").unwrap_or(3));

        let is_light = match theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        let theme_id = if is_light { theme_light_id } else { theme_dark_id };
        let (initial_theme, is_dark) = registry.resolve(theme_id, &theme_mode);
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
        let opacity = if app_settings.blur_enabled {
            normalize_opacity(app_settings.opacity)
        } else {
            1.0
        };
        let cursor_blink = app_settings.cursor_blink;
        crate::log_line(&format!(
            "settings loaded: theme={} mode={} font={} size={:.1} line={:.2} opacity={:.2}",
            app_settings.theme,
            app_settings.theme_mode,
            font_family,
            font_size,
            line_height,
            opacity
        ));

        let font = resolve_terminal_font(&font_family);

        let mut view = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            next_epoch: 1,
            focus,
            theme: initial_theme,
            theme_transition: None,
            font,
            theme_id,
            theme_dark_id,
            theme_light_id,
            theme_mode,
            last_theme_check: std::time::Instant::now(),
            registry,
            settings_open: false,
            settings_tab_open: false,
            profile_menu: false,
            palette_open: false,
            palette_category: crate::ui::palette::PaletteCategory::All,
            palette_category_menu_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_anim_index: 0.0,
            rename_open: false,
            rename_value: String::new(),
            pending_close_tab: None,
            pending_close_window: false,
            font_size,
            cursor_style,
            cursor_on: true,
            cursor_breath_phase: 0.0,
            cursor_last_typing: std::time::Instant::now(),
            settings_focus: cx.focus_handle(),
            palette_focus: cx.focus_handle(),
            rename_focus: cx.focus_handle(),
            search_focus: cx.focus_handle(),
            search_open: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_index: 0,
            palette_closing: false,
            profile_closing: false,
            scroll_anim: None,
            scrollbar_opacity: 0.0,
            last_scroll_time: std::time::Instant::now(),
            island_hud_opacity: 0.0,
            island_hud_hovered: false,
            last_island_hud_time: std::time::Instant::now().checked_sub(std::time::Duration::from_secs(10)).unwrap_or_else(std::time::Instant::now),
            last_background_appearance: None,
            hover_tab: None,
            tab_drag_origin: None,
            tab_drag_candidate: None,
            tab_is_dragging: false,
            hover_mark: None,
            updater_settings: UpdaterSettings {
                auto_check_updates: app_settings.auto_check_updates,
                update_check_interval_mins: app_settings.update_check_interval_mins,
                channel: match app_settings.update_channel.as_str() {
                    "preview" => updater::UpdateChannel::Preview,
                    _ => updater::UpdateChannel::Stable,
                },
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
            update_toast: None,
            update_download_state: UpdateDownloadState::Idle,
            update_cancel_token: None,
            app_settings: app_settings.clone(),
            settings_tab: SettingsTab::from_str_name(&app_settings.last_settings_tab),
            settings_search_query: String::new(),
            settings_search_active: false,
            recording_shortcut: None,
            font_family,
            line_height,
            corner_radius,
            opacity,
            cursor_blink,
            window_was_active: true,
            mouse_down_pane: None,
            mouse_down_point: None,
            mouse_down_sel_type: SelectionType::Simple,
            last_mouse_btn: None,
            last_mouse_cell: None,
            settings_scroll_handle: ScrollHandle::new(),
            palette_scroll_handle: ScrollHandle::new(),
            settings_scroll_pos: 0.0,
            settings_scroll_target: 0.0,
            settings_nav_scroll: 0.0,
            hovered_link: None,
            sidebar_open: true,
            sidebar_expansion: 1.0,
            sidebar_mode: sidebar::SidebarMode::Expanded,
            sidebar_width: 210.0,
            sidebar_current_width: 210.0,
            sidebar_is_resizing: false,
            active_split_resize: None,
            last_settings_mtime: std::fs::metadata(crate::user_data_dir().join("settings.json")).and_then(|m| m.modified()).ok(),
            user_profile,
        };
        crate::rounded_corners::set_dark_mode(is_dark);
        crate::rounded_corners::set_opacity(opacity);

        // Stale-While-Revalidate: scan environment in background without blocking UI startup (<250ms)
        cx.spawn(async move |this, cx| {
            let (tx, rx) = smol::channel::bounded(1);
            crate::discovery::scan_background(move |env| {
                let _ = tx.try_send(env);
            });
            if let Ok(fresh_env) = rx.recv().await {
                let _ = this.update(cx, |view, cx| {
                    if view.user_profile.reconcile(fresh_env) {
                        view.user_profile.save();
                        crate::log_line("user profile & environment discovery reconciled from background scan");
                        cx.notify();
                    }
                });
            }
        }).detach();
        let default_shell_prof = view.get_default_profile();
        let mut restored = false;
        if initial_dir.is_none()
            && view
                .app_settings
                .startup_mode
                .eq_ignore_ascii_case("restore_session")
        {
            restored = view.restore_session(cx);
        }
        if !restored {
            let startup_cwd = initial_dir.or_else(|| {
                if view
                    .app_settings
                    .start_directory
                    .eq_ignore_ascii_case("current")
                {
                    std::env::current_dir().ok().or_else(home_dir)
                } else {
                    home_dir()
                }
            });
            view.new_tab_with_cwd(default_shell_prof, startup_cwd, cx);
        }
        window.focus(&view.focus);

        // Window close is interruptible while shells are still alive and the
        // user asked for confirmation. Confirming calls `remove_window()`,
        // which bypasses this callback; cancelling just clears the dialog.
        let view_handle = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_window, cx| {
            let mut allow = true;
            if let Some(handle) = view_handle.upgrade() {
                let _ = handle.update(cx, |view, cx| {
                    if view.pending_close_window {
                        allow = false;
                    } else if view.app_settings.confirm_close_process && view.has_live_processes() {
                        view.pending_close_window = true;
                        allow = false;
                        cx.notify();
                    } else {
                        view.save_session();
                    }
                });
            }
            allow
        });

        // Motion & breathing ticker: drives continuous kinetic cursor gliding,
        // organic sinusoidal breathing, theme crossfading, palette gliding,
        // auto-hiding scrollbar transitions, settings kinetic scroll, and tab/pane collapses.
        let motion_ticker = cx.spawn(async move |this, cx| {
            let mut last_tick = std::time::Instant::now();
            let mut next_sleep_ms = 4u64;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(next_sleep_ms))
                    .await;
                let now = std::time::Instant::now();
                let dt = (now - last_tick).as_secs_f32().min(0.05);
                last_tick = now;

                let alive = this.update(cx, |view, cx| {
                    let mut needs_paint = false;
                    let mut active_motion = false;

                    // 1. Organic cursor breathing (soft sinusoidal alpha when idle)
                    // Pauses when settings or modal overlays are open to prevent wasted redraws
                    let typing_elapsed = view.cursor_last_typing.elapsed().as_millis();
                    let should_breathe = !view.settings_open
                        && !view.palette_open
                        && !view.profile_menu
                        && !view.rename_open
                        && !view.update_panel_open;
                    if should_breathe && typing_elapsed > 450 {
                        view.cursor_breath_phase =
                            (view.cursor_breath_phase + dt * 4.5) % (std::f32::consts::PI * 2.0);
                        needs_paint = true;
                    } else if view.cursor_breath_phase != 0.0 {
                        view.cursor_breath_phase = 0.0;
                        needs_paint = true;
                    }

                    // 2. Theme crossfade interpolation (zero-flash smooth transition)
                    if let Some((ref from, ref to, ref mut t)) = view.theme_transition {
                        active_motion = true;
                        *t = (*t + dt * 7.0).min(1.0);
                        view.theme = Theme::interpolate(from, to, EASE_OUT.eval(*t));
                        needs_paint = true;
                        if *t >= 1.0 {
                            view.theme = to.clone();
                            view.theme_transition = None;
                        }
                    }

                    // Auto dark/light system sync check (checks Windows registry every 1.5 seconds)
                    if view.theme_mode == "auto" && view.last_theme_check.elapsed().as_millis() > 1500 {
                        view.last_theme_check = std::time::Instant::now();
                        let sys_light = crate::theme::is_system_light_mode();
                        let target_id = if sys_light { view.theme_light_id } else { view.theme_dark_id };
                        let (target, is_dark) = view.registry.resolve(target_id, &view.theme_mode);
                        if target_id != view.theme_id
                            || (view.theme.bg.r - target.bg.r).abs() > 0.05
                            || (view.theme.bg.g - target.bg.g).abs() > 0.05
                            || (view.theme.bg.b - target.bg.b).abs() > 0.05
                        {
                            view.theme_id = target_id;
                            view.theme_transition = Some((view.theme.clone(), target, 0.0));
                            crate::rounded_corners::set_dark_mode(is_dark);
                            view.sync_and_save_settings();
                            needs_paint = true;
                        }
                    }

                    // 3. Command palette floating sliding selection pill
                    if view.palette_open {
                        let target = view.palette_index as f32;
                        let diff = target - view.palette_anim_index;
                        if diff.abs() > 0.005 {
                            active_motion = true;
                            let factor = (1.0 - (-28.0 * dt).exp()).clamp(0.0, 1.0);
                            view.palette_anim_index += diff * factor;
                            needs_paint = true;
                        } else {
                            view.palette_anim_index = target;
                        }
                    }

                    // 4. Floating auto-hiding scrollbar fade
                    let scroll_idle = view.last_scroll_time.elapsed().as_millis();
                    if scroll_idle < 650 {
                        active_motion = true;
                        if view.scrollbar_opacity < 1.0 {
                            view.scrollbar_opacity = (view.scrollbar_opacity + dt * 10.0).min(1.0);
                            needs_paint = true;
                        }
                    } else if view.scrollbar_opacity > 0.0 {
                        active_motion = true;
                        view.scrollbar_opacity = (view.scrollbar_opacity - dt * 3.5).max(0.0);
                        needs_paint = true;
                    }

                    // 4b. Dynamic Island HUD smooth fade: stays awake when hovered while already visible
                    let island_idle = view.last_island_hud_time.elapsed().as_millis();
                    let target_island = if (view.island_hud_hovered && view.island_hud_opacity > 0.5) || island_idle < 2500 {
                        1.0
                    } else {
                        0.0
                    };
                    let diff_island = target_island - view.island_hud_opacity;
                    if diff_island.abs() > 0.005 {
                        active_motion = true;
                        let factor = (1.0 - (-12.0 * dt).exp()).clamp(0.0, 1.0);
                        view.island_hud_opacity += diff_island * factor;
                        needs_paint = true;
                    } else if view.island_hud_opacity != target_island {
                        view.island_hud_opacity = target_island;
                        needs_paint = true;
                    }


                    // 4d. Tab close button smooth hover fade (constant tab width)
                    for (t_ix, tab) in view.tabs.iter_mut().enumerate() {
                        let is_hovered = view.hover_tab == Some(t_ix);
                        let target = if is_hovered {
                            1.0
                        } else if t_ix == view.active {
                            0.50
                        } else {
                            0.0
                        };
                        let diff = target - tab.close_hover_progress;
                        if diff.abs() > 0.005 {
                            active_motion = true;
                            let factor = (1.0 - (-18.0 * dt).exp()).clamp(0.0, 1.0);
                            tab.close_hover_progress += diff * factor;
                            needs_paint = true;
                        } else if tab.close_hover_progress != target {
                            tab.close_hover_progress = target;
                            needs_paint = true;
                        }
                    }

                    // 5. Smooth tab collapse on close
                    let mut tabs_to_remove = Vec::new();
                    for (i, tab) in view.tabs.iter_mut().enumerate() {
                        if tab.closing {
                            active_motion = true;
                            tab.close_progress = (tab.close_progress + dt * 8.0).min(1.0);
                            needs_paint = true;
                            if tab.close_progress >= 1.0 {
                                tabs_to_remove.push(i);
                            }
                        }
                    }
                    for &i in tabs_to_remove.iter().rev() {
                        let removed = view.tabs.remove(i);
                        for pane in removed.panes {
                            pane.session.borrow_mut().kill();
                        }
                        if i < view.active {
                            view.active = view.active.saturating_sub(1);
                        }
                        if view.active >= view.tabs.len() && !view.tabs.is_empty() {
                            view.active = view.tabs.len() - 1;
                        }
                    }
                    if !tabs_to_remove.is_empty() {
                        crate::session::trim_working_set();
                    }

                    // 6. Spatial cursor gliding, split pane expansion and pane collapse
                    for tab in &mut view.tabs {
                        let mut panes_to_remove = Vec::new();
                        for (p_ix, pane) in tab.panes.iter_mut().enumerate() {
                            if let Some((cur_col, cur_row)) = pane.cursor_pos {
                                let (tgt_col, tgt_row) = pane.cursor_target;
                                let d_col = tgt_col - cur_col;
                                let d_row = tgt_row - cur_row;
                                // When moving across more than 2 rows (page jumps, scrolling, large movements in Vim/Neovim)
                                // or across a massive column leap (> 35 cols), snap immediately to prevent dizzying flying/rubberbanding.
                                if d_row.abs() > 2.0 || d_col.abs() > 35.0 {
                                    pane.cursor_pos = Some((tgt_col, tgt_row));
                                    needs_paint = true;
                                } else if d_col.abs() > 0.01 || d_row.abs() > 0.01 {
                                    active_motion = true;
                                    // Snappy Apple spring deceleration (~45ms)
                                    let factor = (1.0 - (-36.0 * dt).exp()).clamp(0.0, 1.0);
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
                                active_motion = true;
                                pane.pane_expansion = (pane.pane_expansion - dt * 10.0).max(0.0);
                                needs_paint = true;
                                if pane.pane_expansion <= 0.0 {
                                    panes_to_remove.push(p_ix);
                                }
                            } else if pane.pane_expansion < 1.0 {
                                active_motion = true;
                                pane.pane_expansion = (pane.pane_expansion + dt * 10.0).min(1.0);
                                needs_paint = true;
                            }

                            let target_focus = if p_ix == tab.active_pane { 1.0 } else { 0.0 };
                            if (pane.focus_progress - target_focus).abs() > 0.01 {
                                active_motion = true;
                                pane.focus_progress = (pane.focus_progress + (target_focus - pane.focus_progress) * (dt * 18.0).min(1.0)).clamp(0.0, 1.0);
                                needs_paint = true;
                            } else if pane.focus_progress != target_focus {
                                pane.focus_progress = target_focus;
                            }
                        }
                        for &p_ix in panes_to_remove.iter().rev() {
                            let removed = tab.panes.remove(p_ix);
                            removed.session.borrow_mut().kill();
                            tab.tile_tree.remove_pane(p_ix);
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
                        if !panes_to_remove.is_empty() {
                            crate::session::trim_working_set();
                        }
                    }

                    // 7. Terminal smooth kinetic scrolling (unified into ticker)
                    if let Some((session, pos, target)) = view.scroll_anim.clone() {
                        view.last_scroll_time = std::time::Instant::now();
                        active_motion = true;
                        let diff = target - pos;
                        let factor = (1.0 - (-18.0 * dt).exp()).clamp(0.0, 1.0);
                        let next = if diff.abs() < 0.02 {
                            target
                        } else {
                            pos + diff * factor
                        };
                        let settled = next == target;
                        view.scroll_anim = if settled {
                            None
                        } else {
                            Some((session.clone(), next, target))
                        };
                        let mut guard = session.borrow_mut();
                        let current = guard.term.renderable_content().display_offset as i32;
                        let target_int = next.ceil() as i32;
                        let delta = target_int - current;
                        if delta != 0 {
                            guard.term.scroll_display(Scroll::Delta(delta));
                        }
                        needs_paint = true;
                    }

                    // 8. Vertical sidebar kinetic expansion/collapse (3 modes: Expanded, Compact, Hidden)
                    let target_width = if view.sidebar_is_resizing {
                        view.sidebar_width
                    } else {
                        match view.sidebar_mode {
                            sidebar::SidebarMode::Expanded => view.sidebar_width,
                            sidebar::SidebarMode::Compact => 52.0,
                            sidebar::SidebarMode::Hidden => 0.0,
                        }
                    };
                    let sidebar_diff = target_width - view.sidebar_current_width;
                    if sidebar_diff.abs() > 0.5 {
                        active_motion = true;
                        let factor = (1.0 - (-22.0 * dt).exp()).clamp(0.0, 1.0);
                        view.sidebar_current_width += sidebar_diff * factor;
                        view.sidebar_expansion = (view.sidebar_current_width / view.sidebar_width.max(1.0)).clamp(0.0, 1.0);
                        view.sidebar_open = view.sidebar_mode != sidebar::SidebarMode::Hidden;
                        needs_paint = true;
                    } else if view.sidebar_current_width != target_width {
                        view.sidebar_current_width = target_width;
                        view.sidebar_expansion = (view.sidebar_current_width / view.sidebar_width.max(1.0)).clamp(0.0, 1.0);
                        view.sidebar_open = view.sidebar_mode != sidebar::SidebarMode::Hidden;
                        needs_paint = true;
                    }

                    if needs_paint {
                        cx.notify();
                    }

                    (active_motion, should_breathe)
                });
                match alive {
                    Ok((active_motion, should_breathe)) => {
                        next_sleep_ms = if active_motion {
                            4
                        } else if should_breathe {
                            16
                        } else {
                            33
                        };
                    }
                    Err(_) => break,
                }
            }
        });
        motion_ticker.detach();

        // Background poll for working directory (PEB) and active foreground processes (Toolhelp32).
        // Polls every 400 ms so tab titles and brand icons update instantly when commands run or exit.
        let cwd_poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(400))
                    .await;
                if this
                    .update(cx, |view, cx| {
                        let mut pids = Vec::new();
                        for tab in &view.tabs {
                            for pane in &tab.panes {
                                if !pane.exited {
                                    if let Some(pid) = pane.session.borrow().pid {
                                        pids.push(pid);
                                    }
                                }
                            }
                        }

                        let fg_map = if !pids.is_empty() {
                            crate::process::get_foreground_processes(&pids)
                        } else {
                            std::collections::HashMap::new()
                        };

                        let mut changed = false;
                        for tab in &mut view.tabs {
                            for pane in &mut tab.panes {
                                if pane.exited {
                                    continue;
                                }

                                let pid_opt = pane.session.borrow().pid;
                                if let Some(pid) = pid_opt {
                                    let fg = fg_map.get(&pid);
                                    let new_cmd = fg.map(|f| f.display_name.clone());
                                    let new_icon = fg.map(|f| f.icon);

                                    if pane.running_cmd != new_cmd || pane.running_icon != new_icon {
                                        pane.running_cmd = new_cmd;
                                        pane.running_icon = new_icon;
                                        changed = true;
                                    }
                                }

                                let Some(cwd) = pid_opt.and_then(crate::cwd::process_cwd) else {
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

    pub(crate) fn active_font_size(&self) -> f32 {
        self.tab_font_size(self.active)
    }

    pub(crate) fn tab_font_size(&self, tab_ix: usize) -> f32 {
        self.tabs.get(tab_ix).and_then(|t| t.font_size).unwrap_or(self.font_size)
    }

    pub(crate) fn trigger_island_hud(&mut self, cx: &mut Context<Self>) {
        if self.app_settings.tab_bar_position != "borderless" {
            return;
        }
        self.last_island_hud_time = std::time::Instant::now();
        cx.notify();
    }

    pub(crate) fn check_settings_reload(&mut self, cx: &mut Context<Self>) {
        if self.settings_open {
            return;
        }
        let path = crate::user_data_dir().join("settings.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if crate::config::is_saved_settings_hash(&content) {
                return;
            }
            if let Ok(m) = std::fs::metadata(&path) {
                if let Ok(mtime) = m.modified() {
                    if self.last_settings_mtime.map_or(true, |last| mtime > last) {
                        self.last_settings_mtime = Some(mtime);
                        crate::config::update_saved_settings_hash(&content);
                        self.reload_settings_from_disk(cx);
                    }
                }
            }
        }
    }
}

impl Render for AtlasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = crate::rounded_corners::is_active();
        if is_active && !self.window_was_active {
            self.maybe_auto_check(true, cx);
            self.check_settings_reload(cx);
        } else if !is_active && self.window_was_active {
            crate::session::trim_working_set();
        }
        self.window_was_active = is_active;

        let is_light = self.theme.is_light();
        let should_blur = self.app_settings.blur_enabled && self.opacity < 0.999 && !is_light;
        let target_appearance = if should_blur {
            if self.opacity <= 0.005 {
                gpui::WindowBackgroundAppearance::Transparent
            } else {
                gpui::WindowBackgroundAppearance::Blurred
            }
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        };
        if self.last_background_appearance != Some(target_appearance) {
            self.last_background_appearance = Some(target_appearance);
            window.set_background_appearance(target_appearance);
        }

        // Keep the native backdrop in sync with the opacity slider. GPUI stays
        // on a transparent swapchain; `rounded_corners` applies the native
        // acrylic accent (GPUI's `Blurred` background, with the DWM system
        // backdrop as a fallback) and is a no-op whenever the slider has not
        // moved.
        crate::rounded_corners::set_opacity(if should_blur { self.opacity } else { 1.0 });
        let is_translucent = should_blur;

        let is_borderless = self.app_settings.tab_bar_position == "borderless";
        let is_island_visible = is_borderless && self.island_hud_opacity > 0.005;
        crate::rounded_corners::set_borderless_mode(is_borderless, is_island_visible);

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

        // Track and persist resized window dimensions when floating (not zoomed/maximized)
        let cur_bounds = window.bounds();
        let cur_w = f32::from(cur_bounds.size.width);
        let cur_h = f32::from(cur_bounds.size.height);
        if !zoomed && cur_w >= 480.0 && cur_h >= 280.0 {
            if (self.app_settings.window_width - cur_w).abs() > 25.0
                || (self.app_settings.window_height - cur_h).abs() > 25.0
            {
                self.app_settings.window_width = cur_w.round();
                self.app_settings.window_height = cur_h.round();
                crate::config::save_app_settings(&self.app_settings);
            }
        }

        // Detect dominant background of the active terminal session via robust histogram sampling
        let active_bg = if let Some(tab) = self.tabs.get(self.active) {
            if let Some(pane) = tab.panes.get(tab.active_pane) {
                Self::detect_pane_bg(pane, &theme)
            } else {
                theme.bg
            }
        } else {
            theme.bg
        };

        // Window chrome and top tab bar use the cohesive application theme
        let tb = theme.clone();

        let term_base = if self.settings_open { theme.elevated_bg } else { active_bg };

        // Inactive chrome ink (tab titles, bar icons). In translucent mode the
        // muted token sits on a semi-transparent bar, so over a bright blurred
        // wallpaper it washes out; lift it 40% of the way toward the active
        // tone to hold roughly 3:1. Opaque mode keeps the theme token as-is.
        let chrome_ink = if is_translucent {
            crate::theme::lerp_color(tb.chrome_fg, tb.chrome_fg_active, 0.40)
        } else {
            tb.chrome_fg
        };

        let _is_tui_active = if let Some(tab) = self.tabs.get(self.active) {
            if let Some(pane) = tab.panes.get(tab.active_pane) {
                Self::is_pane_tui(pane, &theme)
            } else {
                false
            }
        } else {
            false
        };

        let is_sidebar_mode = self.app_settings.tab_bar_position == "sidebar";
        let is_sidebar_visible = is_sidebar_mode
            && self.sidebar_mode != sidebar::SidebarMode::Hidden
            && self.sidebar_current_width > 1.0;

        let (chrome_bg, sheet_bg, container_bg) = if is_translucent {
            let op = self.opacity.clamp(0.0, 1.0);
            if op <= 0.005 {
                let chrome = Rgba {
                    a: 0.0,
                    ..tb.panel_bg
                };
                let sheet = Rgba {
                    a: 0.0,
                    ..term_base
                };
                (chrome, sheet, rgba(0x00000000))
            } else {
                let sheet = Rgba {
                    a: op,
                    ..term_base
                };
                let chrome = Rgba {
                    a: (op * 0.95).clamp(0.0, 1.0),
                    ..tb.panel_bg
                };
                (chrome, sheet, rgba(0x00000000))
            }
        } else {
            (tb.panel_bg, term_base, tb.panel_bg)
        };

        let bar = self.render_top_bar(&theme, is_sidebar_visible, chrome_bg, zoomed, cx);

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
            let area = self.render_tiled_workspace(tab, index, &theme, is_translucent, active_bg, cx);
            let mut container = div().flex().flex_col().flex_1().relative().child(area);
            if tab.maximized && tab.panes.len() > 1 {
                let tb_z = theme.clone();
                let unzoom_banner = div()
                    .id("unzoom-banner")
                    .absolute()
                    .top(px(10.0))
                    .right(px(16.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(5.0))
                    .rounded(px(16.0))
                    .bg(Rgba { a: 0.88, ..tb_z.panel_bg })
                    .border_1()
                    .border_color(tb_z.hairline)
                    .shadow_md()
                    .cursor_pointer()
                    .hover(move |s| s.bg(tb_z.card_hover))
                    .child(
                        icons::icon(icons::WINDOW_RESTORE)
                            .size(px(12.0))
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.fg)
                            .child(format!("Pane {} of {} (Zoomed) • Alt+F to restore", tab.active_pane + 1, tab.panes.len())),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.toggle_maximize(cx);
                        window.focus(&view.focus);
                    }));
                container = container.child(unzoom_banner);
            }
            container.into_any_element()
        };

        // iPhone-style expand launch: when switching or creating tabs, expand outward
        // from the center of tab borders with smooth size-up and corner morphing,
        // rather than fading or rising from the bottom.
        let tab_body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .child(body)
            .ios_app_launch(ElementId::named_usize("tab-launch-expand", self.active));

        let body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .child(tab_body)
            .with_animation(
                ElementId::named_usize("body-anim", self.settings_open as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| if delta < 0.999 { el.opacity(delta) } else { el },
            );

        let is_top_tab_mode = self.app_settings.tab_bar_position == "top";
        let is_sidebar_mode = self.app_settings.tab_bar_position == "sidebar";
        let is_sidebar_visible = is_sidebar_mode
            && self.sidebar_mode != sidebar::SidebarMode::Hidden
            && self.sidebar_current_width > 1.0;

        // In Borderless or Sidebar mode, provide clean breathing room at the top so the terminal content
        // is never jammed against the top edge or cut in half when maximized, and floating controls
        // have dedicated clean clearance.
        let sheet_pt = if !is_top_tab_mode {
            if zoomed { px(32.0) } else { px(28.0) }
        } else {
            px(0.0)
        };

        let sheet = div()
            .id(ElementId::named_usize("terminal-sheet", 0))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .pt(sheet_pt)
            .overflow_hidden()
            .bg(sheet_bg)
            .on_drop::<gpui::ExternalPaths>(cx.listener(
                |view, paths: &gpui::ExternalPaths, window, cx| {
                    window.focus(&view.focus);
                    view.handle_file_drop(paths.paths(), cx);
                },
            ))
            .child(body);

        let sheet_with_controls = if !is_top_tab_mode {
            let tb = theme.clone();
            let is_hidden_sidebar = is_sidebar_mode && !is_sidebar_visible;

            let is_borderless = self.app_settings.tab_bar_position == "borderless";
            let is_island_visible = is_borderless && self.island_hud_opacity > 0.005;

            // Top drag band across the window width (leaving clearance for top-right controls):
            let drag_band = if is_island_visible {
                div()
                    .id(ElementId::named_usize("floating-drag-band-split", 0))
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(120.0))
                    .h(px(36.0))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .w(px(140.0))
                            .h(relative(1.0))
                            .flex()
                            .child(caption_spacer(0)),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .h(relative(1.0))
                            .flex()
                            .child(caption_spacer(1)),
                    )
            } else {
                div()
                    .id(ElementId::named_usize("floating-drag-band-full", 0))
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(120.0))
                    .h(px(36.0))
                    .flex()
                    .flex_row()
                    .child(caption_spacer(0))
            };

            // Floating controls in the top-right corner
            let mut controls_bar = div()
                .id(ElementId::named_usize("floating-controls-bar", 0))
                .absolute()
                .top(if zoomed { px(6.0) } else { px(2.0) })
                .right(if zoomed { px(6.0) } else { px(2.0) })
                .h(px(28.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0));

            // If sidebar is hidden in sidebar mode: show subtle toggle pill [ | ] to restore it
            if is_hidden_sidebar {
                controls_bar = controls_bar.child(
                    div()
                        .id(ElementId::named_usize("floating-sidebar-toggle-btn", 0))
                        .w(px(26.0))
                        .h(px(26.0))
                        .mr(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .bg(Rgba { a: 0.14, ..tb.card_hover })
                        .border_1()
                        .border_color(Rgba { a: 0.08, ..tb.hairline })
                        .hover(move |s| s.bg(tb.card_hover))
                        .active(|s| s.opacity(0.75))
                        .cursor_pointer()
                        .child(
                            icons::icon(icons::SIDEBAR_LEFT)
                                .size(px(13.5))
                                .text_color(tb.chrome_fg_active),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                            view.cycle_sidebar_mode(cx);
                        })),
                );
            }

            // Window controls: minimize, maximize/restore, close
            let controls = [
                (icons::WINDOW_MINIMIZE, false),
                (if zoomed { icons::WINDOW_RESTORE } else { icons::WINDOW_MAXIMIZE }, false),
                (icons::CLOSE, true),
            ];
            for (index, (icon_path, is_close)) in controls.into_iter().enumerate() {
                let button_theme = theme.clone();
                let hover_id = HOVER_MARK_WINDOW + index as u64;
                let is_hovered = self.hover_mark == Some(hover_id);
                let hover_bg = if is_close { rgb(0xC42B1C) } else { button_theme.card_hover };
                let ink = if is_hovered {
                    if is_close { rgb(0xFFFFFF) } else { button_theme.chrome_fg_active }
                } else {
                    chrome_ink
                };
                let btn = div()
                    .id(ElementId::named_usize("floating-window-control", index))
                    .w(px(34.0))
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .hover(move |style| style.bg(hover_bg))
                    .active(move |style| style.opacity(0.80))
                    .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                        set_hover_mark(view, hover_id, *hovered, cx);
                    }))
                    .child(
                        icons::icon(icon_path)
                            .size(px(13.0))
                            .text_color(ink),
                    )
                    .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                        crate::rounded_corners::window_button(index);
                        cx.notify();
                    }));
                controls_bar = controls_bar.child(btn);
            }

            sheet.child(drag_band).child(controls_bar)
        } else {
            sheet
        };

        let container = div()
            .id(ElementId::named_usize("terminal-container", 0))
            .relative()
            .flex()
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(container_bg)
            .overflow_hidden()
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _window, cx| {
                if view.sidebar_is_resizing {
                    let mouse_x = f32::from(event.position.x);
                    let new_w = mouse_x.clamp(160.0, 420.0);
                    view.sidebar_width = new_w;
                    view.sidebar_current_width = new_w;
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                if view.sidebar_is_resizing {
                    view.sidebar_is_resizing = false;
                    cx.notify();
                }
            }));

        let container = if is_top_tab_mode {
            // Standard top tab bar layout: bar on top, sheet below
            container
                .flex_col()
                .child(bar)
                .child(sheet_with_controls)
        } else if is_sidebar_visible {
            // Sidebar layout with NO TOP BAR:
            // Left: Full-height sidebar
            // Right: Terminal sheet with floating controls in top-right
            container
                .flex_row()
                .child(self.render_sidebar(&theme, is_translucent, zoomed, cx))
                .child(sheet_with_controls)
        } else {
            // Sidebar Hidden mode OR Borderless layout:
            // 100% full-bleed terminal canvas with floating controls in top-right
            container
                .flex_col()
                .child(sheet_with_controls)
        };

        let mut root = div()
            .id("root-window")
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .font_family("Segoe UI")
            .text_color(tb.chrome_fg_active)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                let keystroke = event.keystroke.clone();
                view.handle_key(&keystroke, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, window, _cx| {
                    if !view.settings_open && !view.palette_open && !view.profile_menu && !view.search_open {
                        window.focus(&view.focus);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _window, cx| {
                if let Some((cand_ix, start_x, start_y)) = view.tab_drag_candidate {
                    let dx = f32::from(event.position.x) - start_x;
                    let dy = f32::from(event.position.y) - start_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist >= 8.0 && !view.tab_is_dragging {
                        view.tab_is_dragging = true;
                        view.tab_drag_origin = Some(cand_ix);
                        cx.notify();
                    }
                }
                if let Some((split_id, axis, start_coord, start_ratio)) = view.active_split_resize {
                    let current_coord = match axis {
                        crate::tiling::SplitAxis::Columns => f32::from(event.position.x),
                        crate::tiling::SplitAxis::Rows => f32::from(event.position.y),
                    };
                    let total_span = match axis {
                        crate::tiling::SplitAxis::Columns => 1000.0,
                        crate::tiling::SplitAxis::Rows => 650.0,
                    };
                    let delta = (current_coord - start_coord) / total_span;
                    let new_ratio = (start_ratio + delta).clamp(0.15, 0.85);
                    if let Some(tab) = view.tabs.get_mut(view.active) {
                        tab.tile_tree.set_split_ratio(split_id, new_ratio);
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    let had_drag = view.tab_drag_candidate.is_some() || view.tab_is_dragging || view.tab_drag_origin.is_some();
                    view.tab_drag_candidate = None;
                    view.tab_is_dragging = false;
                    view.tab_drag_origin = None;
                    if view.active_split_resize.is_some() {
                        view.active_split_resize = None;
                    }
                    if had_drag {
                        cx.notify();
                    }
                }),
            )
            .child(container);

        if self.app_settings.tab_bar_position == "borderless" {
            root = root.child(self.render_island_hud(&theme, cx));
        }
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
            root = root.child(profile_menu(self, &theme, closing, cx));
        }
        if self.palette_open {
            root = root.child(crate::ui::overlay::overlay_backdrop_with_scrim(
                2,
                self.palette_closing,
                rgba(0x0000008C),
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

        // Close confirmation is topmost: it must win over every other overlay.
        if self.pending_close_tab.is_some() || self.pending_close_window {
            root = root.child(overlay_backdrop(
                5,
                false,
                |view, window, cx| {
                    view.cancel_pending_close(window, cx);
                },
                cx,
            ));
            root = root.child(self.confirm_close_dialog(&theme, cx));
        }

        if let Some((ref msg, is_ok, inst)) = self.update_toast {
            let is_busy_updater = matches!(
                self.update_download_state,
                UpdateDownloadState::ReadyToRestart { .. } | UpdateDownloadState::Downloading { .. }
            );
            if is_busy_updater || inst.elapsed() < std::time::Duration::from_millis(4200) {
                root = root.child(self.render_update_toast(msg, is_ok, &theme, cx));
            }
        }

        if self.search_open {
            root = root.child(self.search_bar(&theme, cx));
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_opacity;
    use crate::view::AtlasView;
    use alacritty_terminal::term::TermMode;

    #[test]
    fn saved_full_opacity_survives_load() {
        assert_eq!(normalize_opacity(1.0), 1.0);
        assert_eq!(normalize_opacity(0.999), 0.999);
        assert_eq!(normalize_opacity(0.85), 0.85);
        assert_eq!(normalize_opacity(0.15), 0.15);
        assert_eq!(normalize_opacity(0.0), 0.0);
        assert!((normalize_opacity(67.0) - 0.67).abs() < 1e-4);
        assert!((normalize_opacity(66.0) - 0.66).abs() < 1e-4);
        assert_eq!(normalize_opacity(100.0), 1.0);
    }

    #[test]
    fn mouse_reporting_sgr_and_normal_modes() {
        let empty_mods = gpui::Modifiers::default();

        // SGR mode: Mouse down on Left button at (10, 5) -> \x1b[<0;10;5M
        let sgr_mode = TermMode::SGR_MOUSE | TermMode::MOUSE_REPORT_CLICK;
        let down_seq = AtlasView::format_mouse_sequence(sgr_mode, 0, 10, 5, false, empty_mods);
        assert_eq!(String::from_utf8(down_seq).unwrap(), "\x1b[<0;10;5M");

        // SGR mode: Mouse UP on Left button at (10, 5) -> \x1b[<0;10;5m (lowercase 'm')
        let up_seq = AtlasView::format_mouse_sequence(sgr_mode, 0, 10, 5, true, empty_mods);
        assert_eq!(String::from_utf8(up_seq).unwrap(), "\x1b[<0;10;5m");

        // SGR mode: Mouse drag / motion at (12, 6) -> \x1b[<32;12;6M
        let drag_seq = AtlasView::format_mouse_sequence(sgr_mode, 32, 12, 6, false, empty_mods);
        assert_eq!(String::from_utf8(drag_seq).unwrap(), "\x1b[<32;12;6M");

        // SGR mode: Scroll wheel up (btn 64) -> \x1b[<64;10;5M
        let scroll_seq = AtlasView::format_mouse_sequence(sgr_mode, 64, 10, 5, false, empty_mods);
        assert_eq!(String::from_utf8(scroll_seq).unwrap(), "\x1b[<64;10;5M");

        // Normal xterm mode: Press Left button at (1, 1) -> \x1b[M 32+0 32+1 32+1
        let normal_mode = TermMode::MOUSE_REPORT_CLICK;
        let norm_down = AtlasView::format_mouse_sequence(normal_mode, 0, 1, 1, false, empty_mods);
        assert_eq!(norm_down, vec![0x1b, b'[', b'M', 32, 33, 33]);

        // Normal xterm mode: Release at (1, 1) -> button 3 (32+3 = 35)
        let norm_up = AtlasView::format_mouse_sequence(normal_mode, 0, 1, 1, true, empty_mods);
        assert_eq!(norm_up, vec![0x1b, b'[', b'M', 35, 33, 33]);
    }

    #[test]
    fn test_tab_reordering_invariants() {
        let mut items = vec!["Tab 0", "Tab 1", "Tab 2", "Tab 3"];
        let mut active = 0;

        // Reorder slot 0 -> slot 2
        let from = 0;
        let to = 2;
        let item = items.remove(from);
        items.insert(to, item);
        if active == from {
            active = to;
        } else if from < active && to >= active {
            active = active.saturating_sub(1);
        } else if from > active && to <= active {
            active = (active + 1).min(items.len() - 1);
        }

        assert_eq!(items, vec!["Tab 1", "Tab 2", "Tab 0", "Tab 3"]);
        assert_eq!(active, 2);

        // Reorder slot 3 -> slot 1
        let from = 3;
        let to = 1;
        let item = items.remove(from);
        items.insert(to, item);
        if active == from {
            active = to;
        } else if from < active && to >= active {
            active = active.saturating_sub(1);
        } else if from > active && to <= active {
            active = (active + 1).min(items.len() - 1);
        }

        assert_eq!(items, vec!["Tab 1", "Tab 3", "Tab 2", "Tab 0"]);
        // Since active was 2 ("Tab 0" was at 2), and from=3 to=1, active was >= to and < from, so active increments: 2 -> 3 ("Tab 0" is now at index 3)
        assert_eq!(active, 3);
        assert_eq!(items[active], "Tab 0");
    }
}
