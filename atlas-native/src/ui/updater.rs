//! Software update popup overlay.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, Rgba, div, px, rgb,
};

use crate::icons;
use crate::theme::Theme;
use crate::updater;
use crate::view::AtlasView;
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS, OVERLAY_OUT_MS};
use super::controls::pill_button;

impl AtlasView {
    pub fn render_update_panel(&self, theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
        let item_hover = theme.card_hover;
        let mut panel = div()
            .id(ElementId::named_usize("update-panel", 0))
            .absolute()
            .top(px(40.0))
            .right(px(14.0))
            .w(px(330.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .rounded(px(10.0))
            .bg(theme.elevated_bg)
            .border_1()
            .border_color(theme.elevated_border)
            .shadow_lg();

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        icons::icon(icons::ATLAS_MARK)
                            .size(px(14.0))
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.chrome_fg_active)
                            .child("Software Update"),
                    ),
            )
            .child(
                div()
                    .id(ElementId::named_usize("update-close-btn", 0))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_color(theme.faint_fg)
                    .hover(move |s| s.bg(item_hover).text_color(theme.chrome_fg_active))
                    .child(icons::icon(icons::CLOSE).size(px(11.0)).text_color(theme.faint_fg))
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.dismiss_update_panel(cx);
                    })),
            );

        panel = panel.child(header);

        if let Some(ref update) = self.available_update {
            let ver_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.fg)
                        .child(format!("v{}", update.version)),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.5))
                        .rounded(px(4.0))
                        .bg(Rgba { a: 0.16, ..theme.accent })
                        .text_size(px(10.5))
                        .text_color(theme.accent)
                        .child("Available"),
                );

            let title_line = div()
                .text_size(px(12.0))
                .text_color(theme.chrome_fg)
                .child(update.title.clone());

            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(ver_row)
                .child(title_line);

            if !update.notes.trim().is_empty() {
                body = body.child(
                    div()
                        .max_h(px(100.0))
                        .overflow_hidden()
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .bg(theme.panel_bg)
                        .border_1()
                        .border_color(theme.hairline)
                        .text_size(px(11.0))
                        .text_color(theme.chrome_fg)
                        .child(update.notes.trim().to_string()),
                );
            }

            let download_target = update
                .installer_url
                .clone()
                .unwrap_or_else(|| update.html_url.clone());

            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pt(px(4.0))
                .child(
                    div()
                        .id(ElementId::named_usize("update-skip-action", 0))
                        .h(px(26.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .hover(move |s| s.bg(item_hover))
                        .text_size(px(11.5))
                        .text_color(theme.faint_fg)
                        .child("Skip this version")
                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                            view.skip_current_update(cx);
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id(ElementId::named_usize("update-later-action", 0))
                                .h(px(26.0))
                                .px(px(10.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.0))
                                .hover(move |s| s.bg(item_hover))
                                .text_size(px(11.5))
                                .text_color(theme.chrome_fg)
                                .child("Later")
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.dismiss_update_panel(cx);
                                })),
                        )
                        .child(
                            div()
                                .id(ElementId::named_usize("update-install-action", 0))
                                .h(px(26.0))
                                .px(px(12.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.0))
                                .bg(theme.accent)
                                .hover(move |s| s.opacity(0.88))
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xFFFFFF))
                                .child("Install update")
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    cx.open_url(&download_target);
                                    view.dismiss_update_panel(cx);
                                })),
                        ),
                );

            panel = panel.child(body).child(actions);
        } else {
            let last_checked_text = self
                .updater_settings
                .last_check_time
                .map(updater::format_time_ago)
                .unwrap_or_else(|| "Never".to_string());

            let is_checking = self.is_checking_updates;
            let status_line = if is_checking {
                "Checking for updates…".to_string()
            } else if let Some(ref msg) = self.manual_status_message {
                format!("{msg} · Last checked: {last_checked_text}")
            } else {
                format!(
                    "Atlas Terminal is up to date (v{}) · Last checked: {last_checked_text}",
                    env!("CARGO_PKG_VERSION")
                )
            };

            let body = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.chrome_fg)
                        .child(status_line),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .pt(px(2.0))
                        .child(pill_button(
                            if is_checking {
                                "Checking…".to_string()
                            } else {
                                "Check now".to_string()
                            },
                            70,
                            false,
                            theme,
                            |view, cx| {
                                view.check_updates_manual(cx);
                            },
                            cx,
                        )),
                );

            panel = panel.child(body);
        }

        panel.with_animation(
            ElementId::named_usize("update-anim", closing as usize),
            Animation::new(Duration::from_millis(if closing {
                OVERLAY_OUT_MS
            } else {
                OVERLAY_IN_MS
            }))
            .with_easing(ease_apple_snappy),
            move |el, delta| {
                let t = if closing { 1.0 - delta } else { delta };
                el.opacity(t).top(px(40.0 - 6.0 * (1.0 - t)))
            },
        )
        .into_any_element()
    }
}
