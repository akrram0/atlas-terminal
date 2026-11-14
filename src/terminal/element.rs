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
    IntoElement, LayoutId, PathBuilder, Pixels, Rgba, StrikethroughStyle, Style, TextRun, UnderlineStyle,
    Window, fill, point, px, relative, rgba,
};

use crate::session::Session;
use crate::theme::Theme;
use crate::ui::fonts::icon_font;
use super::procedural::{CharClass, char_class, ProceduralDraw, render_procedural};

pub struct TerminalElement {
    pub session: Rc<RefCell<Session>>,
    pub theme: Theme,
    pub active_bg: Rgba,
    pub font: Font,
    pub font_size: f32,
    pub line_height: f32,
    pub cursor_on: bool,
    pub cursor_override: Option<CursorShape>,
    pub cursor_motion: crate::workspace::CursorMotionState,
    pub cursor_alpha: f32,
    pub subpixel_y: f32,
    pub scrollbar_opacity: f32,
    pub scrollbar_dragging: bool,
    pub scrollbar_mode: String,
    pub hovered_link: Option<crate::url::DetectedUrl>,
    pub search_matches: Vec<(alacritty_terminal::index::Point, alacritty_terminal::index::Point)>,
    pub active_search_match: Option<usize>,
    pub ghost_text: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
struct CellStyle {
    fg: Rgba,
    bg: Rgba,
    bold: bool,
    italic: bool,
    underline: bool,
    undercurl: bool,
    is_link: bool,
    underline_color: Option<Rgba>,
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

        let is_tui = self.session.borrow().term.mode().contains(TermMode::ALT_SCREEN);
        let (pad_x, pad_y) = if is_tui {
            (0.0, 0.0)
        } else {
            (Self::PAD_X, Self::PAD_Y)
        };

        let avail_w = (f32::from(bounds.size.width) - pad_x * 2.0).max(cell_width);
        let avail_h = (f32::from(bounds.size.height) - pad_y * 2.0).max(cell_height);
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
        let is_tui = self.session.borrow().term.mode().contains(TermMode::ALT_SCREEN);
        let (pad_x, pad_y) = if is_tui {
            (0.0, 0.0)
        } else {
            (Self::PAD_X, Self::PAD_Y)
        };
        let avail_w = (f32::from(bounds.size.width) - pad_x * 2.0).max(cell_width);
        let cols = (avail_w / cell_width).floor().max(1.0) as usize;
        let avail_h = (f32::from(bounds.size.height) - pad_y * 2.0).max(cell_height);
        let max_rows = (avail_h / cell_height).floor().max(1.0) as usize;

        // Integer pixel snapping: rounded cell sizes + whole-pixel origin with padding
        let left = (f32::from(bounds.left()) + pad_x).round();
        let top = (f32::from(bounds.top()) + pad_y).round();

        // Store active grid dimensions for mouse selection hit testing
        self.session.borrow_mut().grid_bounds = Some((left, top, cell_width, cell_height));

        // Snapshot the grid into paint-ready rows (no borrows outlive this block).
        let (rows, cursor_cell, cursor_shape, selection_range, max_active_row, offset, placed_images, cur_history, is_alt_screen, is_target_wide) = {
            let session = self.session.borrow();
            let is_alt_screen = session.term.mode().contains(TermMode::ALT_SCREEN);
            let show_cursor = session.term.mode().contains(TermMode::SHOW_CURSOR);
            let content = session.term.renderable_content();
            let offset = content.display_offset as i32;
            let cursor_shape = self.cursor_override.unwrap_or(content.cursor.shape);
            let cursor_row = content.cursor.point.line.0 + offset;
            let cursor_col = content.cursor.point.column.0;
            // The cursor is logically linked to the input field / prompt line.
            // If the application hides the cursor (\x1b[?25l) or scrolls into history,
            // the hardware cursor must NOT be drawn over UI elements, tabs, or history text.
            let cursor_cell = if show_cursor && cursor_row >= 0 && (cursor_row as usize) < max_rows {
                Some((cursor_row as usize, cursor_col))
            } else {
                None
            };
            let cursor_inverts = matches!(
                cursor_shape,
                CursorShape::Block
            );
            let is_settled = self.cursor_motion.is_settled;

            let selection_range = session.term.selection.as_ref().and_then(|s| s.to_range(&session.term));
            let placed_images = session.placed_images.clone();
            let cur_history = session.term.history_size() as i32;
            let mut is_target_wide = false;
            let mut rows: Vec<Row> = Vec::new();
            for indexed in content.display_iter {
                let point = indexed.point;
                let cell = indexed.cell;

                let virtual_row = point.line.0 + offset;
                if virtual_row < 0 || (virtual_row as usize) >= max_rows {
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
                if cursor_cell == Some((row_index, col)) && flags.contains(Flags::WIDE_CHAR) {
                    is_target_wide = true;
                }

                // Second half of a wide char: belongs to the previous run.
                if flags.contains(Flags::WIDE_CHAR_SPACER) {
                    if let Some(last) = row.runs.last_mut() {
                        last.cells += 1;
                    }
                    continue;
                }

                let mut fg = theme.resolve(cell.fg, flags.contains(Flags::DIM), &over);
                let mut bg = theme.resolve(cell.bg, false, &over);

                let mut is_search_active = false;
                let mut is_search_match = false;
                if !self.search_matches.is_empty() {
                    for (i, (start, end)) in self.search_matches.iter().enumerate() {
                        if point >= *start && point <= *end {
                            if self.active_search_match == Some(i) {
                                is_search_active = true;
                                break;
                            } else {
                                is_search_match = true;
                            }
                        }
                    }
                }
                if is_search_active {
                    bg = Rgba { a: 0.55, ..theme.accent };
                    fg = theme.elevated_bg;
                } else if is_search_match {
                    bg = Rgba { a: 0.25, ..theme.accent };
                }

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
                if cursor_inverts && is_settled && self.cursor_on && cursor_cell == Some((row_index, col)) {
                    // Invert foreground ink only when settled so text remains crisp and legible
                    // on top of the kinetic cursor, without flashing or strobing while moving.
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
                // High-contrast, fully opaque foreground ink prevents subpixel glyph edge erosion on transparent swapchains
                fg.a = 1.0;
                let is_undercurl = flags.contains(Flags::UNDERCURL);
                let is_any_underline = flags.intersects(Flags::ALL_UNDERLINES);
                let has_underline = is_link_hovered || is_any_underline || is_undercurl;
                let underline_color = cell.underline_color().map(|c| theme.resolve(c, false, &over));

                let style = CellStyle {
                    fg,
                    bg,
                    bold: flags.contains(Flags::BOLD) && !is_icon,
                    italic: flags.contains(Flags::ITALIC) && !is_icon,
                    underline: has_underline,
                    undercurl: is_undercurl && !is_link_hovered,
                    is_link: is_link_hovered,
                    underline_color,
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
            let last_content_row = rows.iter().rposition(|r| {
                !r.text.trim().is_empty() || r.runs.iter().any(|run| run.style.bg != default_bg)
            });
            let cursor_row_idx = if cursor_row >= 0 { (cursor_row as usize) + 1 } else { 0 };
            let active_row_count = match last_content_row {
                Some(last_idx) => (last_idx + 1).max(cursor_row_idx),
                None => cursor_row_idx,
            };
            let max_active_row = if is_alt_screen || offset > 0 {
                max_rows
            } else {
                active_row_count.min(max_rows)
            };
            while rows.len() < max_rows {
                rows.push(Row::default());
            }
            (
                rows,
                cursor_cell,
                cursor_shape,
                selection_range,
                max_active_row,
                offset,
                placed_images,
                cur_history,
                is_alt_screen,
                is_target_wide,
            )
        };

        self.session.borrow_mut().max_active_row = max_active_row;

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if self.active_bg.a > 0.01 {
                window.paint_quad(fill(bounds, self.active_bg));
            }
    
        // Paint inline ghost-text autocomplete suggestion
        if let Some(ref ghost) = self.ghost_text {
            if let Some((target_row, target_col)) = cursor_cell {
                let cell_w = cell_width;
                let ghost_col = if cursor_shape == CursorShape::Block {
                    target_col + 1
                } else {
                    target_col
                };
                if ghost_col < cols {
                    let gx0 = left + cell_w * ghost_col as f32;
                    let gy0 = top + cell_height * target_row as f32 + self.subpixel_y;
                    let ghost_fg = Rgba {
                        r: default_cursor.r,
                        g: default_cursor.g,
                        b: default_cursor.b,
                        a: 0.38,
                    };
                    let ghost_run = TextRun {
                        len: ghost.len(),
                        font: self.font.clone(),
                        color: ghost_fg.into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = text_system.shape_line(
                        ghost.clone().into(),
                        font_size,
                        &[ghost_run],
                        Some(px(cell_w)),
                    );
                    let _ = line.paint(
                        point(px(gx0), px(gy0)),
                        px(cell_height),
                        window,
                        cx,
                    );
                }
            }
        }

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
                let is_tui = is_alt_screen || active_bg.a > 0.01;
                let flush = |col: usize, span_start: usize, span_bg: Rgba, window: &mut Window| {
                    let should_skip = if is_tui {
                        span_bg == active_bg
                    } else {
                        span_bg == default_bg || span_bg == active_bg
                    };
                    if !should_skip && col > span_start {
                        let rx0 = x(span_start);
                        let rx1 = x(col);
                        let ry1 = y1;

                        let quad = fill(
                            Bounds::from_corners(
                                point(px(rx0), px(y0)),
                                point(px(rx1), px(ry1)),
                            ),
                            span_bg,
                        );
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
                let sel_span = if selection_range.is_none() {
                    None
                } else if let Some(ref range) = selection_range {
                    let row_line = Line(row_index as i32 - offset);
                    if range.is_block {
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
                            let c0 = start.column.0.min(end.column.0);
                            let c1 = start.column.0.max(end.column.0).min(cols.saturating_sub(1));
                            Some((c0, c1))
                        } else if row_line == start.line {
                            Some((start.column.0.min(cols.saturating_sub(1)), cols.saturating_sub(1)))
                        } else if row_line == end.line {
                            Some((0, end.column.0.min(cols.saturating_sub(1))))
                        } else {
                            Some((0, cols.saturating_sub(1)))
                        }
                    }
                } else {
                    None
                };

                if let Some((sel_c0, sel_c1)) = sel_span {
                    if sel_c1 >= sel_c0 {
                        let sel_bg = Rgba {
                            r: theme.accent.r,
                            g: theme.accent.g,
                            b: theme.accent.b,
                            a: 0.38,
                        };
                        let rx0 = x(sel_c0);
                        let rx1 = x(sel_c1 + 1).min(left + cell_width * cols as f32);
                        let ry1 = y1;

                        let quad = fill(
                            Bounds::from_corners(
                                point(px(rx0), px(y0)),
                                point(px(rx1), px(ry1)),
                            ),
                            sel_bg,
                        );
                        window.paint_quad(quad);
                    }
                }
            }
        });

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

        // Smooth rigid cursor with clean glide and organic breathing alpha
        let avail_h = (f32::from(bounds.size.height) - pad_y * 2.0).max(cell_height);
        let max_rows = (avail_h / cell_height).floor().max(1.0) as usize;

        if let Some((target_row, target_col)) = cursor_cell.filter(|_| self.cursor_on) {
            let target_col = target_col.min(cols.saturating_sub(1));
            let target_row = target_row.min(max_rows.saturating_sub(1));
            let tgt_col_f = target_col as f32;
            let tgt_row_f = target_row as f32;

            let base_w = if is_target_wide { cell_width * 2.0 } else { cell_width };
            let base_h = cell_height;

            let is_settled = self.cursor_motion.is_settled;
            let cursor_color = Rgba {
                r: default_cursor.r,
                g: default_cursor.g,
                b: default_cursor.b,
                a: (default_cursor.a * self.cursor_alpha).clamp(0.0, 1.0),
            };

            let quad = |rx0: f32, ry0: f32, rx1: f32, ry1: f32, color: Rgba, window: &mut Window| {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(px(rx0), px(ry0)),
                        point(px(rx1), px(ry1)),
                    ),
                    color,
                ));
            };

            if is_settled {
                let cx0 = (left + cell_width * tgt_col_f).round().clamp(left, left + avail_w);
                let cy0 = (top + cell_height * tgt_row_f + self.subpixel_y).round().clamp(top, top + avail_h);
                let cx1 = (cx0 + base_w).min(left + avail_w);
                let cy1 = (cy0 + base_h).min(top + avail_h);

                match cursor_shape {
                    CursorShape::Block => quad(cx0, cy0, cx1, cy1, cursor_color, window),
                    CursorShape::HollowBlock => {
                        let t = 1.0;
                        quad(cx0, cy0, cx1, cy0 + t, cursor_color, window);
                        quad(cx0, cy1 - t, cx1, cy1, cursor_color, window);
                        quad(cx0, cy0, cx0 + t, cy1, cursor_color, window);
                        quad(cx1 - t, cy0, cx1, cy1, cursor_color, window);
                    }
                    CursorShape::Beam => quad(cx0, cy0, (cx0 + 2.0).min(left + avail_w), cy1, cursor_color, window),
                    CursorShape::Underline => quad(cx0, (cy1 - 2.0).max(top), cx1, cy1, cursor_color, window),
                    CursorShape::Hidden => {}
                }
            } else {
                // 4-corner warp smear:
                let prev_col = self.cursor_motion.prev_pos.0;
                let prev_row = self.cursor_motion.prev_pos.1;
                let dx = tgt_col_f - prev_col;
                let dy = tgt_row_f - prev_row;

                let elapsed = self.cursor_motion.elapsed_secs;
                let duration = self.cursor_motion.duration_secs.max(0.001);

                // Lead corners move fast (contract tail); trailing corners lag behind
                let trail_size = 0.75f32;
                let dur_lead = duration * (1.0 - trail_size);
                let dur_trail = duration;
                let dur_side = (dur_lead + dur_trail) * 0.5;

                let sx = if dx > 0.0 { 1.0 } else if dx < 0.0 { -1.0 } else { 0.0 };
                let sy = if dy > 0.0 { 1.0 } else if dy < 0.0 { -1.0 } else { 0.0 };

                let get_corner_prog = |cx_dir: f32, cy_dir: f32| -> f32 {
                    let dot = cx_dir * sx + cy_dir * sy;
                    let dur = if dot > 0.5 {
                        dur_lead
                    } else if dot > -0.5 {
                        dur_side
                    } else {
                        dur_trail
                    };
                    let corner_t = (elapsed / dur).clamp(0.0, 1.0);
                    // EaseOutCirc: sqrt(1 - (t - 1)^2)
                    (1.0 - (corner_t - 1.0) * (corner_t - 1.0)).sqrt()
                };

                let p_tl = get_corner_prog(-1.0, -1.0);
                let p_tr = get_corner_prog(1.0, -1.0);
                let p_br = get_corner_prog(1.0, 1.0);
                let p_bl = get_corner_prog(-1.0, 1.0);

                let px0 = (left + cell_width * prev_col).clamp(left, left + avail_w);
                let py0 = (top + cell_height * prev_row + self.subpixel_y).clamp(top, top + avail_h);

                let tx0 = (left + cell_width * tgt_col_f).clamp(left, left + avail_w);
                let ty0 = (top + cell_height * tgt_row_f + self.subpixel_y).clamp(top, top + avail_h);

                let tl = (px0 + (tx0 - px0) * p_tl, py0 + (ty0 - py0) * p_tl);
                let tr = (px0 + base_w + (tx0 - px0) * p_tr, py0 + (ty0 - py0) * p_tr);
                let br = (px0 + base_w + (tx0 - px0) * p_br, py0 + base_h + (ty0 - py0) * p_br);
                let bl = (px0 + (tx0 - px0) * p_bl, py0 + base_h + (ty0 - py0) * p_bl);

                // For axis-aligned motion (dx == 0 or dy == 0), paint exact sharp rectangle
                if dx.abs() < 0.01 {
                    // Vertical motion (exact match for line jumps / user's frame)
                    let min_y = tl.1.min(tr.1).min(bl.1).min(br.1);
                    let max_y = tl.1.max(tr.1).max(bl.1).max(br.1);
                    let shape_w = match cursor_shape {
                        CursorShape::Beam => 2.0f32,
                        _ => base_w,
                    };
                    quad(tx0, min_y, (tx0 + shape_w).min(left + avail_w), max_y, cursor_color, window);
                } else if dy.abs() < 0.01 {
                    // Horizontal motion (typing / backspace)
                    let min_x = tl.0.min(tr.0).min(bl.0).min(br.0);
                    let max_x = tl.0.max(tr.0).max(bl.0).max(br.0);
                    let shape_h = match cursor_shape {
                        CursorShape::Underline => 2.0f32,
                        _ => base_h,
                    };
                    let y_start = match cursor_shape {
                        CursorShape::Underline => ty0 + base_h - 2.0,
                        _ => ty0,
                    };
                    quad(min_x, y_start, max_x, (y_start + shape_h).min(top + avail_h), cursor_color, window);
                } else {
                    // 2D diagonal motion: paint convex quad path
                    let mut b = PathBuilder::fill();
                    b.move_to(point(px(tl.0), px(tl.1)));
                    b.line_to(point(px(tr.0), px(tr.1)));
                    b.line_to(point(px(br.0), px(br.1)));
                    b.line_to(point(px(bl.0), px(bl.1)));
                    b.close();
                    if let Ok(path) = b.build() {
                        window.paint_path(path, cursor_color);
                    }
                }
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
                        match render_procedural(
                            ch,
                            segment.x0,
                            y0,
                            cell_width,
                            cell_height,
                            segment.style.fg,
                        ) {
                            Some(ProceduralDraw::Rects(rects, color)) => {
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
                            Some(ProceduralDraw::Path(path, color)) => {
                                window.paint_path(path, color);
                            }
                            None => {}
                        }
                    }
                    continue;
                }
                let is_wavy = segment.style.undercurl && !segment.style.is_link;
                let ul_color = segment.style.underline_color.unwrap_or(segment.style.fg);
                let text_run = TextRun {
                    len: segment.text.len(),
                    font: segment.font.clone(),
                    color: segment.style.fg.into(),
                    background_color: None,
                    underline: (segment.style.is_link || segment.style.underline || segment.style.undercurl).then(|| UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(ul_color.into()),
                        wavy: is_wavy,
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
                // than 1px off its snapped spot), so long runs can neve
                // accumulate drift off the grid. Segments with wide glyphs
                // (CJK, emoji) or combining marks flow naturally — thei
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
        // Floating scrollbar indicator & drag handle (Apple/macOS style)
        // Suppressed in TUI / alt-screen mode so it never defaces full-screen TUI apps.
        // --------------------------------------------------------------------
        let is_always = self.scrollbar_mode == "always";
        let is_hidden = self.scrollbar_mode == "hidden";
        let effective_opacity = if is_hidden {
            0.0
        } else if self.scrollbar_dragging || is_always {
            1.0
        } else {
            self.scrollbar_opacity
        };

        if !is_alt_screen {
            let (history, offset) = {
                let session = self.session.borrow();
                (session.term.history_size(), session.term.renderable_content().display_offset)
            };
            let display_rows = rows.len().max(1);
            let total_lines = history + display_rows;
            let view_height = f32::from(bounds.size.height);
            let thumb_height = ((display_rows as f32 / total_lines as f32) * view_height)
                .max(24.0)
                .min(view_height - 8.0);
            let scroll_ratio = if history > 0 {
                (history.saturating_sub(offset)) as f32 / history.max(1) as f32
            } else {
                1.0
            };
            let travel_space = (view_height - thumb_height - 8.0).max(0.0);
            let thumb_top = f32::from(bounds.top()) + 4.0 + travel_space * scroll_ratio;
            let thumb_right = f32::from(bounds.right()) - 3.0;
            let thumb_left = thumb_right - 6.0;

            // Record geometry for mouse drag & click hit testing
            self.session.borrow_mut().scrollbar_geometry = Some(crate::session::ScrollbarGeometry {
                bounds_right: f32::from(bounds.right()),
                bounds_top: f32::from(bounds.top()),
                bounds_bottom: f32::from(bounds.bottom()),
                thumb_top,
                thumb_height,
                travel_space,
                history,
                offset,
            });

            if effective_opacity > 0.01 && history > 0 {
                // Subtle track quad (Apple/macOS style) when hovered, dragging, or always-visible
                if self.scrollbar_dragging || is_always || effective_opacity > 0.6 {
                    let mut track_quad = fill(
                        Bounds::from_corners(
                            point(px(thumb_left - 1.0), px(f32::from(bounds.top()) + 4.0)),
                            point(px(thumb_right + 1.0), px(f32::from(bounds.bottom()) - 4.0)),
                        ),
                        Rgba {
                            r: theme.hairline.r,
                            g: theme.hairline.g,
                            b: theme.hairline.b,
                            a: (0.15 * effective_opacity).clamp(0.0, 1.0),
                        },
                    );
                    track_quad.corner_radii = Corners {
                        top_left: px(3.0),
                        top_right: px(3.0),
                        bottom_left: px(3.0),
                        bottom_right: px(3.0),
                    };
                    window.paint_quad(track_quad);
                }

                // Thumb quad
                let thumb_color = if self.scrollbar_dragging {
                    theme.accent
                } else if theme.is_light() {
                    Rgba {
                        r: theme.chrome_fg.r,
                        g: theme.chrome_fg.g,
                        b: theme.chrome_fg.b,
                        a: (0.65 * effective_opacity).clamp(0.0, 1.0),
                    }
                } else {
                    Rgba {
                        r: theme.chrome_fg.r,
                        g: theme.chrome_fg.g,
                        b: theme.chrome_fg.b,
                        a: (0.50 * effective_opacity).clamp(0.0, 1.0),
                    }
                };

                let mut thumb_quad = fill(
                    Bounds::from_corners(
                        point(px(thumb_left), px(thumb_top)),
                        point(px(thumb_right), px(thumb_top + thumb_height)),
                    ),
                    thumb_color,
                );
                thumb_quad.corner_radii = Corners {
                    top_left: px(3.0),
                    top_right: px(3.0),
                    bottom_left: px(3.0),
                    bottom_right: px(3.0),
                };
                window.paint_quad(thumb_quad);
            }
        }
    }
}
