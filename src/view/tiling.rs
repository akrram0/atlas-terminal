use std::rc::Rc;
use std::time::Duration;

use alacritty_terminal::index::Side;
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use gpui::{
    div, px, relative, rgba, Animation, AnimationExt, AnyElement, ClipboardItem,
    Context, ElementId, ExternalPaths, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Rgba,
    ScrollWheelEvent, Styled,
};
use crate::terminal::TerminalElement;
use crate::theme::Theme;
use crate::tiling::{SplitAxis, TileNode};
use crate::ui::animation::{ease_apple_snappy, BODY_IN_MS};
use crate::view::AtlasView;
use crate::workspace::Tab;

impl AtlasView {
    /// Render the terminal workspace according to the Hyprland binary tiling tree.
    pub(crate) fn render_tiled_workspace(
        &self,
        tab: &Tab,
        tab_ix: usize,
        theme: &Theme,
        is_translucent: bool,
        active_bg: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if tab.maximized && tab.panes.len() > 1 {
            // Maximized pane: single full-bleed pane with floating unzoom banner
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(self.render_single_pane(
                    tab.active_pane,
                    tab,
                    tab_ix,
                    theme,
                    is_translucent,
                    active_bg,
                    false,
                    cx,
                ))
                .into_any_element()
        } else if tab.panes.len() <= 1 {
            // Single pane tab: full-bleed canvas with no gaps or mini header
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(self.render_single_pane(
                    0,
                    tab,
                    tab_ix,
                    theme,
                    is_translucent,
                    active_bg,
                    false,
                    cx,
                ))
                .into_any_element()
        } else {
            // Hyprland binary tiling workspace with configurable gaps
            let gap = self.app_settings.tiling_gap.clamp(0.0, 16.0);
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .p(px(gap))
                .child(self.render_tile_node(
                    &tab.tile_tree.root,
                    tab,
                    tab_ix,
                    theme,
                    is_translucent,
                    active_bg,
                    1.0,
                    1.0,
                    cx,
                ))
                .into_any_element()
        }
    }

    /// Recursively render a TileNode (either a leaf pane or a split container).
    fn render_tile_node(
        &self,
        node: &TileNode,
        tab: &Tab,
        tab_ix: usize,
        theme: &Theme,
        is_translucent: bool,
        active_bg: Rgba,
        scale_w: f32,
        scale_h: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            TileNode::Leaf(pane_ix) => {
                if *pane_ix < tab.panes.len() {
                    self.render_single_pane(
                        *pane_ix,
                        tab,
                        tab_ix,
                        theme,
                        is_translucent,
                        active_bg,
                        true,
                        cx,
                    )
                } else {
                    div().flex_1().into_any_element()
                }
            }
            TileNode::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => {
                let split_id = *id;
                let split_axis = *axis;
                let split_ratio = *ratio;
                let tb_divider = theme.clone();

                let gap = self.app_settings.tiling_gap.clamp(0.0, 16.0);
                let div_size = gap.max(2.0);

                match split_axis {
                    SplitAxis::Columns => {
                        let divider = div()
                            .id(ElementId::named_usize("split-divider-v", split_id as usize))
                            .w(px(8.0_f32.max(div_size)))
                            .h(relative(1.0))
                            .flex_none()
                            .cursor_col_resize()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(move |s| s.bg(Rgba { a: 0.15, ..tb_divider.accent }))
                            .child(
                                div()
                                    .w(px(div_size))
                                    .h(relative(1.0))
                                    .rounded(px(div_size * 0.5))
                                    .bg(theme.hairline)
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                                    view.active_split_resize = Some((
                                        split_id,
                                        SplitAxis::Columns,
                                        f32::from(event.position.x),
                                        split_ratio,
                                        scale_w,
                                    ));
                                    cx.notify();
                                }),
                            );

                        div()
                            .id(ElementId::named_usize("tile-split-c", split_id as usize))
                            .flex()
                            .flex_row()
                            .flex_1()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .child(
                                div()
                                    .flex_none()
                                    .w(relative(split_ratio))
                                    .min_w(px(40.0))
                                    .h(relative(1.0))
                                    .flex()
                                    .flex_col()
                                    .child(self.render_tile_node(
                                        first,
                                        tab,
                                        tab_ix,
                                        theme,
                                        is_translucent,
                                        active_bg,
                                        scale_w * split_ratio,
                                        scale_h,
                                        cx,
                                    )),
                            )
                            .child(divider)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(40.0))
                                    .h(relative(1.0))
                                    .flex()
                                    .flex_col()
                                    .child(self.render_tile_node(
                                        second,
                                        tab,
                                        tab_ix,
                                        theme,
                                        is_translucent,
                                        active_bg,
                                        scale_w * (1.0 - split_ratio),
                                        scale_h,
                                        cx,
                                    )),
                            )
                            .into_any_element()
                    }
                    SplitAxis::Rows => {
                        let divider = div()
                            .id(ElementId::named_usize("split-divider-h", split_id as usize))
                            .h(px(8.0_f32.max(div_size)))
                            .w(relative(1.0))
                            .flex_none()
                            .cursor_row_resize()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(move |s| s.bg(Rgba { a: 0.15, ..tb_divider.accent }))
                            .child(
                                div()
                                    .h(px(div_size))
                                    .w(relative(1.0))
                                    .rounded(px(div_size * 0.5))
                                    .bg(theme.hairline)
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                                    view.active_split_resize = Some((
                                        split_id,
                                        SplitAxis::Rows,
                                        f32::from(event.position.y),
                                        split_ratio,
                                        scale_h,
                                    ));
                                    cx.notify();
                                }),
                            );

                        div()
                            .id(ElementId::named_usize("tile-split-r", split_id as usize))
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .child(
                                div()
                                    .flex_none()
                                    .h(relative(split_ratio))
                                    .min_h(px(30.0))
                                    .w(relative(1.0))
                                    .flex()
                                    .flex_col()
                                    .child(self.render_tile_node(
                                        first,
                                        tab,
                                        tab_ix,
                                        theme,
                                        is_translucent,
                                        active_bg,
                                        scale_w,
                                        scale_h * split_ratio,
                                        cx,
                                    )),
                            )
                            .child(divider)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(30.0))
                                    .w(relative(1.0))
                                    .flex()
                                    .flex_col()
                                    .child(self.render_tile_node(
                                        second,
                                        tab,
                                        tab_ix,
                                        theme,
                                        is_translucent,
                                        active_bg,
                                        scale_w,
                                        scale_h * (1.0 - split_ratio),
                                        cx,
                                    )),
                            )
                            .into_any_element()
                    }
                }
            }
        }
    }

    /// Render a single pane card with its mini header bar, borders, and TerminalElement.
    fn render_single_pane(
        &self,
        pane_ix: usize,
        tab: &Tab,
        _tab_ix: usize,
        theme: &Theme,
        is_translucent: bool,
        active_bg: Rgba,
        is_tiled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(pane) = tab.panes.get(pane_ix) else {
            return div().flex_1().into_any_element();
        };

        let pane_down = pane_ix;
        let pane_move = pane_ix;
        let pane_up = pane_ix;
        let pane_rc = pane_ix;

        let is_light = theme.is_light();
        let pane_is_tui = Self::is_pane_tui(pane, theme);
        let pane_bg = if pane_is_tui {
            Self::detect_pane_bg(pane, theme)
        } else if is_translucent && !is_light {
            rgba(0x00000000)
        } else {
            active_bg
        };

        let is_focused_pane = pane_ix == tab.active_pane;
        let mut cell = div()
            .id(ElementId::named_usize("pane-cell", pane_ix))
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden();

        if pane.pane_expansion < 0.999 {
            cell = cell.opacity(pane.pane_expansion);
        }

        if is_tiled {
            let gap = self.app_settings.tiling_gap.clamp(0.0, 16.0);
            let corner = if gap <= 1.0 { px(0.0) } else { px(8.0) };
            let border_color = crate::theme::lerp_color(theme.hairline, theme.accent, pane.focus_progress);
            cell = cell
                .rounded(corner)
                .border_1()
                .border_color(border_color)
                .shadow_md()
                .bg(if pane_is_tui {
                    pane_bg
                } else if is_translucent {
                    rgba(0x00000000)
                } else if is_focused_pane || is_light {
                    theme.bg
                } else {
                    theme.panel_bg
                });
        } else {
            cell = cell.bg(if pane_is_tui {
                pane_bg
            } else if is_light {
                theme.bg
            } else {
                rgba(0x00000000)
            });
        }

        if self
            .hovered_link
            .as_ref()
            .map_or(false, |(p, _)| *p == pane_ix)
        {
            cell = cell.cursor_pointer();
        }

        let pane_drop = pane_ix;
        cell = cell
            .on_drop::<ExternalPaths>(cx.listener(
                move |view, paths: &ExternalPaths, _window, cx| {
                    if let Some(tab) = view.tabs.get_mut(view.active) {
                        tab.active_pane = pane_drop;
                    }
                    view.handle_file_drop(paths.paths(), cx);
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    if view.palette_open || view.settings_open {
                        return;
                    }
                    view.mouse_down_pane = Some(pane_down);
                    if let Some(tab) = view.tabs.get_mut(view.active) {
                        if tab.active_pane != pane_down {
                            tab.active_pane = pane_down;
                            cx.notify();
                        }
                    }
                    window.focus(&view.focus);

                    // Scrollbar hit testing: track click (page up/down) or thumb drag
                    let scrollbar_geom = view
                        .tabs
                        .get(view.active)
                        .and_then(|tab| tab.panes.get(pane_down))
                        .and_then(|pane| pane.session.borrow().scrollbar_geometry);

                    if let Some(geom) = scrollbar_geom {
                        let mx = f32::from(event.position.x);
                        let my = f32::from(event.position.y);
                        if mx >= geom.bounds_right - 16.0
                            && mx <= geom.bounds_right + 4.0
                            && my >= geom.bounds_top
                            && my <= geom.bounds_bottom
                        {
                            if geom.history > 0 {
                                if my < geom.thumb_top {
                                    view.scroll(15);
                                } else if my > geom.thumb_top + geom.thumb_height {
                                    view.scroll(-15);
                                } else {
                                    let scroll_ratio = (geom.history.saturating_sub(geom.offset)) as f32 / geom.history.max(1) as f32;
                                    view.scrollbar_drag = Some(crate::view::ScrollbarDragState {
                                        pane_ix: pane_down,
                                        start_mouse_y: my,
                                        start_scroll_ratio: scroll_ratio,
                                        travel_space: geom.travel_space,
                                        history: geom.history,
                                    });
                                }
                                view.scrollbar_opacity = 1.0;
                                view.last_scroll_time = std::time::Instant::now();
                                cx.notify();
                            }
                            view.mouse_down_pane = None;
                            return;
                        }
                    }

                    // Ctrl+Click opens URL at clicked grid point
                    if event.modifiers.control {
                        if let Some((pt, _)) = view.point_from_mouse(event.position, pane_down) {
                            if let Some(tab) = view.tabs.get(view.active) {
                                if let Some(pane) = tab.panes.get(pane_down) {
                                    let term = &pane.session.borrow().term;
                                    if let Some(detected) = crate::url::find_url_at_point(term, pt)
                                    {
                                        crate::url::open_target(
                                            &detected.url,
                                            &view.app_settings.default_editor,
                                            pane.cwd.as_deref(),
                                        );
                                        view.mouse_down_pane = None;
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_down) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_down)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            0,
                                            col,
                                            row,
                                            false,
                                            event.modifiers,
                                        );
                                        view.last_mouse_btn = Some(0);
                                        view.last_mouse_cell = Some((col, row));
                                    }
                                    return;
                                }
                            }
                        }
                    }

                    if let Some((pt, side)) = view.point_from_mouse(event.position, pane_down) {
                        if let Some(tab) = view.tabs.get_mut(view.active) {
                            if let Some(p) = tab.panes.get_mut(pane_down) {
                                let sel_type = match event.click_count {
                                    2 => SelectionType::Semantic,
                                    3 => SelectionType::Lines,
                                    _ => SelectionType::Simple,
                                };
                                view.mouse_down_pane = Some(pane_down);
                                view.mouse_down_point = Some(pt);
                                view.mouse_down_sel_type = sel_type;
                                let mut s = p.session.borrow_mut();
                                if event.modifiers.shift {
                                    if let Some(ref mut sel) = s.term.selection {
                                        sel.update(pt, side);
                                    }
                                } else {
                                    s.term.selection = Some(
                                        Selection::new(sel_type, pt, side),
                                    );
                                }
                                cx.notify();
                            }
                        }
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    if view.palette_open || view.settings_open {
                        return;
                    }
                    if let Some(tab) = view.tabs.get_mut(view.active) {
                        if tab.active_pane != pane_down {
                            tab.active_pane = pane_down;
                            cx.notify();
                        }
                    }
                    window.focus(&view.focus);
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_down) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_down)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            1,
                                            col,
                                            row,
                                            false,
                                            event.modifiers,
                                        );
                                        view.last_mouse_btn = Some(1);
                                        view.last_mouse_cell = Some((col, row));
                                    }
                                    return;
                                }
                            }
                        }
                    }
                    view.paste(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    if view.palette_open || view.settings_open {
                        return;
                    }
                    if let Some(tab) = view.tabs.get_mut(view.active) {
                        if tab.active_pane != pane_rc {
                            tab.active_pane = pane_rc;
                            cx.notify();
                        }
                    }
                    window.focus(&view.focus);
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_rc) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_rc)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            2,
                                            col,
                                            row,
                                            false,
                                            event.modifiers,
                                        );
                                        view.last_mouse_btn = Some(2);
                                        view.last_mouse_cell = Some((col, row));
                                    }
                                    return;
                                }
                            }
                        }
                    }
                    let selected = view.tabs.get(view.active).and_then(|t| {
                        t.panes
                            .get(pane_rc)
                            .and_then(|p| p.session.borrow().term.selection_to_string())
                    });
                    if let Some(text) = selected {
                        if !text.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if let Some(pane) = tab.panes.get_mut(pane_rc) {
                                    pane.session.borrow_mut().term.selection = None;
                                }
                            }
                        }
                    } else {
                        view.paste(cx);
                    }
                    window.focus(&view.focus);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                    view.last_mouse_btn = None;
                    view.last_mouse_cell = None;
                    view.mouse_down_pane = None;
                    view.mouse_down_point = None;
                    if view.scrollbar_drag.is_some() {
                        view.scrollbar_drag = None;
                        cx.notify();
                        return;
                    }
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_up) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_up)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            0,
                                            col,
                                            row,
                                            true,
                                            event.modifiers,
                                        );
                                    }
                                    return;
                                }
                            }
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(move |view, event: &MouseUpEvent, _window, _cx| {
                    view.last_mouse_btn = None;
                    view.last_mouse_cell = None;
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_up) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_up)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            1,
                                            col,
                                            row,
                                            true,
                                            event.modifiers,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |view, event: &MouseUpEvent, _window, _cx| {
                    view.last_mouse_btn = None;
                    view.last_mouse_cell = None;
                    if !event.modifiers.shift {
                        if let Some(tab) = view.tabs.get(view.active) {
                            if let Some(pane) = tab.panes.get(pane_up) {
                                if Self::is_mouse_reporting(pane) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_up)
                                    {
                                        Self::report_mouse_event(
                                            pane,
                                            2,
                                            col,
                                            row,
                                            true,
                                            event.modifiers,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }),
            )
            .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                // Scrollbar dragging active
                if let Some(ref drag) = view.scrollbar_drag {
                    let my = f32::from(event.position.y);
                    let delta_y = my - drag.start_mouse_y;
                    let delta_ratio = if drag.travel_space > 0.0 {
                        delta_y / drag.travel_space
                    } else {
                        0.0
                    };
                    let new_ratio = (drag.start_scroll_ratio + delta_ratio).clamp(0.0, 1.0);
                    let new_offset = (drag.history as f32 * (1.0 - new_ratio)).round() as usize;
                    let new_offset = new_offset.min(drag.history);

                    if let Some(tab) = view.tabs.get(view.active) {
                        if let Some(p) = tab.panes.get(drag.pane_ix) {
                            let current_offset = p.session.borrow().term.renderable_content().display_offset;
                            let diff = new_offset as i32 - current_offset as i32;
                            if diff != 0 {
                                p.session.borrow_mut().term.scroll_display(alacritty_terminal::grid::Scroll::Delta(diff));
                                view.scrollbar_opacity = 1.0;
                                view.last_scroll_time = std::time::Instant::now();
                                cx.notify();
                            }
                        }
                    }
                    return;
                }

                // Scrollbar hover detection (wakes up indicator)
                let hover_geom = view
                    .tabs
                    .get(view.active)
                    .and_then(|tab| tab.panes.get(pane_move))
                    .and_then(|pane| pane.session.borrow().scrollbar_geometry);

                if let Some(geom) = hover_geom {
                    let mx = f32::from(event.position.x);
                    let my = f32::from(event.position.y);
                    if mx >= geom.bounds_right - 16.0
                        && mx <= geom.bounds_right + 4.0
                        && my >= geom.bounds_top
                        && my <= geom.bounds_bottom
                    {
                        if view.scrollbar_opacity < 0.99 {
                            view.scrollbar_opacity = 1.0;
                            view.last_scroll_time = std::time::Instant::now();
                            cx.notify();
                        }
                    }
                }

                if !event.modifiers.shift {
                    if let Some(tab) = view.tabs.get(view.active) {
                        if let Some(pane) = tab.panes.get(pane_move) {
                            if Self::is_mouse_reporting(pane) {
                                let mode = *pane.session.borrow().term.mode();
                                let is_motion = mode.contains(TermMode::MOUSE_MOTION);
                                let is_drag = mode.contains(TermMode::MOUSE_DRAG);
                                if is_motion || (is_drag && view.last_mouse_btn.is_some()) {
                                    if let Some((col, row)) =
                                        view.grid_coords_from_mouse(event.position, pane_move)
                                    {
                                        if view.last_mouse_cell != Some((col, row)) {
                                            view.last_mouse_cell = Some((col, row));
                                            let btn = view.last_mouse_btn.unwrap_or(3).saturating_add(32);
                                            Self::report_mouse_event(
                                                pane,
                                                btn,
                                                col,
                                                row,
                                                false,
                                                event.modifiers,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Hovered hyperlink check
                if let Some((pt, _)) = view.point_from_mouse(event.position, pane_move) {
                    if let Some(tab) = view.tabs.get(view.active) {
                        if let Some(p) = tab.panes.get(pane_move) {
                            let link = crate::url::find_url_at_point(&p.session.borrow().term, pt);
                            let new_target = link.map(|l| (pane_move, l));
                            if view.hovered_link != new_target {
                                view.hovered_link = new_target;
                                cx.notify();
                            }
                        }
                    }
                }
                // Text selection drag
                if view.mouse_down_pane == Some(pane_move) {
                    if let Some(start_pt) = view.mouse_down_point {
                        if let Some((pt, side)) = view.point_from_mouse(event.position, pane_move) {
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if let Some(p) = tab.panes.get_mut(pane_move) {
                                    let mut sess = p.session.borrow_mut();
                                    let sel_type = view.mouse_down_sel_type;
                                    if let Some(ref mut sel) = sess.term.selection {
                                        sel.update(pt, side);
                                    } else {
                                        let mut sel = Selection::new(
                                            sel_type,
                                            start_pt,
                                            Side::Right,
                                        );
                                        sel.update(pt, side);
                                        sess.term.selection = Some(sel);
                                    }
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            }))
            .on_scroll_wheel(cx.listener(
                move |view, event: &ScrollWheelEvent, _window, cx| {
                    if view.palette_open || view.settings_open || view.profile_menu {
                        return;
                    }
                    if event.modifiers.control {
                        let delta = match event.delta {
                            gpui::ScrollDelta::Lines(d) => d.y,
                            gpui::ScrollDelta::Pixels(d) => f32::from(d.y),
                        };
                        let cur = view.active_font_size();
                        if delta > 0.0 {
                            view.adjust_font_size(cur + 1.0, cx);
                        } else if delta < 0.0 {
                            view.adjust_font_size(cur - 1.0, cx);
                        }
                        return;
                    }
                    if !event.modifiers.shift {
                        let is_reporting = view
                            .tabs
                            .get(view.active)
                            .and_then(|tab| tab.panes.get(pane_down))
                            .map_or(false, Self::is_mouse_reporting);
                        if is_reporting {
                            let coords = view.grid_coords_from_mouse(event.position, pane_down);
                            if let Some(tab) = view.tabs.get_mut(view.active) {
                                if let Some(pane) = tab.panes.get_mut(pane_down) {
                                    pane.cursor_pos = Some(pane.cursor_target);
                                    let delta_y = match event.delta {
                                        gpui::ScrollDelta::Lines(delta) => delta.y,
                                        gpui::ScrollDelta::Pixels(delta) => {
                                            let dy = f32::from(delta.y);
                                            if dy.abs() < 0.1 {
                                                0.0
                                            } else if dy > 0.0 {
                                                1.0
                                            } else {
                                                -1.0
                                            }
                                        }
                                    };
                                    if delta_y != 0.0 {
                                        if let Some((col, row)) = coords {
                                            let btn = if delta_y > 0.0 { 64 } else { 65 };
                                            Self::report_mouse_event(
                                                pane,
                                                btn,
                                                col,
                                                row,
                                                false,
                                                event.modifiers,
                                            );
                                        }
                                    }
                                    return;
                                }
                            }
                        }
                    }
                    let speed = view.app_settings.scroll_speed.clamp(0.2, 3.0);
                    let lines = match event.delta {
                        gpui::ScrollDelta::Lines(delta) => (delta.y as f32) * 0.65 * speed,
                        gpui::ScrollDelta::Pixels(delta) => (f32::from(delta.y) / 32.0) * speed,
                    };
                    view.scroll_smooth(lines, cx);
                },
            ));

        if pane.exited {
            cell = cell
                .flex()
                .flex_col()
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
            let tab_font = tab.font_size.unwrap_or(self.font_size);
            let cell_height = (tab_font * self.line_height).round().max(1.0);
            let subpixel_y = if let Some((bound, pos, _)) = &self.scroll_anim {
                if Rc::ptr_eq(bound, &pane.session) {
                    (pos - pos.ceil()) * cell_height
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let typing_recent = self.cursor_last_typing.elapsed().as_millis() < 450;
            let cursor_alpha = if !self.cursor_blink || typing_recent {
                1.0
            } else {
                0.25 + 0.75 * (self.cursor_breath_phase.sin() * 0.5 + 0.5)
            };


        // Compute ghost autocomplete suggestion for active pane
        let ghost_text = if pane_ix == tab.active_pane {
            let session = pane.session.borrow();
            let is_alt_screen = session.term.mode().contains(alacritty_terminal::term::TermMode::ALT_SCREEN);
            let show_cursor = session.term.mode().contains(alacritty_terminal::term::TermMode::SHOW_CURSOR);
            let is_scrolled = session.term.renderable_content().display_offset > 0;

            if !is_alt_screen && show_cursor && !is_scrolled {
                let content = session.term.renderable_content();
                let cursor_row = content.cursor.point.line;
                let cursor_col = content.cursor.point.column.0;
                let grid = session.term.grid();
                let cols = session.cols;

                // Check if cursor is at the end of input (cells after cursor_col are empty)
                let is_end_of_input = (cursor_col..cols).all(|c| {
                    grid[cursor_row][alacritty_terminal::index::Column(c)].c == ' '
                });

                if is_end_of_input && cursor_col > 0 {
                    let mut line_buf = String::new();
                    for c in 0..cursor_col {
                        line_buf.push(grid[cursor_row][alacritty_terminal::index::Column(c)].c);
                    }
                    let res = crate::tools::system::jump::predict_ghost_suggestion(&line_buf, pane.cwd.as_deref());
                    *self.active_ghost_suggestion.borrow_mut() = res.clone();
                    res
                } else {
                    *self.active_ghost_suggestion.borrow_mut() = None;
                    None
                }
            } else {
                *self.active_ghost_suggestion.borrow_mut() = None;
                None
            }
        } else {
            None
        };

            cell = cell.child(TerminalElement {
                session: pane.session.clone(),
                theme: theme.clone(),
                active_bg: if is_translucent && !pane_is_tui {
                    if is_light {
                        theme.bg
                    } else {
                        rgba(0x00000000)
                    }
                } else {
                    pane_bg
                },
                font: self.font.clone(),
                font_size: tab_font,
                line_height: self.line_height,
                cursor_on: self.cursor_on,
                cursor_override: self.cursor_style,
                cursor_pos: pane.cursor_pos,
                cursor_alpha,
                subpixel_y,
                cursor_velocity: pane.cursor_velocity,
                cursor_trail_mode: self.app_settings.cursor_trail_mode.clone(),
                cursor_trail: pane.cursor_trail.clone(),
                cursor_shape_morph: self.app_settings.cursor_shape_morph,
                cursor_shape_progress: pane.cursor_shape_progress,
                cursor_exit_glow: self.app_settings.cursor_exit_glow,
                scrollbar_opacity: self.scrollbar_opacity,
                scrollbar_dragging: self.scrollbar_drag.as_ref().map_or(false, |d| d.pane_ix == pane_ix),
                scrollbar_mode: self.app_settings.scrollbar_mode.clone(),
                hovered_link: self.hovered_link.as_ref().and_then(|(p, hl)| {
                    if *p == pane_ix {
                        Some(hl.clone())
                    } else {
                        None
                    }
                }),
                search_matches: if self.search_open {
                    self.search_matches.clone()
                } else {
                    Vec::new()
                },
                ghost_text: ghost_text.clone(),
                active_search_match: if self.search_open {
                    Some(self.search_index)
                } else {
                    None
                },
            });
        }

        cell.with_animation(
            ElementId::named_usize("pane-in", pane.epoch as usize),
            Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
            |el, delta| el.opacity(delta),
        )
        .into_any_element()
    }
}
