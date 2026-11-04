use gpui::prelude::*;
use gpui::{
    div, px, relative, rgba, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, Rgba,
};
use crate::icons;
use crate::theme::Theme;
use crate::workspace::{format_tab_title, session_icon};
use super::AtlasView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarMode {
    #[default]
    Expanded,
    Compact,
    Hidden,
}

impl AtlasView {
    pub(crate) fn render_sidebar(
        &self,
        theme: &Theme,
        is_translucent: bool,
        zoomed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tb = theme.clone();
        let current_w = self.sidebar_current_width;
        let sb_width = current_w.round().max(0.0);

        let sidebar_bg = if is_translucent {
            Rgba {
                a: (self.opacity * 0.90).clamp(0.0, 1.0),
                ..tb.panel_bg
            }
        } else {
            tb.panel_bg
        };

        let chrome_ink = if is_translucent {
            crate::theme::lerp_color(tb.chrome_fg, tb.chrome_fg_active, 0.40)
        } else {
            tb.chrome_fg
        };

        let default_prof = self.get_default_profile();
        let is_compact = sb_width < 100.0;

        // =========================================================================
        // 1. COMPACT MODE (Icon Rail — 52px slim dock)
        // =========================================================================
        if is_compact {
            let sb_header_h = if zoomed { px(36.0) } else { px(32.0) };
            let sb_header_pt = if zoomed { px(4.0) } else { px(0.0) };

            // Sidebar cycle button at top of rail (Compact -> Expanded -> Hidden)
            let header = div()
                .h(sb_header_h)
                .pt(sb_header_pt)
                .flex()
                .items_center()
                .justify_center()
                .border_b_1()
                .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.03 })
                .child(
                    div()
                        .id(ElementId::named_usize("sidebar-compact-cycle-btn", 0))
                        .w(px(26.0))
                        .h(px(26.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |s| s.bg(tb.card_hover))
                        .active(|s| s.opacity(0.75))
                        .cursor_pointer()
                        .child(
                            icons::icon(icons::SIDEBAR_LEFT)
                                .size(px(14.0))
                                .text_color(tb.accent),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                            view.cycle_sidebar_mode(cx);
                        })),
                );

            // New Tab button below cycle header (left-click default profile, right-click menu)
            let new_tab_btn = div()
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id(ElementId::named_usize("sidebar-compact-new-tab", 0))
                        .w(px(26.0))
                        .h(px(26.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |s| s.bg(tb.card_hover))
                        .active(|s| s.opacity(0.75))
                        .cursor_pointer()
                        .child(icons::icon(icons::PLUS).size(px(13.0)).text_color(tb.chrome_fg))
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.new_tab(default_prof, cx);
                            window.focus(&view.focus);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                                view.toggle_profile_menu(cx);
                            }),
                        ),
                );

            // Icon tiles stack
            let mut tab_list = div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .py(px(4.0))
                .overflow_hidden();

            for ix in 0..self.tabs.len() {
                let active = ix == self.active && !self.settings_open;
                let tab = &self.tabs[ix];
                let pane = tab.panes.get(tab.active_pane);
                let icon = match pane {
                    Some(pane) => session_icon(&pane.title, pane.profile),
                    None => icons::TERMINAL,
                };

                let (card_bg, card_border, icon_color) = if active {
                    (tb.elevated_bg, tb.elevated_border, tb.accent)
                } else {
                    (rgba(0x00000000), rgba(0x00000000), chrome_ink)
                };

                let is_hovered = self.hover_tab == Some(ix);

                let mut tile = div()
                    .id(ElementId::named_usize("sidebar-compact-tab", ix))
                    .relative()
                    .w(px(36.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(card_bg)
                    .border_1()
                    .border_color(card_border)
                    .cursor_pointer()
                    .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                        if !*hovered && view.hover_tab == Some(ix) {
                            view.hover_tab = None;
                            cx.notify();
                        } else if *hovered && view.hover_tab != Some(ix) {
                            view.hover_tab = Some(ix);
                            cx.notify();
                        }
                    }));

                if !tab.closing {
                    tile = tile
                        .hover(move |s| s.bg(tb.card_hover).text_color(tb.chrome_fg_active))
                        .active(|s| s.opacity(0.82))
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
                }

                tile = tile.child(
                    icons::icon(icon)
                        .size(px(16.0))
                        .text_color(icon_color),
                );

                // Quick close button badge on hover
                if is_hovered && !tab.closing {
                    tile = tile.child(
                        div()
                            .id(ElementId::named_usize("sidebar-compact-close-btn", ix))
                            .absolute()
                            .top(px(-3.0))
                            .right(px(-3.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(tb.elevated_bg)
                            .border_1()
                            .border_color(tb.elevated_border)
                            .shadow_sm()
                            .hover(move |s| s.bg(tb.card_hover))
                            .active(|s| s.opacity(0.75))
                            .cursor_pointer()
                            .child(
                                icons::icon(icons::CLOSE)
                                    .size(px(9.0))
                                    .text_color(tb.chrome_fg_active),
                            )
                            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                                view.close_tab(ix, cx);
                                window.focus(&view.focus);
                            })),
                    );
                }

                tab_list = tab_list.child(tile);
            }

            // Compact Footer
            let is_settings_active = self.settings_open;
            let footer = div()
                .h(px(42.0))
                .flex()
                .items_center()
                .justify_center()
                .border_t_1()
                .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.03 })
                .child(
                    div()
                        .id(ElementId::named_usize("sidebar-compact-settings", 0))
                        .w(px(32.0))
                        .h(px(32.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .bg(if is_settings_active { tb.elevated_bg } else { rgba(0x00000000) })
                        .border_1()
                        .border_color(if is_settings_active { tb.elevated_border } else { rgba(0x00000000) })
                        .hover(move |s| s.bg(tb.card_hover))
                        .active(|s| s.opacity(0.8))
                        .cursor_pointer()
                        .child(
                            icons::icon(icons::SETTINGS)
                                .size(px(16.0))
                                .text_color(if is_settings_active { tb.accent } else { chrome_ink }),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.toggle_settings(window, cx);
                        })),
                );

            return div()
                .id(ElementId::named_usize("vertical-sidebar-panel", 0))
                .flex()
                .flex_col()
                .w(px(sb_width))
                .min_w(px(sb_width))
                .max_w(px(sb_width))
                .h(relative(1.0))
                .min_h(px(0.0))
                .overflow_hidden()
                .bg(sidebar_bg)
                .border_r_1()
                .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.04 })
                .child(header)
                .child(new_tab_btn)
                .child(tab_list)
                .child(footer);
        }

        // =========================================================================
        // 2. EXPANDED MODE (Full Workspace Sidebar with Titles and Resize Handle)
        // =========================================================================
        let is_menu_open = self.profile_menu;
        let capsule_bg = if is_menu_open {
            tb.elevated_bg
        } else if is_translucent {
            Rgba { a: 0.12, ..tb.card_hover }
        } else {
            tb.card_hover
        };
        let capsule_border = if is_menu_open {
            tb.elevated_border
        } else {
            Rgba { a: 0.05, ..tb.hairline }
        };

        let plus_btn = div()
            .id(ElementId::named_usize("sidebar-new-tab-btn", 0))
            .w(px(22.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_l_full()
            .hover(move |s| s.bg(tb.card_hover))
            .active(|s| s.opacity(0.75))
            .cursor_pointer()
            .child(icons::icon(icons::PLUS).size(px(12.5)).text_color(tb.chrome_fg))
            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                view.new_tab(default_prof, cx);
                window.focus(&view.focus);
            }));

        let divider = div()
            .w(px(1.0))
            .h(px(12.0))
            .bg(Rgba { a: 0.20, ..tb.elevated_border });

        let chevron_btn = div()
            .id(ElementId::named_usize("sidebar-chevron-btn", 0))
            .w(px(20.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_r_full()
            .bg(if is_menu_open { tb.card_hover } else { rgba(0x00000000) })
            .hover(move |s| s.bg(tb.card_hover))
            .active(|s| s.opacity(0.75))
            .cursor_pointer()
            .child(icons::icon(icons::CHEVRON_DOWN).size(px(10.5)).text_color(tb.chrome_fg))
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.toggle_profile_menu(cx);
                window.focus(&view.focus);
            }));

        let new_tab_capsule = div()
            .id(ElementId::named_usize("sidebar-launch-capsule", 0))
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .rounded_full()
            .bg(capsule_bg)
            .border_1()
            .border_color(capsule_border)
            .shadow_sm()
            .child(plus_btn)
            .child(divider)
            .child(chevron_btn);

        let sb_header_h = if zoomed { px(36.0) } else { px(32.0) };
        let sb_header_pt = if zoomed { px(4.0) } else { px(0.0) };

        let header = div()
            .h(sb_header_h)
            .pt(sb_header_pt)
            .px(px(10.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.03 })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .id(ElementId::named_usize("sidebar-expanded-cycle-btn", 0))
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.0))
                            .hover(move |s| s.bg(tb.card_hover))
                            .active(|s| s.opacity(0.75))
                            .cursor_pointer()
                            .child(
                                icons::icon(icons::SIDEBAR_LEFT)
                                    .size(px(15.0))
                                    .text_color(tb.accent),
                            )
                            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                view.cycle_sidebar_mode(cx);
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tb.faint_fg)
                            .child("TABS"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h(relative(1.0))
                    .flex()
                    .child(crate::ui::overlay::caption_spacer(2)),
            )
            .child(new_tab_capsule);

        let mut tab_list = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(3.0))
            .p(px(8.0))
            .overflow_hidden();

        for ix in 0..self.tabs.len() {
            let active = ix == self.active && !self.settings_open;
            let tab = &self.tabs[ix];
            let pane = tab.panes.get(tab.active_pane);
            let label = match pane {
                Some(pane) => tab.custom_title.clone().unwrap_or_else(|| format_tab_title(pane)),
                None => "Terminal".to_string(),
            };
            let icon = match pane {
                Some(pane) => session_icon(&pane.title, pane.profile),
                None => icons::TERMINAL,
            };

            let (pill_bg, pill_border, pill_fg, pill_icon) = if active {
                (
                    tb.elevated_bg,
                    tb.elevated_border,
                    tb.chrome_fg_active,
                    tb.accent,
                )
            } else {
                (rgba(0x00000000), rgba(0x00000000), chrome_ink, chrome_ink)
            };

            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };

            let mut tab_row = div()
                .id(ElementId::named_usize("sidebar-tab-row", ix))
                .h(px(32.0))
                .w(relative(1.0))
                .px(px(6.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .rounded(px(6.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg)
                .cursor_pointer();

            let is_dragged = self.tab_drag_origin == Some(ix);
            if is_dragged {
                tab_row = tab_row.opacity(0.80).shadow_md();
            }

            if !tab.closing {
                tab_row = tab_row
                    .hover(move |s| s.bg(pill_hover).text_color(tb.chrome_fg_active))
                    .active(|s| s.opacity(0.82))
                    .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                        if *hovered {
                            if let Some(source_ix) = view.tab_drag_origin {
                                if source_ix != ix {
                                    view.reorder_tab(source_ix, ix, cx);
                                }
                            }
                        }
                        if !*hovered && view.hover_tab == Some(ix) {
                            view.hover_tab = None;
                            cx.notify();
                        } else if *hovered && view.hover_tab != Some(ix) {
                            view.hover_tab = Some(ix);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseDownEvent, _window, cx| {
                            view.tab_drag_origin = Some(ix);
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            if view.tab_drag_origin.is_some() {
                                view.tab_drag_origin = None;
                                cx.notify();
                            }
                        }),
                    )
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
            }

            // Left vertical accent indicator bar
            let indicator = if active {
                div().w(px(3.0)).h(px(16.0)).rounded(px(1.5)).bg(tb.accent)
            } else {
                div().w(px(3.0)).h(px(16.0)).bg(rgba(0x00000000))
            };

            tab_row = tab_row
                .child(indicator)
                .child(
                    div()
                        .text_color(pill_icon)
                        .child(icons::icon(icon).size(px(15.0)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child(label),
                );

            if !tab.closing {
                let close_alpha = tab.close_hover_progress.clamp(0.0, 1.0);
                tab_row = tab_row.child(
                    div()
                        .id(ElementId::named_usize("sidebar-tab-close", ix))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .opacity(close_alpha)
                        .hover(move |s| s.bg(tb.card_hover))
                        .active(|s| s.opacity(0.75))
                        .child(icons::icon(icons::CLOSE).size(px(11.0)).text_color(tb.faint_fg))
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        })),
                );
            }

            tab_list = tab_list.child(tab_row);
        }

        // Footer with Settings item
        let is_settings_active = self.settings_open;
        let footer_pill_bg = if is_settings_active { tb.elevated_bg } else { rgba(0x00000000) };
        let footer_pill_border = if is_settings_active { tb.elevated_border } else { rgba(0x00000000) };
        let footer_icon_color = if is_settings_active { tb.accent } else { chrome_ink };

        let footer = div()
            .h(px(42.0))
            .p(px(6.0))
            .border_t_1()
            .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.03 })
            .child(
                div()
                    .id(ElementId::named_usize("sidebar-settings-btn", 0))
                    .h(px(30.0))
                    .w(relative(1.0))
                    .px(px(8.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .rounded(px(6.0))
                    .bg(footer_pill_bg)
                    .border_1()
                    .border_color(footer_pill_border)
                    .text_color(if is_settings_active { tb.chrome_fg_active } else { chrome_ink })
                    .hover(move |s| s.bg(tb.card_hover).text_color(tb.chrome_fg_active))
                    .active(|s| s.opacity(0.8))
                    .cursor_pointer()
                    .child(
                        icons::icon(icons::SETTINGS)
                            .size(px(15.0))
                            .text_color(footer_icon_color),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(if is_settings_active { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                            .child("Settings"),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.toggle_settings(window, cx);
                    })),
            );

        // Interactive right-edge resize handle
        let resize_handle = div()
            .id(ElementId::named_usize("sidebar-resize-handle", 0))
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(5.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                    view.sidebar_is_resizing = true;
                    cx.notify();
                }),
            );

        div()
            .id(ElementId::named_usize("vertical-sidebar-panel", 0))
            .relative()
            .flex()
            .flex_col()
            .w(px(sb_width))
            .min_w(px(sb_width))
            .max_w(px(sb_width))
            .h(relative(1.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.04 })
            .child(header)
            .child(tab_list)
            .child(footer)
            .child(resize_handle)
    }
}
