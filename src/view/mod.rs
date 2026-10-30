//! AtlasView - core view coordinator, window lifecycle, and primary rendering orchestration.

pub mod actions;
pub mod input;
pub mod scroll;
pub mod sidebar;
pub mod update;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, ClipboardItem, Context,
    ElementId, FocusHandle, Font, FontFallbacks, FontWeight, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, Rgba, ScrollHandle,
    ScrollWheelEvent, SharedString, Window, div, px, relative, rgb, rgba,
};

use crate::config::{load_app_settings, AppSettingsFile, SettingsTab};
use crate::icons;
use crate::session::Session;
use crate::terminal::TerminalElement;
use crate::theme::{Theme, ThemeRegistry};
use crate::ui::animation::{ease_apple_snappy, BODY_IN_MS, EASE_OUT};
use crate::ui::overlay::{
    caption_spacer, overlay_backdrop, profile_menu, set_hover_mark,
    HOVER_MARK_CAPSULE_CHEVRON, HOVER_MARK_CAPSULE_PLUS, HOVER_MARK_WINDOW,
};
use crate::updater::{self, ReleaseInfo, UpdaterSettings};
use crate::workspace::{
    format_tab_title, git_branch, git_cwd, home_dir, session_icon,
    SplitAxis, Tab,
};

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
    /// Tab pill currently under the pointer: its hover-only close mark.
    pub(crate) hover_tab: Option<usize>,
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
    pub(crate) recording_shortcut: Option<String>,
    pub(crate) font_family: String,
    pub(crate) line_height: f32,
    pub(crate) corner_radius: f32,
    pub(crate) opacity: f32,
    pub(crate) cursor_blink: bool,
    pub(crate) window_was_active: bool,
    pub(crate) mouse_down_pane: Option<usize>,
    pub(crate) last_mouse_btn: Option<u8>,
    pub(crate) last_mouse_cell: Option<(usize, usize)>,
    pub(crate) settings_scroll_handle: ScrollHandle,
    pub(crate) settings_scroll_pos: f32,
    pub(crate) settings_scroll_target: f32,
    pub(crate) hovered_link: Option<(usize, crate::url::DetectedUrl)>,
    pub(crate) sidebar_open: bool,
    pub(crate) sidebar_expansion: f32,
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

/// A saved 100% opacity is a real user choice (the settings toggle offers it),
/// so loading must not silently turn it back into the old 85% acrylic default.
pub(crate) fn normalize_opacity(saved: f32) -> f32 {
    if saved >= 0.999 {
        1.0
    } else {
        saved.clamp(0.15, 1.0)
    }
}

impl AtlasView {
    pub fn new(window: &mut Window, initial_dir: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let registry = ThemeRegistry::standard();
        let app_settings = load_app_settings();
        let theme_mode = if app_settings.theme_mode == "auto" {
            "dark".to_string()
        } else {
            app_settings.theme_mode.clone()
        };
        let theme_id = registry
            .find_by_name_or_id(&app_settings.theme)
            .unwrap_or(0);
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
        let opacity = if app_settings.blur_enabled || app_settings.opacity < 0.999 {
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
            theme_mode,
            last_theme_check: std::time::Instant::now(),
            registry,
            settings_open: false,
            settings_tab_open: false,
            profile_menu: false,
            palette_open: false,
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
            hover_tab: None,
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
            settings_tab: SettingsTab::General,
            recording_shortcut: None,
            font_family,
            line_height,
            corner_radius,
            opacity,
            cursor_blink,
            window_was_active: true,
            mouse_down_pane: None,
            last_mouse_btn: None,
            last_mouse_cell: None,
            settings_scroll_handle: ScrollHandle::new(),
            settings_scroll_pos: 0.0,
            settings_scroll_target: 0.0,
            hovered_link: None,
            sidebar_open: true,
            sidebar_expansion: 1.0,
        };
        crate::rounded_corners::set_dark_mode(is_dark);
        crate::rounded_corners::set_opacity(opacity);
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
                        let (target, is_dark) = view.registry.resolve(view.theme_id, &view.theme_mode);
                        if (view.theme.bg.r - target.bg.r).abs() > 0.05
                            || (view.theme.bg.g - target.bg.g).abs() > 0.05
                            || (view.theme.bg.b - target.bg.b).abs() > 0.05
                        {
                            view.theme_transition = Some((view.theme.clone(), target, 0.0));
                            crate::rounded_corners::set_dark_mode(is_dark);
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

                    // 8. Vertical sidebar kinetic expansion/collapse
                    let target_sidebar = if view.sidebar_open { 1.0f32 } else { 0.0f32 };
                    let sidebar_diff = target_sidebar - view.sidebar_expansion;
                    if sidebar_diff.abs() > 0.005 {
                        active_motion = true;
                        let factor = (1.0 - (-18.0 * dt).exp()).clamp(0.0, 1.0);
                        view.sidebar_expansion = if sidebar_diff.abs() < 0.01 {
                            target_sidebar
                        } else {
                            view.sidebar_expansion + sidebar_diff * factor
                        };
                        needs_paint = true;
                    } else if view.sidebar_expansion != target_sidebar {
                        view.sidebar_expansion = target_sidebar;
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

        // Working-directory poll: reads each live shell's PEB every 1.5 s so
        // the pane title can track `cd` like a real terminal. Repaints only
        // when a path actually changed.
        let cwd_poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(800))
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
}

impl Render for AtlasView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = crate::rounded_corners::is_active();
        if is_active && !self.window_was_active {
            self.maybe_auto_check(true, cx);
        }
        self.window_was_active = is_active;

        // Keep the native backdrop in sync with the opacity slider. GPUI stays
        // on a transparent swapchain; `rounded_corners` applies the native
        // acrylic accent (GPUI's `Blurred` background, with the DWM system
        // backdrop as a fallback) and is a no-op whenever the slider has not
        // moved.
        crate::rounded_corners::set_opacity(self.opacity);
        let is_translucent = self.opacity < 0.999;

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

        let is_tui_active = if let Some(tab) = self.tabs.get(self.active) {
            if let Some(pane) = tab.panes.get(tab.active_pane) {
                Self::is_pane_tui(pane, &theme)
            } else {
                false
            }
        } else {
            false
        };

        let (chrome_bg, sheet_bg, container_bg) = if is_translucent {
            // In translucent mode: sheet and container must be fully transparent so
            // the native acrylic accent (GPUI's `Blurred` background) shows
            // through. Only the titlebar chrome gets a semi-transparent tint
            // for legibility.
            //
            // The bar fill ramps from a 0.56 floor up to 0.90 as the slider
            // approaches opaque. The old `opacity * 0.70` curve collapsed to
            // 0.105 at 15% opacity, which let the wallpaper swallow the tab
            // titles; the floor keeps the 36px bar dark (or light) enough for
            // text at every slider position while the wallpaper still reads
            // through it.
            //
            // The terminal element also skips default-cell backgrounds, so the
            // cleared swapchain keeps alpha 0 across the terminal area. GPUI 0.2.2's
            // DestBlendAlpha=ONE only over-accumulates overlapping translucent
            // chrome layers; it does not force the empty terminal area opaque.
            let chrome = Rgba {
                a: (0.56 + self.opacity * 0.34).clamp(0.56, 0.90),
                ..tb.panel_bg
            };
            if is_tui_active {
                (chrome, active_bg, tb.panel_bg)
            } else {
                (chrome, rgba(0x00000000), rgba(0x00000000))
            }
        } else {
            (tb.panel_bg, term_base, tb.panel_bg)
        };

        let bar_h = if zoomed { px(50.0) } else { px(42.0) };
        let bar_pt = if zoomed { px(8.0) } else { px(0.0) };
        let bar_pl = if zoomed { px(18.0) } else { px(12.0) };

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .w(relative(1.0))
            .h(bar_h)
            .pt(bar_pt)
            .pl(bar_pl)
            .bg(chrome_bg)
            .border_b_1()
            .border_color(tb.hairline);

        // Left Anchor: Unified Split Capsule ( + │ ▾ )
        let default_prof = self.get_default_profile();
        let is_menu_open = self.profile_menu;
        let capsule_bg = tb.elevated_bg;
        let capsule_border = if is_menu_open { tb.accent } else { tb.elevated_border };

        let plus_hovered = self.hover_mark == Some(HOVER_MARK_CAPSULE_PLUS);
        let chevron_hovered = self.hover_mark == Some(HOVER_MARK_CAPSULE_CHEVRON);

        let plus_ink = if plus_hovered {
            tb.fg
        } else {
            tb.chrome_fg_active
        };

        let chevron_ink = if is_menu_open || chevron_hovered {
            tb.fg
        } else {
            tb.chrome_fg_active
        };

        let plus_btn = div()
            .id(ElementId::named_usize("capsule-plus", 0))
            .w(px(30.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_l_full()
            .hover(move |s| s.bg(tb.card_hover))
            .active(move |s| s.opacity(0.75))
            .cursor_pointer()
            .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                set_hover_mark(view, HOVER_MARK_CAPSULE_PLUS, *hovered, cx);
            }))
            .child(
                icons::icon(icons::PLUS)
                    .size(px(15.5))
                    .text_color(plus_ink),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                view.new_tab(default_prof, cx);
                window.focus(&view.focus);
            }));

        let divider = div()
            .w(px(1.0))
            .h(px(16.0))
            .bg(tb.elevated_border);

        let chevron_btn = div()
            .id(ElementId::named_usize("capsule-chevron", 0))
            .w(px(26.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_r_full()
            .bg(if is_menu_open { tb.card_hover } else { rgba(0x00000000) })
            .hover(move |s| s.bg(tb.card_hover))
            .active(move |s| s.opacity(0.75))
            .cursor_pointer()
            .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                set_hover_mark(view, HOVER_MARK_CAPSULE_CHEVRON, *hovered, cx);
            }))
            .child(
                icons::icon(icons::CHEVRON_DOWN)
                    .size(px(13.5))
                    .text_color(chevron_ink),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.toggle_profile_menu(cx);
                window.focus(&view.focus);
            }));

        let capsule = div()
            .id(ElementId::named_usize("topbar-launch-capsule", 0))
            .h(px(32.0))
            .flex()
            .flex_row()
            .items_center()
            .rounded_full()
            .bg(capsule_bg)
            .border_1()
            .border_color(capsule_border)
            .shadow_sm()
            .child(plus_btn)
            .child(divider)
            .child(chevron_btn);

        let is_sidebar_mode = self.app_settings.tab_bar_position == "sidebar";
        if is_sidebar_mode {
            let is_sidebar_active = self.sidebar_open && self.sidebar_expansion > 0.3;
            let sidebar_btn = div()
                .id(ElementId::named_usize("sidebar-toggle-btn", 0))
                .w(px(30.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(if is_sidebar_active { Rgba { a: 0.12, ..tb.accent } } else { rgba(0x00000000) })
                .hover(move |s| s.bg(tb.card_hover))
                .active(|s| s.opacity(0.75))
                .cursor_pointer()
                .child(
                    icons::icon(icons::SIDEBAR_LEFT)
                        .size(px(16.0))
                        .text_color(if is_sidebar_active { tb.accent } else { tb.chrome_fg_active }),
                )
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    view.toggle_sidebar(cx);
                }));

            bar = bar.child(sidebar_btn);
            bar = bar.child(div().w(px(2.0)));
            bar = bar.child(capsule);
            bar = bar.child(div().w(px(6.0)));

            if let Some(tab) = self.tabs.get(self.active) {
                let pane = tab.panes.get(tab.active_pane);
                let label = match pane {
                    Some(pane) => tab.custom_title.clone().unwrap_or_else(|| format_tab_title(pane)),
                    None => "Terminal".to_string(),
                };
                let win_title = if label == "Terminal" || label.is_empty() {
                    "Atlas Terminal".to_string()
                } else {
                    format!("{label} — Atlas Terminal")
                };
                crate::rounded_corners::set_window_title(&win_title);
            }

            if self.sidebar_expansion < 0.6 {
                let active_tab_title = if self.settings_open {
                    "Settings".to_string()
                } else if let Some(tab) = self.tabs.get(self.active) {
                    if let Some(pane) = tab.panes.get(tab.active_pane) {
                        tab.custom_title.clone().unwrap_or_else(|| format_tab_title(pane))
                    } else {
                        "Terminal".to_string()
                    }
                } else {
                    "Terminal".to_string()
                };

                bar = bar.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .h(px(28.0))
                        .rounded(px(6.0))
                        .bg(tb.card_hover)
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tb.chrome_fg_active)
                                .truncate()
                                .child(active_tab_title),
                        ),
                );
            }
        } else {
            bar = bar.child(capsule);
            bar = bar.child(div().w(px(6.0)));

        // Horizontal tabs: active tab is a clean rounded rect (elevated bg + subtle border)
        // carrying the running process's own mark; inactive tabs are quiet with folder icon.
        // Tab close is visible on active and hover.
        for ix in 0..self.tabs.len() {
            let active = ix == self.active && !self.settings_open;
            let tab = &self.tabs[ix];
            let pane = tab.panes.get(tab.active_pane);
            let label = match pane {
                Some(pane) => tab.custom_title.clone().unwrap_or_else(|| {
                    format_tab_title(pane)
                }),
                None => "Terminal".to_string(),
            };
            if active {
                let win_title = if label == "Terminal" || label.is_empty() {
                    "Atlas Terminal".to_string()
                } else {
                    format!("{label} — Atlas Terminal")
                };
                crate::rounded_corners::set_window_title(&win_title);
            }
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
                (rgba(0x00000000), rgba(0x00000000), chrome_ink, chrome_ink)
            };
            // The active pill must stay an opaque fill on hover: card_hover is
            // a translucent wash that would drop the title over a bright
            // blurred wallpaper in translucent mode.
            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };
            let pill_hot = tb.chrome_fg_active;

            let (max_w, min_w, px_pad, opacity) = if tab.closing {
                let scale = (1.0 - EASE_OUT.eval(tab.close_progress)).max(0.0);
                (240.0 * scale, 0.0, 12.0 * scale, scale)
            } else {
                (240.0, 48.0, 12.0, 1.0)
            };

            let mut pill = div()
                .id(ElementId::named_usize("tab-pill", ix))
                .h(px(32.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .px(px(px_pad))
                .max_w(px(max_w))
                .min_w(px(min_w))
                .opacity(opacity)
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            if !tab.closing {
                pill = pill
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .active(move |style| style.opacity(0.82))
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
                        .child(icons::icon(icon).size(px(16.5)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child(label),
                );

            let show_close = (active || self.hover_tab == Some(ix)) && !tab.closing;
            if show_close {
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", ix))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .active(move |style| style.opacity(0.75))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(12.0))
                                .text_color(tb.faint_fg),
                        )
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

        // Settings tab pill: stays mounted on the tab bar until the user explicitly closes it.
        // If settings_open is true: renders in the active tab state (elevated bg, accent icon).
        // If settings_open is false: renders in the inactive tab state (quiet transparent, clickable to reopen).
        if self.settings_tab_open {
            let active = self.settings_open;
            let (pill_bg, pill_border, pill_fg, pill_icon) = if active {
                (
                    tb.elevated_bg,
                    tb.elevated_border,
                    tb.chrome_fg_active,
                    tb.accent,
                )
            } else {
                (rgba(0x00000000), rgba(0x00000000), chrome_ink, chrome_ink)
            };
            // The active pill must stay an opaque fill on hover: card_hover is
            // a translucent wash that would drop the title over a bright
            // blurred wallpaper in translucent mode.
            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };
            let pill_hot = tb.chrome_fg_active;

            let mut settings_pill = div()
                .id(ElementId::named_usize("tab-pill-settings", 0))
                .h(px(32.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .px(px(12.0))
                .max_w(px(240.0))
                .min_w(px(80.0))
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            if !active {
                settings_pill = settings_pill
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .active(move |style| style.opacity(0.82))
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = true;
                        window.focus(&view.settings_focus);
                        cx.notify();
                    }));
            }

            settings_pill = settings_pill
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(|view, _: &MouseDownEvent, window, cx| {
                        view.settings_tab_open = false;
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .text_color(pill_icon)
                        .child(icons::icon(icons::SETTINGS).size(px(16.5)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child("Settings"),
                )
                .child(
                    div()
                        .id(ElementId::named_usize("tab-close-settings", 0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .active(move |style| style.opacity(0.75))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(12.0))
                                .text_color(tb.faint_fg),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            view.settings_tab_open = false;
                            view.settings_open = false;
                            window.focus(&view.focus);
                            cx.notify();
                        })),
                );

            bar = bar.child(settings_pill);
        }
        }

        bar = bar.child(caption_spacer(0));

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
            let (label, show_dot) = match &self.update_download_state {
                UpdateDownloadState::Downloading { percent, .. } => {
                    (format!("Downloading {}%", (percent * 100.0) as u32), false)
                }
                UpdateDownloadState::Verifying { .. } => {
                    ("Verifying…".to_string(), false)
                }
                UpdateDownloadState::ReadyToRestart { .. } => {
                    ("Restart to Apply".to_string(), true)
                }
                UpdateDownloadState::Failed { .. } => {
                    ("Update Error".to_string(), true)
                }
                _ => (format!("v{}", update.version), true),
            };
            let mut badge = div()
                .id(ElementId::named_usize("topbar-update-badge", 0))
                .h(px(28.0))
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
                .active(move |s| s.opacity(0.75));

            if show_dot {
                badge = badge.child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(badge_ink));
            }

            badge = badge
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    if let Some(ref info) = view.available_update {
                        let msg = format!("Atlas Terminal v{} is available!", info.version);
                        view.show_update_toast(msg, true, cx);
                    } else {
                        view.check_updates_manual(cx);
                    }
                }));

            bar = bar.child(badge);
        } else if self.is_checking_updates {
            bar = bar.child(
                div()
                    .h(px(28.0))
                    .px(px(9.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .rounded(px(6.0))
                    .text_color(tb.faint_fg)
                    .child(icons::icon(icons::REFRESH).size(px(13.0)).text_color(tb.faint_fg))
                    .child(div().text_size(px(12.0)).child("Checking...")),
            );
        }

        bar = bar.child(div().w(px(6.0)));

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
                chrome_ink
            };
            let btn = div()
                .id(ElementId::named_usize("window-control", index))
                .w(px(46.0))
                .h(relative(1.0))
                .flex()
                .items_center()
                .justify_center()
                .hover(move |style| style.bg(hover_bg))
                .active(move |style| style.opacity(0.80));
            let btn = btn
                .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                    set_hover_mark(view, hover_id, *hovered, cx);
                }))
                .child(
                    icons::icon(icon_path)
                        .size(px(14.5))
                        .text_color(ink),
                )
                .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                    crate::rounded_corners::window_button(index);
                    cx.notify();
                }));
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
                .min_w(px(0.0))
                .min_h(px(0.0))
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    let keystroke = event.keystroke.clone();
                    view.handle_key(&keystroke, window, cx);
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

            // One cell per visible pane with a hairline divider between;
            // maximizing hides the inactive panes without killing their shells.
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

                let pane_down = pane_ix;
                let pane_move = pane_ix;
                let pane_up = pane_ix;
                let pane_rc = pane_ix;

                let pane_is_tui = Self::is_pane_tui(pane, &theme);
                let pane_bg = if pane_is_tui {
                    Self::detect_pane_bg(pane, &theme)
                } else if is_translucent {
                    rgba(0x00000000)
                } else {
                    active_bg
                };

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
                    .bg(if pane_is_tui {
                        pane_bg
                    } else if tab.panes.len() > 1 && !is_focused_pane {
                        Rgba { a: 0.05, ..theme.chrome_fg }
                    } else {
                        rgba(0x00000000)
                    });
                if tab.panes.len() > 1 {
                    cell = cell
                        .border_1()
                        .border_color(if is_focused_pane {
                            Rgba { a: 0.28, ..theme.accent }
                        } else {
                            rgba(0x00000000)
                        });
                }
                if self.hovered_link.as_ref().map_or(false, |(p, _)| *p == pane_ix) {
                    cell = cell.cursor_pointer();
                }
                let pane_drop = pane_ix;
                cell = cell
                    .on_drop::<gpui::ExternalPaths>(cx.listener(
                        move |view, paths: &gpui::ExternalPaths, window, cx| {
                            window.focus(&view.focus);
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                tab.active_pane = pane_drop;
                            }
                            view.handle_file_drop(paths.paths(), cx);
                        },
                    ))
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

                            // Ctrl+Click opens URL at clicked grid point
                            if event.modifiers.control {
                                if let Some((pt, _)) = view.point_from_mouse(event.position, pane_down) {
                                    if let Some(tab) = view.tabs.get(view.active) {
                                        if let Some(pane) = tab.panes.get(pane_down) {
                                            let term = &pane.session.borrow().term;
                                            if let Some(detected) = crate::url::find_url_at_point(term, pt) {
                                                crate::url::open_target(&detected.url, &view.app_settings.default_editor, pane.cwd.as_deref());
                                                view.mouse_down_pane = None;
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_down) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_down) {
                                                let mut btn = 0;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}M");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                                view.last_mouse_btn = Some(0);
                                                view.last_mouse_cell = Some((col, row));
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                            if let Some((pt, side)) = view.point_from_mouse(event.position, pane_down) {
                                if let Some(tab) = view.tabs.get_mut(view.active) {
                                    if let Some(p) = tab.panes.get_mut(pane_down) {
                                        let sel_type = match event.click_count {
                                            2 => SelectionType::Semantic,
                                            3.. => SelectionType::Lines,
                                            _ => SelectionType::Simple,
                                        };
                                        p.session.borrow_mut().term.selection =
                                            Some(Selection::new(sel_type, pt, side));
                                        cx.notify();
                                    }
                                }
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if tab.active_pane != pane_down {
                                    tab.active_pane = pane_down;
                                    cx.notify();
                                }
                            }
                            window.focus(&view.focus);
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_down) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_down) {
                                                let mut btn = 1;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}M");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                                view.last_mouse_btn = Some(1);
                                                view.last_mouse_cell = Some((col, row));
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                            view.paste(cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if tab.active_pane != pane_rc {
                                    tab.active_pane = pane_rc;
                                    cx.notify();
                                }
                            }
                            window.focus(&view.focus);
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_rc) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_rc) {
                                                let mut btn = 2;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}M");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                                view.last_mouse_btn = Some(2);
                                                view.last_mouse_cell = Some((col, row));
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                            view.handle_terminal_right_click(window, cx);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseUpEvent, window, cx| {
                            view.mouse_down_pane = None;
                            view.last_mouse_btn = None;
                            view.last_mouse_cell = None;
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_up) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_up) {
                                                let mut btn = 0;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}m");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                            if let Some(tab) = view.tabs.get(view.active) {
                                if let Some(pane) = tab.panes.get(pane_up) {
                                    let selected_text = {
                                        let sess = pane.session.borrow();
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
                    .on_mouse_up(
                        MouseButton::Middle,
                        cx.listener(move |view, event: &MouseUpEvent, _window, _cx| {
                            view.last_mouse_btn = None;
                            view.last_mouse_cell = None;
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_up) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_up) {
                                                let mut btn = 1;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}m");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                            }
                                        }
                                    }
                                }
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseUpEvent, _window, _cx| {
                            view.last_mouse_btn = None;
                            view.last_mouse_cell = None;
                            if !event.modifiers.shift {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_up) {
                                        if Self::is_mouse_reporting(pane) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_up) {
                                                let mut btn = 2;
                                                if event.modifiers.alt { btn += 8; }
                                                if event.modifiers.control { btn += 16; }
                                                let seq = format!("\x1b[<{btn};{col};{row}m");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                            }
                                        }
                                    }
                                }
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                        if !event.modifiers.shift {
                            if let Some(tab) = view.tabs.get(view.active) {
                                if let Some(pane) = tab.panes.get(pane_move) {
                                    if Self::is_mouse_reporting(pane) {
                                        let mode = *pane.session.borrow().term.mode();
                                        let is_motion = mode.contains(TermMode::MOUSE_MOTION);
                                        let is_drag = mode.contains(TermMode::MOUSE_DRAG);
                                        if is_motion || (is_drag && view.last_mouse_btn.is_some()) {
                                            if let Some((col, row)) = view.grid_coords_from_mouse(event.position, pane_move) {
                                                if view.last_mouse_cell == Some((col, row)) {
                                                    return;
                                                }
                                                view.last_mouse_cell = Some((col, row));

                                                let btn_val = view.last_mouse_btn.unwrap_or(3);
                                                let mut cb = btn_val + 32;
                                                if event.modifiers.alt { cb += 8; }
                                                if event.modifiers.control { cb += 16; }
                                                let seq = format!("\x1b[<{cb};{col};{row}M");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                            }
                                        }
                                        return;
                                    }
                                }
                            }
                        }
                        if event.modifiers.control {
                            let mut found = None;
                            if let Some((pt, _)) = view.point_from_mouse(event.position, pane_move) {
                                if let Some(tab) = view.tabs.get(view.active) {
                                    if let Some(pane) = tab.panes.get(pane_move) {
                                        let term = &pane.session.borrow().term;
                                        if let Some(detected) = crate::url::find_url_at_point(term, pt) {
                                            found = Some((pane_move, detected));
                                        }
                                    }
                                }
                            }
                            if view.hovered_link != found {
                                view.hovered_link = found;
                                cx.notify();
                            }
                        } else if view.hovered_link.is_some() {
                            view.hovered_link = None;
                            cx.notify();
                        }
                        if view.mouse_down_pane == Some(pane_move) {
                            if let Some((pt, side)) = view.point_from_mouse(event.position, pane_move) {
                                if let Some(tab) = view.tabs.get_mut(view.active) {
                                    if let Some(p) = tab.panes.get_mut(pane_move) {
                                        let mut sess = p.session.borrow_mut();
                                        if let Some(ref mut sel) = sess.term.selection {
                                            sel.update(pt, side);
                                            cx.notify();
                                        }
                                    }
                                }
                            }
                        }
                    }))
                    .on_scroll_wheel(cx.listener(move |view, event: &ScrollWheelEvent, _window, cx| {
                        if event.modifiers.control {
                            let delta = match event.delta {
                                gpui::ScrollDelta::Lines(d) => d.y,
                                gpui::ScrollDelta::Pixels(d) => f32::from(d.y),
                            };
                            if delta > 0.0 {
                                view.adjust_font_size(view.font_size + 1.0, cx);
                            } else if delta < 0.0 {
                                view.adjust_font_size(view.font_size - 1.0, cx);
                            }
                            return;
                        }
                        if !event.modifiers.shift {
                            let is_reporting = view.tabs.get(view.active)
                                .and_then(|tab| tab.panes.get(pane_down))
                                .map_or(false, Self::is_mouse_reporting);
                            if is_reporting {
                                let coords = view.grid_coords_from_mouse(event.position, pane_down);
                                if let Some(tab) = view.tabs.get_mut(view.active) {
                                    if let Some(pane) = tab.panes.get_mut(pane_down) {
                                        pane.cursor_pos = Some(pane.cursor_target);
                                        let delta_y = match event.delta {
                                            gpui::ScrollDelta::Lines(delta) => delta.y,
                                            gpui::ScrollDelta::Pixels(delta) => {
                                                let dy = f32::from(delta.y);
                                                if dy.abs() < 0.1 { 0.0 } else if dy > 0.0 { 1.0 } else { -1.0 }
                                            }
                                        };
                                        if delta_y != 0.0 {
                                            if let Some((col, row)) = coords {
                                                let btn = if delta_y > 0.0 { 64 } else { 65 };
                                                let mut cb = btn;
                                                if event.modifiers.alt { cb += 8; }
                                                if event.modifiers.control { cb += 16; }
                                                let seq = format!("\x1b[<{cb};{col};{row}M");
                                                pane.session.borrow_mut().write_bytes(seq.as_bytes());
                                            }
                                        }
                                        return;
                                    }
                                }
                            }
                        }
                        let lines = match event.delta {
                            gpui::ScrollDelta::Lines(delta) => delta.y as f32,
                            gpui::ScrollDelta::Pixels(delta) => f32::from(delta.y) / 18.0,
                        };
                        view.scroll_smooth(lines, cx);
                    }));
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
                            (pos - pos.ceil()) * cell_height
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
                        // In translucent mode, keep active_bg transparent so TUI apps like OpenCode
                        // paint their solid backgrounds with rounded edge quads rather than becoming transparent blur.
                        active_bg: if is_translucent && !pane_is_tui { rgba(0x00000000) } else { pane_bg },
                        font: self.font.clone(),
                        font_size: self.font_size,
                        line_height: self.line_height,
                        cursor_on: self.cursor_on,
                        cursor_override: self.cursor_style,
                        cursor_pos: pane.cursor_pos,
                        cursor_alpha,
                        subpixel_y,
                        scrollbar_opacity: self.scrollbar_opacity,
                        hovered_link: self.hovered_link.as_ref().and_then(|(p, hl)| {
                            if *p == pane_ix {
                                Some(hl.clone())
                            } else {
                                None
                            }
                        }),
                        search_matches: if self.search_open { self.search_matches.clone() } else { Vec::new() },
                        active_search_match: if self.search_open { Some(self.search_index) } else { None },
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
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .child(body)
            .with_animation(
                ElementId::named_usize("body-anim", self.settings_open as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta),
            );

        let sheet = div()
            .id(ElementId::named_usize("terminal-sheet", 0))
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(sheet_bg)
            .on_drop::<gpui::ExternalPaths>(cx.listener(
                |view, paths: &gpui::ExternalPaths, window, cx| {
                    window.focus(&view.focus);
                    view.handle_file_drop(paths.paths(), cx);
                },
            ))
            .child(body);

        let is_sidebar_mode = self.app_settings.tab_bar_position == "sidebar";
        let content_row: AnyElement = if is_sidebar_mode {
            let mut row = div()
                .flex()
                .flex_row()
                .flex_1()
                .w(relative(1.0))
                .min_w(px(0.0))
                .min_h(px(0.0))
                .overflow_hidden();

            if self.sidebar_expansion > 0.005 {
                row = row.child(self.render_sidebar(&theme, is_translucent, cx));
            }
            row = row.child(sheet);
            row.into_any_element()
        } else {
            sheet.into_any_element()
        };

        let container = div()
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
            .child(bar)
            .child(content_row);

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .font_family("Segoe UI")
            .text_color(tb.chrome_fg_active)
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

        if let Some((ref msg, is_ok, _)) = self.update_toast {
            root = root.child(self.render_update_toast(msg, is_ok, &theme, cx));
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

    #[test]
    fn saved_full_opacity_survives_load() {
        assert_eq!(normalize_opacity(1.0), 1.0);
        assert_eq!(normalize_opacity(0.999), 1.0);
        assert_eq!(normalize_opacity(0.85), 0.85);
        assert_eq!(normalize_opacity(0.15), 0.15);
        assert_eq!(normalize_opacity(0.0), 0.15);
    }
}
