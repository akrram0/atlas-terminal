//! Window frame components: Dynamic Island HUD, titlebar drag bands, and window controls.

use gpui::prelude::*;
use gpui::{
    div, px, rgb, rgba, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, Rgba,
};

use crate::icons;
use crate::motion::Motion;
use crate::theme::Theme;
use crate::workspace::{format_tab_title, session_icon};
use super::{AtlasView, UpdateDownloadState};

impl AtlasView {
    pub(crate) fn render_island_hud(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.app_settings.tab_bar_position != "borderless" {
            return div().into_any_element();
        }

        let opacity = self.island_hud_opacity;
        if opacity <= 0.005 {
            return div().into_any_element();
        }

        let active_tab_ix = self.active;
        let total_tabs = self.tabs.len();
        let active_pane = self.tabs.get(active_tab_ix).and_then(|t| t.panes.get(t.active_pane));
        let shell_icon = active_pane.map(|p| session_icon(&p.title, p.profile)).unwrap_or(icons::TERMINAL);
        let active_title = if let Some(tab) = self.tabs.get(active_tab_ix) {
            if let Some(pane) = tab.panes.get(tab.active_pane) {
                tab.custom_title.clone().unwrap_or_else(|| format_tab_title(pane))
            } else {
                "Tab".to_string()
            }
        } else {
            "Tab".to_string()
        };

        let tb = theme.clone();
        // Apple Dynamic Island dark glass style:
        let bg_color = Rgba { r: 0.09, g: 0.10, b: 0.14, a: 0.96 };
        let border_color = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.18 };
        let text_color = tb.fg;
        let chrome_fg = tb.chrome_fg;
        let accent_color = tb.accent;

        // Slide down offset: -60px when hidden, 0px when fully open
        let slide_y = (1.0 - opacity) * -60.0;

        let mut hud = div()
            .id(ElementId::named_usize("dynamic-island-hud", 0))
            .mt(px(8.0 + slide_y))
            .w(px(720.0))
            .h(px(46.0))
            .opacity(opacity)
            .rounded(px(23.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .shadow_xl()
            .px(px(14.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_default()
            .on_hover(cx.listener(|view, hovered: &bool, _window, cx| {
                if view.island_hud_opacity > 0.5 {
                    view.island_hud_hovered = *hovered;
                    if *hovered {
                        view.last_island_hud_time = std::time::Instant::now();
                    }
                    cx.notify();
                }
            }));

        // 1. Left side: Active shell icon + position badge (e.g. "2 / 4") + title
        let left_section = div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                icons::icon(shell_icon)
                    .size(px(16.0))
                    .text_color(accent_color),
            )
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(6.0))
                    .bg(Rgba { a: 0.18, ..accent_color })
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(accent_color)
                    .child(format!("{}/{}", active_tab_ix + 1, total_tabs.max(1))),
            )
            .child(
                div()
                    .max_w(px(120.0))
                    .overflow_hidden()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(active_title),
            );

        // 2. Center section: 1-Click Tab Switcher Pills
        let mut tab_dots = div()
            .flex()
            .items_center()
            .gap(px(4.0));

        for i in 0..total_tabs.min(6) {
            let is_current = i == active_tab_ix;
            let dot_theme = theme.clone();
            let dot = div()
                .id(ElementId::named_usize("island-tab-pill", i))
                .px(px(8.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(if is_current { accent_color } else { Rgba { a: 0.12, ..dot_theme.card_hover } })
                .text_color(if is_current { dot_theme.elevated_bg } else { text_color })
                .text_size(px(11.5))
                .font_weight(if is_current { FontWeight::BOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .hover(move |s| s.opacity(0.85))
                .child(format!("Tab {}", i + 1))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.active = i;
                    view.trigger_island_hud(cx);
                    cx.notify();
                }));
            tab_dots = tab_dots.child(dot);
        }

        // 3. Right section: 1-Click Quick Actions (WSL, pwsh, Update/Restart, Split, Close)
        let tb2 = theme.clone();
        let mut right_section = div()
            .flex()
            .items_center()
            .gap(px(4.0));

        // 1-Click WSL Launcher
        right_section = right_section.child(
            div()
                .id(ElementId::named_usize("island-add-wsl", 0))
                .px(px(8.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .rounded(px(6.0))
                .bg(Rgba { a: 0.14, ..accent_color })
                .border_1()
                .border_color(Rgba { a: 0.28, ..accent_color })
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent_color)
                .cursor_pointer()
                .hover(|s| s.opacity(0.80))
                .child("+ WSL")
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    view.new_tab(crate::session::Profile::Wsl, cx);
                    view.trigger_island_hud(cx);
                })),
        );

        // 1-Click PowerShell Launcher
        right_section = right_section.child(
            div()
                .id(ElementId::named_usize("island-add-pwsh", 0))
                .px(px(8.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .rounded(px(6.0))
                .bg(Rgba { a: 0.08, ..chrome_fg })
                .border_1()
                .border_color(Rgba { a: 0.16, ..chrome_fg })
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(chrome_fg)
                .cursor_pointer()
                .hover(|s| s.opacity(0.80))
                .child("+ pwsh")
                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    view.new_tab(crate::session::Profile::PowerShell, cx);
                    view.trigger_island_hud(cx);
                })),
        );

        // 1-Click Update / Restart Button
        if matches!(self.update_download_state, UpdateDownloadState::ReadyToRestart { .. }) {
            right_section = right_section.child(
                div()
                    .id(ElementId::named_usize("island-restart-btn", 0))
                    .px(px(9.0))
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded(px(6.0))
                    .bg(rgb(0x107C41))
                    .text_color(rgb(0xFFFFFF))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .child("🔄 Restart")
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.apply_update_and_restart(cx);
                    })),
            );
        } else if let UpdateDownloadState::Downloading { percent, .. } = self.update_download_state {
            right_section = right_section.child(
                div()
                    .id(ElementId::named_usize("island-downloading-btn", 0))
                    .px(px(9.0))
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded(px(6.0))
                    .bg(accent_color)
                    .text_color(tb2.elevated_bg)
                    .text_size(px(11.5))
                    .font_weight(FontWeight::BOLD)
                    .child(format!("⏳ {percent}%")),
            );
        } else if self.available_update.is_some() {
            right_section = right_section.child(
                div()
                    .id(ElementId::named_usize("island-update-btn", 0))
                    .px(px(9.0))
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded(px(6.0))
                    .bg(accent_color)
                    .text_color(tb2.elevated_bg)
                    .text_size(px(11.5))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .child("⚡ Update")
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.start_download_update(cx);
                    })),
            );
        }

        // Split Pane Button
        let split_btn = div()
            .id(ElementId::named_usize("island-split-pane", 0))
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(rgba(0x00000000))
            .hover(move |s| s.bg(tb2.card_hover))
            .cursor_pointer()
            .child(
                icons::icon(icons::SPLIT_COLUMNS)
                    .size(px(13.0))
                    .text_color(chrome_fg),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                view.split_active(crate::workspace::SplitAxis::Columns, cx);
                view.trigger_island_hud(cx);
            }));

        // Close Tab Button
        let tb3 = theme.clone();
        let close_btn = div()
            .id(ElementId::named_usize("island-close-tab", 0))
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(rgba(0x00000000))
            .hover(move |s| s.bg(tb3.card_hover))
            .cursor_pointer()
            .child(
                icons::icon(icons::CLOSE)
                    .size(px(13.0))
                    .text_color(chrome_fg),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                view.close_pane(cx);
                view.trigger_island_hud(cx);
            }));

        // 1-Click Settings Button
        let tb4 = theme.clone();
        let settings_btn = div()
            .id(ElementId::named_usize("island-settings", 0))
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(rgba(0x00000000))
            .hover(move |s| s.bg(tb4.card_hover))
            .cursor_pointer()
            .child(
                icons::icon(icons::SETTINGS)
                    .size(px(13.0))
                    .text_color(chrome_fg),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.toggle_settings(window, cx);
            }));

        right_section = right_section.child(split_btn).child(close_btn).child(settings_btn);

        hud = hud
            .child(left_section)
            .child(tab_dots)
            .child(right_section);

        div()
            .id(ElementId::named_usize("island-hud-container", 0))
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_col()
            .items_center()
            .child(hud.pop_in("dynamic-island-hud-motion"))
            .into_any_element()
    }
}
