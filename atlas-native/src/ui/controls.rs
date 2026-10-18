//! UI control components: stepper, toggle pill, and pill button.

use gpui::prelude::*;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement,
    Rgba, div, px,
};

use crate::icons;
use crate::theme::Theme;
use crate::view::AtlasView;

/// Numerical stepper with native SVG minus and plus icons (zero unicode icons).
pub fn stepper(
    id: usize,
    val_text: String,
    theme: &Theme,
    on_dec: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    on_inc: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let hover_bg = theme.card_hover;
    let fg = theme.chrome_fg_active;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .id(ElementId::named_usize("stepper-dec", id))
                .w(px(26.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .border_1()
                .border_color(theme.hairline)
                .hover(move |s| s.bg(hover_bg))
                .child(icons::icon(icons::MINUS).size(px(10.0)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_dec(view, cx);
                })),
        )
        .child(
            div()
                .min_w(px(60.0))
                .h(px(26.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(theme.soft_bg)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.chrome_fg_active)
                .child(val_text),
        )
        .child(
            div()
                .id(ElementId::named_usize("stepper-inc", id))
                .w(px(26.0))
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .border_1()
                .border_color(theme.hairline)
                .hover(move |s| s.bg(hover_bg))
                .child(icons::icon(icons::PLUS).size(px(10.0)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_inc(view, cx);
                })),
        )
        .into_any_element()
}

/// Settings toggle pill with native SVG checkmark when active (zero unicode icons).
pub fn toggle_pill(
    id: usize,
    enabled: bool,
    theme: &Theme,
    on_toggle: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let active_bg = Rgba {
        a: 0.16,
        ..theme.accent
    };
    let active_border = Rgba {
        a: 0.35,
        ..theme.accent
    };
    let hover_bg = theme.card_hover;
    let mut btn = div()
        .id(ElementId::named_usize("settings-toggle", id))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(if enabled { active_border } else { theme.hairline })
        .hover(move |s| s.bg(hover_bg))
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_toggle(view, cx);
        }));

    if enabled {
        btn = btn
            .bg(active_bg)
            .text_color(theme.accent)
            .child(icons::icon(icons::CHECK).size(px(11.0)).text_color(theme.accent))
            .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Enabled"));
    } else {
        btn = btn
            .text_color(theme.chrome_fg)
            .child(div().text_size(px(12.0)).child("Disabled"));
    }
    btn.into_any_element()
}

/// One settings option pill; `active` paints the selected state.
pub fn pill_button(
    label: String,
    id: usize,
    active: bool,
    theme: &Theme,
    on_click: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let active_bg = Rgba {
        a: 0.16,
        ..theme.chrome_fg_active
    };
    let hover_bg = theme.card_hover;
    let fg = if active {
        theme.chrome_fg_active
    } else {
        theme.chrome_fg
    };
    let mut button = div()
        .id(ElementId::named_usize("settings-pill", id))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .text_size(px(12.0))
        .text_color(fg)
        .hover(move |style| style.bg(hover_bg))
        .child(label)
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_click(view, cx);
        }));
    if active {
        button = button.bg(active_bg).font_weight(FontWeight::MEDIUM);
    }
    button.into_any_element()
}
