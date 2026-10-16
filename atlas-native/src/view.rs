//! AtlasView — the whole UI, drawn by Rust.
//!
//! Structure: frameless window -> titlebar (brand + search pill + window
//! controls, with the drag band split into the two gaps around the search)
//! -> body: session sidebar (Tabs · LOCAL items · New tab + gear) beside the
//! main column (breadcrumb header · terminal · status bar). The terminal
//! itself is a custom gpui `Element` that walks the alacritty grid and paints
//! cell backgrounds, text runs and the cursor directly on the GPU canvas.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClickEvent, Context, Element, ElementId,
    FocusHandle, Font, FontFallbacks, GlobalElementId, InspectorElementId, IntoElement,
    KeyDownEvent, Keystroke, LayoutId, MouseButton, MouseDownEvent, Pixels, Render, Rgba,
    ScrollWheelEvent, StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, canvas, div,
    fill, point, px, relative, rgb, rgba,
};

use crate::keymap;
use crate::session::{Profile, Session, SessionMsg};
use crate::theme::Theme;

/// Preferred terminal monos, best first: Cascadia (Windows 11 inbox) then
/// Consolas. `terminal_font` picks the first family the text system resolves.
const FONT_CASCADE: &[&str] = &["Cascadia Code", "Cascadia Mono", "Consolas"];
const FONT_FALLBACK: &str = "Consolas";
const FONT_SIZE: f32 = 14.0;
const LINE_SCALE: f32 = 1.3;

/// Motion timings (ms) — short and quiet, Zed-style: overlay entrances and
/// exits, the sidebar resize, the settings/terminal body swap and the
/// sidebar row entrance.
const OVERLAY_IN_MS: u64 = 170;
const OVERLAY_OUT_MS: u64 = 110;
const SIDEBAR_MS: u64 = 190;
const BODY_IN_MS: u64 = 150;
const TAB_IN_MS: u64 = 160;

pub struct AtlasView {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    next_epoch: u64,
    focus: FocusHandle,
    theme: Theme,
    /// Terminal font for the cell grid.
    font: Font,
    /// Caption-window font: native Segoe caption glyphs when present (`None`
    /// falls back to the bundled Nerd Font window glyphs).
    caption_font: Option<Font>,
    /// Light/dark switch (settings page + Ctrl+Shift+D).
    dark: bool,
    /// Settings page visibility (replaces the terminal body).
    settings_open: bool,
    /// New-tab profile menu (the ▾ caret) visibility.
    profile_menu: bool,
    /// Sidebar visibility; when collapsed a slim rail keeps the controls.
    sidebar_open: bool,
    /// Command palette (Ctrl+Shift+P / search pill / pane search icon).
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    /// Rename-tab dialog (pane pen icon).
    rename_open: bool,
    rename_value: String,
    /// Pane-header "⋯" menu visibility.
    menu_open: bool,
    /// Terminal font size in px (settings page).
    font_size: f32,
    /// Cursor override; `None` follows the shell's DECSCUSR.
    cursor_style: Option<CursorShape>,
    /// Blink phase of the cursor (palette/rename carets share it).
    cursor_on: bool,
    /// Focus handles for the overlay surfaces.
    settings_focus: FocusHandle,
    palette_focus: FocusHandle,
    rename_focus: FocusHandle,
    /// Fade-out flags: an overlay stays mounted while it animates away and a
    /// timer task unmounts it (cleared whenever the open flag drops).
    palette_closing: bool,
    menu_closing: bool,
    profile_closing: bool,
    /// Smooth scrolling: the pane being eased plus (position, target) display
    /// offsets in lines. Bound to a session so tab switches stay safe.
    scroll_anim: Option<(Rc<RefCell<Session>>, f32, f32)>,
    /// One easing task per wheel gesture burst; stops when the anim settles.
    scroll_task: bool,
}

struct Tab {
    id: u64,
    /// User-set name (Rename dialog / palette); wins over the shell title.
    custom_title: Option<String>,
    /// Split panes, left to right; always at least one.
    panes: Vec<Pane>,
    /// Pane that owns the keyboard, the caret and the status bar.
    active_pane: usize,
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
}

impl AtlasView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut view = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            next_epoch: 1,
            focus,
            theme: Theme::dark(),
            font: terminal_font(window),
            caption_font: caption_font(window),
            dark: true,
            settings_open: false,
            profile_menu: false,
            sidebar_open: true,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            rename_open: false,
            rename_value: String::new(),
            menu_open: false,
            font_size: FONT_SIZE,
            cursor_style: None,
            cursor_on: true,
            settings_focus: cx.focus_handle(),
            palette_focus: cx.focus_handle(),
            rename_focus: cx.focus_handle(),
            palette_closing: false,
            menu_closing: false,
            profile_closing: false,
            scroll_anim: None,
            scroll_task: false,
        };
        view.new_tab(Profile::PowerShell, cx);

        // Hardware-terminal style blinking cursor: 530 ms phase flip. Typing
        // resets it to solid (see `handle_key`), so it never blinks under the
        // fingers.
        let blink = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(530))
                    .await;
                if this
                    .update(cx, |view, cx| {
                        view.cursor_on = !view.cursor_on;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        blink.detach();

        // Working-directory poll: reads each live shell's PEB every 1.5 s so
        // the sidebar subtitle and breadcrumb track `cd` like a real
        // terminal. Repaints only when a path actually changed.
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

        view
    }

    fn new_tab(&mut self, profile: Profile, cx: &mut Context<Self>) {
        let id = self.next_id;
        let epoch = self.next_epoch;
        self.next_id += 1;
        self.next_epoch += 1;

        // A fresh terminal should be visible, not hidden behind settings.
        self.settings_open = false;

        match Session::spawn(80, 24, home_dir(), profile) {
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
                    }],
                    active_pane: 0,
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
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.new_tab(Profile::PowerShell, cx);
            return;
        }
        if index < self.active {
            self.active -= 1;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
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

        match Session::spawn(80, 24, home_dir(), profile) {
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
    /// spawned to the right and takes focus.
    fn split_active(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let profile = match tab.panes.get(tab.active_pane) {
            Some(pane) => pane.profile,
            None => return,
        };
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        match Session::spawn(80, 24, home_dir(), profile) {
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
                    });
                    tab.active_pane = tab.panes.len() - 1;
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
        if tab.panes.len() <= 1 {
            let index = self.active;
            self.close_tab(index, cx);
            return;
        }
        tab.panes.remove(tab.active_pane);
        if tab.active_pane >= tab.panes.len() {
            tab.active_pane = tab.panes.len() - 1;
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
                        SessionMsg::Output(bytes) => pane.session.borrow_mut().feed(&bytes),
                        SessionMsg::Event(Event::Title(title)) => {
                            if !title.trim().is_empty() {
                                pane.title = title;
                            }
                        }
                        SessionMsg::Event(Event::Exit) => pane.exited = true,
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
                    // Exponential approach: quick at first, gentle at the end.
                    let next = if (target - pos).abs() < 0.35 {
                        target
                    } else {
                        pos + (target - pos) * 0.38
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
                    let delta = next.round() as i32 - current;
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

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.dark = !self.dark;
        self.theme = if self.dark { Theme::dark() } else { Theme::cream() };
        cx.notify();
    }

    fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.profile_menu = false;
            self.palette_open = false;
            window.focus(&self.settings_focus);
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
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

    /// Run a palette / pane-menu command; both overlays fade out on the way.
    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        if self.menu_open {
            self.dismiss_menu(cx);
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
                self.split_active(cx);
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
            Command::ToggleSidebar => {
                self.sidebar_open = !self.sidebar_open;
                window.focus(&self.focus);
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

    /// The pane-header "⋯" button: open the menu, or fade it away.
    fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu_closing {
            self.menu_closing = false;
        } else if self.menu_open {
            self.dismiss_menu(cx);
        } else {
            self.menu_open = true;
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

    /// Fade the pane menu out, then unmount it.
    fn dismiss_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu_closing {
            return;
        }
        self.menu_closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.menu_closing {
                    view.menu_closing = false;
                    view.menu_open = false;
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
        if self.menu_open {
            self.dismiss_menu(cx);
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
            .child(icon_glyph(ICON_SEARCH, 14.0))
            .child(text_row);

        let mut list = div().flex().flex_col().p(px(6.0));
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
            for (index, (command, icon, label, shortcut)) in matches.into_iter().enumerate() {
                let hover = theme.card_hover;
                let mut row = div()
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
                    .child(icon_glyph(icon, 13.0))
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
                if index == self.palette_index {
                    row = row.bg(theme.card_active);
                }
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
                .with_easing(ease_out_cubic),
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
                Animation::new(Duration::from_millis(OVERLAY_IN_MS)).with_easing(ease_out_cubic),
                |el, delta| el.opacity(delta).top(px(120.0 - 8.0 * (1.0 - delta))),
            )
            .into_any_element()
    }

    /// Pane-header "⋯" menu: quick tab actions under the header.
    fn pane_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let entries: [(Command, &'static str, &'static str, &'static str); 6] = [
            (Command::NewTab, ICON_PLUS, "New tab", "Ctrl Shift T"),
            (Command::NewTabWsl, ICON_UBUNTU, "New Ubuntu (WSL) tab", ""),
            (Command::SplitPane, ICON_COLUMNS, "Split pane to the right", ""),
            (Command::RestartTab, ICON_REFRESH, "Restart pane", ""),
            (Command::ClosePane, ICON_X, "Close pane", ""),
            (Command::CloseTab, ICON_X, "Close tab", "Ctrl Shift W"),
        ];
        let mut menu = div()
            .absolute()
            .top(px(84.0))
            .right(px(12.0))
            .w(px(230.0))
            .flex()
            .flex_col()
            .p(px(5.0))
            .rounded(px(10.0))
            .bg(theme.elevated_bg)
            .border_1()
            .border_color(theme.elevated_border)
            .shadow_lg();
        for (index, (command, icon, label, shortcut)) in entries.into_iter().enumerate() {
            let hover = theme.card_hover;
            menu = menu.child(
                div()
                    .id(ElementId::named_usize("pane-menu-item", index))
                    .h(px(30.0))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(9.0))
                    .rounded(px(6.0))
                    .text_size(px(12.0))
                    .text_color(theme.chrome_fg)
                    .hover(move |style| style.bg(hover))
                    .child(icon_glyph(icon, 12.5))
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.chrome_fg_active)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.faint_fg)
                            .child(shortcut),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.run_command(command, window, cx);
                    })),
            );
        }
        let closing = self.menu_closing;
        menu.with_animation(
            ElementId::named_usize("pane-menu-anim", closing as usize),
            Animation::new(Duration::from_millis(if closing {
                OVERLAY_OUT_MS
            } else {
                OVERLAY_IN_MS
            }))
            .with_easing(ease_out_cubic),
            move |el, delta| {
                let t = if closing { 1.0 - delta } else { delta };
                el.opacity(t).top(px(84.0 - 6.0 * (1.0 - t)))
            },
        )
        .into_any_element()
    }

    /// Full settings page shown in place of the terminal body.
    fn settings_page(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let card = || {
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .p(px(16.0))
                .rounded(px(12.0))
                .bg(theme.elevated_bg)
                .border_1()
                .border_color(theme.elevated_border)
        };
        let section = |label: &'static str| -> AnyElement {
            div()
                .text_size(px(10.5))
                .text_color(theme.faint_fg)
                .child(label)
                .into_any_element()
        };
        let row_label = |label: &'static str| -> AnyElement {
            div()
                .w(px(96.0))
                .text_size(px(12.5))
                .text_color(theme.chrome_fg)
                .child(label)
                .into_any_element()
        };

        let theme_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Theme"))
            .child(pill_button(
                "Light".to_string(),
                10,
                !self.dark,
                theme,
                |view, cx| {
                    if view.dark {
                        view.toggle_theme(cx);
                    }
                },
                cx,
            ))
            .child(pill_button(
                "Dark".to_string(),
                11,
                self.dark,
                theme,
                |view, cx| {
                    if !view.dark {
                        view.toggle_theme(cx);
                    }
                },
                cx,
            ));

        let font_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Font size"))
            .child(pill_button(
                "−".to_string(),
                20,
                false,
                theme,
                |view, cx| {
                    view.font_size = (view.font_size - 1.0).max(10.0);
                    cx.notify();
                },
                cx,
            ))
            .child(
                div()
                    .min_w(px(46.0))
                    .flex()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(theme.chrome_fg_active)
                    .child(format!("{} px", self.font_size as i32)),
            )
            .child(pill_button(
                "+".to_string(),
                21,
                false,
                theme,
                |view, cx| {
                    view.font_size = (view.font_size + 1.0).min(24.0);
                    cx.notify();
                },
                cx,
            ));

        let cursor_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Cursor"))
            .child(pill_button(
                "Auto".to_string(),
                30,
                self.cursor_style.is_none(),
                theme,
                |view, cx| {
                    view.cursor_style = None;
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Block".to_string(),
                31,
                self.cursor_style == Some(CursorShape::Block),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Block);
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Bar".to_string(),
                32,
                self.cursor_style == Some(CursorShape::Beam),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Beam);
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Under".to_string(),
                33,
                self.cursor_style == Some(CursorShape::Underline),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Underline);
                    cx.notify();
                },
                cx,
            ));

        let link_hot = theme.chrome_fg_active;
        let about = card()
            .child(section("ABOUT"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Atlas Terminal 0.1.0"),
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

        div()
            .id(ElementId::named_usize("settings-page", 0))
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
            .bg(theme.bg)
            .track_focus(&self.settings_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    view.settings_open = false;
                    window.focus(&view.focus);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .id(ElementId::named_usize("settings-scroll", 0))
                    .flex()
                    .flex_row()
                    .justify_center()
                    .flex_1()
                    .w(relative(1.0))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w(px(620.0))
                            .flex()
                            .flex_col()
                            .gap(px(14.0))
                            .py(px(24.0))
                            .child(card().child(section("APPEARANCE")).child(theme_row))
                            .child(
                                card()
                                    .child(section("TERMINAL"))
                                    .child(font_row)
                                    .child(cursor_row),
                            )
                            .child(about),
                    ),
            )
            .into_any_element()
    }

    fn handle_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // Solid cursor while typing; the blink task flips it back on its own.
        self.cursor_on = true;

        if key == "escape" && (self.settings_open || self.profile_menu || self.menu_open) {
            self.settings_open = false;
            if self.profile_menu {
                self.dismiss_profile(cx);
            }
            if self.menu_open {
                self.dismiss_menu(cx);
            }
            cx.notify();
            return;
        }

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.new_tab(Profile::PowerShell, cx),
                "w" => {
                    let index = self.active;
                    self.close_tab(index, cx);
                }
                "v" => self.paste(cx),
                "d" => self.toggle_theme(cx),
                "p" => self.open_palette(window, cx),
                "tab" => {
                    self.cycle(-1);
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        if modifiers.control && !modifiers.shift && key == "tab" {
            self.cycle(1);
            cx.notify();
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
        // A dismissal that raced another state change must not stay armed.
        if !self.palette_open {
            self.palette_closing = false;
        }
        if !self.menu_open {
            self.menu_closing = false;
        }
        if !self.profile_menu {
            self.profile_closing = false;
        }
        let theme = self.theme.clone();

        // ---- Titlebar ------------------------------------------------
        // Brand · drag gap · centred search pill · drag gap · window
        // controls. The two gaps report themselves as caption slots 0/1 so
        // presses there run the native move loop.
        let tb = theme.clone();
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w(relative(1.0))
            .h(px(38.0))
            .pl(px(12.0))
            .bg(tb.panel_bg)
            .border_b_1()
            .border_color(tb.hairline);

        bar = bar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(18.0))
                        .h(px(18.0))
                        .rounded(px(5.0))
                        .bg(rgb(0x2F6FED))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(0xFFFFFF))
                        .child(icon_glyph(ICON_TERMINAL, 9.5)),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(tb.chrome_fg_active)
                        .child("Atlas Terminal"),
                ),
        );

        bar = bar.child(caption_spacer(0));

        let search_theme = theme.clone();
        let pill_hover = search_theme.card_hover;
        let pill_text = if self.palette_open && !self.palette_query.is_empty() {
            self.palette_query.clone()
        } else {
            "Search commands, tabs and history".to_string()
        };
        let pill_fg = if self.palette_open {
            search_theme.chrome_fg_active
        } else {
            search_theme.faint_fg
        };
        bar = bar.child(
            div()
                .id(ElementId::named_usize("search-pill", 0))
                .w(px(360.0))
                .h(px(28.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(12.0))
                .rounded(px(14.0))
                .bg(search_theme.search_bg)
                .border_1()
                .border_color(search_theme.search_border)
                .text_color(pill_fg)
                .hover(move |style| style.bg(pill_hover))
                .child(icon_glyph(ICON_SEARCH, 13.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .truncate()
                        .child(pill_text),
                )
                .child(
                    div()
                        .h(px(18.0))
                        .px(px(7.0))
                        .flex()
                        .items_center()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(search_theme.hairline)
                        .text_size(px(10.0))
                        .text_color(search_theme.faint_fg)
                        .child("Ctrl Shift P"),
                )
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.open_palette(window, cx);
                })),
        );

        bar = bar.child(caption_spacer(1));

        // Window controls: native Segoe caption glyphs when available,
        // bundled Nerd fallbacks otherwise; the maximize slot is zoomed-aware
        // and shows the restore mark while the window is maximized.
        let caption = self.caption_font.as_ref();
        let zoomed = crate::rounded_corners::is_zoomed();
        let (min_glyph, max_glyph, close_glyph, glyph_size) = if caption.is_some()
        {
            (
                GLYPH_MIN,
                if zoomed { GLYPH_RESTORE } else { GLYPH_MAX },
                GLYPH_CLOSE,
                10.5,
            )
        } else {
            (
                ICON_WINDOW_MIN,
                if zoomed {
                    ICON_WINDOW_RESTORE
                } else {
                    ICON_WINDOW_MAX
                },
                ICON_WINDOW_CLOSE,
                13.0,
            )
        };
        for (index, (glyph, is_close)) in [
            (min_glyph, false),
            (max_glyph, false),
            (close_glyph, true),
        ]
        .into_iter()
        .enumerate()
        {
            let button_theme = theme.clone();
            let hover_bg = if is_close {
                rgb(0xC42B1C)
            } else {
                button_theme.card_hover
            };
            let hot = if is_close {
                rgb(0xFFFFFF)
            } else {
                button_theme.chrome_fg_active
            };
            let idle = button_theme.chrome_fg;
            bar = bar.child(
                div()
                    .id(ElementId::named_usize("window-control", index))
                    .w(px(46.0))
                    .h(px(38.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(idle)
                    .hover(move |style| style.bg(hover_bg).text_color(hot))
                    .child(caption_glyph(self.caption_font.as_ref(), glyph, glyph_size))
                    .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                        crate::rounded_corners::window_button(index);
                        // Repaint so the maximize/restore glyph tracks the
                        // window state this click just changed.
                        cx.notify();
                    })),
            );
        }

        // ---- Terminal area -------------------------------------------
        let body: AnyElement = if self.settings_open {
            self.settings_page(&theme, cx)
        } else if self.tabs.is_empty() {
            div().flex_1().into_any_element()
        } else {
            let index = self.active.min(self.tabs.len() - 1);
            let tab = &self.tabs[index];

            let mut area = div()
                .id(ElementId::named_usize("terminal-area", 0))
                .flex()
                .flex_row()
                .flex_1()
                .w(relative(1.0))
                .h(relative(1.0))
                .bg(theme.bg)
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

            // One cell per pane, left to right, with a hairline gutter.
            for (pane_ix, pane) in tab.panes.iter().enumerate() {
                if pane_ix > 0 {
                    area = area.child(div().w(px(1.0)).h(relative(1.0)).bg(theme.hairline));
                }
                let mut cell = div()
                    .id(ElementId::named_usize("pane-cell", pane_ix))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h(relative(1.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseDownEvent, window, cx| {
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if tab.active_pane != pane_ix {
                                    tab.active_pane = pane_ix;
                                    cx.notify();
                                }
                            }
                            window.focus(&view.focus);
                        }),
                    );
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
                    cell = cell.child(TerminalElement {
                        session: pane.session.clone(),
                        theme: theme.clone(),
                        font: self.font.clone(),
                        font_size: self.font_size,
                        cursor_on: self.cursor_on,
                        cursor_override: self.cursor_style,
                    });
                }
                area = area.child(cell.with_animation(
                    ElementId::named_usize("pane-in", pane.epoch as usize),
                    Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_out_cubic),
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
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_out_cubic),
                |el, delta| el.opacity(delta),
            );

        // ---- Sidebar --------------------------------------------------
        // Tabs list on the left: item per session + New tab / settings row.
        let mut sidebar = div()
            .flex()
            .flex_col()
            .h(relative(1.0))
            .bg(theme.panel_bg)
            .border_r_1()
            .border_color(theme.hairline);

        sidebar = sidebar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(40.0))
                .pl(px(14.0))
                .pr(px(8.0))
                .gap(px(2.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.0))
                        .text_color(theme.chrome_fg_active)
                        .child("Tabs"),
                )
                .child(chrome_action_button(
                    ElementId::named_usize("sidebar-action", 0),
                    ICON_COLUMNS,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                    |view, window, cx| {
                        view.split_active(cx);
                        window.focus(&view.focus);
                    },
                    cx,
                ))
                .child(chrome_action_button(
                    ElementId::named_usize("sidebar-action", 1),
                    ICON_PANEL,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                    |view, window, cx| {
                        view.sidebar_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    },
                    cx,
                )),
        );

        sidebar = sidebar.child(
            div()
                .px(px(14.0))
                .pt(px(8.0))
                .pb(px(4.0))
                .text_size(px(10.5))
                .text_color(theme.faint_fg)
                .child("LOCAL"),
        );

        let mut list = div()
            .id(ElementId::named_usize("sidebar-list", 0))
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll();

        for ix in 0..self.tabs.len() {
            let active = ix == self.active;
            let tab = &self.tabs[ix];

            // Name from the shell-reported title; subtitle: the live location
            // (shell title path first — WSL reports its Linux cwd there — and
            // the PEB poll as fallback), plus a dead-pane note.
            let Some(pane) = tab.panes.get(tab.active_pane) else {
                continue;
            };
            let name = tab
                .custom_title
                .clone()
                .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)));
            let shell = shell_label(&pane.session.borrow().shell);
            let base = pane_location(pane).unwrap_or_else(|| shell.to_string());
            let subtitle = if pane.exited {
                format!("{base} · exited")
            } else {
                base
            };
            let (icon_bg, icon) =
                session_icon(&format!("{} {}", profile_title(pane.profile), pane.title));
            let show_dot = !active && !pane.exited;

            let item_theme = theme.clone();
            let mut item = div()
                .id(ElementId::named_usize("sidebar-tab", ix))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .mx(px(10.0))
                .mb(px(2.0))
                .h(px(50.0))
                .pl(px(12.0))
                .pr(px(8.0))
                .rounded(px(10.0));

            if active {
                item = item.bg(item_theme.card_active);
            } else {
                let hover = item_theme.card_hover;
                item = item.hover(move |style| style.bg(hover));
            }

            // Active marker: accent bar on the item's left edge.
            if active {
                let accent = item_theme.accent;
                item = item.child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(9.0))
                        .bottom(px(9.0))
                        .w(px(3.0))
                        .rounded_full()
                        .bg(accent),
                );
            }

            // Icon tile with an optional running dot on its top-right corner
            // (as in the mock).
            let mut tile = div()
                .relative()
                .w(px(30.0))
                .h(px(30.0))
                .rounded(px(9.0))
                .bg(icon_bg)
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xFFFFFF))
                .child(icon_glyph(icon, 14.0));
            if show_dot {
                let good = item_theme.good;
                let ring = item_theme.panel_bg;
                tile = tile.child(
                    div()
                        .absolute()
                        .right(px(-3.0))
                        .top(px(-3.0))
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(good)
                        .border_2()
                        .border_color(ring),
                );
            }

            // Clicking the tile/title area activates the tab; the close
            // button is a sibling so it can never double-fire activation.
            item = item.child(
                div()
                    .id(ElementId::named_usize("sidebar-tab-main", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .flex_1()
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.active = ix;
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    }))
                    .child(tile)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(item_theme.chrome_fg_active)
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(item_theme.chrome_fg)
                                    .truncate()
                                    .child(subtitle),
                            ),
                    ),
            );

            // Close affordance on the active item (same as the mock).
            if active {
                let close_hover = item_theme.card_hover;
                let close_hot = item_theme.chrome_fg_active;
                item = item.child(
                    div()
                        .id(ElementId::named_usize("sidebar-close", ix))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .text_color(item_theme.chrome_fg)
                        .hover(move |style| style.bg(close_hover).text_color(close_hot))
                        .child(icon_glyph(ICON_X, 12.0))
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        })),
                );
            }

            list = list.child(item.with_animation(
                ElementId::named_usize("tab-in", tab.id as usize),
                Animation::new(Duration::from_millis(TAB_IN_MS)).with_easing(ease_out_cubic),
                |el, delta| el.opacity(delta).left(px(-12.0 * (1.0 - delta))),
            ));
        }

        sidebar = sidebar.child(list);

        // Bottom bar: New tab (click spawns PowerShell, ▾ opens the profile
        // menu) + settings gear (popover anchor).
        let new_tab_theme = theme.clone();
        let new_hover = new_tab_theme.card_active;
        let caret_hover = new_hover;
        let gear_theme = theme.clone();
        let gear_hover = gear_theme.card_hover;
        let gear_idle = gear_theme.chrome_fg;
        let gear_hot = gear_theme.chrome_fg_active;
        sidebar = sidebar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .p(px(10.0))
                .child(
                    // Visual container only: the click surface and the caret
                    // are siblings inside it, so opening the menu never also
                    // spawns a tab.
                    div()
                        .flex_1()
                        .h(px(34.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .rounded(px(9.0))
                        .bg(new_tab_theme.card_hover)
                        .border_1()
                        .border_color(new_tab_theme.elevated_border)
                        .child(
                            div()
                                .id(ElementId::named_usize("new-tab", 0))
                                .flex_1()
                                .h_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(7.0))
                                .pl(px(12.0))
                                .rounded(px(9.0))
                                .text_color(new_tab_theme.chrome_fg_active)
                                .hover(move |style| style.bg(new_hover))
                                .child(icon_glyph(ICON_PLUS, 11.5))
                                .child(div().text_size(px(12.5)).child("New tab"))
                                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                    view.new_tab(Profile::PowerShell, cx);
                                    window.focus(&view.focus);
                                })),
                        )
                        .child(
                            div()
                                .w(px(1.0))
                                .h(px(16.0))
                                .bg(new_tab_theme.hairline),
                        )
                        .child(
                            div()
                                .id(ElementId::named_usize("profile-caret", 0))
                                .w(px(28.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(9.0))
                                .text_color(new_tab_theme.chrome_fg)
                                .hover(move |style| style.bg(caret_hover))
                                .child(icon_glyph(ICON_CHEVRON_DOWN, 10.0))
                                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                    view.toggle_profile_menu(cx);
                                    window.focus(&view.focus);
                                })),
                        ),
                )
                .child(
                    div()
                        .id(ElementId::named_usize("settings-toggle", 0))
                        .w(px(32.0))
                        .h(px(34.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(9.0))
                        .text_color(gear_idle)
                        .hover(move |style| style.bg(gear_hover).text_color(gear_hot))
                        .child(icon_glyph(ICON_GEAR, 14.0))
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.toggle_settings(window, cx);
                        })),
                ),
        );

        // Collapsed state: a slim rail keeps expand + new-tab reachable.
        let (content, from_w, to_w): (AnyElement, f32, f32) = if self.sidebar_open {
            (sidebar.into_any_element(), 46.0, 254.0)
        } else {
            let rail_theme = theme.clone();
            let mut rail = div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .h(relative(1.0))
                .pt(px(9.0))
                .bg(rail_theme.panel_bg)
                .border_r_1()
                .border_color(rail_theme.hairline);
            rail = rail.child(chrome_action_button(
                ElementId::named_usize("rail-action", 0),
                ICON_PANEL,
                13.0,
                28.0,
                28.0,
                7.0,
                &rail_theme,
                |view, window, cx| {
                    view.sidebar_open = true;
                    window.focus(&view.focus);
                    cx.notify();
                },
                cx,
            ));
            rail = rail.child(chrome_action_button(
                ElementId::named_usize("rail-action", 1),
                ICON_PLUS,
                13.0,
                28.0,
                28.0,
                7.0,
                &rail_theme,
                |view, window, cx| {
                    view.new_tab(Profile::PowerShell, cx);
                    window.focus(&view.focus);
                },
                cx,
            ));
            (rail.into_any_element(), 254.0, 46.0)
        };

        // Animate the width and fade the inbound content so the collapse /
        // expand glides; contents stretch to the animated width.
        let open = self.sidebar_open;
        let sidebar: AnyElement = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .h(relative(1.0))
            .child(content)
            .with_animation(
                ElementId::named_usize("sidebar-anim", open as usize),
                Animation::new(Duration::from_millis(SIDEBAR_MS)).with_easing(ease_in_out_cubic),
                move |el, delta| {
                    let opacity = if open {
                        ((delta - 0.25) / 0.75).max(0.0)
                    } else {
                        delta
                    };
                    el.w(px(from_w + (to_w - from_w) * delta)).opacity(opacity)
                },
            )
            .into_any_element();

        // ---- Pane header ---------------------------------------------
        // Breadcrumb (folder · parent › leaf) + view actions.
        let header_source = match self.tabs.get(self.active) {
            Some(tab) => match tab.panes.get(tab.active_pane) {
                Some(pane) => pane_raw_location(pane).unwrap_or_else(|| {
                    tab.custom_title
                        .clone()
                        .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)))
                }),
                None => "PowerShell".to_string(),
            },
            None => "PowerShell".to_string(),
        };
        let (crumb_a, crumb_b) = breadcrumbs(&header_source);
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .w(relative(1.0))
            .h(px(40.0))
            .pl(px(16.0))
            .pr(px(8.0))
            .bg(theme.panel_bg)
            .border_b_1()
            .border_color(theme.hairline)
            .text_color(theme.chrome_fg);

        if self.settings_open {
            // Settings header: gear + title + close.
            let close_hover = theme.card_hover;
            let close_hot = theme.chrome_fg_active;
            header = header
                .child(icon_glyph(ICON_GEAR, 13.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.chrome_fg_active)
                        .child("Settings"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .id(ElementId::named_usize("settings-close", 0))
                        .w(px(24.0))
                        .h(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .text_color(theme.chrome_fg)
                        .hover(move |style| style.bg(close_hover).text_color(close_hot))
                        .child(icon_glyph(ICON_X, 13.0))
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.settings_open = false;
                            window.focus(&view.focus);
                            cx.notify();
                        })),
                );
        } else {
            header = header.child(icon_glyph(ICON_FOLDER, 13.0));
            if let Some(parent) = crumb_a {
                header = header
                    .child(div().text_size(px(13.0)).truncate().child(parent))
                    .child(
                        div()
                            .text_color(theme.faint_fg)
                            .child(icon_glyph(ICON_CHEVRON, 10.0)),
                    );
            }
            header = header
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.chrome_fg_active)
                        .truncate()
                        .child(crumb_b),
                )
                .child(div().flex_1())
                .child(chrome_action_button(
                    ElementId::named_usize("pane-action", 0),
                    ICON_SEARCH,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                    |view, window, cx| {
                        view.open_palette(window, cx);
                    },
                    cx,
                ))
                .child(chrome_action_button(
                    ElementId::named_usize("pane-action", 1),
                    ICON_PEN,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                    |view, window, cx| {
                        view.open_rename(window, cx);
                    },
                    cx,
                ))
                .child(chrome_action_button(
                    ElementId::named_usize("pane-action", 2),
                    ICON_ELLIPSIS,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                    |view, window, cx| {
                        view.toggle_menu(cx);
                        window.focus(&view.focus);
                    },
                    cx,
                ));
        }

        // ---- Status bar ----------------------------------------------
        let (cols, rows, shell, exited, branch, status_left) = match self.active_pane() {
            Some(pane) => {
                let session = pane.session.borrow();
                let cols = session.cols;
                let rows = session.rows;
                let shell = shell_label(&session.shell);
                drop(session);
                let left = pane_location(pane)
                    .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)));
                (cols, rows, shell, pane.exited, pane.branch.clone(), left)
            }
            None => (0, 0, "PowerShell", false, None, "PowerShell".to_string()),
        };
        // Mock-style left segment: git branch when the working directory is a
        // repository, the directory itself otherwise.
        let (status_icon, status_left) = match branch {
            Some(branch) => (ICON_BRANCH, branch),
            None => (ICON_FOLDER, status_left),
        };
        let (state_label, state_color) = if exited {
            ("Exited", theme.ansi[1])
        } else {
            ("Connected", theme.good)
        };
        let status = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .h(px(30.0))
            .pl(px(16.0))
            .pr(px(14.0))
            .bg(theme.panel_bg)
            .border_t_1()
            .border_color(theme.hairline)
            .text_size(px(11.5))
            .text_color(theme.chrome_fg)
            .child(icon_glyph(status_icon, 11.0))
            .child(
                div()
                    .text_color(theme.chrome_fg_active)
                    .child(status_left),
            )
            .child(div().w(px(1.0)).h(px(12.0)).bg(theme.hairline))
            .child(div().child(shell))
            .child(div().flex_1())
            .child(div().child(format!("{cols} × {rows}")))
            .child(div().child("UTF-8"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .text_color(state_color)
                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(state_color))
                    .child(div().child(state_label)),
            );

        let main_col = div()
            .flex()
            .flex_col()
            .flex_1()
            .h(relative(1.0))
            .child(header)
            .child(body)
            .child(status);

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .bg(theme.panel_bg)
            .font_family("Segoe UI")
            .text_color(theme.fg)
            .child(bar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w(relative(1.0))
                    .child(sidebar)
                    .child(main_col),
            );

        // Overlays paint last: each is a dim click-catcher plus its panel, in
        // stacking order (profile menu, pane menu, palette, rename dialog).
        // The `*_closing` flags drive fade-outs; dismissal tasks unmount them.
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
        if self.menu_open {
            root = root.child(overlay_backdrop(
                1,
                self.menu_closing,
                |view, _window, cx| {
                    view.dismiss_menu(cx);
                },
                cx,
            ));
            root = root.child(self.pane_menu(&theme, cx));
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

        root
    }
}

// ----------------------------------------------------------------------------
// Motion
// ----------------------------------------------------------------------------

/// Cubic ease-out for entrances: quick takeoff, soft landing.
fn ease_out_cubic(d: f32) -> f32 {
    1.0 - (1.0 - d).powi(3)
}

/// Cubic ease-in-out for layout resizes.
fn ease_in_out_cubic(d: f32) -> f32 {
    if d < 0.5 {
        4.0 * d * d * d
    } else {
        1.0 - (-2.0 * d + 2.0).powi(3) / 2.0
    }
}

// ----------------------------------------------------------------------------
// Terminal renderer
// ----------------------------------------------------------------------------

struct TerminalElement {
    session: Rc<RefCell<Session>>,
    theme: Theme,
    font: Font,
    font_size: f32,
    cursor_on: bool,
    cursor_override: Option<CursorShape>,
}

/// How a cell's glyph is shaped: with the terminal font, isolated from its
/// neighbours (emoji, CJK…), or with the bundled Nerd symbols font.
#[derive(Clone, Copy, PartialEq)]
enum CharClass {
    Text,
    Other,
    Icon,
}

fn char_class(c: char) -> CharClass {
    let u = c as u32;
    match u {
        // Private use areas: Nerd Font icons, Powerline separators, …
        0xE000..=0xF8FF | 0xF0000..=0x10FFFD => CharClass::Icon,
        // Latin-ish ranges the terminal font covers: keep runs long.
        0x20..=0x2BFF => CharClass::Text,
        // Emoji, CJK, exotic scripts: shape in isolation so their glyph
        // advances can never shift the rest of the row off the cell grid.
        _ => CharClass::Other,
    }
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
        let cell_width = text_system
            .em_advance(font_id, font_size)
            .map(f32::from)
            .unwrap_or(8.4)
            .max(1.0);
        let cell_height = f32::from(font_size) * LINE_SCALE;

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
        window.paint_quad(fill(bounds, theme.bg));

        let text_system = window.text_system().clone();
        let font_size = px(self.font_size);
        let font_id = text_system.resolve_font(&self.font);
        let cell_width = text_system
            .em_advance(font_id, font_size)
            .map(f32::from)
            .unwrap_or(8.4)
            .max(1.0);
        let cell_height = f32::from(font_size) * LINE_SCALE;

        let left = f32::from(bounds.left());
        let top = f32::from(bounds.top());

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

                let mut fg = theme.resolve(cell.fg, flags.contains(Flags::DIM));
                let mut bg = theme.resolve(cell.bg, false);
                if flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
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

                let style = CellStyle {
                    fg,
                    bg,
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
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

        for (row_index, row) in rows.iter().enumerate() {
            let y0 = top + cell_height * row_index as f32;
            let y1 = y0 + cell_height;
            let x = |col: usize| left + cell_width * col as f32;

            // Cell backgrounds.
            let mut col = 0usize;
            for run in &row.runs {
                if run.style.bg != theme.bg {
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(px(x(col)), px(y0)),
                            point(px(x(col + run.cells)), px(y1)),
                        ),
                        run.style.bg,
                    ));
                }
                col += run.cells;
            }

            // Cursor: solid block fills the cell (its colours were inverted
            // above, so the glyph stays readable); hollow block is an outline;
            // beam and underline are bars. All shapes sit on exact cell bounds.
            if let Some((cursor_row, cursor_col)) = cursor_cell.filter(|_| self.cursor_on) {
                if cursor_row == row_index {
                    let x0 = x(cursor_col);
                    let x1 = x0 + cell_width;
                    let color = theme.cursor;
                    let quad = |rx0: f32, ry0: f32, rx1: f32, ry1: f32, window: &mut Window| {
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(px(rx0), px(ry0)),
                                point(px(rx1), px(ry1)),
                            ),
                            color,
                        ));
                    };
                    match cursor_shape {
                        CursorShape::Block => quad(x0, y0, x1, y1, window),
                        CursorShape::HollowBlock => {
                            let t = 1.0;
                            quad(x0, y0, x1, y0 + t, window);
                            quad(x0, y1 - t, x1, y1, window);
                            quad(x0, y0, x0 + t, y1, window);
                            quad(x1 - t, y0, x1, y1, window);
                        }
                        CursorShape::Beam => quad(x0, y0, x0 + 2.0, y1, window),
                        CursorShape::Underline => quad(x0, y1 - 2.0, x1, y1, window),
                        CursorShape::Hidden => {}
                    }
                }
            }

            // Text: one shaped line per segment, each pinned to its starting
            // grid column, so glyph advances (emoji, CJK, Nerd icons) can
            // never drag text — or the cursor — off the cell grid. Icon runs
            // are split per glyph so every icon lands on its own cell.
            struct Segment<'a> {
                text: &'a str,
                x0: f32,
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
                if run.style.bold {
                    font = font.bold();
                }
                if run.style.italic {
                    font = font.italic();
                }
                if run.style.class == CharClass::Icon {
                    let mut cell = col;
                    let mut offset = 0usize;
                    for c in run_text.chars() {
                        let end = offset + c.len_utf8();
                        segments.push(Segment {
                            text: &run_text[offset..end],
                            x0: x(cell),
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
                let line = text_system.shape_line(
                    segment.text.to_string().into(),
                    font_size,
                    &[text_run],
                    None,
                );
                let _ = line.paint(point(px(segment.x0), px(y0)), px(cell_height), window, cx);
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

/// Codicon (stroke) chrome icons.
const ICON_X: &str = "\u{EA76}"; // nf-cod-close
const ICON_PLUS: &str = "\u{EA60}"; // nf-cod-add
const ICON_GEAR: &str = "\u{EAF8}"; // nf-cod-gear
const ICON_SEARCH: &str = "\u{EA6D}"; // nf-cod-search
const ICON_FOLDER: &str = "\u{EA83}"; // nf-cod-folder
const ICON_CHEVRON: &str = "\u{EAB6}"; // nf-cod-chevron_right
const ICON_PEN: &str = "\u{EA73}"; // nf-cod-edit
const ICON_ELLIPSIS: &str = "\u{EA7C}"; // nf-cod-ellipsis
const ICON_BRANCH: &str = "\u{EC6F}"; // nf-cod-git_branch
const ICON_CHEVRON_DOWN: &str = "\u{EAB4}"; // nf-cod-chevron_down
const ICON_COLUMNS: &str = "\u{EB56}"; // nf-cod-split_horizontal
const ICON_PANEL: &str = "\u{EBF3}"; // nf-cod-layout_panel_left

/// Session-tile icons (white mark on a colored tile, as in the mock).
const ICON_TERMINAL: &str = "\u{F120}"; // nf-fa-terminal
const ICON_UBUNTU: &str = "\u{EF72}"; // nf-fa-ubuntu (brand mark)
const ICON_CLOUD: &str = "\u{F0C2}"; // nf-fa-cloud
const ICON_SCREEN: &str = "\u{F108}"; // nf-fa-desktop

/// Caption-window glyphs: native Segoe codepoints when the OS caption font is
/// available, bundled Font Awesome fallbacks otherwise. The maximize slot is
/// zoomed-aware and shows the restore mark instead.
const GLYPH_MIN: &str = "\u{E921}"; // Segoe caption minimize
const GLYPH_MAX: &str = "\u{E922}"; // Segoe caption maximize
const GLYPH_RESTORE: &str = "\u{E923}"; // Segoe caption restore
const GLYPH_CLOSE: &str = "\u{E947}"; // Segoe caption close
const ICON_WINDOW_MIN: &str = "\u{F2D1}"; // nf-fa-window_minimize
const ICON_WINDOW_MAX: &str = "\u{F2D0}"; // nf-fa-window_maximize
const ICON_WINDOW_RESTORE: &str = "\u{F2D2}"; // nf-fa-window_restore
const ICON_WINDOW_CLOSE: &str = "\u{F2D3}"; // nf-fa-window_close

const ICON_CHEVRON_LEFT: &str = "\u{EAB5}"; // nf-cod-chevron_left
const ICON_REFRESH: &str = "\u{EB37}"; // nf-cod-refresh
const ICON_COLOR_MODE: &str = "\u{EAC6}"; // nf-cod-color_mode
const ICON_TEXT_SIZE: &str = "\u{EB69}"; // nf-cod-text_size

/// Commands offered by the palette and the pane "⋯" menu.
#[derive(Clone, Copy)]
enum Command {
    NewTab,
    NewTabWsl,
    SplitPane,
    ClosePane,
    CloseTab,
    NextTab,
    PrevTab,
    RestartTab,
    RenameTab,
    ToggleTheme,
    OpenSettings,
    ToggleSidebar,
    FontInc,
    FontDec,
}

/// Palette entries: command, icon, label, shortcut hint.
const COMMANDS: [(Command, &str, &str, &str); 14] = [
    (Command::NewTab, ICON_PLUS, "New tab", "Ctrl Shift T"),
    (Command::NewTabWsl, ICON_UBUNTU, "New Ubuntu (WSL) tab", ""),
    (
        Command::SplitPane,
        ICON_COLUMNS,
        "Split pane to the right",
        "",
    ),
    (Command::ClosePane, ICON_X, "Close pane", ""),
    (Command::CloseTab, ICON_X, "Close tab", "Ctrl Shift W"),
    (Command::NextTab, ICON_CHEVRON, "Next tab", "Ctrl Tab"),
    (
        Command::PrevTab,
        ICON_CHEVRON_LEFT,
        "Previous tab",
        "Ctrl Shift Tab",
    ),
    (Command::RestartTab, ICON_REFRESH, "Restart pane", ""),
    (Command::RenameTab, ICON_PEN, "Rename tab", ""),
    (
        Command::ToggleTheme,
        ICON_COLOR_MODE,
        "Toggle light / dark theme",
        "Ctrl Shift D",
    ),
    (Command::OpenSettings, ICON_GEAR, "Open settings", ""),
    (Command::ToggleSidebar, ICON_PANEL, "Toggle sidebar", ""),
    (Command::FontInc, ICON_TEXT_SIZE, "Increase font size", ""),
    (Command::FontDec, ICON_TEXT_SIZE, "Decrease font size", ""),
];

/// One Nerd Font glyph, centred in its parent; the parent decides the color.
fn icon_glyph(glyph: &'static str, size: f32) -> impl IntoElement {
    caption_glyph(None, glyph, size)
}

/// A glyph rendered in an explicit font: `None` means the bundled Nerd
/// symbols. The font is cloned so the returned element owns everything.
fn caption_glyph(font: Option<&Font>, glyph: &'static str, size: f32) -> impl IntoElement {
    let font = font.cloned().unwrap_or_else(icon_font);
    div()
        .flex()
        .items_center()
        .justify_center()
        .font(font)
        .text_size(px(size))
        .child(glyph)
}

/// A flexible spacer in the titlebar that reports its painted bounds to the
/// window subclass as drag-band slot `slot` (see `rounded_corners`).
fn caption_spacer(slot: usize) -> impl IntoElement {
    div()
        .id(ElementId::named_usize("caption-spacer", slot))
        .flex_1()
        .h(px(38.0))
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

/// Profile picker behind the New-tab caret, anchored above the bottom bar.
/// Entries reuse the sidebar's icon table so the new session arrives already
/// looking like its mock entry.
fn profile_menu(theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
    let entries: [(&'static str, Profile); 2] = [
        ("PowerShell", Profile::PowerShell),
        ("Ubuntu (WSL)", Profile::Wsl),
    ];
    let item_hover = theme.card_hover;
    let mut menu = div()
        .absolute()
        .bottom(px(56.0))
        .left(px(10.0))
        .w(px(236.0))
        .flex()
        .flex_col()
        .p(px(6.0))
        .rounded(px(10.0))
        .bg(theme.elevated_bg)
        .border_1()
        .border_color(theme.elevated_border)
        .shadow_lg();
    for (index, (label, profile)) in entries.into_iter().enumerate() {
        let icon = session_icon(&profile_title(profile)).1;
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
                .child(icon_glyph(icon, 13.0))
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
        .with_easing(ease_out_cubic),
        move |el, delta| {
            let t = if closing { 1.0 - delta } else { delta };
            el.opacity(t).bottom(px(56.0 - 6.0 * (1.0 - t)))
        },
    )
    .into_any_element()
}

/// Small icon button (sidebar header, pane header, rail): quiet by default,
/// brightens on hover, runs `on_click` — which owns focus placement, so
/// overlays can take focus without the terminal snatching it back.
fn chrome_action_button(
    id: ElementId,
    glyph: &'static str,
    glyph_size: f32,
    w: f32,
    h: f32,
    radius: f32,
    theme: &Theme,
    on_click: impl Fn(&mut AtlasView, &mut Window, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let hover_bg = theme.card_hover;
    let idle = theme.chrome_fg;
    let hot = theme.chrome_fg_active;
    div()
        .id(id)
        .w(px(w))
        .h(px(h))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .text_color(idle)
        .hover(move |style| style.bg(hover_bg).text_color(hot))
        .child(icon_glyph(glyph, glyph_size))
        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
            on_click(view, window, cx);
        }))
        .into_any_element()
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
            .with_easing(ease_out_cubic),
            move |el, delta| el.opacity(if closing { 1.0 - delta } else { delta }),
        )
        .into_any_element()
}

/// Split a shell-reported title into breadcrumb parts (parent, leaf). A bare
/// drive root like `C:` is dropped; single-component titles become the leaf.
fn breadcrumbs(raw: &str) -> (Option<String>, String) {
    let raw = raw.trim();
    // Unix (WSL) paths keep forward slashes and more context so they never
    // read as Windows paths.
    if raw.starts_with('/') {
        let parts: Vec<&str> = raw.split('/').filter(|part| !part.is_empty()).collect();
        return match parts.len() {
            0 => (None, "/".to_string()),
            1 => (None, format!("/{}", parts[0])),
            2 => (None, format!("/{}", parts.join("/"))),
            n => {
                let mut parent = parts[..n - 1].join("/");
                if parent.chars().count() > 22 {
                    parent = format!("…/{}", parts[n - 2]);
                }
                (Some(format!("/{parent}")), parts[n - 1].to_string())
            }
        };
    }
    let mut parts: Vec<&str> = raw
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    parts.retain(|part| !(part.len() == 2 && part.ends_with(':')));
    let leaf = |part: &str| -> String {
        let part = part.trim_end_matches(".exe");
        let mut out: String = part.chars().take(22).collect();
        if part.chars().count() > 22 {
            out.push('…');
        }
        out
    };
    match parts.len() {
        0 => (None, "PowerShell".to_string()),
        1 => (None, leaf(parts[0])),
        n => (Some(leaf(parts[n - 2])), leaf(parts[n - 1])),
    }
}

/// Absolute path carried by a shell title (`user@host: /mnt/c/Users/me` →
/// `/mnt/c/Users/me`). "Windows PowerShell"-style titles return `None`.
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
    if looks_like_path && !candidate.contains('@') {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Raw location string for a pane: the title path first (WSL shells report
/// their Linux cwd there), the PEB-polled Windows cwd as fallback.
fn pane_raw_location(pane: &Pane) -> Option<String> {
    if let Some(path) = title_path(&pane.title) {
        return Some(path);
    }
    pane.cwd
        .as_ref()
        .map(|cwd| cwd.to_string_lossy().into_owned())
}

/// Short display form of a pane's location (`…/Users/me`, `~\projects\api`).
fn pane_location(pane: &Pane) -> Option<String> {
    pane_raw_location(pane).map(|raw| display_cwd(Path::new(&raw)))
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

/// Icon tile for a session, keyed off its shell-reported title: PowerShell
/// blue terminal, WSL/Ubuntu orange brand mark, remote indigo cloud, dev
/// green desktop.
fn session_icon(title: &str) -> (Rgba, &'static str) {
    let lower = title.to_ascii_lowercase();
    if lower.contains("ubuntu") || lower.contains("wsl") {
        (rgb(0xE95420), ICON_UBUNTU)
    } else if lower.contains("ssh") || lower.contains("edge") || lower.contains("remote") {
        (rgb(0x6E5BE8), ICON_CLOUD)
    } else if lower.contains("dev")
        || lower.contains("server")
        || lower.contains("vite")
        || lower.contains("uvicorn")
        || lower.contains("node")
        || lower.contains("npm")
    {
        (rgb(0x2EA043), ICON_SCREEN)
    } else {
        (rgb(0x2F6FED), ICON_TERMINAL)
    }
}

/// Friendly label for the shell executable that was actually spawned.
fn shell_label(shell: &str) -> &'static str {
    if shell.eq_ignore_ascii_case("pwsh") {
        "PowerShell 7"
    } else if shell.eq_ignore_ascii_case("wsl.exe") {
        "WSL"
    } else {
        "Windows PowerShell"
    }
}

/// Sidebar/default title for a freshly spawned profile.
fn profile_title(profile: Profile) -> String {
    match profile {
        Profile::PowerShell => "PowerShell".to_string(),
        Profile::Wsl => "Ubuntu".to_string(),
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

/// Caption-button glyph font: the native Windows caption fonts when present
/// (Win11 "Segoe Fluent Icons", Win10 "Segoe MDL2 Assets"), so the
/// minimize / maximize / close marks match the OS chrome exactly, as in the
/// mock. `None` falls back to the bundled Nerd Font window glyphs.
fn caption_font(window: &Window) -> Option<Font> {
    let available = window.text_system().all_font_names();
    ["Segoe Fluent Icons", "Segoe MDL2 Assets"]
        .iter()
        .find(|family| {
            available
                .iter()
                .any(|name| name.eq_ignore_ascii_case(**family))
        })
        .map(|family| gpui::font(*family))
}

/// Bundled Nerd Font symbols (embedded in the exe, registered at startup).
/// Resolved from gpui's custom in-memory font collection.
fn icon_font() -> Font {
    gpui::font("Symbols Nerd Font Mono")
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
    let hover_bg = theme.card_active;
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
        button = button.bg(active_bg);
    }
    button.into_any_element()
}

/// Sidebar/pane label for a tab: the shell-reported title when it reads as a
/// real name (e.g. "vite"), the profile label when it is shell-ish, a prompt
/// string (`user@host: /path`) or a bare path (the path itself is shown in
/// the subtitle/breadcrumb instead).
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

/// Mock-style working-directory label: `C:\Users\me\projects\api` ->
/// `~\projects\api`; deeper paths keep the root and the last two parts.
fn display_cwd(cwd: &Path) -> String {
    let lossy = cwd.to_string_lossy();
    let trimmed = lossy.trim_end_matches(['\\', '/']);

    // Unix paths (WSL, MSYS): keep forward slashes and show the tail.
    if trimmed.starts_with('/') {
        let parts: Vec<&str> = trimmed.split('/').filter(|part| !part.is_empty()).collect();
        return match parts.len() {
            0 => "/".to_string(),
            1 => format!("/{}", parts[0]),
            2 => format!("/{}/{}", parts[0], parts[1]),
            n => format!("…/{}/{}", parts[n - 2], parts[n - 1]),
        };
    }

    let home = std::env::var_os("USERPROFILE").map(|home| home.to_string_lossy().into_owned());
    let home = home.as_deref().map(|home| home.trim_end_matches(['\\', '/']));

    let (root, rest) = match home {
        Some(home) if trimmed.eq_ignore_ascii_case(home) => return "~".to_string(),
        Some(home)
            if trimmed.len() > home.len()
                && trimmed.to_ascii_lowercase().starts_with(&home.to_ascii_lowercase())
                && matches!(trimmed.as_bytes()[home.len()], b'\\' | b'/') =>
        {
            (
                "~".to_string(),
                trimmed[home.len()..]
                    .trim_start_matches(['\\', '/'])
                    .to_string(),
            )
        }
        _ => match trimmed.split_once(":\\") {
            Some((drive, rest)) => (format!("{drive}:\\"), rest.to_string()),
            None => (String::new(), trimmed.to_string()),
        },
    };

    let parts: Vec<&str> = rest
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    match parts.len() {
        0 => root,
        1 => format!("{root}{}", parts[0]),
        2 => format!("{root}{}\\{}", parts[0], parts[1]),
        n => format!("{root}…\\{}", parts[n - 2..].join("\\")),
    }
}
