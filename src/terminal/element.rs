//! Terminal element: custom GPUI Element that walks the alacritty grid and
//! paints cell backgrounds, selection quads, text runs, procedural glyphs,
//! cursor, and the auto-hiding scrollbar on the GPU canvas.

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    App, Bounds, ContentMask, Corners, Element, ElementId, Font, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Rgba, StrikethroughStyle, Style, TextRun, UnderlineStyle,
    Window, fill, point, px, relative, rgba,
};

use crate::session::Session;
use crate::theme::Theme;
use crate::ui::fonts::icon_font;
use super::procedural::{CharClass, char_class, procedural_cell};

pub struct TerminalElement {
    pub session: Rc<RefCell<Session>>,
    pub theme: Theme,
    pub active_bg: Rgba,
    pub font: Font,
    pub font_size: f32,
    pub line_height: f32,
    pub cursor_on: bool,
    pub cursor_override: Option<CursorShape>,
    pub cursor_pos: Option<(f32, f32)>,
    pub cursor_alpha: f32,
    pub subpixel_y: f32,
    pub scrollbar_opacity: f32,
    pub hovered_link: Option<crate::url::DetectedUrl>,
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

impl TerminalElement {
    pub const PAD_X: f32 = 10.0;
    pub const PAD_Y: f32 = 10.0;
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
        let test_run = TextRun {
            len: 10,
            font: self.font.clone(),
            color: rgba(0xFFFFFFFF).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let test_line = text_system.shape_line("0123456789".into(), font_size, &[test_run], None);
        let measured_advance = f32::from(test_line.width) / 10.0;
        let cell_width = if measured_advance > 2.0 {
            measured_advance
        } else {
            text_system
                .em_advance(font_id, font_size)
                .map(f32::from)
                .unwrap_or(7.8)
                .max(1.0)
        };
        let cell_height = (f32::from(font_size) * self.line_height).round().max(1.0);

        let avail_w = (f32::from(bounds.size.width) - Self::PAD_X * 2.0).max(cell_width);
        let avail_h = (f32::from(bounds.size.height) - Self::PAD_Y * 2.0).max(cell_height);
        let cols = (avail_w / cell_width).floor().max(1.0) as usize;
        let rows = (avail_h / cell_height).floor().max(1.0) as usize;
        self.session.borrow_mut().resize(cols, rows, cell_width, cell_height);
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

        let text_system = window.text_system().clone();
        let font_size = px(self.font_size);
        let font_id = text_system.resolve_font(&self.font);
        let test_run = TextRun {
            len: 10,
            font: self.font.clone(),
            color: default_bg.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let test_line = text_system.shape_line("0123456789".into(), font_size, &[test_run], None);
        let measured_advance = f32::from(test_line.width) / 10.0;
        let cell_width = if measured_advance > 2.0 {
            measured_advance
        } else {
            text_system
                .em_advance(font_id, font_size)
                .map(f32::from)
                .unwrap_or(7.8)
                .max(1.0)
        };
        let cell_height = (f32::from(font_size) * self.line_height).round().max(1.0);
        let avail_w = (f32::from(bounds.size.width) - Self::PAD_X * 2.0).max(cell_width);
        let cols = (avail_w / cell_width).floor().max(1.0) as usize;
        let avail_h = (f32::from(bounds.size.height) - Self::PAD_Y * 2.0).max(cell_height);
        let max_rows = (avail_h / cell_height).floor().max(1.0) as usize;

        // Integer pixel snapping: rounded cell sizes + whole-pixel origin with padding
        let left = (f32::from(bounds.left()) + Self::PAD_X).round();
        let top = (f32::from(bounds.top()) + Self::PAD_Y).round();

        // Store active grid dimensions for mouse selection hit testing
        self.session.borrow_mut().grid_bounds = Some((left, top, cell_width, cell_height));

        // Snapshot the grid into paint-ready rows (no borrows outlive this block).
        let (rows, cursor_cell, cursor_shape, selection_range, offset, placed_images, cur_history, is_alt_screen) = {
            let session = self.session.borrow();
            let is_alt_screen = session.term.mode().contains(TermMode::ALT_SCREEN);
            let content = session.term.renderable_content();
            let offset = content.display_offset as i32;
            let cursor_shape = self.cursor_override.unwrap_or(content.cursor.shape);
            let cursor_row = content.cursor.point.line.0 + offset;
            let cursor_col = content.cursor.point.column.0;
            // The cursor is logically linked to the input field / prompt line.
            // When the user scrolls up into history, the input line moves below the visible
            // viewport (cursor_row >= max_rows), so the cursor must NOT be drawn over history text.
            let cursor_cell = if cursor_row >= 0 && (cursor_row as usize) < max_rows {
                Some((cursor_row as usize, cursor_col))
            } else {
                None
            };
            let cursor_inverts = matches!(
                cursor_shape,
                CursorShape::Block
            );

            let selection_range = session.term.selection.as_ref().and_then(|s| s.to_range(&session.term));
            let placed_images = session.placed_images.clone();
            let cur_history = session.term.history_size() as i32;
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
                let is_link_hovered = self
                    .hovered_link
                    .as_ref()
                    .map_or(false, |hl| hl.contains(point));
                if is_link_hovered {
                    fg = theme.accent;
                }
                if flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if cursor_inverts && self.cursor_on && cursor_cell == Some((row_index, col)) {
                    // Invert foreground ink only so text remains crisp and legible on top of the kinetic cursor.
                    // NEVER swap or modify cell background: modifying bg causes the cell background pass to
                    // paint a static duplicate quad that separates from the animated cursor during motion.
                    fg = default_bg;
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

                let is_icon = char_class(ch) == CharClass::Icon;
                let style = CellStyle {
                    fg,
                    bg,
                    bold: flags.contains(Flags::BOLD) && !is_icon,
                    italic: flags.contains(Flags::ITALIC) && !is_icon,
                    underline: is_link_hovered || flags.intersects(Flags::ALL_UNDERLINES),
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
            (
                rows,
                cursor_cell,
                cursor_shape,
                selection_range,
                offset,
                placed_images,
                cur_history,
                is_alt_screen,
            )
        };

        let is_zoomed = crate::rounded_corners::is_zoomed();
        let corner_r = if is_zoomed { px(0.0) } else { px(13.0) };
        let total_rows = rows.len();

        for (row_index, row) in rows.iter().enumerate() {
            let y0 = top + cell_height * row_index as f32 + self.subpixel_y;
            let y1 = y0 + cell_height;
            let x = |col: usize| left + cell_width * col as f32;

            // Cell backgrounds: one quad per run of equal colour, so
            // highlight strips (prompt rows) paint as a single seamless rectangle.
            let mut col = 0usize;
            let mut span_start = 0usize;
            let active_bg = self.active_bg;
            let mut span_bg = active_bg;
            let is_bottom_row = row_index + 1 >= total_rows || row_index + 1 >= max_rows;
            let is_tui = is_alt_screen || active_bg.a > 0.01;
            let flush = |col: usize, span_start: usize, span_bg: Rgba, window: &mut Window| {
                let should_skip = if is_tui {
                    span_bg == active_bg
                } else {
                    span_bg == default_bg || span_bg == active_bg
                };
                if !should_skip && col > span_start {
                    let is_left_edge = span_start == 0;
                    let is_right_edge = col >= cols;
                    let rx0 = if is_left_edge { f32::from(bounds.left()) } else { x(span_start) };
                    let rx1 = if is_right_edge { f32::from(bounds.right()) } else { x(col) };
                    let ry1 = if is_bottom_row { f32::from(bounds.bottom()) } else { y1 };

                    let mut quad = fill(
                        Bounds::from_corners(
                            point(px(rx0), px(y0)),
                            point(px(rx1), px(ry1)),
                        ),
                        span_bg,
                    );
                    if is_bottom_row && (is_left_edge || is_right_edge) {
                        quad.corner_radii = Corners {
                            top_left: px(0.0),
                            top_right: px(0.0),
                            bottom_left: if is_left_edge { corner_r } else { px(0.0) },
                            bottom_right: if is_right_edge { corner_r } else { px(0.0) },
                        };
                    }
                    window.paint_quad(quad);
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

            // Selection highlight layer: paints seamlessly behind intact text runs
            // so text shaping and character advances never jitter during drag selection.
            if let Some(ref range) = selection_range {
                let row_line = Line(row_index as i32 - offset);
                let row_sel = if range.is_block {
                    let (min_col, max_col) = if range.start.column <= range.end.column {
                        (range.start.column.0, range.end.column.0)
                    } else {
                        (range.end.column.0, range.start.column.0)
                    };
                    let (min_line, max_line) = if range.start.line <= range.end.line {
                        (range.start.line.0, range.end.line.0)
                    } else {
                        (range.end.line.0, range.start.line.0)
                    };
                    if row_line.0 >= min_line && row_line.0 <= max_line {
                        Some((min_col, max_col.min(cols.saturating_sub(1))))
                    } else {
                        None
                    }
                } else {
                    let (start, end) = if range.start <= range.end {
                        (range.start, range.end)
                    } else {
                        (range.end, range.start)
                    };
                    if row_line < start.line || row_line > end.line {
                        None
                    } else if row_line == start.line && row_line == end.line {
                        Some((start.column.0, end.column.0.min(cols.saturating_sub(1))))
                    } else if row_line == start.line {
                        Some((start.column.0, cols.saturating_sub(1)))
                    } else if row_line == end.line {
                        Some((0, end.column.0.min(cols.saturating_sub(1))))
                    } else {
                        Some((0, cols.saturating_sub(1)))
                    }
                };

                if let Some((sel_c0, sel_c1)) = row_sel {
                    if sel_c1 >= sel_c0 {
                        let sel_bg = Rgba {
                            r: theme.accent.r,
                            g: theme.accent.g,
                            b: theme.accent.b,
                            a: 0.38,
                        };
                        let is_left_edge = sel_c0 == 0;
                        let is_right_edge = sel_c1 + 1 >= cols;
                        let rx0 = if is_left_edge { f32::from(bounds.left()) } else { x(sel_c0) };
                        let rx1 = if is_right_edge { f32::from(bounds.right()) } else { x(sel_c1 + 1) };
                        let ry1 = if is_bottom_row { f32::from(bounds.bottom()) } else { y1 };

                        let mut quad = fill(
                            Bounds::from_corners(
                                point(px(rx0), px(y0)),
                                point(px(rx1), px(ry1)),
                            ),
                            sel_bg,
                        );
                        if is_bottom_row && (is_left_edge || is_right_edge) {
                            quad.corner_radii = Corners {
                                top_left: px(0.0),
                                top_right: px(0.0),
                                bottom_left: if is_left_edge { corner_r } else { px(0.0) },
                                bottom_right: if is_right_edge { corner_r } else { px(0.0) },
                            };
                        }
                        window.paint_quad(quad);
                    }
                }
            }
        }

        // --------------------------------------------------------------------
        // Inline Images (OSC 1337 iTerm2 protocol)
        // Painted on GPU after background/selection quads and before text/cursor.
        // --------------------------------------------------------------------
        if !placed_images.is_empty() {
            let num_screen_rows = rows.len().max(1);
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                for img in &placed_images {
                    // Do not render primary screen images in alternate screen (OpenCode, Vim, etc.),
                    // and do not render alternate screen images in primary screen.
                    if img.is_alt_screen != is_alt_screen {
                        continue;
                    }

                    let grid_line = if is_alt_screen {
                        img.anchor_line
                    } else {
                        img.anchor_line - cur_history
                    };
                    let screen_row = grid_line + offset;

                    let img_bottom_row = screen_row + img.rows as i32;
                    if img_bottom_row <= 0 || screen_row >= num_screen_rows as i32 {
                        continue;
                    }

                    let max_cols = cols.saturating_sub(img.col);
                    let render_cols = img.cols.min(max_cols);
                    if render_cols == 0 {
                        continue;
                    }

                    let img_x = left + cell_width * img.col as f32;
                    let img_y = top + cell_height * screen_row as f32 + self.subpixel_y;
                    let img_w = (cell_width * render_cols as f32).min(avail_w);
                    let img_h = cell_height * img.rows as f32;

                    let image_bounds = Bounds::from_corners(
                        point(px(img_x), px(img_y)),
                        point(px((img_x + img_w).min(left + avail_w)), px(img_y + img_h)),
                    );

                    let _ = window.paint_image(
                        image_bounds,
                        Corners::default(),
                        img.image.clone(),
                        0,
                        false,
                    );
                }
            });
        }

        // Smooth kinetic cursor with organic breathing alpha
        let avail_h = (f32::from(bounds.size.height) - Self::PAD_Y * 2.0).max(cell_height);
        let max_rows = (avail_h / cell_height).floor().max(1.0) as usize;

        if let Some((target_row, target_col)) = cursor_cell.filter(|_| self.cursor_on) {
            let target_col = target_col.min(cols.saturating_sub(1));
            let target_row = target_row.min(max_rows.saturating_sub(1));
            let tgt_col_f = target_col as f32;
            let tgt_row_f = target_row as f32;
            let (cur_col, cur_row) = match self.cursor_pos {
                Some((c, r)) if (c - tgt_col_f).abs() < 15.0 && (r - tgt_row_f).abs() < 10.0 => {
                    (c.min(cols.saturating_sub(1) as f32), r.min(max_rows.saturating_sub(1) as f32))
                }
                _ => (tgt_col_f, tgt_row_f),
            };
            let is_settled = (cur_col - tgt_col_f).abs() < 0.01 && (cur_row - tgt_row_f).abs() < 0.01;
            let cx0 = if is_settled {
                (left + cell_width * tgt_col_f).round().clamp(left, left + avail_w)
            } else {
                (left + cell_width * cur_col).clamp(left, left + avail_w)
            };
            let cy0 = if is_settled {
                (top + cell_height * tgt_row_f + self.subpixel_y).round().clamp(top, top + avail_h)
            } else {
                (top + cell_height * cur_row + self.subpixel_y).clamp(top, top + avail_h)
            };
            let cx1 = (cx0 + cell_width).min(left + avail_w);
            let cy1 = (cy0 + cell_height).min(top + avail_h);
            let cursor_color = Rgba {
                r: default_cursor.r,
                g: default_cursor.g,
                b: default_cursor.b,
                a: (default_cursor.a * self.cursor_alpha).clamp(0.0, 1.0),
            };
            let quad = |rx0: f32, ry0: f32, rx1: f32, ry1: f32, window: &mut Window| {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(px(rx0), px(ry0)),
                        point(px(rx1), px(ry1)),
                    ),
                    cursor_color,
                ));
            };
            match cursor_shape {
                CursorShape::Block => quad(cx0, cy0, cx1, cy1, window),
                CursorShape::HollowBlock => {
                    let t = 1.0;
                    quad(cx0, cy0, cx1, cy0 + t, window);
                    quad(cx0, cy1 - t, cx1, cy1, window);
                    quad(cx0, cy0, cx0 + t, cy1, window);
                    quad(cx1 - t, cy0, cx1, cy1, window);
                }
                CursorShape::Beam => quad(cx0, cy0, cx0 + 2.0, cy1, window),
                CursorShape::Underline => quad(cx0, cy1 - 2.0, cx1, cy1, window),
                CursorShape::Hidden => {}
            }
        }

        for (row_index, row) in rows.iter().enumerate() {
            let y0 = top + cell_height * row_index as f32 + self.subpixel_y;
            let x = |col: usize| left + cell_width * col as f32;

            // Text: one shaped line per segment, each pinned to its starting
            // grid column, so glyph advances (emoji, CJK, Nerd icons) can
            // never drag text — or the cursor — off the cell grid. Icon runs
            // are split per glyph so every icon lands on its own cell.
            struct Segment<'a> {
                text: &'a str,
                x0: f32,
                col: usize,
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
                if run.style.bold && run.style.class != CharClass::Icon {
                    font = font.bold();
                }
                if run.style.italic && run.style.class != CharClass::Icon {
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
                            col: cell,
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
                        col,
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
                // Do not paint text over placed inline images. In OSC 1337 image protocol,
                // the graphic occupies these character cells; any ConPTY reflow or wrapped text
                // landing inside the image's bounds must not deface the image.
                let covered_by_image = placed_images.iter().any(|img| {
                    if img.is_alt_screen != is_alt_screen {
                        return false;
                    }
                    let grid_line = if is_alt_screen {
                        img.anchor_line
                    } else {
                        img.anchor_line - cur_history
                    };
                    let screen_row = grid_line + offset;
                    let r = row_index as i32;
                    let seg_end_col = segment.col + segment.cells;
                    r >= screen_row
                        && r < screen_row + img.rows as i32
                        && segment.col < img.col + img.cols
                        && seg_end_col > img.col
                });
                if covered_by_image {
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

        // --------------------------------------------------------------------
        // Floating auto-hiding scrollbar indicator (Apple/macOS style)
        // --------------------------------------------------------------------
        if self.scrollbar_opacity > 0.01 {
            let (history, offset) = {
                let session = self.session.borrow();
                (session.term.history_size(), session.term.renderable_content().display_offset)
            };
            if history > 0 {
                let display_rows = rows.len().max(1);
                let total_lines = history + display_rows;
                let view_height = f32::from(bounds.size.height);
                let thumb_height = ((display_rows as f32 / total_lines as f32) * view_height)
                    .max(24.0)
                    .min(view_height - 8.0);
                let scroll_ratio = (history.saturating_sub(offset)) as f32 / history.max(1) as f32;
                let travel_space = (view_height - thumb_height - 8.0).max(0.0);
                let thumb_top = f32::from(bounds.top()) + 4.0 + travel_space * scroll_ratio;
                let thumb_right = f32::from(bounds.right()) - 4.0;
                let thumb_left = thumb_right - 4.0;

                let mut thumb_quad = fill(
                    Bounds::from_corners(
                        point(px(thumb_left), px(thumb_top)),
                        point(px(thumb_right), px(thumb_top + thumb_height)),
                    ),
                    Rgba {
                        r: theme.chrome_fg.r,
                        g: theme.chrome_fg.g,
                        b: theme.chrome_fg.b,
                        a: (theme.chrome_fg.a * 0.45 * self.scrollbar_opacity).clamp(0.0, 1.0),
                    },
                );
                thumb_quad.corner_radii = Corners {
                    top_left: px(2.0),
                    top_right: px(2.0),
                    bottom_left: px(2.0),
                    bottom_right: px(2.0),
                };
                window.paint_quad(thumb_quad);
            }
        }
    }
}
