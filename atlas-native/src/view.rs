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

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Element, ElementId, FocusHandle, Font,
    FontFallbacks, GlobalElementId, InspectorElementId, IntoElement, KeyDownEvent, Keystroke,
    LayoutId, MouseButton, MouseDownEvent, Pixels, Render, Rgba, ScrollWheelEvent,
    StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, canvas, div, fill, point, px,
    relative, rgb,
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

pub struct AtlasView {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    next_epoch: u64,
    focus: FocusHandle,
    theme: Theme,
    font: Font,
    /// Light/dark switch (settings panel + Ctrl+Shift+D).
    dark: bool,
    /// Settings popover visibility.
    settings_open: bool,
    /// New-tab profile menu (the ▾ caret) visibility.
    profile_menu: bool,
    /// Terminal font size in px (settings panel).
    font_size: f32,
    /// Cursor override; `None` follows the shell's DECSCUSR.
    cursor_style: Option<CursorShape>,
    /// Blink phase of the cursor.
    cursor_on: bool,
}

struct Tab {
    id: u64,
    epoch: u64,
    title: String,
    session: Rc<RefCell<Session>>,
    /// Which profile this tab runs (kept so Restart reuses it).
    profile: Profile,
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
            dark: true,
            settings_open: false,
            profile_menu: false,
            font_size: FONT_SIZE,
            cursor_style: None,
            cursor_on: true,
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
                            if tab.exited {
                                continue;
                            }
                            let Some(cwd) =
                                tab.session.borrow().pid.and_then(crate::cwd::process_cwd)
                            else {
                                continue;
                            };
                            if tab.cwd.as_deref() != Some(cwd.as_path()) {
                                tab.cwd = Some(cwd.clone());
                                changed = true;
                            }
                            let branch = git_branch(&cwd);
                            if branch != tab.branch {
                                tab.branch = branch;
                                changed = true;
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

        match Session::spawn(80, 24, home_dir(), profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd);
                self.tabs.push(Tab {
                    id,
                    epoch,
                    title: profile_title(profile),
                    session: session.clone(),
                    profile,
                    cwd,
                    branch: None,
                    exited: false,
                });
                self.active = self.tabs.len() - 1;
                Self::watch(id, epoch, session, rx, cx);
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

    fn restart_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let id = tab.id;
        let epoch = self.next_epoch;
        self.next_epoch += 1;

        match Session::spawn(80, 24, home_dir(), tab.profile) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                let cwd = session.borrow().pid.and_then(crate::cwd::process_cwd);
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.epoch = epoch;
                    tab.session = session.clone();
                    tab.cwd = cwd;
                    tab.branch = None;
                    tab.title = profile_title(tab.profile);
                    tab.exited = false;
                }
                Self::watch(id, epoch, session, rx, cx);
                crate::log_line(&format!("tab {id}: shell restarted"));
            }
            Err(err) => {
                eprintln!("atlas-native: failed to restart shell: {err:#}");
                crate::log_line(&format!("tab {id}: restart failed: {err:#}"));
            }
        }
        cx.notify();
    }

    /// Drain one session's output/event stream into the UI.
    ///
    /// The task exits when the tab is gone or the session's channel closes.
    /// Stale messages from a previous session of the same tab are ignored.
    fn watch(
        id: u64,
        epoch: u64,
        session: Rc<RefCell<Session>>,
        rx: smol::channel::Receiver<SessionMsg>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(msg) = rx.recv().await {
                let keep = this.update(cx, |view, cx| {
                    let Some(tab) = view.tabs.iter_mut().find(|tab| tab.id == id) else {
                        return false;
                    };
                    if tab.epoch != epoch || !Rc::ptr_eq(&tab.session, &session) {
                        return true;
                    }
                    match msg {
                        SessionMsg::Output(bytes) => tab.session.borrow_mut().feed(&bytes),
                        SessionMsg::Event(Event::Title(title)) => {
                            if !title.trim().is_empty() {
                                tab.title = title;
                            }
                        }
                        SessionMsg::Event(Event::Exit) => tab.exited = true,
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

    fn active_mode(&self) -> TermMode {
        self.active_tab()
            .map(|tab| *tab.session.borrow().term.mode())
            .unwrap_or_else(TermMode::empty)
    }

    fn write_to_shell(&self, bytes: &[u8]) {
        if let Some(tab) = self.active_tab() {
            tab.session.borrow_mut().write_bytes(bytes);
        }
    }

    fn scroll(&self, lines: i32) {
        if let Some(tab) = self.active_tab() {
            tab.session
                .borrow_mut()
                .term
                .scroll_display(Scroll::Delta(lines));
        }
    }

    fn paste(&self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        let bracketed = tab
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
        tab.session.borrow_mut().write_bytes(&bytes);
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

    fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.profile_menu = false;
        }
        cx.notify();
    }

    fn toggle_profile_menu(&mut self, cx: &mut Context<Self>) {
        self.profile_menu = !self.profile_menu;
        if self.profile_menu {
            self.settings_open = false;
        }
        cx.notify();
    }

    fn handle_key(&mut self, ks: &Keystroke, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // Solid cursor while typing; the blink task flips it back on its own.
        self.cursor_on = true;

        if key == "escape" && (self.settings_open || self.profile_menu) {
            self.settings_open = false;
            self.profile_menu = false;
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

        // Dead session: only R restarts.
        if self.active_tab().map(|tab| tab.exited).unwrap_or(false) {
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
                .text_color(search_theme.faint_fg)
                .child(icon_glyph(ICON_SEARCH, 13.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .truncate()
                        .child("Search commands, tabs and history"),
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
                ),
        );

        bar = bar.child(caption_spacer(1));

        for (index, (glyph, is_close)) in [
            (ICON_MIN, false),
            (ICON_MAX, false),
            (ICON_CLOSE, true),
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
                    .child(icon_glyph(glyph, 13.0))
                    .on_click(cx.listener(move |_view, _: &ClickEvent, _window, _cx| {
                        crate::rounded_corners::window_button(index);
                    })),
            );
        }

        // ---- Terminal area -------------------------------------------
        let body: AnyElement = if self.tabs.is_empty() {
            div().flex_1().into_any_element()
        } else {
            let index = self.active.min(self.tabs.len() - 1);

            let mut area = div()
                .id(ElementId::named_usize("terminal-area", 0))
                .flex()
                .flex_col()
                .flex_1()
                .w(relative(1.0))
                .bg(theme.bg)
                .px(px(20.0))
                .py(px(12.0))
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                    let keystroke = event.keystroke.clone();
                    view.handle_key(&keystroke, cx);
                }))
                .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                    let lines = match event.delta {
                        gpui::ScrollDelta::Lines(delta) => delta.y as i32,
                        gpui::ScrollDelta::Pixels(delta) => (f32::from(delta.y) / 18.0) as i32,
                    };
                    if lines != 0 {
                        view.scroll(lines);
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _: &MouseDownEvent, window, _cx| {
                        window.focus(&view.focus);
                    }),
                );

            if self.tabs[index].exited {
                area = area
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
                            .child("Press R to restart this tab"),
                    );
            } else {
                area = area.child(TerminalElement {
                    session: self.tabs[index].session.clone(),
                    theme: theme.clone(),
                    font: self.font.clone(),
                    font_size: self.font_size,
                    cursor_on: self.cursor_on,
                    cursor_override: self.cursor_style,
                });
            }

            area.into_any_element()
        };

        // ---- Sidebar --------------------------------------------------
        // Tabs list on the left: item per session + New tab / settings row.
        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(254.0))
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
                .child(chrome_icon_button(
                    ElementId::named_usize("sidebar-action", 0),
                    ICON_COLUMNS,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
                ))
                .child(chrome_icon_button(
                    ElementId::named_usize("sidebar-action", 1),
                    ICON_PANEL,
                    13.0,
                    24.0,
                    24.0,
                    6.0,
                    &theme,
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

            // Name from the shell-reported title; subtitle: the live working
            // directory (PEB poll), falling back to the shell label, plus a
            // dead-session note.
            let name = tab_name(&tab.title);
            let shell = shell_label(&tab.session.borrow().shell);
            let base = match &tab.cwd {
                Some(cwd) => display_cwd(cwd),
                None => shell.to_string(),
            };
            let subtitle = if tab.exited {
                format!("{base} · exited")
            } else {
                base
            };
            let (icon_bg, icon) = session_icon(&tab.title);
            let show_dot = !active && !tab.exited;

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

            list = list.child(item);
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
                            view.toggle_settings(cx);
                            window.focus(&view.focus);
                        })),
                ),
        );

        // ---- Pane header ---------------------------------------------
        // Breadcrumb (folder · parent › leaf) + view actions.
        let header_source = self
            .active_tab()
            .map(|tab| match tab.cwd.as_ref() {
                Some(cwd) => cwd.to_string_lossy().into_owned(),
                None => tab_name(&tab.title),
            })
            .unwrap_or_else(|| "PowerShell".to_string());
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
            .text_color(theme.chrome_fg)
            .child(icon_glyph(ICON_FOLDER, 13.0));

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
            .child(div().flex_1());
        for (index, glyph) in [ICON_SEARCH, ICON_PEN, ICON_ELLIPSIS].into_iter().enumerate() {
            header = header.child(chrome_icon_button(
                ElementId::named_usize("pane-action", index),
                glyph,
                13.0,
                24.0,
                24.0,
                6.0,
                &theme,
            ));
        }

        // ---- Status bar ----------------------------------------------
        let (cols, rows, shell, exited, branch, status_left) = match self.active_tab() {
            Some(tab) => {
                let session = tab.session.borrow();
                let cols = session.cols;
                let rows = session.rows;
                let shell = shell_label(&session.shell);
                drop(session);
                let left = match &tab.cwd {
                    Some(cwd) => display_cwd(cwd),
                    None => tab_name(&tab.title),
                };
                (cols, rows, shell, tab.exited, tab.branch.clone(), left)
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

        // Profile menu floats above the bottom bar; paint it before the
        // settings popover so the settings panel wins overlap contests.
        if self.profile_menu {
            root = root.child(profile_menu(&theme, cx));
        }

        // Settings popover floats above everything; paint it last.
        if self.settings_open {
            root = root.child(settings_panel(
                &theme,
                self.dark,
                self.font_size,
                self.cursor_style,
                cx,
            ));
        }

        root
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
// Chrome icons are Font Awesome glyphs from the bundled Nerd Font (the same
// icon set terminals use), so no hand-drawn vectors. Colors are inherited:
// the parent button sets idle text color, its hover style sets the hot one.
// ----------------------------------------------------------------------------

/// Font Awesome codepoints inside the bundled Symbols Nerd Font Mono.
const ICON_MIN: &str = "\u{F2D1}"; // nf-fa-window_minimize
const ICON_MAX: &str = "\u{F2D0}"; // nf-fa-window_maximize
const ICON_CLOSE: &str = "\u{F2D3}"; // nf-fa-window_close
const ICON_X: &str = "\u{F00D}"; // nf-fa-xmark
const ICON_PLUS: &str = "\u{F067}"; // nf-fa-plus
const ICON_GEAR: &str = "\u{F013}"; // nf-fa-gear
const ICON_SEARCH: &str = "\u{EA6D}"; // nf-cod-search
const ICON_FOLDER: &str = "\u{EA83}"; // nf-cod-folder
const ICON_CHEVRON: &str = "\u{F105}"; // nf-fa-angle_right
const ICON_PEN: &str = "\u{EA73}"; // nf-cod-edit
const ICON_ELLIPSIS: &str = "\u{EA7C}"; // nf-cod-ellipsis
const ICON_TERMINAL: &str = "\u{F120}"; // nf-fa-terminal
const ICON_CUBE: &str = "\u{F1B2}"; // nf-fa-cube
const ICON_CLOUD: &str = "\u{F0C2}"; // nf-fa-cloud
const ICON_COLUMNS: &str = "\u{F0DB}"; // nf-fa-table_columns
const ICON_PANEL: &str = "\u{F022}"; // nf-fa-rectangle_list
const ICON_BRANCH: &str = "\u{F126}"; // nf-fa-code_branch
const ICON_CHEVRON_DOWN: &str = "\u{F078}"; // nf-fa-chevron_down

/// One Nerd Font glyph, centred in its parent; the parent decides the color.
fn icon_glyph(glyph: &'static str, size: f32) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .font(icon_font())
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
fn profile_menu(theme: &Theme, cx: &mut Context<AtlasView>) -> AnyElement {
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
                    view.profile_menu = false;
                    window.focus(&view.focus);
                })),
        );
    }
    menu.into_any_element()
}

/// Small decorative icon button (sidebar header, pane header): quiet by
/// default, brightens on hover, never steals focus from the terminal.
fn chrome_icon_button(
    id: ElementId,
    glyph: &'static str,
    glyph_size: f32,
    w: f32,
    h: f32,
    radius: f32,
    theme: &Theme,
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
        .into_any_element()
}

/// Split a shell-reported title into breadcrumb parts (parent, leaf). A bare
/// drive root like `C:` is dropped; single-component titles become the leaf.
fn breadcrumbs(raw: &str) -> (Option<String>, String) {
    let mut parts: Vec<&str> = raw
        .trim()
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

/// Icon tile for a session, keyed off its shell-reported title: PowerShell
/// blue terminal, WSL/Ubuntu orange cube, remote indigo cloud, dev green.
fn session_icon(title: &str) -> (Rgba, &'static str) {
    let lower = title.to_ascii_lowercase();
    if lower.contains("ubuntu") || lower.contains("wsl") {
        (rgb(0xE95420), ICON_CUBE)
    } else if lower.contains("ssh") || lower.contains("edge") || lower.contains("remote") {
        (rgb(0x6E5BE8), ICON_CLOUD)
    } else if lower.contains("dev")
        || lower.contains("server")
        || lower.contains("vite")
        || lower.contains("uvicorn")
        || lower.contains("node")
        || lower.contains("npm")
    {
        (rgb(0x2EA043), ICON_TERMINAL)
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
        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
            on_click(view, cx);
            window.focus(&view.focus);
        }));
    if active {
        button = button.bg(active_bg);
    }
    button.into_any_element()
}

/// Settings popover: appearance, font size, cursor style.
fn settings_panel(
    theme: &Theme,
    dark: bool,
    font_size: f32,
    cursor_style: Option<CursorShape>,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let section = |label: &'static str| -> AnyElement {
        div()
            .pt(px(4.0))
            .text_size(px(10.5))
            .text_color(theme.chrome_fg)
            .child(label)
            .into_any_element()
    };
    let row_label = |label: &'static str| -> AnyElement {
        div()
            .w(px(78.0))
            .text_size(px(12.0))
            .text_color(theme.chrome_fg)
            .child(label)
            .into_any_element()
    };

    let close_hover = theme.card_active;
    let close_hot = theme.chrome_fg_active;
    let close_button = div()
        .id(ElementId::named_usize("settings-close", 0))
        .w(px(20.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .text_color(theme.chrome_fg)
        .hover(move |style| style.bg(close_hover).text_color(close_hot))
        .child(icon_glyph(ICON_X, 12.0))
        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
            view.settings_open = false;
            window.focus(&view.focus);
            cx.notify();
        }));

    let header_row = div()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .flex_1()
                .text_size(px(13.0))
                .text_color(theme.chrome_fg_active)
                .child("Settings"),
        )
        .child(close_button);

    let theme_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(row_label("Theme"))
        .child(pill_button(
            "Light".to_string(),
            10,
            !dark,
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
            dark,
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
                .child(format!("{} px", font_size as i32)),
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
            cursor_style.is_none(),
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
            cursor_style == Some(CursorShape::Block),
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
            cursor_style == Some(CursorShape::Beam),
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
            cursor_style == Some(CursorShape::Underline),
            theme,
            |view, cx| {
                view.cursor_style = Some(CursorShape::Underline);
                cx.notify();
            },
            cx,
        ));

    div()
        .absolute()
        .left(px(10.0))
        .bottom(px(52.0))
        .w(px(300.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(14.0))
        .rounded(px(10.0))
        .bg(theme.elevated_bg)
        .border_1()
        .border_color(theme.elevated_border)
        .shadow_lg()
        .child(header_row)
        .child(section("APPEARANCE"))
        .child(theme_row)
        .child(section("TERMINAL"))
        .child(font_row)
        .child(cursor_row)
        .into_any_element()
}

/// Sidebar/pane label for a tab: the shell-reported title when it reads as a
/// name (e.g. "vite"), the friendly shell name when it is shell-ish or a
/// bare path (the path itself is the subtitle/breadcrumb).
fn tab_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let shellish = matches!(
        lower.as_str(),
        "powershell" | "pwsh" | "windows powershell" | "powershell 7"
    );
    if trimmed.is_empty() || shellish || trimmed.contains(['\\', '/']) {
        return "PowerShell".to_string();
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
