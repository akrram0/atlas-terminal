//! Keyboard chords, mouse drag selection, coordinate hit-testing, and clipboard bridging.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use gpui::{ClipboardItem, Context, Keystroke, Pixels, Rgba, Window};

use crate::config::{
    default_shortcuts, format_keystroke_chord, ACTION_CLEAR_SCROLLBACK,
    ACTION_CLOSE_TAB, ACTION_COPY, ACTION_FIND, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE,
    ACTION_FULLSCREEN, ACTION_NEW_TAB, ACTION_NEW_WINDOW, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_OPEN_URL_PICKER,
    ACTION_PALETTE, ACTION_PASTE, ACTION_PREV_TAB, ACTION_SCROLL_LINE_DOWN, ACTION_SCROLL_LINE_UP,
    ACTION_SCROLL_PAGE_DOWN, ACTION_SCROLL_PAGE_UP,
    ACTION_SCROLL_TO_BOTTOM, ACTION_SCROLL_TO_TOP, ACTION_SELECT_ALL, ACTION_SPLIT_HORIZONTAL,
    ACTION_SPLIT_VERTICAL, ACTION_TOGGLE_MAXIMIZE, ACTION_TOGGLE_SIDEBAR, ACTION_TOGGLE_THEME,
    ACTION_NEXT_THEME, ACTION_PREV_THEME, ACTION_TOGGLE_DARK_LIGHT, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT, ACTION_ZOOM_RESET, ACTION_PROFILE_MENU, ACTION_NEW_TAB_WSL, ACTION_NEW_TAB_POWERSHELL,
    ACTION_NEW_TAB_CMD, ACTION_NEW_TAB_GITBASH, ACTION_NEW_TAB_OPENCODE, ACTION_NEW_TAB_ANTIGRAVITY,
};
use crate::keymap;
use crate::session::Profile;
use crate::theme::Theme;
use crate::workspace::{Pane, SplitAxis};

use super::AtlasView;

impl AtlasView {
    pub(crate) fn paste_text(&self, text: &str, cx: &mut Context<Self>) {
        let Some(pane) = self.active_pane() else {
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

    pub(crate) fn paste(&self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                self.paste_text(&text, cx);
            }
        }
    }

    pub(crate) fn cycle(&mut self, delta: i32) {
        if self.tabs.is_empty() {
            return;
        }
        let len = self.tabs.len() as i32;
        self.active = ((self.active as i32 + delta).rem_euclid(len)) as usize;
    }

    /// Toggle between Dark and Light mode.
    pub(crate) fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let is_currently_light = self.theme.is_light();
        let new_mode = if is_currently_light { "dark" } else { "light" };
        self.set_theme_mode(new_mode, cx);
        let preset_name = self.registry.preset(self.theme_id).name.clone();
        let mode_label = if new_mode == "light" { "Light Mode" } else { "Dark Mode" };
        self.show_update_toast(format!("{} ({})", mode_label, preset_name), true, cx);
    }

    /// Cycle to the next theme preset (filtered by current mode support).
    pub fn next_theme(&mut self, cx: &mut Context<Self>) {
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        let valid_indices: Vec<usize> = (0..self.registry.len())
            .filter(|&i| {
                let p = self.registry.preset(i);
                if is_light { p.supports_light() } else { p.supports_dark() }
            })
            .collect();

        if valid_indices.is_empty() {
            return;
        }

        let current_idx = if is_light { self.theme_light_id } else { self.theme_dark_id };
        let current_pos = valid_indices.iter().position(|&i| i == current_idx).unwrap_or(0);
        let next_idx = valid_indices[(current_pos + 1) % valid_indices.len()];

        if is_light {
            self.set_light_theme(next_idx, cx);
        } else {
            self.set_dark_theme(next_idx, cx);
        }
        let preset = self.registry.preset(next_idx);
        let name = if is_light { preset.light_name() } else { preset.dark_name() };
        let mode_label = if is_light { "Light" } else { "Dark" };
        self.show_update_toast(format!("Theme: {} ({})", name, mode_label), true, cx);
    }

    /// Cycle to the previous theme preset (filtered by current mode support).
    pub fn prev_theme(&mut self, cx: &mut Context<Self>) {
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        let valid_indices: Vec<usize> = (0..self.registry.len())
            .filter(|&i| {
                let p = self.registry.preset(i);
                if is_light { p.supports_light() } else { p.supports_dark() }
            })
            .collect();

        if valid_indices.is_empty() {
            return;
        }

        let current_idx = if is_light { self.theme_light_id } else { self.theme_dark_id };
        let current_pos = valid_indices.iter().position(|&i| i == current_idx).unwrap_or(0);
        let prev_idx = if current_pos == 0 {
            valid_indices[valid_indices.len() - 1]
        } else {
            valid_indices[current_pos - 1]
        };

        if is_light {
            self.set_light_theme(prev_idx, cx);
        } else {
            self.set_dark_theme(prev_idx, cx);
        }
        let preset = self.registry.preset(prev_idx);
        let name = if is_light { preset.light_name() } else { preset.dark_name() };
        let mode_label = if is_light { "Light" } else { "Dark" };
        self.show_update_toast(format!("Theme: {} ({})", name, mode_label), true, cx);
    }

    /// Set dark theme preset.
    pub(crate) fn set_dark_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_dark_id = index.min(self.registry.len() - 1);
        self.app_settings.theme_dark = self.registry.preset(self.theme_dark_id).id.to_string();
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        if !is_light {
            self.theme_id = self.theme_dark_id;
            let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
            self.theme_transition = Some((self.theme.clone(), target, 0.0));
            crate::rounded_corners::set_dark_mode(is_dark);
        }
        self.sync_and_save_settings();
        cx.notify();
    }

    /// Set light theme preset.
    pub(crate) fn set_light_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_light_id = index.min(self.registry.len() - 1);
        self.app_settings.theme_light = self.registry.preset(self.theme_light_id).id.to_string();
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        if is_light {
            self.theme_id = self.theme_light_id;
            let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
            self.theme_transition = Some((self.theme.clone(), target, 0.0));
            crate::rounded_corners::set_dark_mode(is_dark);
        }
        self.sync_and_save_settings();
        cx.notify();
    }

    /// Apply a registry preset to the active mode.
    #[allow(dead_code)]
    pub(crate) fn set_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        if is_light {
            self.set_light_theme(index, cx);
        } else {
            self.set_dark_theme(index, cx);
        }
    }

    pub(crate) fn set_theme_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.theme_mode = mode.to_string();
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        self.theme_id = if is_light { self.theme_light_id } else { self.theme_dark_id };
        let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        self.theme_transition = Some((self.theme.clone(), target, 0.0));
        crate::rounded_corners::set_dark_mode(is_dark);
        self.sync_and_save_settings();
        cx.notify();
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
        Profile::from_name(&self.user_profile.default_shell)
    }

    pub fn adjust_font_size(&mut self, new_size: f32, cx: &mut Context<Self>) {
        let clamped = new_size.clamp(8.0, 36.0);
        self.scroll_anim = None;
        self.font_size = clamped;
        self.app_settings.font_size = clamped;
        crate::config::save_app_settings(&self.app_settings);
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.font_size = Some(clamped);
            for pane in &mut tab.panes {
                pane.cursor_motion.snap_to(pane.cursor_motion.target_pos);
            }
        }
        self.show_update_toast(format!("Font size: {:.0}px", clamped), true, cx);
        cx.notify();
    }

    pub fn reset_font_size(&mut self, cx: &mut Context<Self>) {
        self.scroll_anim = None;
        let default_sz = crate::config::AppSettingsFile::default().font_size;
        self.font_size = default_sz;
        self.app_settings.font_size = default_sz;
        crate::config::save_app_settings(&self.app_settings);
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.font_size = None;
            for pane in &mut tab.panes {
                pane.cursor_motion.snap_to(pane.cursor_motion.target_pos);
            }
        }
        self.show_update_toast(format!("Font size reset: {:.0}px", default_sz), true, cx);
        cx.notify();
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

    pub fn select_all_viewport(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane() {
            let mut session = pane.session.borrow_mut();
            let offset = session.term.renderable_content().display_offset as i32;
            let screen_lines = session.term.screen_lines() as i32;
            let cols = session.cols.saturating_sub(1);
            let is_alt = session.term.mode().contains(TermMode::ALT_SCREEN);
            let active_lines = if is_alt || offset > 0 || session.max_active_row == 0 {
                screen_lines
            } else {
                (session.max_active_row as i32).min(screen_lines)
            };
            let start = Point::new(Line(0 - offset), Column(0));
            let end = Point::new(Line(active_lines.saturating_sub(1) - offset), Column(cols));
            let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
            sel.update(end, Side::Right);
            session.term.selection = Some(sel);
            cx.notify();
        }
    }

    pub fn open_buffer_urls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let urls = if let Some(pane) = self.active_pane() {
            crate::url::find_all_urls_in_viewport(&pane.session.borrow().term)
        } else {
            Vec::new()
        };

        if urls.is_empty() {
            self.show_update_toast("No URLs found in visible terminal buffer".to_string(), false, cx);
            return;
        }

        if urls.len() == 1 {
            let u = urls[0].clone();
            self.show_update_toast(format!("Opening {u}"), true, cx);
            crate::url::open_url(&u);
            self.focus_main(window);
        } else {
            self.palette_open = true;
            self.palette_closing = false;
            self.palette_query = "open link".to_string();
            self.palette_index = 0;
            self.palette_anim_index = 0.0;
            self.profile_menu = false;
            window.focus(&self.palette_focus);
            cx.notify();
        }
    }

    pub fn grid_coords_from_mouse(&self, mouse_pos: gpui::Point<Pixels>, pane_ix: usize) -> Option<(usize, usize)> {
        let tab = self.tabs.get(self.active)?;
        let pane = tab.panes.get(pane_ix)?;
        let session = pane.session.borrow();
        let (left, top, cell_width, cell_height) = session.grid_bounds?;
        let mx = f32::from(mouse_pos.x);
        let my = f32::from(mouse_pos.y);
        let cols = session.cols.max(1);
        let rows = session.rows.max(1);
        let col = (((mx - left) / cell_width).floor() as isize + 1).clamp(1, cols as isize) as usize;
        let row = (((my - top) / cell_height).floor() as isize + 1).clamp(1, rows as isize) as usize;
        Some((col, row))
    }

    pub fn point_from_mouse(&self, mouse_pos: gpui::Point<Pixels>, pane_ix: usize) -> Option<(Point, Side)> {
        let tab = self.tabs.get(self.active)?;
        let pane = tab.panes.get(pane_ix)?;
        let session = pane.session.borrow();
        let (left, top, cell_width, cell_height) = session.grid_bounds?;
        let mx = f32::from(mouse_pos.x);
        let my = f32::from(mouse_pos.y);
        let cols = session.cols.max(1);
        let rows = session.rows.max(1);

        let rel_y = my - top;
        let raw_row = (rel_y / cell_height).floor() as isize;
        let screen_row = raw_row.clamp(0, (rows.saturating_sub(1)) as isize) as i32;

        let rel_x = mx - left;
        let raw_col = (rel_x / cell_width).floor() as isize;
        let (col, side) = if raw_col < 0 {
            (0, Side::Left)
        } else if raw_col >= cols as isize {
            (cols.saturating_sub(1), Side::Right)
        } else {
            let c = raw_col as usize;
            let in_cell_x = rel_x - (c as f32 * cell_width);
            let side = if in_cell_x < (cell_width * 0.5) {
                Side::Left
            } else {
                Side::Right
            };
            (c, side)
        };

        let offset = session.term.renderable_content().display_offset as i32;
        let line = Line(screen_row - offset);
        Some((Point::new(line, Column(col)), side))
    }

    #[allow(dead_code)]
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
                pane.session.borrow_mut().clear_selection();
            }
            cx.notify();
        } else {
            // Paste from clipboard
            self.paste(cx);
            cx.notify();
        }
    }

    pub(crate) fn is_mouse_reporting(pane: &Pane) -> bool {
        let mode = *pane.session.borrow().term.mode();
        mode.intersects(
            TermMode::MOUSE_REPORT_CLICK
                | TermMode::MOUSE_DRAG
                | TermMode::MOUSE_MOTION,
        )
    }

    /// Format mouse escape sequences according to current terminal mode.
    /// Supports SGR mouse reporting mode (\x1b[<btn;col;rowM / \x1b[<btn;col;rowm)
    /// and standard xterm mouse mode fallback (\x1b[M{cb}{cx}{cy}).
    pub(crate) fn format_mouse_sequence(
        mode: TermMode,
        btn: u8,
        col: usize,
        row: usize,
        is_release: bool,
        modifiers: gpui::Modifiers,
    ) -> Vec<u8> {
        let is_sgr = mode.contains(TermMode::SGR_MOUSE);

        let mut mod_flags = 0;
        if modifiers.alt {
            mod_flags += 8;
        }
        if modifiers.control {
            mod_flags += 16;
        }

        if is_sgr {
            let final_btn = btn.saturating_add(mod_flags);
            let suffix = if is_release { 'm' } else { 'M' };
            format!("\x1b[<{final_btn};{col};{row}{suffix}").into_bytes()
        } else {
            let cb = if is_release {
                3 + mod_flags
            } else {
                btn + mod_flags
            };
            let b = 32u8.saturating_add(cb);
            let cx = 32u8.saturating_add((col as u8).min(223));
            let cy = 32u8.saturating_add((row as u8).min(223));
            vec![b'\x1b', b'[', b'M', b, cx, cy]
        }
    }

    /// Format and transmit mouse events to the PTY session.
    pub(crate) fn report_mouse_event(
        pane: &Pane,
        btn: u8,
        col: usize,
        row: usize,
        is_release: bool,
        modifiers: gpui::Modifiers,
    ) {
        let mode = *pane.session.borrow().term.mode();
        let bytes = Self::format_mouse_sequence(mode, btn, col, row, is_release, modifiers);
        pane.session.borrow_mut().write_bytes(&bytes);
    }

    pub(crate) fn detect_pane_bg(pane: &Pane, theme: &Theme) -> Rgba {
        let sess = pane.session.borrow();
        let over = *sess.term.colors();
        let (default_bg, _, _) = theme.defaults(&over);
        let rows = sess.rows;
        let cols = sess.cols;
        if rows < 3 || cols < 3 {
            return default_bg;
        }

        // Only detect custom background if the pane is an active full-screen TUI (alternate screen or OpenCode)
        let is_alt = sess.term.mode().contains(TermMode::ALT_SCREEN);
        let is_opencode = pane.profile == Profile::OpenCode
            || pane.title.starts_with("OC")
            || pane.title.to_lowercase().contains("opencode");
        if !is_alt && !is_opencode {
            return default_bg;
        }

        let content = sess.term.renderable_content();
        let mut counts: Vec<(alacritty_terminal::vte::ansi::Color, usize)> = Vec::new();
        let mut total_sampled = 0usize;

        // Sample the visible viewport, focusing on the middle 70% of rows and columns
        // to avoid top headers, tabs, and bottom status bars/command lines.
        let min_line = (rows as f32 * 0.15) as i32;
        let max_line = (rows as f32 * 0.85) as i32;
        let min_col = (cols as f32 * 0.15) as usize;
        let max_col = (cols as f32 * 0.85) as usize;

        for indexed in content.display_iter {
            let line = indexed.point.line.0;
            let col = indexed.point.column.0;
            if line >= min_line && line <= max_line && col >= min_col && col <= max_col {
                total_sampled += 1;
                let color = indexed.cell.bg;
                if let Some(entry) = counts.iter_mut().find(|(c, _)| *c == color) {
                    entry.1 += 1;
                } else {
                    counts.push((color, 1));
                }
            }
        }

        // Must cover at least 30% of the sampled viewport and at least 30 cells
        let min_threshold = (total_sampled * 30 / 100).max(30);
        if let Some((most_frequent_color, count)) = counts.into_iter().max_by_key(|(_, count)| *count) {
            if count >= min_threshold && most_frequent_color != alacritty_terminal::vte::ansi::Color::Named(alacritty_terminal::vte::ansi::NamedColor::Background) {
                return theme.resolve(most_frequent_color, false, &over);
            }
        }

        default_bg
    }

    pub(crate) fn is_pane_tui(pane: &Pane, _theme: &Theme) -> bool {
        let is_alt = pane.session.borrow().term.mode().contains(TermMode::ALT_SCREEN);
        if is_alt
            || pane.profile == Profile::OpenCode
            || pane.title.starts_with("OC")
            || pane.title.to_lowercase().contains("opencode")
        {
            return true;
        }
        false
    }

    pub(crate) fn handle_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // Solid cursor while typing; the motion ticker flips it back to breathing on its own.
        self.cursor_on = true;
        self.cursor_last_typing = std::time::Instant::now();

        // The close-confirmation dialog is modal: Enter confirms, Esc cancels,
        // and every other keystroke is swallowed instead of reaching the shell.
        if self.pending_close_tab.is_some() || self.pending_close_window {
            match key {
                "escape" | "esc" => self.cancel_pending_close(window, cx),
                "enter" | "return" => self.confirm_pending_close(window, cx),
                _ => {}
            }
            return;
        }

        // When What's New modal is open, route keystrokes to dismiss or block leaking into shell.
        if self.whats_new_open {
            if key == "escape" || key == "esc" || key == "enter" || key == "return" || key == "space" {
                self.close_whats_new(window, cx);
                return;
            }
            return;
        }

        // When search is active, route all keystrokes exclusively to the search handler.
        // Prevent any typed characters from leaking into the underlying shell PTY.
        if self.search_open {
            if modifiers.control && (key == "f" || key == "F") {
                self.close_search(window, cx);
                return;
            }
            self.handle_search_key(ks, window, cx);
            return;
        }

        // When palette is active, route all keystrokes exclusively to the palette handler.
        // Prevent any typed characters from leaking into the underlying shell PTY.
        if self.palette_open {
            if (modifiers.control && (key == "k" || key == "K"))
                || (modifiers.control && modifiers.shift && (key == "p" || key == "P"))
            {
                self.close_palette(window, cx);
                return;
            }
            self.handle_palette_key(ks, window, cx);
            return;
        }

        // Clear hovered link if control is released
        if self.hovered_link.is_some() && !modifiers.control {
            self.hovered_link = None;
            cx.notify();
        }

        if key == "escape" || key == "esc" {
            if self.search_open {
                self.close_search(window, cx);
                return;
            }
            if self.recording_shortcut.is_some() {
                self.recording_shortcut = None;
                cx.notify();
                return;
            }
            if self.settings_open {
                if !self.settings_search_query.is_empty() || self.settings_search_active {
                    self.settings_search_query.clear();
                    self.settings_search_active = false;
                    cx.notify();
                    return;
                }
                self.settings_open = false;
                window.focus(&self.focus);
                cx.notify();
                return;
            }
            if self.profile_menu {
                self.dismiss_profile(cx);
                cx.notify();
                return;
            }
            if self.update_panel_open {
                self.dismiss_update_panel(cx);
                cx.notify();
                return;
            }
            // Overlays checked: if none were active, fall through to terminal/shell!
        }

                // Handle Settings Live Search typing
        if self.settings_open {
            if let Some(chord) = format_keystroke_chord(ks) {
                if chord == "ctrl+f" {
                    self.settings_search_active = true;
                    cx.notify();
                    return;
                }
            }
            if self.settings_search_active {
                if key == "backspace" {
                    self.settings_search_query.pop();
                    cx.notify();
                    return;
                }
                if !ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform {
                    if let Some(c) = &ks.key_char {
                        self.settings_search_query.push_str(c);
                        cx.notify();
                        return;
                    } else if ks.key.chars().count() == 1 {
                        self.settings_search_query.push_str(&ks.key);
                        cx.notify();
                        return;
                    }
                }
            }
        }

        // Dynamic shortcuts matching
        if let Some(chord) = format_keystroke_chord(ks) {
            if chord == self.get_shortcut(ACTION_OPEN_SETTINGS) {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    self.settings_tab_open = true;
                    // Keep active settings tab and scroll position
                    window.focus(&self.settings_focus);
                } else {
                    window.focus(&self.focus);
                }
                cx.notify();
                return;
            }

            if !self.settings_open {
                // Palette first: Ctrl+K must always open the command palette,
                // even if a user remapped it onto another action's chord.
                if chord == self.get_shortcut(ACTION_PALETTE) || chord == "ctrl+k" || chord == "ctrl+shift+p" {
                    self.open_palette(window, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_FIND) || chord == "ctrl+f" {
                    self.open_search(window, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_TOGGLE_SIDEBAR) || chord == "ctrl+b" {
                    self.cycle_sidebar_mode(cx);
                    return;
                }
                // Browser & Kitty tab navigation chords:
                if chord == "ctrl+tab" || chord == "ctrl+shift+]" || chord == "ctrl+pagedown" || chord == self.get_shortcut(ACTION_NEXT_TAB) {
                    self.cycle(1);
                    self.trigger_island_hud(cx);
                    cx.notify();
                    return;
                }
                if chord == "ctrl+shift+tab" || chord == "ctrl+shift+[" || chord == "ctrl+pageup" || chord == self.get_shortcut(ACTION_PREV_TAB) {
                    self.cycle(-1);
                    self.trigger_island_hud(cx);
                    cx.notify();
                    return;
                }
                // New Window (Ctrl+Shift+N):
                if chord == "ctrl+shift+n" || chord == self.get_shortcut(ACTION_NEW_WINDOW) {
                    self.new_window(cx);
                    return;
                }
                // Browser & Terminal new tab (Ctrl+T):
                if chord == "ctrl+t" || chord == self.get_shortcut(ACTION_NEW_TAB) {
                    let prof = self.get_default_profile();
                    self.new_tab(prof, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                // Next Theme preset (Alt+T / Ctrl+Shift+T):
                if chord == "alt+t" || chord == "ctrl+shift+t" || chord == self.get_shortcut(ACTION_NEXT_THEME) {
                    self.next_theme(cx);
                    self.trigger_island_hud(cx);
                    return;
                }
                // Previous Theme preset (Alt+Shift+T):
                if chord == "alt+shift+t" || chord == self.get_shortcut(ACTION_PREV_THEME) {
                    self.prev_theme(cx);
                    self.trigger_island_hud(cx);
                    return;
                }
                // Toggle Dark / Light mode (Ctrl+Shift+L / Alt+Shift+D):
                if chord == "ctrl+shift+l"
                    || chord == "alt+shift+d"
                    || chord == self.get_shortcut(ACTION_TOGGLE_DARK_LIGHT)
                    || (chord == self.get_shortcut(ACTION_TOGGLE_THEME) && chord != "alt+t" && chord != "ctrl+shift+t")
                {
                    self.toggle_theme(cx);
                    self.trigger_island_hud(cx);
                    return;
                }
                if chord == "ctrl+w" || chord == "ctrl+shift+w" || chord == "alt+w" || chord == "alt+q" || chord == self.get_shortcut(ACTION_CLOSE_TAB) {
                    self.close_pane(cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                // Linux Tiling & Kitty split pane chords (Alt+Enter / Alt+V / Ctrl+Shift+Enter / Ctrl+Shift+E):
                let is_split_vertical = chord == "alt+enter"
                    || chord == "alt+v"
                    || chord == "ctrl+shift+enter"
                    || chord == "ctrl+shift+e"
                    || chord == self.get_shortcut(ACTION_SPLIT_VERTICAL)
                    || (modifiers.alt && (key == "enter" || key == "return" || key == "\r" || key == "\n"));
                if is_split_vertical {
                    self.split_active(SplitAxis::Columns, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }

                let is_split_horizontal = chord == "alt+d"
                    || chord == "ctrl+shift+d"
                    || chord == "ctrl+shift+o"
                    || chord == "alt+shift+h"
                    || chord == self.get_shortcut(ACTION_SPLIT_HORIZONTAL)
                    || (modifiers.alt && (key == "d" || key == "D"));
                if is_split_horizontal {
                    self.split_active(SplitAxis::Rows, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                // Tiling layout axis toggle (Columns ↔ Rows):
                if chord == "alt+space" || chord == "alt+s" || chord == "ctrl+shift+\\" || chord == "ctrl+\\" {
                    self.toggle_split_axis(cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                // Tiling pane directional navigation (Hyprland spatial: Alt+H/J/K/L + Alt+Arrows):
                if chord == "alt+l" || chord == "alt+right" || chord == self.get_shortcut(ACTION_FOCUS_NEXT_PANE) {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            if let Some(next) = tab.tile_tree.find_neighbor(tab.active_pane, crate::tiling::Direction::Right) {
                                tab.active_pane = next;
                            } else {
                                tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                            }
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == "alt+h" || chord == "alt+left" || chord == self.get_shortcut(ACTION_FOCUS_PREV_PANE) {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            if let Some(prev) = tab.tile_tree.find_neighbor(tab.active_pane, crate::tiling::Direction::Left) {
                                tab.active_pane = prev;
                            } else {
                                tab.active_pane = if tab.active_pane == 0 { tab.panes.len() - 1 } else { tab.active_pane - 1 };
                            }
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == "alt+k" || chord == "alt+up" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            if let Some(up) = tab.tile_tree.find_neighbor(tab.active_pane, crate::tiling::Direction::Up) {
                                tab.active_pane = up;
                                cx.notify();
                                return;
                            }
                        }
                    }
                }
                if chord == "alt+j" || chord == "alt+down" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            if let Some(down) = tab.tile_tree.find_neighbor(tab.active_pane, crate::tiling::Direction::Down) {
                                tab.active_pane = down;
                                cx.notify();
                                return;
                            }
                        }
                    }
                }
                // Tiling pane resizing (Alt+Ctrl+Arrows, Alt+Ctrl+H/J/K/L, Alt+Shift+Arrows):
                if chord == "ctrl+alt+left" || chord == "ctrl+alt+h" || chord == "alt+shift+left" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.tile_tree.adjust_active_ratio(tab.active_pane, -0.05);
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == "ctrl+alt+right" || chord == "ctrl+alt+l" || chord == "alt+shift+right" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.tile_tree.adjust_active_ratio(tab.active_pane, 0.05);
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == "ctrl+alt+up" || chord == "ctrl+alt+k" || chord == "alt+shift+up" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.tile_tree.adjust_active_ratio(tab.active_pane, -0.05);
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                if chord == "ctrl+alt+down" || chord == "ctrl+alt+j" || chord == "alt+shift+down" {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.tile_tree.adjust_active_ratio(tab.active_pane, 0.05);
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                // Tiling pane zoom / maximize (Alt+Z / Alt+F / Alt+M / Ctrl+Shift+M):
                if chord == "alt+z" || chord == "alt+f" || chord == "alt+m" || chord == "ctrl+shift+m" || chord == self.get_shortcut(ACTION_TOGGLE_MAXIMIZE) {
                    self.toggle_maximize(cx);
                    self.focus_main(window);
                    return;
                }
                // Shell / Profile Switcher shortcuts:
                if chord == self.get_shortcut(ACTION_PROFILE_MENU) || chord == "ctrl+shift+p" || chord == "ctrl+alt+t" {
                    self.toggle_profile_menu(cx);
                    self.trigger_island_hud(cx);
                    return;
                }
                // Direct Profile Tab Launchers:
                if chord == self.get_shortcut(ACTION_NEW_TAB_POWERSHELL)
                    || chord == "ctrl+shift+1"
                    || chord == "ctrl+shift+!"
                    || chord == "ctrl+alt+1"
                    || (modifiers.control && modifiers.shift && (key == "1" || key == "!"))
                {
                    self.new_tab(Profile::PowerShell, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEW_TAB_WSL)
                    || chord == "ctrl+shift+2"
                    || chord == "ctrl+shift+@"
                    || chord == "ctrl+alt+2"
                    || (modifiers.control && modifiers.shift && (key == "2" || key == "@"))
                {
                    self.new_tab(Profile::Wsl, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEW_TAB_CMD)
                    || chord == "ctrl+shift+3"
                    || chord == "ctrl+shift+#"
                    || chord == "ctrl+alt+3"
                    || (modifiers.control && modifiers.shift && (key == "3" || key == "#"))
                {
                    self.new_tab(Profile::Cmd, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEW_TAB_GITBASH)
                    || chord == "ctrl+shift+4"
                    || chord == "ctrl+shift+$"
                    || chord == "ctrl+alt+4"
                    || (modifiers.control && modifiers.shift && (key == "4" || key == "$"))
                {
                    self.new_tab(Profile::GitBash, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEW_TAB_OPENCODE)
                    || chord == "ctrl+shift+5"
                    || chord == "ctrl+shift+%"
                    || chord == "ctrl+alt+5"
                    || (modifiers.control && modifiers.shift && (key == "5" || key == "%"))
                {
                    self.new_tab(Profile::OpenCode, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
                    return;
                }
                if chord == self.get_shortcut(ACTION_NEW_TAB_ANTIGRAVITY)
                    || chord == "ctrl+shift+6"
                    || chord == "ctrl+shift+^"
                    || chord == "ctrl+alt+6"
                    || (modifiers.control && modifiers.shift && (key == "6" || key == "^"))
                {
                    self.new_tab(Profile::Antigravity, cx);
                    self.trigger_island_hud(cx);
                    self.focus_main(window);
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
                    let cur = self.active_font_size();
                    self.adjust_font_size(cur + 1.0, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_OUT) {
                    let cur = self.active_font_size();
                    self.adjust_font_size(cur - 1.0, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_RESET) {
                    self.reset_font_size(cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_SELECT_ALL) {
                    self.select_all_viewport(cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_LINE_UP)
                    || chord == "ctrl+shift+up"
                    || chord == "shift+up"
                {
                    self.scroll(1);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_LINE_DOWN)
                    || chord == "ctrl+shift+down"
                    || chord == "shift+down"
                {
                    self.scroll(-1);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_PAGE_UP) {
                    self.scroll(15);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_PAGE_DOWN) {
                    self.scroll(-15);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_TO_TOP) {
                    self.scroll(100_000);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_SCROLL_TO_BOTTOM) {
                    self.scroll(-100_000);
                    cx.notify();
                    return;
                }
                if chord == self.get_shortcut(ACTION_OPEN_URL_PICKER) {
                    self.open_buffer_urls(window, cx);
                    return;
                }
            }
        }

        // Mouseless shortcut: Ctrl+Tab and Ctrl+Shift+Tab
        if modifiers.control && !modifiers.alt {
            if key == "tab" {
                if modifiers.shift {
                    self.cycle(-1);
                } else {
                    self.cycle(1);
                }
                self.trigger_island_hud(cx);
                cx.notify();
                return;
            }
        }

        // Mouseless shortcut: Ctrl+1 through Ctrl+9 to switch tabs directly
        if modifiers.control && !modifiers.shift && !modifiers.alt {
            if let Ok(num) = key.parse::<usize>() {
                if (1..=9).contains(&num) {
                    let target = if num == 9 {
                        self.tabs.len().saturating_sub(1)
                    } else {
                        num - 1
                    };
                    if target < self.tabs.len() {
                        self.active = target;
                        self.settings_open = false;
                        self.trigger_island_hud(cx);
                        self.focus_main(window);
                        cx.notify();
                        return;
                    }
                }
            }

            match key {
                "pageup" => {
                    self.cycle(-1);
                    self.trigger_island_hud(cx);
                    cx.notify();
                    return;
                }
                "pagedown" => {
                    self.cycle(1);
                    self.trigger_island_hud(cx);
                    cx.notify();
                    return;
                }
                "0" => {
                    self.reset_font_size(cx);
                    return;
                }
                _ => {}
            }
        }

        // Mouseless shortcut: Alt+Arrows (only when Ctrl is NOT held) to cycle focus between split panes
        if modifiers.alt && !modifiers.control && !modifiers.shift {
            match key {
                "arrowright" | "arrowdown" | "right" | "down" => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                "arrowleft" | "arrowup" | "left" | "up" => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = if tab.active_pane == 0 {
                                tab.panes.len() - 1
                            } else {
                                tab.active_pane - 1
                            };
                            self.trigger_island_hud(cx);
                            cx.notify();
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        // Standard terminal behavior: Ctrl+C copies if there is an active text selection,
        // otherwise falls through to the keymap (SIGINT); Ctrl+V always pastes.
        if modifiers.control && !modifiers.shift && !modifiers.alt {
            match key {
                "c" => {
                    let has_selection = self.active_pane().map(|p| {
                        let s = p.session.borrow();
                        s.term.selection.is_some()
                            && s.term.selection_to_string().map(|t| !t.is_empty()).unwrap_or(false)
                    }).unwrap_or(false);
                    if has_selection {
                        self.copy_selection(cx);
                        if let Some(p) = self.active_pane() {
                            p.session.borrow_mut().clear_selection();
                        }
                        cx.notify();
                        return;
                    }
                    // No selection: let Ctrl+C reach `keymap::to_bytes` as \x03.
                }
                "v" => {
                    self.paste(cx);
                    self.focus_main(window);
                    return;
                }
                _ => {}
            }
        }

        // Escape: deselect text if active
        if (key == "escape" || key == "esc") && !modifiers.control && !modifiers.alt && !modifiers.shift {
            if let Some(pane) = self.active_pane() {
                if pane.session.borrow().term.selection.is_some() {
                    pane.session.borrow_mut().clear_selection();
                    cx.notify();
                    return;
                }
            }
        }

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.next_theme(cx),
                "l" => self.toggle_theme(cx),
                "m" => {
                    self.toggle_maximize(cx);
                    self.focus_main(window);
                }
                "p" => self.open_palette(window, cx),
                "o" => self.open_buffer_urls(window, cx),
                "a" => self.select_all_viewport(cx),
                "arrowup" | "up" => {
                    self.scroll(1);
                    cx.notify();
                    return;
                }
                "arrowdown" | "down" => {
                    self.scroll(-1);
                    cx.notify();
                    return;
                }
                "{" | "[" => {
                    self.cycle(-1);
                    cx.notify();
                    return;
                }
                "}" | "]" => {
                    self.cycle(1);
                    cx.notify();
                    return;
                }
                _ => {}
            }
            return;
        }

        if modifiers.shift && !modifiers.control && !modifiers.alt {
            match key {
                "pageup" => {
                    self.scroll(15);
                    cx.notify();
                    return;
                }
                "pagedown" => {
                    self.scroll(-15);
                    cx.notify();
                    return;
                }
                "home" => {
                    self.scroll(100_000);
                    cx.notify();
                    return;
                }
                "end" => {
                    self.scroll(-100_000);
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


        // Ghost text acceptance: Right Arrow, Tab, or End accepts the active inline suggestion
        if !modifiers.control && !modifiers.alt && (key == "arrowright" || key == "right" || key == "tab" || key == "end") {
            let ghost_opt = self.active_ghost_suggestion.borrow_mut().take();
            if let Some(ghost) = ghost_opt {
                let bytes = ghost.into_bytes();
                let ink_enabled = self.app_settings.ascii_ink;
                let letter_anim = self.app_settings.letter_animations;
                let ink_size = self.app_settings.ascii_ink_size.clone();

                if ink_enabled || letter_anim {
                    if let Some(pane) = self.active_pane_mut() {
                        let (cur_col, cur_row, cols, rows) = {
                            let sess = pane.session.borrow();
                            let content = sess.term.renderable_content();
                            let offset = content.display_offset as i32;
                            let r = (content.cursor.point.line.0 + offset).max(0) as usize;
                            let c = content.cursor.point.column.0;
                            (c, r, sess.cols, sess.rows)
                        };
                        pane.ascii_ink.resize(cols, rows);
                        pane.ascii_ink.set_scale_from_name(&ink_size);
                        if ink_enabled {
                            pane.ascii_ink.splash(cur_col as f32, cur_row as f32, 1.2, 1);
                        }
                        if letter_anim {
                            pane.ascii_ink.pop_letter(cur_col, cur_row);
                            pane.ascii_ink.jostle_line(cur_col, cur_row);
                        }
                    }
                }
                self.write_to_shell(&bytes);
                cx.notify();
                return;
            }
        }

        // Escape: clear ghost suggestion
        if (key == "escape" || key == "esc") && !modifiers.control && !modifiers.alt && !modifiers.shift {
            if self.active_ghost_suggestion.borrow().is_some() {
                *self.active_ghost_suggestion.borrow_mut() = None;
                cx.notify();
                return;
            }
        }

        let mode = self.active_mode();
        if let Some(bytes) = keymap::to_bytes(ks, mode) {
            let ink_enabled = self.app_settings.ascii_ink;
            let letter_anim = self.app_settings.letter_animations;
            let ink_size = self.app_settings.ascii_ink_size.clone();

            if ink_enabled || letter_anim {
                if let Some(pane) = self.active_pane_mut() {
                    let (cur_col, cur_row, cols, rows) = {
                        let sess = pane.session.borrow();
                        let content = sess.term.renderable_content();
                        let offset = content.display_offset as i32;
                        let r = (content.cursor.point.line.0 + offset).max(0) as usize;
                        let c = content.cursor.point.column.0;
                        (c, r, sess.cols, sess.rows)
                    };
                    pane.ascii_ink.resize(cols, rows);
                    pane.ascii_ink.set_scale_from_name(&ink_size);

                    let is_backspace = key == "backspace" || bytes == [0x08] || bytes == [0x7f];
                    let is_enter = key == "enter" || key == "return" || bytes == [b'\r'] || bytes == [b'\n'];

                    if is_backspace {
                        if ink_enabled {
                            let target_col = cur_col.saturating_sub(1) as f32;
                            pane.ascii_ink.dissolve(target_col, cur_row as f32);
                        }
                    } else if is_enter {
                        if ink_enabled {
                            pane.ascii_ink.streak(cur_col as f32, cur_row as f32, 0.0, (cur_row + 1) as f32, 1);
                        }
                    } else if !modifiers.control && !modifiers.alt && (key.chars().count() == 1 || bytes.iter().all(|&b| b >= 32)) {
                        if ink_enabled {
                            pane.ascii_ink.splash(cur_col as f32, cur_row as f32, 1.0, 2);
                        }
                        if letter_anim {
                            pane.ascii_ink.pop_letter(cur_col, cur_row);
                            pane.ascii_ink.jostle_line(cur_col, cur_row);
                        }
                    }
                }
            }

            self.write_to_shell(&bytes);
            cx.notify();
        }
    }
}
