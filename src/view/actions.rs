//! Tab and pane lifecycle, process spawning, split views, and window actions.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::TermMode;
use gpui::prelude::*;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement, KeyDownEvent, Keystroke,
    MouseButton, Rgba, Window, div, px, rgb, rgba, svg,
};

use crate::session::{Profile, Session, SessionMsg};
use crate::theme::Theme;
use crate::ui::animation::OVERLAY_OUT_MS;
use crate::workspace::{home_dir, profile_title, Pane, SplitAxis, Tab};

use super::AtlasView;

impl AtlasView {
    /// Grid size to spawn a new PTY at: the active pane's current grid when
    /// one exists, so a fresh shell starts at (roughly) its final width
    /// instead of the classic 80x24 and reflowing under its first prompt.
    pub(crate) fn default_grid_size(&self) -> (usize, usize) {
        self.active_pane()
            .map(|pane| {
                let session = pane.session.borrow();
                (session.cols, session.rows)
            })
            .filter(|(cols, rows)| *cols > 0 && *rows > 0)
            .unwrap_or_else(|| {
                // Compute initial grid columns/rows from the default window bounds (1100x720)
                // and font metrics so the child shell never spawns in an undersized 80x24 box.
                let cell_w = 7.8f32;
                let cell_h = (self.font_size * self.line_height).round().max(1.0);
                let cols = ((1100.0 - 16.0) / cell_w).floor().max(80.0) as usize;
                let rows = ((720.0 - 70.0) / cell_h).floor().max(35.0) as usize;
                (cols, rows)
            })
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

        let spawn_cwd = cwd_override.or_else(|| {
            // "Start directory" setting: home (default) or the active pane's
            // current directory. An explicit override (CLI directory) wins.
            let start_current = self.app_settings.start_directory.eq_ignore_ascii_case("current");
            if start_current {
                self.active_pane()
                    .and_then(|pane| {
                        pane.session
                            .borrow()
                            .pid
                            .and_then(crate::cwd::process_cwd)
                            .or_else(|| pane.cwd.clone())
                    })
                    .or_else(|| std::env::current_dir().ok())
                    .or_else(home_dir)
            } else {
                home_dir()
            }
        });
        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(
            init_cols,
            init_rows,
            spawn_cwd.clone(),
            profile,
            self.app_settings.scrollback_lines,
        ) {
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
                        cmd_start: None,
                        running_cmd: None,
                    }],
                    active_pane: 0,
                    axis: SplitAxis::Columns,
                    maximized: false,
                    closing: false,
                    close_progress: 0.0,
                    close_hover_progress: 0.0,
                });
                self.active = self.tabs.len() - 1;
                self.save_session();
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
        // "Confirm before closing active process": hold the close until the
        // dialog is answered. `close_tab_now` is the unconditional path used by
        // the dialog and by non-interactive callers.
        if self.app_settings.confirm_close_process && self.tab_is_live(index) {
            self.pending_close_tab = Some(index);
            self.pending_close_window = false;
            cx.notify();
            return;
        }
        self.close_tab_now(index, cx);
    }

    /// Whether the tab still owns a shell that has not exited.
    pub(crate) fn tab_is_live(&self, index: usize) -> bool {
        self.tabs
            .get(index)
            .map(|tab| tab.panes.iter().any(|pane| !pane.exited))
            .unwrap_or(false)
    }

    /// Whether any tab in the window still owns a live shell.
    pub(crate) fn has_live_processes(&self) -> bool {
        self.tabs
            .iter()
            .any(|tab| tab.panes.iter().any(|pane| !pane.exited))
    }

    pub(crate) fn close_tab_now(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let open_count = self.tabs.iter().filter(|t| !t.closing).count();
        if open_count <= 1 {
            self.tabs.remove(index);
            self.hover_tab = None;
            if self.tabs.is_empty() {
                let profile = self.get_default_profile();
                self.new_tab(profile, cx);
                return;
            }
            if index < self.active {
                self.active = self.active.saturating_sub(1);
            }
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            }
            self.save_session();
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
        self.save_session();
        cx.notify();
    }

    /// Answer "yes" to the close-confirmation dialog.
    pub(crate) fn confirm_pending_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_close_window {
            self.save_session();
            self.pending_close_window = false;
            self.pending_close_tab = None;
            window.remove_window();
        } else if let Some(index) = self.pending_close_tab.take() {
            self.close_tab_now(index, cx);
            self.focus_main(window);
        }
    }

    /// Dismiss the close-confirmation dialog without closing anything.
    pub(crate) fn cancel_pending_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_close_tab = None;
        self.pending_close_window = false;
        self.focus_main(window);
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
        match Session::spawn(
            init_cols,
            init_rows,
            home_dir(),
            profile,
            self.app_settings.scrollback_lines,
        ) {
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
            Some(pane) => {
                let live_cwd = pane.session.borrow().pid.and_then(crate::cwd::process_cwd);
                (pane.profile, live_cwd.or_else(|| pane.cwd.clone()))
            }
            None => return,
        };
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        let spawn_cwd = inherit_cwd.or_else(home_dir);
        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(
            init_cols,
            init_rows,
            spawn_cwd.clone(),
            profile,
            self.app_settings.scrollback_lines,
        ) {
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
                        cmd_start: None,
                        running_cmd: None,
                    });
                    tab.active_pane = tab.panes.len() - 1;
                    tab.axis = axis;
                    tab.maximized = false;
                }
                self.save_session();
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
        self.save_session();
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
    pub(crate) fn watch(
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
                            let trimmed_new = title.trim();
                            if !trimmed_new.is_empty() {
                                let default_title = profile_title(pane.profile);
                                let is_shell_prompt = trimmed_new.eq_ignore_ascii_case("powershell")
                                    || trimmed_new.eq_ignore_ascii_case("pwsh")
                                    || trimmed_new.eq_ignore_ascii_case("cmd")
                                    || trimmed_new.eq_ignore_ascii_case("cmd.exe")
                                    || trimmed_new.eq_ignore_ascii_case(&default_title)
                                    || (trimmed_new.contains('@') && trimmed_new.contains(':'));

                                if is_shell_prompt {
                                    if let Some(start) = pane.cmd_start.take() {
                                        let elapsed = start.elapsed().as_secs();
                                        let cmd_name = pane.running_cmd.take().unwrap_or_else(|| "Command".to_string());
                                        if elapsed >= view.app_settings.notify_threshold_secs
                                            && view.app_settings.notify_on_complete
                                            && !crate::rounded_corners::is_active()
                                        {
                                            crate::platform::send_desktop_notification(
                                                "Atlas Terminal",
                                                &format!("{cmd_name} finished in {elapsed}s"),
                                            );
                                        }
                                    }
                                } else if pane.cmd_start.is_none() {
                                    pane.cmd_start = Some(std::time::Instant::now());
                                    pane.running_cmd = Some(trimmed_new.to_string());
                                }
                                pane.title = title;
                            }
                        }
                        SessionMsg::Event(Event::Exit) => {
                            pane.exited = true;
                            if let Some(start) = pane.cmd_start.take() {
                                let elapsed = start.elapsed().as_secs();
                                let cmd_name = pane.running_cmd.take().unwrap_or_else(|| "Process".to_string());
                                if elapsed >= view.app_settings.notify_threshold_secs
                                    && view.app_settings.notify_on_complete
                                    && !crate::rounded_corners::is_active()
                                {
                                    crate::platform::send_desktop_notification(
                                        "Atlas Terminal",
                                        &format!("{cmd_name} exited in {elapsed}s"),
                                    );
                                }
                            }
                        }
                        SessionMsg::Event(Event::TextAreaSizeRequest(formatter)) => {
                            let (cols, rows, cell_w, cell_h) = {
                                let s = pane.session.borrow();
                                (
                                    s.cols as u16,
                                    s.rows as u16,
                                    s.cell_width.round().max(1.0) as u16,
                                    s.cell_height.round().max(1.0) as u16,
                                )
                            };
                            let reply = formatter(alacritty_terminal::event::WindowSize {
                                num_lines: rows,
                                num_cols: cols,
                                cell_width: cell_w,
                                cell_height: cell_h,
                            });
                            pane.session.borrow_mut().write_bytes(reply.as_bytes());
                        }
                        SessionMsg::Event(Event::PtyWrite(s)) => {
                            pane.session.borrow_mut().write_bytes(s.as_bytes());
                        }
                        SessionMsg::Event(Event::ColorRequest(index, formatter)) => {
                            // OSC 4/10/11/12 colour query (`\e]11;?`): answe
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

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// Pane that owns the keyboard and the status bar.
    pub(crate) fn active_pane(&self) -> Option<&Pane> {
        self.tabs
            .get(self.active)
            .and_then(|tab| tab.panes.get(tab.active_pane))
    }

    pub(crate) fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let active = self.active;
        self.tabs
            .get_mut(active)
            .and_then(|tab| {
                let p = tab.active_pane;
                tab.panes.get_mut(p)
            })
    }

    pub(crate) fn active_mode(&self) -> TermMode {
        self.active_pane()
            .map(|pane| *pane.session.borrow().term.mode())
            .unwrap_or_else(TermMode::empty)
    }

    pub(crate) fn write_to_shell(&self, bytes: &[u8]) {
        if let Some(pane) = self.active_pane() {
            pane.session.borrow_mut().write_bytes(bytes);
        }
    }

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

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        cx.notify();
    }

    pub(crate) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.settings_tab_open = true;
            self.settings_tab = crate::config::SettingsTab::General;
            self.settings_scroll_pos = 0.0;
            self.settings_scroll_target = 0.0;
            self.settings_scroll_handle.set_offset(gpui::Point::default());
            window.focus(&self.settings_focus);
        } else {
            window.focus(&self.focus);
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

    /// Modal confirmation shown when `confirm_close_process` is on and the
    /// target tab (or the window) still has a live shell.
    pub(crate) fn confirm_close_dialog(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let window_close = self.pending_close_window;
        let title = if window_close {
            "Close Atlas Terminal?"
        } else {
            "Close running tab?"
        };
        let body = if window_close {
            "One or more shells are still running. Close the window anyway?"
        } else {
            "This tab is still running. Close it anyway?"
        };
        let confirm_label = if window_close { "Close window" } else { "Close tab" };

        let cancel = div()
            .id(ElementId::named_usize("confirm-close-cancel", 0))
            .h(px(28.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.elevated_border)
            .bg(theme.card_hover)
            .text_size(px(12.0))
            .text_color(theme.chrome_fg_active)
            .hover({
                let hover = theme.card_hover;
                move |style| style.bg(hover)
            })
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.cancel_pending_close(window, cx);
            }))
            .child("Cancel");

        let confirm = div()
            .id(ElementId::named_usize("confirm-close-ok", 0))
            .h(px(28.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.accent)
            .bg(Rgba {
                a: 0.22,
                ..theme.accent
            })
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.accent)
            .hover({
                let hover = Rgba {
                    a: 0.32,
                    ..theme.accent
                };
                move |style| style.bg(hover)
            })
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.confirm_pending_close(window, cx);
            }))
            .child(confirm_label);

        div()
            .absolute()
            .top(px(140.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("confirm-close-panel", 0))
                    .w(px(400.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(16.0))
                    .rounded(px(12.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.chrome_fg_active)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.chrome_fg)
                            .child(body),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.0))
                            .child(cancel)
                            .child(confirm),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn handle_file_drop(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let is_wsl = self
            .active_pane()
            .map(|p| matches!(p.profile, Profile::Wsl))
            .unwrap_or(false);
        let formatted = crate::workspace::format_dropped_paths(paths, is_wsl);
        if !formatted.is_empty() {
            self.paste_text(&formatted, cx);
        }
    }

    pub(crate) fn save_session(&self) {
        let tabs: Vec<crate::workspace::SavedTab> = self
            .tabs
            .iter()
            .filter(|t| !t.closing)
            .map(|t| {
                let panes = t
                    .panes
                    .iter()
                    .filter(|p| !p.closing)
                    .map(|p| {
                        let cwd = p
                            .session
                            .borrow()
                            .pid
                            .and_then(crate::cwd::process_cwd)
                            .or_else(|| p.cwd.clone());
                        crate::workspace::SavedPane {
                            profile: p.profile.as_str().to_string(),
                            cwd,
                        }
                    })
                    .collect();
                let axis = if t.axis == SplitAxis::Rows {
                    "rows".to_string()
                } else {
                    "columns".to_string()
                };
                crate::workspace::SavedTab {
                    custom_title: t.custom_title.clone(),
                    active_pane: t.active_pane,
                    axis,
                    panes,
                }
            })
            .collect();

        if tabs.is_empty() {
            return;
        }

        let saved = crate::workspace::SavedSession {
            active_tab: self.active,
            tabs,
        };

        if let Ok(json) = serde_json::to_string_pretty(&saved) {
            let session_path = crate::user_data_dir().join("sessions").join("last_session.json");
            let _ = std::fs::write(session_path, json);
        }
    }

    pub(crate) fn restore_session(&mut self, cx: &mut Context<Self>) -> bool {
        let session_path = crate::user_data_dir().join("sessions").join("last_session.json");
        let Ok(data) = std::fs::read_to_string(&session_path) else {
            return false;
        };
        let Ok(saved) = serde_json::from_str::<crate::workspace::SavedSession>(&data) else {
            return false;
        };
        if saved.tabs.is_empty() {
            return false;
        }

        let (init_cols, init_rows) = self.default_grid_size();
        let mut restored_tabs = Vec::new();

        for saved_tab in saved.tabs {
            let tab_id = self.next_id;
            self.next_id += 1;
            let mut restored_panes = Vec::new();

            for saved_pane in saved_tab.panes {
                let epoch = self.next_epoch;
                self.next_epoch += 1;
                let profile = Profile::from_name(&saved_pane.profile);
                let cwd = saved_pane.cwd;

                match Session::spawn(
                    init_cols,
                    init_rows,
                    cwd.clone(),
                    profile,
                    self.app_settings.scrollback_lines,
                ) {
                    Ok((session, rx)) => {
                        let session = Rc::new(RefCell::new(session));
                        let live_cwd = session.borrow().pid.and_then(crate::cwd::process_cwd).or(cwd);
                        restored_panes.push(Pane {
                            epoch,
                            profile,
                            title: profile_title(profile),
                            session: session.clone(),
                            cwd: live_cwd,
                            branch: None,
                            exited: false,
                            cursor_pos: None,
                            cursor_target: (0.0, 0.0),
                            pane_expansion: 1.0,
                            closing: false,
                            cmd_start: None,
                            running_cmd: None,
                        });
                        Self::watch(epoch, session, rx, cx);
                    }
                    Err(err) => {
                        eprintln!("failed to restore pane {}: {err:#}", saved_pane.profile);
                    }
                }
            }

            if !restored_panes.is_empty() {
                let axis = if saved_tab.axis.eq_ignore_ascii_case("rows") {
                    SplitAxis::Rows
                } else {
                    SplitAxis::Columns
                };
                let active_pane = saved_tab.active_pane.min(restored_panes.len().saturating_sub(1));
                restored_tabs.push(Tab {
                    id: tab_id,
                    custom_title: saved_tab.custom_title,
                    panes: restored_panes,
                    active_pane,
                    axis,
                    maximized: false,
                    closing: false,
                    close_progress: 0.0,
                    close_hover_progress: 0.0,
                });
            }
        }

        if restored_tabs.is_empty() {
            return false;
        }

        self.tabs = restored_tabs;
        self.active = saved.active_tab.min(self.tabs.len().saturating_sub(1));
        cx.notify();
        true
    }

    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        if self.palette_open {
            self.close_palette(window, cx);
        }
        if self.rename_open {
            self.close_rename(window, cx);
        }
        window.focus(&self.search_focus);
        if !self.search_query.is_empty() {
            self.perform_search(cx);
        }
        cx.notify();
    }

    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.search_matches.clear();
        self.focus_main(window);
        cx.notify();
    }

    pub fn perform_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.trim().to_lowercase();
        self.search_matches.clear();
        self.search_index = 0;

        if query.is_empty() {
            cx.notify();
            return;
        }

        let session = match self.active_pane() {
            Some(pane) => pane.session.clone(),
            None => return,
        };

        let s_borrow = session.borrow();
        let term = &s_borrow.term;
        let history_size = term.history_size();
        let screen_lines = term.screen_lines();
        let total_cols = term.columns();

        let mut lines: Vec<(Line, Vec<(Point, char)>, bool)> = Vec::new();
        let start_line = -(history_size as i32);
        let end_line = (screen_lines as i32) - 1;

        for l in start_line..=end_line {
            let line = Line(l);
            let row = &term.grid()[line];
            let mut cells = Vec::new();
            let mut wraps = false;

            for c in 0..total_cols {
                let col = Column(c);
                let cell = &row[col];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                if cell.flags.contains(Flags::WRAPLINE) {
                    wraps = true;
                }
                let ch = if (cell.c as u32) < 32 && cell.c != '\t' {
                    ' '
                } else {
                    cell.c
                };
                cells.push((Point::new(line, col), ch));
            }
            lines.push((line, cells, wraps));
        }

        struct LogLine {
            text: String,
            points: Vec<Point>,
        }
        let mut logical_lines: Vec<LogLine> = Vec::new();
        let mut cur_log = LogLine {
            text: String::new(),
            points: Vec::new(),
        };

        for (_, cells, wraps_next) in lines {
            for (pt, ch) in cells {
                cur_log.text.push(ch);
                cur_log.points.push(pt);
            }
            if !wraps_next {
                logical_lines.push(cur_log);
                cur_log = LogLine {
                    text: String::new(),
                    points: Vec::new(),
                };
            }
        }
        if !cur_log.text.is_empty() {
            logical_lines.push(cur_log);
        }
        drop(s_borrow);

        let mut matches = Vec::new();
        let query_chars_len = query.chars().count();
        for log in logical_lines {
            let lower_text = log.text.to_lowercase();
            let mut search_from = 0;
            while let Some(byte_idx) = lower_text[search_from..].find(&query) {
                let actual_byte_idx = search_from + byte_idx;
                let char_start = lower_text[..actual_byte_idx].chars().count();
                let char_end = char_start + query_chars_len - 1;

                if char_end < log.points.len() {
                    let start_pt = log.points[char_start];
                    let end_pt = log.points[char_end];
                    matches.push((start_pt, end_pt));
                }
                search_from = actual_byte_idx + query.len().max(1);
            }
        }

        self.search_matches = matches;
        if !self.search_matches.is_empty() {
            self.search_index = 0;
            self.scroll_to_search_match(0, cx);
        }

        cx.notify();
    }

    pub fn scroll_to_search_match(&mut self, match_ix: usize, cx: &mut Context<Self>) {
        let target_line = match self.search_matches.get(match_ix) {
            Some((start, _)) => start.line.0,
            None => return,
        };
        let session = match self.active_pane() {
            Some(pane) => pane.session.clone(),
            None => return,
        };
        let (screen_lines, history_size, current_offset) = {
            let s = session.borrow();
            (s.term.screen_lines(), s.term.history_size(), s.term.renderable_content().display_offset as i32)
        };
        let desired_offset = (-target_line - (screen_lines as i32 / 2)).clamp(0, history_size as i32);
        let delta = desired_offset - current_offset;
        if delta != 0 {
            session.borrow_mut().term.scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
        }
        cx.notify();
    }

    pub fn next_search_match(&mut self, cx: &mut Context<Self>) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_index = (self.search_index + 1) % self.search_matches.len();
        self.scroll_to_search_match(self.search_index, cx);
        cx.notify();
    }

    pub fn prev_search_match(&mut self, cx: &mut Context<Self>) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_index = if self.search_index == 0 {
            self.search_matches.len() - 1
        } else {
            self.search_index - 1
        };
        self.scroll_to_search_match(self.search_index, cx);
        cx.notify();
    }

    pub fn handle_search_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        match ks.key.as_str() {
            "escape" => self.close_search(window, cx),
            "enter" => {
                if ks.modifiers.shift {
                    self.prev_search_match(cx);
                } else {
                    self.next_search_match(cx);
                }
            }
            "up" => self.prev_search_match(cx),
            "down" => self.next_search_match(cx),
            "backspace" => {
                if self.search_query.pop().is_some() {
                    self.perform_search(cx);
                }
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    let raw_typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else {
                        ks.key_char
                            .as_deref()
                            .map(|text| {
                                text.chars().filter(|c| !c.is_control()).collect()
                            })
                            .unwrap_or_default()
                    };
                    let typed = crate::keymap::adjust_for_caps_lock(&raw_typed, ks.modifiers.shift);
                    if !typed.is_empty() {
                        self.search_query.push_str(&typed);
                        self.perform_search(cx);
                    }
                }
            }
        }
    }

    pub(crate) fn search_bar(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let match_count = self.search_matches.len();
        let count_text = if self.search_query.is_empty() {
            String::new()
        } else if match_count == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", self.search_index + 1, match_count)
        };

        let is_no_results = match_count == 0 && !self.search_query.is_empty();

        let query_text = if self.search_query.is_empty() {
            div()
                .text_size(px(12.0))
                .text_color(theme.chrome_fg)
                .child("Find in terminal...")
        } else {
            div()
                .text_size(px(12.0))
                .text_color(theme.chrome_fg_active)
                .child(self.search_query.clone())
        };

        let caret = div()
            .w(px(1.5))
            .h(px(14.0))
            .bg(if self.cursor_on { theme.accent } else { rgba(0x00000000) });

        let prev_btn = div()
            .id(ElementId::named_usize("search-prev", 0))
            .w(px(22.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .hover(|s| s.bg(theme.card_hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _, _, cx| {
                view.prev_search_match(cx);
            }))
            .child(svg().path(crate::icons::CHEVRON_UP).size(px(13.0)).text_color(theme.chrome_fg_active));

        let next_btn = div()
            .id(ElementId::named_usize("search-next", 0))
            .w(px(22.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .hover(|s| s.bg(theme.card_hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _, _, cx| {
                view.next_search_match(cx);
            }))
            .child(svg().path(crate::icons::CHEVRON_DOWN).size(px(13.0)).text_color(theme.chrome_fg_active));

        let close_btn = div()
            .id(ElementId::named_usize("search-close", 0))
            .w(px(22.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .hover(|s| s.bg(theme.card_hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _, window, cx| {
                view.close_search(window, cx);
            }))
            .child(svg().path(crate::icons::CLOSE).size(px(11.0)).text_color(theme.chrome_fg));

        let mut row = div()
            .id(ElementId::named_usize("search-bar-pill", 0))
            .absolute()
            .top(px(44.0))
            .right(px(20.0))
            .h(px(34.0))
            .flex()
            .flex_row()
            .items_center()
            .bg(theme.elevated_bg)
            .border_1()
            .border_color(theme.elevated_border)
            .rounded(px(8.0))
            .shadow_md()
            .px(px(8.0))
            .gap(px(6.0))
            .track_focus(&self.search_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                view.handle_search_key(&event.keystroke, window, cx);
            }))
            .child(
                svg()
                    .path(crate::icons::SEARCH)
                    .size(px(14.0))
                    .text_color(theme.chrome_fg),
            )
            .child(
                div()
                    .min_w(px(140.0))
                    .max_w(px(220.0))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap(px(1.0))
                    .child(query_text)
                    .child(caret),
            );

        if !count_text.is_empty() {
            row = row.child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_no_results { rgb(0xE06C75) } else { theme.chrome_fg })
                    .px(px(4.0))
                    .child(count_text),
            );
        }

        row = row
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme.elevated_border))
            .child(prev_btn)
            .child(next_btn)
            .child(close_btn);

        row.into_any_element()
    }
}
