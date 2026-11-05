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
use crate::workspace::{format_tab_title, session_icon};
use super::{AtlasView, UpdateDownloadState};

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
        let default_prof = self.get_default_profile();
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
            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                view.new_tab(default_prof, cx);
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
        bar = bar.child(div().w(px(6.0)));

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
            // The active pill must stay an opaque fill on hover: card_hover is
            // a translucent wash that would drop the title over a bright
            // blurred wallpaper in translucent mode.
            let pill_hover = if active {
                crate::theme::lerp_color(tb.elevated_bg, tb.accent, 0.10)
            } else {
                tb.card_hover
            };
            let pill_hot = tb.chrome_fg_active;

            let (max_w, min_w, px_pad, opacity) = if tab.closing {
                let scale = (1.0 - EASE_OUT.eval(tab.close_progress)).max(0.0);
                (240.0 * scale, 0.0, 12.0 * scale, scale)
            } else {
                (240.0, 48.0, 12.0, 1.0)
            };

            let mut pill = div()
                .id(ElementId::named_usize("tab-pill", ix))
                .h(px(32.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .px(px(px_pad))
                .max_w(px(max_w))
                .min_w(px(min_w))
                .opacity(opacity)
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            let is_dragged = self.tab_is_dragging && self.tab_drag_origin == Some(ix);
            if is_dragged {
                pill = pill
                    .bg(tb.card_hover)
                    .border_color(tb.accent)
                    .border_2()
                    .shadow_xl()
                    .cursor_grabbing();
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
                        .text_color(pill_icon)
                        .child(icons::icon(icon).size(px(16.5)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .truncate()
                        .child(label),
                );

            let close_alpha = tab.close_hover_progress.clamp(0.0, 1.0);
            if !tab.closing {
                pill = pill.child(
                    div()
                        .id(ElementId::named_usize("tab-close", ix))
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
            let tab_element = pill.uno_card_drag(
                ElementId::named_usize(
                    if is_dragged { "tab-uno-drag" } else { "tab-uno-settle" },
                    tab.id as usize,
                ),
                is_dragged,
            ).into_any_element();
            bar = bar.child(tab_element);
        }

        // Settings tab pill: stays mounted on the tab bar until the user explicitly closes it.
        if self.settings_tab_open {
            let active = self.settings_open;
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
            let pill_hot = tb.chrome_fg_active;

            let mut settings_pill = div()
                .id(ElementId::named_usize("tab-pill-settings", 0))
                .h(px(32.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .px(px(12.0))
                .max_w(px(240.0))
                .min_w(px(80.0))
                .overflow_hidden()
                .flex_shrink()
                .rounded(px(7.0))
                .bg(pill_bg)
                .border_1()
                .border_color(pill_border)
                .text_color(pill_fg);

            if !active {
                settings_pill = settings_pill
                    .hover(move |style| style.bg(pill_hover).text_color(pill_hot))
                    .active(move |style| style.opacity(0.82))
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = true;
                        window.focus(&view.settings_focus);
                        cx.notify();
                    }));
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
                        .text_color(pill_icon)
                        .child(icons::icon(icons::SETTINGS).size(px(16.5)).text_color(pill_icon)),
                )
                .child(
                    div()
                        .text_size(px(13.5))
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
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |style| style.bg(tb.card_hover))
                        .active(move |style| style.opacity(0.75))
                        .child(
                            icons::icon(icons::CLOSE)
                                .size(px(12.0))
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

            bar = bar.child(settings_pill.slide_in_right("settings-tab-pill-motion"));
        }

        bar = bar.child(caption_spacer(0));

        if let Some(ref update) = self.available_update {
            let badge_ink = tb.accent;
            let badge_bg = Rgba {
                a: 0.16,
                ..tb.accent
            };
            let badge_border = Rgba {
                a: 0.28,
                ..tb.accent
            };
            let (label, show_dot) = match &self.update_download_state {
                UpdateDownloadState::Downloading { percent, .. } => {
                    (format!("Downloading {}%", (percent * 100.0) as u32), false)
                }
                UpdateDownloadState::Verifying { .. } => {
                    ("Verifying…".to_string(), false)
                }
                UpdateDownloadState::ReadyToRestart { .. } => {
                    ("Restart to Apply".to_string(), true)
                }
                UpdateDownloadState::Failed { .. } => {
                    ("Update Error".to_string(), true)
                }
                _ => (format!("v{}", update.version), true),
            };
            let mut badge = div()
                .id(ElementId::named_usize("topbar-update-badge", 0))
                .h(px(28.0))
                .px(px(9.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .rounded(px(6.0))
                .bg(badge_bg)
                .border_1()
                .border_color(badge_border)
                .text_color(badge_ink)
                .hover(move |s| s.opacity(0.85))
                .active(move |s| s.opacity(0.75));

            if show_dot {
                badge = badge.child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(badge_ink));
            }

            badge = badge
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    if let Some(ref info) = view.available_update {
                        let msg = format!("Atlas Terminal v{} is available!", info.version);
                        view.show_update_toast(msg, true, cx);
                    } else {
                        view.check_updates_manual(cx);
                    }
                }));

            bar = bar.child(badge.breathe("topbar-update-badge-breathe"));
        } else if self.is_checking_updates {
            bar = bar.child(
                div()
                    .h(px(28.0))
                    .px(px(9.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .rounded(px(6.0))
                    .text_color(tb.faint_fg)
                    .child(icons::icon(icons::REFRESH).size(px(13.0)).text_color(tb.faint_fg))
                    .child(div().text_size(px(12.0)).child("Checking..."))
                    .breathe("topbar-checking-breathe"),
            );
        }

        bar = bar.child(div().w(px(6.0)));

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
            bar = bar.child(ctrl);
        }

        bar
    }
}
