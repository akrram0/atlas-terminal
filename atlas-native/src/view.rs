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
    AnyElement, App, Bounds, Canvas, ClickEvent, Context, Element, ElementId, FocusHandle, Font,
    GlobalElementId, InspectorElementId, IntoElement, KeyDownEvent, Keystroke, LayoutId,
    MouseButton, MouseDownEvent, PathBuilder, Pixels, Render, Rgba, ScrollWheelEvent,
    StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, WindowControlArea, canvas, div,
    fill, point, px, relative, rgb,
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
            font: gpui::font(FONT_FAMILY),
        };
        view.new_tab(cx);
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

    fn handle_key(&mut self, ks: &Keystroke, cx: &mut Context<Self>) {
        let modifiers = &ks.modifiers;
        let key = ks.key.as_str();

        // App-level chords (swallowed so they never leak into the shell).
        if modifiers.control && modifiers.shift {
            match key {
                "t" => self.new_tab(cx),
                "w" => {
                    let index = self.active;
                    self.close_tab(index, cx);
                }
                "v" => self.paste(cx),
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
                    .hover(move |style| style.bg(close_hover_bg))
                    .child(drawn_icon(ChromeIcon::XSmall, close_idle, close_hot, 1.1).size_full())
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
                .hover(move |style| style.bg(plus_hover))
                .child(drawn_icon(ChromeIcon::Plus, plus_idle, plus_hot, 1.2).size_full())
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.new_tab(cx);
                    window.focus(&view.focus);
                })),
        );

        // Spacer: the only drag region, so tabs and buttons stay clickable.
        // (A Drag area on the whole bar would shadow every child control at
        // the OS hit-test level.)
        bar = bar.child(
            div()
                .id(ElementId::named_usize("titlebar-drag", 0))
                .flex_1()
                .h(px(46.0))
                .window_control_area(WindowControlArea::Drag),
        );

        for (index, (icon, area)) in [
            (ChromeIcon::Min, WindowControlArea::Min),
            (ChromeIcon::Max, WindowControlArea::Max),
            (ChromeIcon::Close, WindowControlArea::Close),
        ]
        .into_iter()
        .enumerate()
        {
            let button_theme = theme.clone();
            let is_close = area == WindowControlArea::Close;
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
                    .hover(move |style| style.bg(hover_bg))
                    .window_control_area(area)
                    .child(drawn_icon(icon, idle, hot, 1.0).size_full()),
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
                    .hover(move |style| style.bg(header_hover))
                    .child(drawn_icon(ChromeIcon::XSmall, header_idle, header_hot, 1.1).size_full())
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        let index = view.active;
                        view.close_tab(index, cx);
                        window.focus(&view.focus);
                    })),
            );

        div()
            .flex()
            .flex_col()
            .w(relative(1.0))
            .h(relative(1.0))
            .bg(theme.bg)
            .text_color(theme.fg)
            .child(bar)
            .child(header)
            .child(body)
    }
}

// ----------------------------------------------------------------------------
// Terminal renderer
// ----------------------------------------------------------------------------

struct TerminalElement {
    session: Rc<RefCell<Session>>,
    theme: Theme,
    font: Font,
}

#[derive(Clone, Copy, PartialEq)]
struct CellStyle {
    fg: Rgba,
    bg: Rgba,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
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
        let font_size = px(FONT_SIZE);
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
        let font_size = px(FONT_SIZE);
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
            let cursor_shape = content.cursor.shape;
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
                if cursor_inverts && cursor_cell == Some((row_index, col)) {
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
            if let Some((cursor_row, cursor_col)) = cursor_cell {
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

            // Text.
            if !row.text.is_empty() {
                let text_runs: Vec<TextRun> = row
                    .runs
                    .iter()
                    .map(|run| {
                        let mut font = self.font.clone();
                        if run.style.bold {
                            font = font.bold();
                        }
                        if run.style.italic {
                            font = font.italic();
                        }
                        TextRun {
                            len: run.bytes,
                            font,
                            color: run.style.fg.into(),
                            background_color: None,
                            underline: run.style.underline.then(|| UnderlineStyle {
                                thickness: px(1.0),
                                color: Some(run.style.fg.into()),
                                wavy: false,
                            }),
                            strikethrough: run.style.strike.then(|| StrikethroughStyle {
                                thickness: px(1.0),
                                color: Some(run.style.fg.into()),
                            }),
                        }
                    })
                    .collect();

                let line = text_system.shape_line(row.text.clone().into(), font_size, &text_runs, None);
                let _ = line.paint(point(px(left), px(y0)), px(cell_height), window, cx);
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
// Drawn icons — vector strokes instead of font glyphs, crisp at any DPI and
// independent of whatever fonts happen to be installed.
// ----------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum ChromeIcon {
    Min,
    Max,
    Close,
    XSmall,
    Plus,
}

/// Line segments forming `icon`, centred in a `w` x `h` box.
fn icon_segments(icon: ChromeIcon, w: f32, h: f32) -> Vec<((f32, f32), (f32, f32))> {
    let cx = w / 2.0;
    let cy = h / 2.0;
    match icon {
        ChromeIcon::Min => vec![((cx - 5.0, cy), (cx + 5.0, cy))],
        ChromeIcon::Max => {
            let (x0, y0, x1, y1) = (cx - 4.5, cy - 4.5, cx + 4.5, cy + 4.5);
            vec![
                ((x0, y0), (x1, y0)),
                ((x1, y0), (x1, y1)),
                ((x1, y1), (x0, y1)),
                ((x0, y1), (x0, y0)),
            ]
        }
        ChromeIcon::Close => vec![
            ((cx - 4.5, cy - 4.5), (cx + 4.5, cy + 4.5)),
            ((cx + 4.5, cy - 4.5), (cx - 4.5, cy + 4.5)),
        ],
        ChromeIcon::XSmall => vec![
            ((cx - 3.5, cy - 3.5), (cx + 3.5, cy + 3.5)),
            ((cx + 3.5, cy - 3.5), (cx - 3.5, cy + 3.5)),
        ],
        ChromeIcon::Plus => vec![
            ((cx - 4.5, cy), (cx + 4.5, cy)),
            ((cx, cy - 4.5), (cx, cy + 4.5)),
        ],
    }
}

/// An icon painted as stroked lines; switches from `idle` to `hot` while the
/// pointer is over its bounds.
fn drawn_icon(icon: ChromeIcon, idle: Rgba, hot: Rgba, width: f32) -> Canvas<()> {
    canvas(
        move |_bounds, _window, _cx| (),
        move |bounds, _, window, _cx| {
            let hovered = bounds.contains(&window.mouse_position());
            let color = if hovered { hot } else { idle };
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let mut builder = PathBuilder::stroke(px(width));
            for ((ax, ay), (bx, by)) in icon_segments(icon, w, h) {
                builder.move_to(point(bounds.left() + px(ax), bounds.top() + px(ay)));
                builder.line_to(point(bounds.left() + px(bx), bounds.top() + px(by)));
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
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
