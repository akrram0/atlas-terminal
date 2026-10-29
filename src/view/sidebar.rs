use gpui::{
    div, px, relative, rgba, ClickEvent, Context, ElementId, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Rgba, Styled, Window,
};
use crate::icons;
use crate::theme::Theme;
use crate::workspace::{format_tab_title, session_icon};
use super::AtlasView;

impl AtlasView {
    pub(crate) fn render_sidebar(
        &self,
        theme: &Theme,
        is_translucent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tb = theme.clone();
        let sb_width = (210.0 * self.sidebar_expansion).round();

        let sidebar_bg = if is_translucent {
            Rgba {
                a: (0.40 + self.opacity * 0.40).clamp(0.40, 0.85),
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

        // 1. Sidebar Header ("TABS", New Tab, Collapse)
        let header = div()
            .h(px(38.0))
            .px(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(tb.hairline)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tb.faint_fg)
                    .child("TABS"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .id(ElementId::named_usize("sidebar-new-tab", 0))
                            .w(px(24.0))
                            .h(px(24.0))
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
                            })),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("sidebar-hide-btn", 0))
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .hover(move |s| s.bg(tb.card_hover))
                            .active(|s| s.opacity(0.75))
                            .cursor_pointer()
                            .child(icons::icon(icons::CHEVRON_LEFT).size(px(14.0)).text_color(tb.chrome_fg))
                            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                view.sidebar_open = false;
                                cx.notify();
                            })),
                    ),
            );

        // 2. Tab items stack
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

            let is_hovered = self.hover_tab == Some(ix);

            let mut tab_row = div()
                .id(ElementId::named_usize("sidebar-tab-row", ix))
                .h(px(32.0))
                .w(relative(1.0))
                .px(px(8.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .rounded(px(6.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg)
                .cursor_pointer();

            if !tab.closing {
                tab_row = tab_row
                    .hover(move |s| s.bg(pill_hover).text_color(tb.chrome_fg_active))
                    .active(|s| s.opacity(0.82))
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
            }

            tab_row = tab_row
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

            if (active || is_hovered) && !tab.closing {
                tab_row = tab_row.child(
                    div()
                        .id(ElementId::named_usize("sidebar-tab-close", ix))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
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

        // 3. Footer with Settings item
        let is_settings_active = self.settings_open;
        let footer_pill_bg = if is_settings_active { tb.elevated_bg } else { rgba(0x00000000) };
        let footer_pill_border = if is_settings_active { tb.elevated_border } else { rgba(0x00000000) };
        let footer_icon_color = if is_settings_active { tb.accent } else { chrome_ink };

        let footer = div()
            .h(px(42.0))
            .p(px(6.0))
            .border_t_1()
            .border_color(tb.hairline)
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

        div()
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
            .border_color(tb.hairline)
            .child(header)
            .child(tab_list)
            .child(footer)
    }
}
