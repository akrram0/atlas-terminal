//! Tab and pane lifecycle, process spawning, split views, and window actions.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::term::TermMode;
use gpui::prelude::*;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement, Rgba, Window, div, px,
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
                    .and_then(|pane| pane.cwd.clone())
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

    /// Answer "yes" to the close-confirmation dialog.
    pub(crate) fn confirm_pending_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_close_window {
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
            Some(pane) => (pane.profile, pane.cwd.clone()),
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
                            if !title.trim().is_empty() {
                                pane.title = title;
                            }
                        }
                        SessionMsg::Event(Event::Exit) => pane.exited = true,
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

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// Pane that owns the keyboard and the status bar.
    pub(crate) fn active_pane(&self) -> Option<&Pane> {
        self.tabs
            .get(self.active)
            .and_then(|tab| tab.panes.get(tab.active_pane))
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
}
