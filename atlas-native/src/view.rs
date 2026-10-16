//! AtlasView — the whole UI, drawn by Rust.
//!
//! Structure: frameless window -> titlebar (drag region + tab pills + window
//! controls) -> terminal area. The terminal itself is a custom gpui `Element`
//! that walks the alacritty grid and paints cell backgrounds, text runs and
//! the cursor directly on the GPU canvas.

use std::cell::RefCell;
use std::path::PathBuf;
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
use crate::session::{Session, SessionMsg};
use crate::theme::Theme;

const FONT_FAMILY: &str = "Consolas";
const FONT_SIZE: f32 = 15.0;
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
            theme: Theme::cream(),
            font: terminal_font(),
            dark: false,
            settings_open: false,
            font_size: FONT_SIZE,
            cursor_style: None,
            cursor_on: true,
        };
        view.new_tab(cx);

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

        view
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_id;
        let epoch = self.next_epoch;
        self.next_id += 1;
        self.next_epoch += 1;

        match Session::spawn(80, 24, home_dir()) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                self.tabs.push(Tab {
                    id,
                    epoch,
                    title: "PowerShell".to_string(),
                    session: session.clone(),
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
            self.new_tab(cx);
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

        match Session::spawn(80, 24, home_dir()) {
            Ok((session, rx)) => {
                let session = Rc::new(RefCell::new(session));
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.epoch = epoch;
                    tab.session = session.clone();
                    tab.exited = false;
                    tab.title = "PowerShell".to_string();
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
                        SessionMsg::Event(Event::Title(title)) => tab.title = title,
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
        cx.notify();
    }

    fn handle_key(&mut self, ks: &Keystroke, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // Solid cursor while typing; the blink task flips it back on its own.
        self.cursor_on = true;

        if key == "escape" && self.settings_open {
            self.settings_open = false;
            cx.notify();
            return;
        }

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.new_tab(cx),
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
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w(relative(1.0))
            .h(px(46.0))
            .pl(px(12.0))
            .gap(px(8.0))
            .bg(theme.chrome_bg)
            .border_b_1()
            .border_color(theme.hairline);

        for ix in 0..self.tabs.len() {
            let active = ix == self.active;
            let title = short_title(&self.tabs[ix].title);
            let tab_theme = theme.clone();

            // Wrapper carries the pill styling only — it has no click handler,
            // so the close button (a sibling of the title, not a child of a
            // clickable) can never double-fire with tab activation.
            let mut tab = div()
                .id(ElementId::named_usize("tab", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(7.0))
                .h(px(30.0))
                .px(px(14.0))
                .rounded_full()
                .text_size(px(13.0))
                .text_color(if active {
                    tab_theme.chrome_fg_active
                } else {
                    tab_theme.chrome_fg
                });

            if active {
                let bg = tab_theme.tab_active_bg;
                let border = tab_theme.hairline;
                tab = tab.bg(bg).border_1().border_color(border);
            } else {
                let hover = tab_theme.tab_hover_bg;
                tab = tab.hover(move |style| style.bg(hover));
            }

            tab = tab.child(
                div()
                    .id(ElementId::named_usize("tab-title", ix))
                    .child(title)
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.active = ix;
                        window.focus(&view.focus);
                        cx.notify();
                    })),
            );

            let close_theme = theme.clone();
            let close_idle = close_theme.chrome_fg;
            let close_hot = close_theme.chrome_fg_active;
            let close_hover_bg = close_theme.tab_hover_bg;
            tab = tab.child(
                div()
                    .id(ElementId::named_usize("tab-close", ix))
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .text_color(close_idle)
                    .hover(move |style| style.bg(close_hover_bg).text_color(close_hot))
                    .child(icon_glyph(ICON_X, 12.0))
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.close_tab(ix, cx);
                        window.focus(&view.focus);
                    })),
            );

            bar = bar.child(tab);
        }

        let plus_theme = theme.clone();
        let plus_hover = plus_theme.tab_hover_bg;
        let plus_idle = plus_theme.chrome_fg;
        let plus_hot = plus_theme.chrome_fg_active;
        bar = bar.child(
            div()
                .id(ElementId::named_usize("new-tab", 0))
                .h(px(30.0))
                .w(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .text_color(plus_idle)
                .hover(move |style| style.bg(plus_hover).text_color(plus_hot))
                .child(icon_glyph(ICON_PLUS, 14.0))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.new_tab(cx);
                    window.focus(&view.focus);
                })),
        );

        // Spacer: the only drag region, so tabs and buttons stay clickable.
        // Its bounds are reported to the window subclass below, which turns
        // presses here into a native move loop via HTCAPTION.
        bar = bar.child(
            div()
                .id(ElementId::named_usize("titlebar-drag", 0))
                .flex_1()
                .h(px(46.0))
                .relative()
                .child(
                    canvas(
                        move |_bounds, _window, _cx| (),
                        move |bounds, _, _window, _cx| {
                            crate::rounded_corners::set_caption_rect(
                                f32::from(bounds.origin.x),
                                f32::from(bounds.origin.y),
                                f32::from(bounds.size.width),
                                f32::from(bounds.size.height),
                            );
                        },
                    )
                    .absolute()
                    .inset_0(),
                ),
        );

        // Settings gear, grouped with the window controls.
        let gear_theme = theme.clone();
        let gear_hover = gear_theme.tab_hover_bg;
        let gear_idle = gear_theme.chrome_fg;
        let gear_hot = gear_theme.chrome_fg_active;
        bar = bar.child(
            div()
                .id(ElementId::named_usize("settings-toggle", 0))
                .w(px(36.0))
                .h(px(30.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .text_color(gear_idle)
                .hover(move |style| style.bg(gear_hover).text_color(gear_hot))
                .child(icon_glyph(ICON_GEAR, 15.0))
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.toggle_settings(cx);
                    window.focus(&view.focus);
                })),
        );

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
                button_theme.tab_hover_bg
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
                    .h(px(46.0))
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
                .px(px(16.0))
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

        // ---- Pane header ---------------------------------------------
        // Thin row under the titlebar: active tab title + close affordance.
        // (Split actions stay out until real splits exist.)
        let header_title = self
            .active_tab()
            .map(|tab| short_title(&tab.title))
            .unwrap_or_else(|| "PowerShell".to_string());
        let header_hover = theme.tab_hover_bg;
        let header_idle = theme.chrome_fg;
        let header_hot = theme.chrome_fg_active;
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .w(relative(1.0))
            .h(px(34.0))
            .px(px(14.0))
            .gap(px(8.0))
            .bg(theme.bg)
            .border_b_1()
            .border_color(theme.hairline)
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.5))
                    .text_color(theme.chrome_fg)
                    .child(header_title),
            )
            .child(
                div()
                    .id(ElementId::named_usize("pane-close", 0))
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .text_color(header_idle)
                    .hover(move |style| style.bg(header_hover).text_color(header_hot))
                    .child(icon_glyph(ICON_X, 12.0))
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        let index = view.active;
                        view.close_tab(index, cx);
                        window.focus(&view.focus);
                    })),
            );

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .bg(theme.bg)
            .text_color(theme.fg)
            .child(bar)
            .child(header)
            .child(body);

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

/// Terminal font: Consolas for the cell grid; emoji fonts as shaping
/// fallbacks. Nerd icon cells are drawn with the dedicated bundled font
/// instead (see `icon_font`), because fallback families are looked up in the
/// system font collection, where an in-memory font does not appear.
fn terminal_font() -> Font {
    let mut font = gpui::font(FONT_FAMILY);
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
    let hover_bg = theme.tab_hover_bg;
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

    let close_hover = theme.tab_hover_bg;
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
        .top(px(52.0))
        .right(px(12.0))
        .w(px(330.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(14.0))
        .rounded(px(10.0))
        .bg(theme.chrome_bg)
        .border_1()
        .border_color(theme.hairline)
        .child(header_row)
        .child(section("APPEARANCE"))
        .child(theme_row)
        .child(section("TERMINAL"))
        .child(font_row)
        .child(cursor_row)
        .into_any_element()
}

/// `C:\WINDOWS\System32` -> `System32`; long titles get an ellipsis.
fn short_title(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "PowerShell".to_string();
    }
    let base = trimmed
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches(".exe");
    let mut short: String = base.chars().take(24).collect();
    if base.chars().count() > 24 {
        short.push('…');
    }
    short
}
