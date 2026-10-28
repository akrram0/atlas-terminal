//! Keyboard chords, mouse drag selection, coordinate hit-testing, and clipboard bridging.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use gpui::{ClipboardItem, Context, Keystroke, Pixels, Rgba, Window};

use crate::config::{
    default_shortcuts, format_keystroke_chord, SettingsTab, ACTION_CLEAR_SCROLLBACK,
    ACTION_CLOSE_TAB, ACTION_COPY, ACTION_FIND, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE,
    ACTION_FULLSCREEN, ACTION_NEW_TAB, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_OPEN_URL_PICKER,
    ACTION_PALETTE, ACTION_PASTE, ACTION_PREV_TAB, ACTION_SCROLL_PAGE_DOWN, ACTION_SCROLL_PAGE_UP,
    ACTION_SCROLL_TO_BOTTOM, ACTION_SCROLL_TO_TOP, ACTION_SELECT_ALL, ACTION_SPLIT_HORIZONTAL,
    ACTION_SPLIT_VERTICAL, ACTION_TOGGLE_MAXIMIZE, ACTION_TOGGLE_THEME, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT, ACTION_ZOOM_RESET,
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
        Profile::from_name(&self.app_settings.default_shell)
    }

    pub fn adjust_font_size(&mut self, new_size: f32, cx: &mut Context<Self>) {
        self.font_size = new_size.clamp(10.0, 24.0);
        self.scroll_anim = None;
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.cursor_pos = None;
            }
        }
        self.sync_and_save_settings();
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
            let start = Point::new(Line(0 - offset), Column(0));
            let end = Point::new(Line(screen_lines.saturating_sub(1) - offset), Column(cols));
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
            return;
        }

        if urls.len() == 1 {
            crate::url::open_url(&urls[0]);
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

    pub(crate) fn is_mouse_reporting(pane: &Pane) -> bool {
        let mode = *pane.session.borrow().term.mode();
        mode.intersects(
            TermMode::MOUSE_REPORT_CLICK
                | TermMode::MOUSE_DRAG
                | TermMode::MOUSE_MOTION,
        )
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

        let content = sess.term.renderable_content();
        let mut counts: Vec<(alacritty_terminal::vte::ansi::Color, usize)> = Vec::new();

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
                let color = indexed.cell.bg;
                if let Some(entry) = counts.iter_mut().find(|(c, _)| *c == color) {
                    entry.1 += 1;
                } else {
                    counts.push((color, 1));
                }
            }
        }

        if let Some((most_frequent_color, count)) = counts.into_iter().max_by_key(|(_, count)| *count) {
            if count >= 10 && most_frequent_color != alacritty_terminal::vte::ansi::Color::Named(alacritty_terminal::vte::ansi::NamedColor::Background) {
                return theme.resolve(most_frequent_color, false, &over);
            }
        }

        default_bg
    }

    pub(crate) fn is_pane_tui(pane: &Pane, theme: &Theme) -> bool {
        if pane.session.borrow().term.mode().contains(TermMode::ALT_SCREEN)
            || pane.profile == Profile::OpenCode
            || pane.title.starts_with("OC")
            || pane.title.to_lowercase().contains("opencode")
        {
            return true;
        }
        // Content heuristic: if the pane displays content with an explicitly detected non-default background color
        let (default_bg, _, _) = theme.defaults(pane.session.borrow().term.colors());
        let detected_bg = Self::detect_pane_bg(pane, theme);
        if detected_bg != default_bg && detected_bg.a > 0.95 {
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
                "escape" => self.cancel_pending_close(window, cx),
                "enter" => self.confirm_pending_close(window, cx),
                _ => {}
            }
            return;
        }

        // Clear hovered link if control is released
        if self.hovered_link.is_some() && !modifiers.control {
            self.hovered_link = None;
            cx.notify();
        }

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
                    self.settings_tab_open = true;
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
                    self.adjust_font_size(self.font_size + 1.0, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_OUT) {
                    self.adjust_font_size(self.font_size - 1.0, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_ZOOM_RESET) {
                    self.adjust_font_size(self.app_settings.font_size, cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_SELECT_ALL) {
                    self.select_all_viewport(cx);
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
                if chord == self.get_shortcut(ACTION_TOGGLE_THEME) {
                    self.toggle_theme(cx);
                    return;
                }
                if chord == self.get_shortcut(ACTION_TOGGLE_MAXIMIZE) {
                    self.toggle_maximize(cx);
                    self.focus_main(window);
                    return;
                }
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
                        self.focus_main(window);
                        cx.notify();
                        return;
                    }
                }
            }

            match key {
                "pageup" => {
                    self.cycle(-1);
                    cx.notify();
                    return;
                }
                "pagedown" => {
                    self.cycle(1);
                    cx.notify();
                    return;
                }
                "0" => {
                    self.adjust_font_size(self.app_settings.font_size, cx);
                    return;
                }
                _ => {}
            }
        }

        // Mouseless shortcut: Alt+Arrows (and Ctrl+Alt+Arrows) to cycle focus between split panes
        if modifiers.alt && !modifiers.shift {
            match key {
                "arrowright" | "arrowdown" | "right" | "down" => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.panes.len() > 1 {
                            tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
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
                            p.session.borrow_mut().term.selection = None;
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

        let mode = self.active_mode();
        if let Some(bytes) = keymap::to_bytes(ks, mode) {
            self.write_to_shell(&bytes);
            cx.notify();
        }
    }
}
