//! UI control components: stepper, toggle pill, and pill button.

use gpui::prelude::*;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement,
    Rgba, div, px, rgb, rgba,
};

use crate::icons;
use crate::theme::Theme;
use crate::view::AtlasView;

fn is_dark(theme: &Theme) -> bool {
    let lum = 0.2126 * theme.elevated_bg.r + 0.7152 * theme.elevated_bg.g + 0.0722 * theme.elevated_bg.b;
    lum < 0.45
}

/// Numerical stepper with native SVG minus and plus icons (zero unicode icons).
pub fn stepper(
    id: usize,
    val_text: String,
    theme: &Theme,
    on_dec: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    on_inc: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let dark = is_dark(theme);
    let hover_bg = theme.card_hover;
    let fg = if dark { rgb(0xE2E8F0) } else { rgb(0x1E293B) };
    let btn_bg = if dark { rgba(0xFFFFFF0D) } else { rgba(0x00000008) };
    let border_c = if dark { rgba(0xFFFFFF18) } else { rgba(0x00000014) };
    let active_press_bg = if dark { rgba(0xFFFFFF22) } else { rgba(0x0000001A) };
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
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_press_bg).opacity(0.82))
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
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
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
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_press_bg).opacity(0.82))
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
    let dark = is_dark(theme);
    let active_bg = Rgba {
        a: 0.22,
        ..theme.accent
    };
    let active_border = theme.accent;
    let idle_bg = if dark { rgba(0xFFFFFF0D) } else { rgba(0x00000008) };
    let idle_border = if dark { rgba(0xFFFFFF18) } else { rgba(0x00000014) };
    let idle_fg = if dark { rgb(0xCBD5E1) } else { rgb(0x334155) };
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
        .border_color(if enabled { active_border } else { idle_border })
        .bg(if enabled { active_bg } else { idle_bg })
        .hover(move |s| s.bg(hover_bg))
        .active(move |s| s.opacity(0.82))
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_toggle(view, cx);
        }));

    if enabled {
        btn = btn
            .text_color(theme.accent)
            .child(icons::icon(icons::CHECK).size(px(11.0)).text_color(theme.accent))
            .child(div().text_size(px(12.0)).font_weight(FontWeight::SEMIBOLD).child("Enabled"));
    } else {
        btn = btn
            .text_color(idle_fg)
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
    let dark = is_dark(theme);
    let hover_bg = theme.card_hover;
    let active_press_bg = if active {
        Rgba {
            a: 0.35,
            ..theme.accent
        }
    } else if dark {
        rgba(0xFFFFFF1C)
    } else {
        rgba(0x00000016)
    };
    let (bg, border_color, fg) = if active {
        (
            Rgba {
                a: 0.22,
                ..theme.accent
            },
            theme.accent,
            theme.accent,
        )
    } else if dark {
        (
            rgba(0xFFFFFF0D),
            rgba(0xFFFFFF18),
            rgb(0xCBD5E1),
        )
    } else {
        (
            rgba(0x0000000A),
            rgba(0x00000014),
            rgb(0x334155),
        )
    };

    let button = div()
        .id(ElementId::named_usize("settings-pill", id))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(border_color)
        .bg(bg)
        .text_size(px(12.0))
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
        .text_color(fg)
        .hover(move |style| style.bg(if active { bg } else { hover_bg }))
        .active(move |style| style.bg(active_press_bg).opacity(0.82))
        .child(label)
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_click(view, cx);
        }));

    button.into_any_element()
}
