//! Top tab bar rendering: launch capsule, horizontal tab pills, settings pill, update badge, and window controls.

use gpui::prelude::*;
use gpui::{
    ClickEvent, Context, ElementId, FontWeight,
    MouseButton, MouseDownEvent, MouseUpEvent, Rgba,
    div, px, relative, rgb, rgba,
};

use crate::icons;
use crate::motion::Motion;
use crate::theme::Theme;
use crate::ui::animation::EASE_OUT;
use crate::ui::overlay::{
    caption_spacer, set_hover_mark,
    HOVER_MARK_CAPSULE_CHEVRON, HOVER_MARK_CAPSULE_PLUS, HOVER_MARK_WINDOW,
};
use crate::workspace::{format_tab_title, session_icon_for_pane};
use super::AtlasView;

impl AtlasView {
    pub(crate) fn render_top_bar(
        &self,
        theme: &Theme,
        is_sidebar_visible: bool,
        chrome_bg: Rgba,
        zoomed: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let tb = theme.clone();
        let chrome_ink = tb.chrome_fg;
        let is_dark_theme = (0.2126 * tb.elevated_bg.r + 0.7152 * tb.elevated_bg.g + 0.0722 * tb.elevated_bg.b) < 0.45;

        // Ghostty-style minimalist top bar:
        // Ultra-compact 32px (36px when zoomed) for maximum vertical terminal lines
        let bar_h = if is_sidebar_visible {
            if zoomed { px(36.0) } else { px(32.0) }
        } else {
            if zoomed { px(48.0) } else { px(40.0) }
        };
        let bar_pt = if zoomed { px(4.0) } else { px(0.0) };
        let bar_pl = if is_sidebar_visible { px(10.0) } else if zoomed { px(14.0) } else { px(10.0) };

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .w(relative(1.0))
            .h(bar_h)
            .pt(bar_pt)
            .pl(bar_pl)
            .bg(chrome_bg)
            .border_b_1()
            .border_color(tb.hairline);

        // Left Anchor: Unified Split Capsule ( + │ ▾ )
        let is_menu_open = self.profile_menu;
        let capsule_bg = tb.elevated_bg;
        let capsule_border = if is_menu_open { tb.accent } else { tb.elevated_border };

        let plus_hovered = self.hover_mark == Some(HOVER_MARK_CAPSULE_PLUS);
        let chevron_hovered = self.hover_mark == Some(HOVER_MARK_CAPSULE_CHEVRON);

        let plus_ink = if plus_hovered {
            tb.fg
        } else {
            tb.chrome_fg_active
        };

        let chevron_ink = if is_menu_open || chevron_hovered {
            tb.fg
        } else {
            tb.chrome_fg_active
        };

        let plus_btn = div()
            .id(ElementId::named_usize("capsule-plus", 0))
            .w(px(30.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_l_full()
            .hover(move |s| s.bg(tb.card_hover))
            .active(move |s| s.opacity(0.75))
            .cursor_pointer()
            .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                set_hover_mark(view, HOVER_MARK_CAPSULE_PLUS, *hovered, cx);
            }))
            .child(
                icons::icon(icons::PLUS)
                    .size(px(15.5))
                    .text_color(plus_ink),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                let prof = view.get_default_profile();
                view.new_tab(prof, cx);
                window.focus(&view.focus);
            }));

        let divider = div()
            .w(px(1.0))
            .h(px(16.0))
            .bg(tb.elevated_border);

        let chevron_btn = div()
            .id(ElementId::named_usize("capsule-chevron", 0))
            .w(px(26.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_r_full()
            .bg(if is_menu_open { tb.card_hover } else { rgba(0x00000000) })
            .hover(move |s| s.bg(tb.card_hover))
            .active(move |s| s.opacity(0.75))
            .cursor_pointer()
            .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                set_hover_mark(view, HOVER_MARK_CAPSULE_CHEVRON, *hovered, cx);
            }))
            .child(
                icons::icon(icons::CHEVRON_DOWN)
                    .size(px(13.5))
                    .text_color(chevron_ink),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.toggle_profile_menu(cx);
                window.focus(&view.focus);
            }));

        let capsule = div()
            .id(ElementId::named_usize("topbar-launch-capsule", 0))
            .h(px(32.0))
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

        if let Some(tab) = self.tabs.get(self.active) {
            let pane = tab.panes.get(tab.active_pane);
            let label = match pane {
                Some(pane) => tab.custom_title.clone().unwrap_or_else(|| format_tab_title(pane)),
                None => "Terminal".to_string(),
            };
            let win_title = if label == "Terminal" || label.is_empty() {
                "Atlas Terminal".to_string()
            } else {
                format!("{label} — Atlas Terminal")
            };
            crate::rounded_corners::set_window_title(&win_title);
        }

        bar = bar.child(capsule);
        bar = bar.child(div().w(px(6.0)).flex_none());

        let mut tabs_tray = div()
            .id(ElementId::named_usize("topbar-tabs-tray", 0))
            .flex_1()
            .min_w(px(0.0))
            .h(bar_h)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .overflow_hidden();

        // Horizontal tabs: active tab is a clean rounded rect (elevated bg + subtle border)
        // carrying the running process's own mark; inactive tabs are quiet with folder icon.
        // Tab close is visible on active and hover.
        for ix in 0..self.tabs.len() {
            let active = ix == self.active && !self.settings_open;
            let tab = &self.tabs[ix];
            let pane = tab.panes.get(tab.active_pane);
            let label = match pane {
                Some(pane) => tab.custom_title.clone().unwrap_or_else(|| {
                    format_tab_title(pane)
                }),
                None => "Terminal".to_string(),
            };
            if active {
                let win_title = if label == "Terminal" || label.is_empty() {
                    "Atlas Terminal".to_string()
                } else {
                    format!("{label} — Atlas Terminal")
                };
                crate::rounded_corners::set_window_title(&win_title);
            }
            let icon = match pane {
                Some(pane) => session_icon_for_pane(pane),
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
                let inactive_bg = if is_dark_theme {
                    rgba(0xFFFFFF0D)
                } else {
                    rgba(0x00000000)
                };
                let inactive_border = if is_dark_theme {
                    rgba(0xFFFFFF12)
                } else {
                    tb.hairline
                };
                (inactive_bg, inactive_border, chrome_ink, chrome_ink)
            };
            // The active pill must stay an opaque fill on hover: card_hover is
            // a translucent wash that would drop the title over a bright
            // blurred wallpaper in translucent mode.
            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };
            let pill_hot = tb.chrome_fg_active;

            let is_dragged = self.tab_is_dragging && self.tab_drag_origin == Some(ix);
            let (max_w, min_w, px_pad, opacity) = if tab.closing {
                let scale = (1.0 - EASE_OUT.eval(tab.close_progress)).max(0.0);
                (180.0 * scale, 0.0, 8.0 * scale, scale)
            } else if is_dragged {
                // Squeezed Uno Card in-hand state: visibly contracts
                (130.0, 48.0, 6.0, 0.95)
            } else if active {
                (180.0, 68.0, 8.0, 1.0)
            } else {
                (160.0, 40.0, 6.0, 1.0)
            };

            let mut pill = div()
                .id(ElementId::named_usize("tab-pill", tab.id as usize))
                .h(px(28.0))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(px_pad))
                .max_w(px(max_w))
                .min_w(px(min_w))
                .opacity(opacity)
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(if is_dragged { tb.elevated_bg } else { pill_bg })
                .border_1()
                .border_color(if is_dragged { tb.accent } else { pill_border })
                .text_color(pill_fg);

            if is_dragged {
                pill = pill
                    .border_2()
                    .shadow_xl()
                    .cursor_grabbing();
            } else if active {
                pill = pill.shadow_sm().cursor_pointer();
            } else {
                pill = pill.cursor_pointer();
            }

            if !tab.closing {
                pill = pill
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .active(move |style| style.opacity(0.82))
                    .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                        if *hovered && view.tab_is_dragging {
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
                        cx.listener(move |view, event: &MouseDownEvent, _window, _cx| {
                            view.tab_drag_candidate = Some((ix, f32::from(event.position.x), f32::from(event.position.y)));
                            view.tab_is_dragging = false;
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            let had_drag = view.tab_drag_candidate.is_some() || view.tab_is_dragging || view.tab_drag_origin.is_some();
                            view.tab_drag_candidate = None;
                            view.tab_is_dragging = false;
                            view.tab_drag_origin = None;
                            if had_drag {
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
                        if !view.tab_is_dragging {
                            view.active = ix;
                            view.settings_open = false;
                            window.focus(&view.focus);
                            cx.notify();
                        }
                    }));
            }

            pill = pill
                .child(
                    div()
                        .flex_none()
                        .text_color(pill_icon)
                        .child(icons::icon(icon).size(px(16.0)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child(label),
                );

            let close_alpha = if active {
                tab.close_hover_progress.clamp(0.4, 1.0)
            } else {
                tab.close_hover_progress.clamp(0.0, 1.0)
            };
            if !tab.closing {
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", tab.id as usize))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .opacity(close_alpha)
                        .hover(move |style| style.bg(tb.card_hover))
                        .active(move |style| style.opacity(0.75))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(11.0))
                                .text_color(tb.faint_fg),
                        )
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.close_tab(ix, cx);
                            window.focus(&view.focus);
                        })),
                );
            }

            if active {
                pill = pill.child(
                    div()
                        .absolute()
                        .bottom(px(0.0))
                        .left(px(6.0))
                        .right(px(6.0))
                        .h(px(2.0))
                        .rounded_full()
                        .bg(tb.accent),
                );
            }
            let tab_element = pill.uno_card_drag(
                ElementId::named_usize(
                    if is_dragged { "tab-uno-drag" } else { "tab-uno-settle" },
                    tab.id as usize,
                ),
                is_dragged,
            ).into_any_element();
            tabs_tray = tabs_tray.child(tab_element);
        }

        // Settings tab pill: stays mounted on the tab bar until the user explicitly closes it.
        // Styled 1:1 identically to terminal tabs (matching Windows Terminal).
        if self.settings_tab_open {
            let active = self.settings_open;
            let (max_w, min_w) = if active {
                (180.0, 68.0)
            } else {
                (160.0, 40.0)
            };
            let (pill_bg, pill_border, pill_fg, pill_icon) = if active {
                (
                    tb.elevated_bg,
                    tb.elevated_border,
                    tb.chrome_fg_active,
                    tb.accent,
                )
            } else {
                let inactive_bg = if is_dark_theme {
                    rgba(0xFFFFFF0D)
                } else {
                    rgba(0x00000000)
                };
                let inactive_border = if is_dark_theme {
                    rgba(0xFFFFFF12)
                } else {
                    tb.hairline
                };
                (inactive_bg, inactive_border, chrome_ink, chrome_ink)
            };
            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };
            let pill_hot = tb.chrome_fg_active;

            let mut settings_pill = div()
                .id(ElementId::named_usize("tab-pill-settings", 0))
                .h(px(28.0))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .max_w(px(max_w))
                .min_w(px(min_w))
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            if active {
                settings_pill = settings_pill.shadow_sm();
            }

            if !active {
                settings_pill = settings_pill
                    .cursor_pointer()
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .active(move |style| style.opacity(0.82))
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = true;
                        window.focus(&view.settings_focus);
                        cx.notify();
                    }));
            } else {
                settings_pill = settings_pill.cursor_pointer();
            }

            settings_pill = settings_pill
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(|view, _: &MouseDownEvent, window, cx| {
                        view.settings_tab_open = false;
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .flex_none()
                        .text_color(pill_icon)
                        .child(icons::icon(icons::SETTINGS).size(px(15.0)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child("Settings"),
                )
                .child(
                    div()
                        .id(ElementId::named_usize("tab-close-settings", 0))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .active(move |style| style.opacity(0.75))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(11.0))
                                .text_color(tb.faint_fg),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            view.settings_tab_open = false;
                            view.settings_open = false;
                            window.focus(&view.focus);
                            cx.notify();
                        })),
                );

            if active {
                settings_pill = settings_pill.child(
                    div()
                        .absolute()
                        .bottom(px(0.0))
                        .left(px(6.0))
                        .right(px(6.0))
                        .h(px(2.0))
                        .rounded_full()
                        .bg(tb.accent),
                );
            }

            tabs_tray = tabs_tray.child(settings_pill.tab_pill_expand("settings-tab-pill-motion"));
        }

        tabs_tray = tabs_tray.child(caption_spacer(0));
        bar = bar.child(tabs_tray);

        // Dedicated guaranteed drag spacer: ensures the window remains draggable even
        // when many tabs are open.
        bar = bar.child(
            div()
                .w(px(24.0))
                .h(bar_h)
                .flex_none()
                .child(caption_spacer(1)),
        );

        let resting_btn_bg = if is_dark_theme { tb.card_hover } else { tb.elevated_bg };
        let resting_btn_border = if is_dark_theme { tb.hairline } else { tb.elevated_border };

        let search_capsule = div()
            .id(ElementId::named_usize("topbar-search-capsule", 0))
            .h(px(26.0))
            .px(px(9.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .rounded(px(6.0))
            .bg(resting_btn_bg)
            .border_1()
            .border_color(resting_btn_border)
            .text_color(tb.chrome_fg)
            .hover(move |s| s.bg(tb.card_hover).border_color(tb.accent).text_color(tb.chrome_fg_active))
            .cursor_pointer()
            .child(icons::icon(icons::SEARCH).size(px(12.5)).text_color(tb.chrome_fg))
            .child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Search (Ctrl+K)"),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.open_palette(window, cx);
            }));

        let is_dark_mode = (0.2126 * tb.elevated_bg.r + 0.7152 * tb.elevated_bg.g + 0.0722 * tb.elevated_bg.b) < 0.45;
        let theme_icon = if is_dark_mode { icons::SUN } else { icons::MOON };
        let theme_btn = div()
            .id(ElementId::named_usize("topbar-theme-toggle", 0))
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(resting_btn_bg)
            .border_1()
            .border_color(resting_btn_border)
            .text_color(tb.chrome_fg)
            .hover(move |s| s.bg(tb.card_hover).border_color(tb.accent).text_color(tb.chrome_fg_active))
            .cursor_pointer()
            .child(icons::icon(theme_icon).size(px(13.5)).text_color(tb.chrome_fg))
            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                view.toggle_theme(cx);
            }));

        // Window controls: clean 44px wide native vector controls filling top bar height
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

        let mut right_cluster = div()
            .id(ElementId::named_usize("topbar-right-cluster", 0))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0));

        if self.pixel_art.enabled {
            let companion_canvas = crate::ui::pixel_art::render_pixel_canvas(
                self.pixel_art.active_preset.clone(),
                self.pixel_art_frame,
                self.pixel_art.color_mode.clone(),
                theme.clone(),
                is_dark_theme,
                24.0,
                24.0,
            );

            let companion_capsule = div()
                .id(ElementId::named_usize("topbar-pixel-companion", 0))
                .h(px(28.0))
                .px(px(4.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(resting_btn_bg)
                .border_1()
                .border_color(resting_btn_border)
                .hover(move |s| s.bg(tb.card_hover).border_color(tb.accent))
                .cursor_pointer()
                .child(companion_canvas)
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    view.settings_tab = crate::config::SettingsTab::PixelArt;
                    view.settings_open = true;
                    view.settings_tab_open = true;
                    view.sync_and_save_settings();
                    cx.notify();
                }));

            right_cluster = right_cluster.child(companion_capsule);
        }

        right_cluster = right_cluster
            .child(search_capsule)
            .child(theme_btn)
            .child(div().w(px(2.0)).flex_none());

        for (index, (icon_path, is_close)) in controls.into_iter().enumerate() {
            let button_theme = theme.clone();
            let hover_id = HOVER_MARK_WINDOW + index as u64;
            let is_hovered = self.hover_mark == Some(hover_id);
            let hover_bg = if is_close {
                rgb(0xC42B1C)
            } else {
                button_theme.card_hover
            };
            let hover_fg = if is_close {
                rgb(0xFFFFFF)
            } else {
                button_theme.chrome_fg_active
            };
            let default_fg = button_theme.chrome_fg;
            let btn_fg = if is_hovered { hover_fg } else { default_fg };
            let btn_bg = if is_hovered { hover_bg } else { rgba(0x00000000) };

            let ctrl = div()
                .id(ElementId::named_usize("topbar-window-control", index))
                .w(px(44.0))
                .h(bar_h)
                .flex()
                .items_center()
                .justify_center()
                .bg(btn_bg)
                .cursor_pointer()
                .on_hover(cx.listener(move |view, hovered: &bool, _window, cx| {
                    set_hover_mark(view, hover_id, *hovered, cx);
                }))
                .child(
                    icons::icon(icon_path)
                        .size(if is_close { px(12.5) } else { px(12.0) })
                        .text_color(btn_fg),
                )
                .on_click(cx.listener(move |_view, _: &ClickEvent, _window, cx| {
                    crate::rounded_corners::window_button(index);
                    cx.notify();
                }));
            right_cluster = right_cluster.child(ctrl);
        }

        bar = bar.child(right_cluster);

        bar
    }
}
