//! AtlasView - core view coordinator, window lifecycle, and primary rendering orchestration.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, ClipboardItem, Context,
    ElementId, FocusHandle, Font, FontFallbacks, FontWeight, IntoElement, KeyDownEvent, Keystroke,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render, Rgba, ScrollHandle,
    ScrollWheelEvent, SharedString, Window, div, px, relative, rgb, rgba,
};

use crate::config::{
    default_shortcuts, format_keystroke_chord, load_app_settings, AppSettingsFile, SettingsTab,
    ACTION_CLEAR_SCROLLBACK, ACTION_CLOSE_TAB, ACTION_COPY, ACTION_FOCUS_NEXT_PANE,
    ACTION_FOCUS_PREV_PANE, ACTION_FULLSCREEN, ACTION_NEW_TAB, ACTION_NEXT_TAB,
    ACTION_OPEN_SETTINGS, ACTION_PALETTE, ACTION_PASTE, ACTION_PREV_TAB, ACTION_SPLIT_HORIZONTAL,
    ACTION_SPLIT_VERTICAL, ACTION_ZOOM_IN, ACTION_ZOOM_OUT,
};
use crate::icons;
use crate::keymap;
use crate::session::{Profile, Session, SessionMsg};
use crate::terminal::TerminalElement;
use crate::theme::{Theme, ThemeRegistry};
use crate::ui::animation::{ease_apple_snappy, BODY_IN_MS, OVERLAY_OUT_MS, EASE_OUT};
use crate::ui::overlay::{
    caption_spacer, overlay_backdrop, profile_menu, set_hover_mark, HOVER_MARK_NEW_TAB,
    HOVER_MARK_WINDOW,
};
use crate::updater::{self, ReleaseInfo, UpdaterSettings};
use crate::workspace::{
    format_tab_title, git_branch, git_cwd, home_dir, profile_title, session_icon, Pane,
    SplitAxis, Tab,
};


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
    /// Workspace (new-tab profile) menu visibility.
    pub(crate) profile_menu: bool,
    /// Command palette (Ctrl+Shift+P / top-bar search icon).
    pub(crate) palette_open: bool,
    pub(crate) palette_query: String,
    pub(crate) palette_index: usize,
    /// Smoothly gliding floating pill index in the command palette.
    pub(crate) palette_anim_index: f32,
    /// Rename-tab dialog (palette).
    pub(crate) rename_open: bool,
    pub(crate) rename_value: String,
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
    pub(crate) app_settings: AppSettingsFile,
    pub(crate) settings_tab: SettingsTab,
    pub(crate) recording_shortcut: Option<String>,
    pub(crate) font_family: String,
    pub(crate) line_height: f32,
    pub(crate) corner_radius: f32,
    pub(crate) opacity: f32,
    pub(crate) blur_enabled: bool,
    pub(crate) cursor_blink: bool,
    pub(crate) window_was_active: bool,
    pub(crate) mouse_down_pane: Option<usize>,
    pub(crate) settings_scroll_handle: ScrollHandle,
    pub(crate) settings_scroll_pos: f32,
    pub(crate) settings_scroll_target: f32,
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

impl AtlasView {
    pub fn new(window: &mut Window, initial_dir: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let registry = ThemeRegistry::standard();
        let app_settings = load_app_settings();
        let theme_mode = app_settings.theme_mode.clone();
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
        let opacity = app_settings.opacity.clamp(0.5, 1.0);
        let blur_enabled = app_settings.blur_enabled;
        let cursor_blink = app_settings.cursor_blink;

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
            settings_tab: SettingsTab::General,
            recording_shortcut: None,
            font_family,
            line_height,
            corner_radius,
            opacity,
            blur_enabled,
            cursor_blink,
            window_was_active: true,
            mouse_down_pane: None,
            settings_scroll_handle: ScrollHandle::new(),
            settings_scroll_pos: 0.0,
            settings_scroll_target: 0.0,
        };
        crate::rounded_corners::set_dark_mode(is_dark);
        crate::rounded_corners::set_blur(blur_enabled);
        let default_shell_prof = view.get_default_profile();
        view.new_tab_with_cwd(default_shell_prof, initial_dir, cx);

        // Motion & breathing ticker: drives continuous kinetic cursor gliding,
        // organic sinusoidal breathing, theme crossfading, palette gliding,
        // auto-hiding scrollbar transitions, settings kinetic scroll, and tab/pane collapses.
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
                                if d_col.abs() > 0.01 || d_row.abs() > 0.01 {
                                    // Snappy Apple spring deceleration (~70ms)
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

                    // 7. Terminal smooth kinetic scrolling (unified into ticker)
                    if let Some((session, pos, target)) = view.scroll_anim.clone() {
                        view.last_scroll_time = std::time::Instant::now();
                        let diff = target - pos;
                        let factor = (1.0 - (-24.0 * dt).exp()).clamp(0.0, 1.0);
                        let next = if diff.abs() < 0.04 {
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
                        let target_int = next.floor() as i32;
                        let delta = target_int - current;
                        if delta != 0 {
                            guard.term.scroll_display(Scroll::Delta(delta));
                        }
                        needs_paint = true;
                    }

                    // 8. Settings kinetic smooth scrolling
                    if view.settings_open {
                        let max_y = f32::from(view.settings_scroll_handle.max_offset().height).max(0.0);
                        if max_y > 0.0 && view.settings_scroll_target > max_y {
                            view.settings_scroll_target = max_y;
                        }
                        let diff = view.settings_scroll_target - view.settings_scroll_pos;
                        if diff.abs() > 0.5 {
                            let factor = (1.0 - (-24.0 * dt).exp()).clamp(0.0, 1.0);
                            view.settings_scroll_pos += diff * factor;
                            view.settings_scroll_handle.set_offset(gpui::point(px(0.0), px(-view.settings_scroll_pos)));
                            needs_paint = true;
                        } else if (view.settings_scroll_pos - view.settings_scroll_target).abs() > 0.01 {
                            view.settings_scroll_pos = view.settings_scroll_target;
                            view.settings_scroll_handle.set_offset(gpui::point(px(0.0), px(-view.settings_scroll_pos)));
                            needs_paint = true;
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

    /// Grid size to spawn a new PTY at: the active pane's current grid when
    /// one exists, so a fresh shell starts at (roughly) its final width
    /// instead of the classic 80Ã—24 and reflowing under its first prompt.
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
        match Session::spawn(init_cols, init_rows, spawn_cwd.clone(), profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd).or(spawn_cwd);
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
                    close_hover_progress: 0.0,
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

    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
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
    pub(crate) fn restart_tab(&mut self, index: usize, cx: &mut Context<Self>) {
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
    pub(crate) fn split_active(&mut self, axis: SplitAxis, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let (profile, inherit_cwd) = match tab.panes.get(tab.active_pane) {
            Some(pane) => (pane.profile, pane.cwd.clone()),
            None => return,
        };
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        let spawn_cwd = inherit_cwd.or_else(home_dir);
        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(init_cols, init_rows, spawn_cwd.clone(), profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd).or(spawn_cwd);
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
    pub(crate) fn close_pane(&mut self, cx: &mut Context<Self>) {
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
    pub(crate) fn toggle_maximize(&mut self, cx: &mut Context<Self>) {
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
                            // with what is actually on screen â€” the current
                            // theme, or a program's own earlier override â€” so
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
    /// accumulate while the gesture is hot; the motion ticker eases towards it.
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
        cx.notify();
    }

    /// Smooth kinetic scrolling for Settings: accumulates delta and eases towards target.
    pub(crate) fn scroll_settings_smooth(&mut self, delta_y: f32, cx: &mut Context<Self>) {
        let current_offset = -f32::from(self.settings_scroll_handle.offset().y);
        // If previous animation has completed or settled, resync position with handle
        if (self.settings_scroll_target - self.settings_scroll_pos).abs() < 0.5 {
            self.settings_scroll_pos = current_offset;
            self.settings_scroll_target = current_offset;
        }
        let max_y = f32::from(self.settings_scroll_handle.max_offset().height).max(0.0);
        let upper_bound = if max_y > 0.0 { max_y } else { 10_000.0 };
        self.settings_scroll_target = (self.settings_scroll_target - delta_y).clamp(0.0, upper_bound);
        cx.notify();
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

    pub(crate) fn cycle(&mut self, delta: i32) {
        if self.tabs.is_empty() {
            return;
        }
        let len = self.tabs.len() as i32;
        self.active = ((self.active as i32 + delta).rem_euclid(len)) as usize;
    }

    /// Cycle to the next theme preset (Ctrl+Shift+T).
    pub(crate) fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = self.registry.next(self.theme_id);
        self.set_theme(next, cx);
    }

    /// Apply a registry preset and keep every mirror in sync (settings pills
    /// and the keyboard shortcut both land here).
    pub(crate) fn set_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_id = index.min(self.registry.len() - 1);
        let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        self.theme_transition = Some((self.theme.clone(), target, 0.0));
        crate::rounded_corners::set_dark_mode(is_dark);
        self.sync_and_save_settings();
        cx.notify();
    }

    pub(crate) fn set_theme_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.theme_mode = mode.to_string();
        let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        self.theme_transition = Some((self.theme.clone(), target, 0.0));
        crate::rounded_corners::set_dark_mode(is_dark);
        self.sync_and_save_settings();
        cx.notify();
    }

    /// Persist active settings and updater state to %APPDATA%/atlas-terminal/settings.json.


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
            "opencode" => Profile::OpenCode,
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



    pub(crate) fn maybe_auto_check(&mut self, is_focus_event: bool, cx: &mut Context<Self>) {
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
    pub(crate) fn check_updates_manual(&mut self, cx: &mut Context<Self>) {
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
        let etag = if is_manual {
            None
        } else {
            self.updater_settings.etag.clone()
        };

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

    pub(crate) fn dismiss_update_panel(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn skip_current_update(&mut self, cx: &mut Context<Self>) {
        if let Some(update) = self.available_update.take() {
            self.updater_settings.skipped_version = Some(update.version);
            self.sync_and_save_settings();
        }
        self.dismiss_update_panel(cx);
    }

    /// Focus whatever is on top: the settings page when it is open, the
    /// terminal otherwise. Overlay dismissal returns keys here.
    pub(crate) fn focus_main(&self, window: &mut Window) {
        if self.settings_open {
            window.focus(&self.settings_focus);
        } else {
            window.focus(&self.focus);
        }
    }



    pub(crate) fn toggle_profile_menu(&mut self, cx: &mut Context<Self>) {
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
    pub(crate) fn dismiss_profile(&mut self, cx: &mut Context<Self>) {
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
                    self.settings_tab = SettingsTab::General;
                    self.settings_scroll_pos = 0.0;
                    self.settings_scroll_target = 0.0;
                    self.settings_scroll_handle.set_offset(gpui::Point::default());
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
        // Workspace pill Â· horizontal pill tabs Â· drag gap Â· settings /
        // palette / new-tab icons Â· native window controls. The gap reports
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

        // Workspace pill (26Ã—20): a 6%-ink translucent fill with the quiet
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

            let (max_w, min_w, px_pad, opacity) = if tab.closing {
                let scale = (1.0 - EASE_OUT.eval(tab.close_progress)).max(0.0);
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

            let show_close = (active || self.hover_tab == Some(ix)) && !tab.closing;
            if show_close {
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", ix))
                        .w(px(16.0))
                        .h(px(16.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(10.0))
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

        // When in settings, render an active Settings tab pill
        if self.settings_open {
            let pill_bg = tb.elevated_bg;
            let pill_border = tb.elevated_border;
            let pill_fg = tb.chrome_fg_active;
            let pill_icon = tb.accent;

            let settings_pill = div()
                .id(ElementId::named_usize("tab-pill-settings", 0))
                .h(px(24.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .max_w(px(220.0))
                .min_w(px(70.0))
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(6.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg)
                .child(
                    div()
                        .text_color(pill_icon)
                        .child(icons::icon(icons::SETTINGS).size(px(14.0)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child("Settings"),
                )
                .child(
                    div()
                        .id(ElementId::named_usize("tab-close-settings", 0))
                        .w(px(16.0))
                        .h(px(16.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(10.0))
                                .text_color(tb.faint_fg),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.settings_open = false;
                            window.focus(&view.focus);
                            cx.notify();
                        })),
                );

            bar = bar.child(settings_pill);
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
                    .child(div().text_size(px(11.5)).child("Checkingâ€¦")),
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
                        view.settings_tab = SettingsTab::General;
                        view.settings_scroll_pos = 0.0;
                        view.settings_scroll_target = 0.0;
                        view.settings_scroll_handle.set_offset(gpui::Point::default());
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
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
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