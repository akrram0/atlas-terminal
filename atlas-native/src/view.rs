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
    FocusHandle, Font, FontFallbacks, FontWeight, GlobalElementId, InspectorElementId,
    IntoElement, KeyDownEvent, Keystroke, LayoutId, MouseButton, MouseDownEvent, Pixels, Render,
    Rgba, ScrollWheelEvent, StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, canvas,
    div, fill, point, px, relative, rgb, rgba,
};

use crate::icons;
use crate::keymap;
use crate::session::{Profile, Session, SessionMsg};
use crate::theme::{Theme, ThemeRegistry};

/// Preferred terminal monos, best first: Cascadia (Windows 11 inbox) then
/// Consolas. `terminal_font` picks the first family the text system resolves.
const FONT_CASCADE: &[&str] = &["Cascadia Code", "Cascadia Mono", "Consolas"];
const FONT_FALLBACK: &str = "Consolas";
const FONT_SIZE: f32 = 13.5;
const LINE_SCALE: f32 = 1.333;

/// Motion timings (ms) — short and quiet, Zed-style: overlay entrances and
/// exits and the settings/terminal body swap.
const OVERLAY_IN_MS: u64 = 170;
const OVERLAY_OUT_MS: u64 = 110;
const BODY_IN_MS: u64 = 150;

pub struct AtlasView {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    next_epoch: u64,
    focus: FocusHandle,
    theme: Theme,
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
    /// Rename-tab dialog (palette).
    rename_open: bool,
    rename_value: String,
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
    profile_closing: bool,
    /// Smooth scrolling: the pane being eased plus (position, target) display
    /// offsets in lines. Bound to a session so tab switches stay safe.
    scroll_anim: Option<(Rc<RefCell<Session>>, f32, f32)>,
    /// One easing task per wheel gesture burst; stops when the anim settles.
    scroll_task: bool,
    /// Tab pill currently under the pointer: its hover-only close mark.
    hover_tab: Option<usize>,
    /// Geometric chrome mark under the pointer (new-tab plus, pane actions).
    /// Painted quads can't inherit hover text colour, so the view tracks it.
    hover_mark: Option<u64>,
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
}

impl AtlasView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let registry = ThemeRegistry::standard();
        let mut view = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            next_epoch: 1,
            focus,
            theme: registry.preset(0).theme.clone(),
            font: terminal_font(window),
            theme_id: 0,
            registry,
            settings_open: false,
            profile_menu: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            rename_open: false,
            rename_value: String::new(),
            font_size: FONT_SIZE,
            cursor_style: None,
            cursor_on: true,
            settings_focus: cx.focus_handle(),
            palette_focus: cx.focus_handle(),
            rename_focus: cx.focus_handle(),
            palette_closing: false,
            profile_closing: false,
            scroll_anim: None,
            scroll_task: false,
            hover_tab: None,
            hover_mark: None,
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

    fn new_tab(&mut self, profile: Profile, cx: &mut Context<Self>) {
        let id = self.next_id;
        let epoch = self.next_epoch;
        self.next_id += 1;
        self.next_epoch += 1;

        // A fresh terminal should be visible, not hidden behind settings.
        self.settings_open = false;

        let (init_cols, init_rows) = self.default_grid_size();
        match Session::spawn(init_cols, init_rows, home_dir(), profile) {
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
                    axis: SplitAxis::Columns,
                    maximized: false,
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
        self.hover_tab = None;
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
        if tab.panes.len() <= 1 {
            let index = self.active;
            self.close_tab(index, cx);
            return;
        }
        tab.panes.remove(tab.active_pane);
        if tab.active_pane >= tab.panes.len() {
            tab.active_pane = tab.panes.len() - 1;
        }
        if tab.panes.len() == 1 {
            tab.maximized = false;
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
                        SessionMsg::Output(bytes) => pane.session.borrow_mut().feed(&bytes),
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

    /// Cycle to the next theme preset (Ctrl+Shift+T).
    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = self.registry.next(self.theme_id);
        self.set_theme(next, cx);
    }

    /// Apply a registry preset and keep every mirror in sync (settings pills
    /// and the keyboard shortcut both land here).
    fn set_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.theme_id = index.min(self.registry.len() - 1);
        self.theme = self.registry.preset(self.theme_id).theme.clone();
        crate::rounded_corners::set_dark_mode(self.theme_id == 1);
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

    /// Full settings page shown in place of the terminal sheet.
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

        let mut theme_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Theme"));
        for index in 0..self.registry.len() {
            theme_row = theme_row.child(pill_button(
                self.registry.preset(index).name.to_string(),
                10 + index,
                self.theme_id == index,
                theme,
                move |view, cx| view.set_theme(index, cx),
                cx,
            ));
        }

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
                                    .child("Atlas Terminal 0.1.0"),
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

        div()
            .id(ElementId::named_usize("settings-page", 0))
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
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

        if key == "escape" && (self.settings_open || self.profile_menu) {
            self.settings_open = false;
            if self.profile_menu {
                self.dismiss_profile(cx);
            }
            cx.notify();
            return;
        }

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.toggle_theme(cx),
                "n" => self.new_tab(Profile::PowerShell, cx),
                "w" => {
                    let index = self.active;
                    self.close_tab(index, cx);
                }
                "v" => {
                    self.paste(cx);
                    self.focus_main(window);
                }
                "p" => self.open_palette(window, cx),
                "tab" => {
                    self.cycle(-1);
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        if modifiers.control && !modifiers.shift && !modifiers.alt {
            match key {
                "tab" => {
                    self.cycle(1);
                    cx.notify();
                    return;
                }
                _ => {}
            }
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
        if !self.profile_menu {
            self.profile_closing = false;
        }
        let theme = self.theme.clone();

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
                    if active {
                        tab_name(&pane.title, &profile_title(pane.profile))
                    } else {
                        dir_label(pane)
                            .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)))
                    }
                }),
                None => "Terminal".to_string(),
            };
            let icon = match pane {
                Some(pane) if active => session_icon(&pane.title, pane.profile),
                _ => icons::FOLDER,
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
            let pill_close = active && self.hover_tab == Some(ix);
            let mut pill = div()
                .id(ElementId::named_usize("tab-pill", ix))
                .h(px(24.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .flex_shrink_0()
                .rounded(px(6.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg)
                .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                .child(
                    div()
                        .text_color(pill_icon)
                        .child(icons::icon(icon).size(px(12.0)).text_color(pill_icon)),
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
                )
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
            if pill_close {
                let close_hot = tb.chrome_fg_active;
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", ix))
                        .w(px(14.0))
                        .h(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .text_color(tb.faint_fg)
                        .hover(move |style| style.text_color(close_hot).bg(tb.card_hover))
                        .child(icons::icon(icons::CLOSE).size(px(9.0)).text_color(tb.faint_fg))
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        })),
                );
            }
            bar = bar.child(pill.with_animation(
                ElementId::named_usize("tab-pill-in", tab.id as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_out_cubic),
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
                .w(px(24.0))
                .h(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .hover(move |style| style.bg(plus_hover))
                .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                    set_hover_mark(view, HOVER_MARK_NEW_TAB, *hovered, cx);
                }))
                .child(icons::icon(icons::PLUS).size(px(12.0)).text_color(plus_ink))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.new_tab(Profile::PowerShell, cx);
                    window.focus(&view.focus);
                })),
        );

        bar = bar.child(caption_spacer(0));

        // Window controls: clean 44px wide native vector controls filling top bar height
        let zoomed = crate::rounded_corners::is_zoomed();
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
            bar = bar.child(
                div()
                    .id(ElementId::named_usize("window-control", index))
                    .w(px(44.0))
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
                            .size(px(12.5))
                            .text_color(ink),
                    )
                    .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                        crate::rounded_corners::window_button(index);
                        cx.notify();
                    })),
            );
        }

        // ---- Terminal area -------------------------------------------
        let index = self.active.min(self.tabs.len().saturating_sub(1));
        let body: AnyElement = if self.settings_open {
            self.settings_page(&theme, cx)
        } else if self.tabs.is_empty() {
            div().flex_1().into_any_element()
        } else {
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
                let mut cell = div()
                    .id(ElementId::named_usize("pane-cell", pane_ix))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .bg(sheet_bg)
                    .px(px(14.0))
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

        // ---- Whisper header + floating sheet --------------------------
        // The sheet is the paper: a quiet title on the left and geometric
        // micro marks (split vertical / split horizontal / close) right.
        //
        // Sheet and header share one background: the active terminal's
        // effective background (an OSC 11 override when a TUI declares one),
        // so the card always reads as a single rounded surface.
        let over = self
            .active_pane()
            .map(|pane| *pane.session.borrow().term.colors())
            .unwrap_or_default();
        let (sheet_bg, _, _) = theme.defaults(&over);
        let header_theme = theme.clone();
        let label = if self.settings_open {
            "Settings".to_string()
        } else {
            match self.tabs.get(index) {
                Some(tab) => match tab.panes.get(tab.active_pane) {
                    Some(pane) => tab
                        .custom_title
                        .clone()
                        .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile))),
                    None => "Atlas".to_string(),
                },
                None => "Atlas".to_string(),
            }
        };
        // The header mark is the running process's own icon — a brand mark
        // only when that brand is actually the process.
        let head_icon = if self.settings_open {
            icons::SETTINGS
        } else {
            match self
                .tabs
                .get(index)
                .and_then(|tab| tab.panes.get(tab.active_pane))
            {
                Some(pane) => session_icon(&pane.title, pane.profile),
                None => icons::TERMINAL,
            }
        };
        let zoomed = crate::rounded_corners::is_zoomed();
        let mut head = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .h(px(28.0))
            .pl(px(14.0))
            .pr(px(10.0))
            .bg(sheet_bg)
            .border_b_1()
            .border_color(theme.hairline)
            .rounded_t(if zoomed { px(0.0) } else { px(10.0) })
            .text_color(header_theme.chrome_fg)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(icons::icon(head_icon).size(px(12.5)).text_color(header_theme.chrome_fg))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    ),
            )
            .child(div().flex_1());
        if self.settings_open {
            let close_hover = theme.card_hover;
            let close_hot = theme.chrome_fg_active;
            head = head.child(
                div()
                    .id(ElementId::named_usize("settings-close", 0))
                    .w(px(22.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_color(theme.faint_fg)
                    .hover(move |style| style.bg(close_hover).text_color(close_hot))
                    .child(icons::icon(icons::CLOSE).size(px(12.0)).text_color(theme.faint_fg))
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    })),
            );
        } else {
            head = head
                .child(micro_action(
                    ElementId::named_usize("pane-action", 0),
                    icons::SPLIT_COLUMNS,
                    HOVER_MARK_ACTION,
                    self.hover_mark == Some(HOVER_MARK_ACTION),
                    &theme,
                    |view, window, cx| {
                        view.split_active(SplitAxis::Columns, cx);
                        window.focus(&view.focus);
                    },
                    cx,
                ))
                .child(micro_action(
                    ElementId::named_usize("pane-action", 1),
                    icons::SPLIT_ROWS,
                    HOVER_MARK_ACTION + 1,
                    self.hover_mark == Some(HOVER_MARK_ACTION + 1),
                    &theme,
                    |view, window, cx| {
                        view.split_active(SplitAxis::Rows, cx);
                        window.focus(&view.focus);
                    },
                    cx,
                ))
                .child(micro_action(
                    ElementId::named_usize("pane-action", 2),
                    icons::CLOSE,
                    HOVER_MARK_ACTION + 2,
                    self.hover_mark == Some(HOVER_MARK_ACTION + 2),
                    &theme,
                    |view, window, cx| {
                        view.close_pane(cx);
                        view.focus_main(window);
                    },
                    cx,
                ));
        }

        let sheet = div()
            .id(ElementId::named_usize("terminal-sheet", 0))
            .flex()
            .flex_col()
            .flex_1()
            .w(relative(1.0))
            .mx(if zoomed { px(0.0) } else { px(8.0) })
            .mb(if zoomed { px(0.0) } else { px(8.0) })
            .rounded(if zoomed { px(0.0) } else { px(10.0) })
            .border_1()
            .border_color(if zoomed { rgba(0x00000000) } else { theme.sheet_border })
            .bg(sheet_bg)
            .child(head)
            .child(body);

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .bg(theme.panel_bg)
            .font_family("Segoe UI")
            .text_color(theme.fg);

        if !zoomed {
            root = root
                .border_1()
                .border_color(theme.hairline);
        }

        root = root
            .child(bar)
            .child(sheet);
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
        // Integer cell metrics: block/box quads and merged background strips
        // must land on whole pixels or hairline seams appear between cells.
        let cell_width = (text_system
            .em_advance(font_id, font_size)
            .map(f32::from)
            .unwrap_or(7.8)
            .max(1.0))
        .round();
        let cell_height = (f32::from(font_size) * LINE_SCALE).round().max(1.0);

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
        window.paint_quad(fill(bounds, default_bg));

        let text_system = window.text_system().clone();
        let font_size = px(self.font_size);
        let font_id = text_system.resolve_font(&self.font);
        let cell_width = (text_system
            .em_advance(font_id, font_size)
            .map(f32::from)
            .unwrap_or(7.8)
            .max(1.0))
        .round();
        let cell_height = (f32::from(font_size) * LINE_SCALE).round().max(1.0);

        // Integer pixel snapping: rounded cell sizes + whole-pixel origin
        // keep every quad boundary crisp.
        let left = f32::from(bounds.left()).round();
        let top = f32::from(bounds.top()).round();

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

                let mut fg = theme.resolve(cell.fg, flags.contains(Flags::DIM), &over);
                let mut bg = theme.resolve(cell.bg, false, &over);
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

            // Cell backgrounds: one quad per run of equal colour, so
            // highlight strips (prompt rows, selections) paint as a single
            // seamless rectangle instead of per-cell quads.
            let mut col = 0usize;
            let mut span_start = 0usize;
            let mut span_bg = default_bg;
            let flush = |col: usize, span_start: usize, span_bg: Rgba, window: &mut Window| {
                if span_bg != default_bg && col > span_start {
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(px(x(span_start)), px(y0)),
                            point(px(x(col)), px(y1)),
                        ),
                        span_bg,
                    ));
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

            // Cursor: solid block fills the cell (its colours were inverted
            // above, so the glyph stays readable); hollow block is an outline;
            // beam and underline are bars. All shapes sit on exact cell bounds.
            if let Some((cursor_row, cursor_col)) = cursor_cell.filter(|_| self.cursor_on) {
                if cursor_row == row_index {
                    let x0 = x(cursor_col);
                    let x1 = x0 + cell_width;
                    let color = default_cursor;
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
                if run.style.bold {
                    font = font.bold();
                }
                if run.style.italic {
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
}

/// Palette entries: command, vector icon path, label, shortcut hint.
const COMMANDS: [(Command, &str, &str, &str); 15] = [
    (Command::NewTab, icons::PLUS, "New tab", "Ctrl Shift N"),
    (Command::NewTabWsl, icons::TERMINAL, "New Ubuntu (WSL) tab", ""),
    (Command::SplitPane, icons::SPLIT_COLUMNS, "Split pane right", ""),
    (Command::SplitPaneDown, icons::SPLIT_ROWS, "Split pane down", ""),
    (
        Command::MaximizePane,
        icons::WINDOW_MAXIMIZE,
        "Toggle maximize pane",
        "",
    ),
    (Command::ClosePane, icons::CLOSE, "Close pane", ""),
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
    let entries: [(&'static str, Profile); 2] = [
        ("PowerShell", Profile::PowerShell),
        ("Ubuntu (WSL)", Profile::Wsl),
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
        .with_easing(ease_out_cubic),
        move |el, delta| {
            let t = if closing { 1.0 - delta } else { delta };
            el.opacity(t).top(px(40.0 - 6.0 * (1.0 - t)))
        },
    )
    .into_any_element()
}

/// Hover-state ids for chrome marks.
const HOVER_MARK_NEW_TAB: u64 = 0;
const HOVER_MARK_ACTION: u64 = 1; // + action index
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

/// Micro pane-header action: a native vector icon with clean visibility that brightens on hover.
fn micro_action(
    id: ElementId,
    icon_path: &'static str,
    hover_id: u64,
    hovered: bool,
    theme: &Theme,
    on_click: impl Fn(&mut AtlasView, &mut Window, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let idle = theme.chrome_fg;
    let hot = theme.chrome_fg_active;
    let ink = if hovered { hot } else { idle };
    let hover_bg = theme.card_hover;
    div()
        .id(id)
        .w(px(24.0))
        .h(px(22.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .hover(move |style| style.bg(hover_bg))
        .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
            set_hover_mark(view, hover_id, *hovered, cx);
        }))
        .child(icons::icon(icon_path).size(px(13.0)).text_color(ink))
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

/// Process mark for a tab, keyed off the shell-reported title (OSC 0/2):
/// CLIs advertise themselves there (`claude`, node tooling, Python, cargo),
/// Linux shells land on the Ubuntu mark, PowerShell on the terminal prompt.
/// A brand mark appears only when that brand is the process actually
/// running — never as decoration. Drawn monochrome (the parent colours it).
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
    } else if matches!(profile, Profile::Wsl) {
        icons::TERMINAL
    } else {
        icons::TERMINAL
    }
}

/// Default title for a freshly spawned profile.
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
        button = button.bg(active_bg);
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
