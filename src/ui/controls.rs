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
        .gap(px(5.0))
        .child(
            div()
                .id(ElementId::named_usize("stepper-dec", id))
                .w(px(32.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_press_bg).opacity(0.82))
                .child(icons::icon(icons::MINUS).size(px(12.5)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_dec(view, cx);
                })),
        )
        .child(
            div()
                .min_w(px(72.0))
                .h(px(32.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .child(val_text),
        )
        .child(
            div()
                .id(ElementId::named_usize("stepper-inc", id))
                .w(px(32.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .bg(btn_bg)
                .border_1()
                .border_color(border_c)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_press_bg).opacity(0.82))
                .child(icons::icon(icons::PLUS).size(px(12.5)).text_color(fg))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    on_inc(view, cx);
                })),
        )
        .into_any_element()
}

/// Modern Apple/macOS-style sliding toggle switcher (zero unicode icons).
pub fn toggle_pill(
    id: usize,
    enabled: bool,
    theme: &Theme,
    on_toggle: impl Fn(&mut AtlasView, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let dark = is_dark(theme);

    // Track styles (44×26 capsule)
    let track_bg = if enabled {
        theme.accent
    } else if dark {
        rgba(0xFFFFFF24)
    } else {
        rgba(0x0000001A)
    };
    let track_border = if enabled {
        theme.accent
    } else if dark {
        rgba(0xFFFFFF18)
    } else {
        rgba(0x00000020)
    };
    let track_hover_bg = if enabled {
        Rgba {
            a: 0.88,
            ..theme.accent
        }
    } else if dark {
        rgba(0xFFFFFF30)
    } else {
        rgba(0x00000026)
    };

    // Thumb styles (20×20 white circular knob)
    let thumb_bg = if enabled {
        rgb(0xFFFFFF)
    } else if dark {
        rgb(0xE2E8F0)
    } else {
        rgb(0xFFFFFF)
    };
    let thumb_border = if dark {
        rgba(0x00000028)
    } else {
        rgba(0x0000001A)
    };

    let mut track = div()
        .id(ElementId::named_usize("settings-toggle", id))
        .w(px(44.0))
        .h(px(26.0))
        .p(px(2.5))
        .flex()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(track_border)
        .bg(track_bg)
        .cursor_pointer()
        .hover(move |s| s.bg(track_hover_bg))
        .active(move |s| s.opacity(0.85));

    if enabled {
        track = track.justify_end();
    } else {
        track = track.justify_start();
    }

    track = track.child(
        div()
            .w(px(20.0))
            .h(px(20.0))
            .rounded_full()
            .border_1()
            .border_color(thumb_border)
            .bg(thumb_bg),
    );

    track
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            on_toggle(view, cx);
        }))
        .into_any_element()
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
        .h(px(32.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.0))
        .border_1()
        .border_color(border_color)
        .bg(bg)
        .text_size(px(13.5))
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

/// Modern Apple/macOS-style segmented control with unique named ElementId scope.
pub fn segmented_control<T: Clone + PartialEq + 'static>(
    name: &'static str,
    options: Vec<(T, String)>,
    selected: &T,
    theme: &Theme,
    on_change: impl Fn(T, &mut AtlasView, &mut Context<AtlasView>) + 'static + Copy,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    let dark = is_dark(theme);
    let container_bg = if dark { rgba(0xFFFFFF0F) } else { rgba(0x0000000A) };
    let container_border = if dark { rgba(0xFFFFFF14) } else { rgba(0x00000012) };

    let mut container = div()
        .flex()
        .flex_row()
        .items_center()
        .p(px(3.0))
        .gap(px(3.0))
        .rounded(px(8.0))
        .bg(container_bg)
        .border_1()
        .border_color(container_border);

    for (idx, (val, label)) in options.into_iter().enumerate() {
        let is_sel = selected == &val;
        let item_val = val.clone();
        let sel_bg = if is_sel {
            if dark { rgba(0xFFFFFF26) } else { rgb(0xFFFFFF) }
        } else {
            rgba(0x00000000)
        };
        let sel_border = if is_sel {
            if dark { rgba(0xFFFFFF20) } else { rgba(0x00000014) }
        } else {
            rgba(0x00000000)
        };
        let sel_fg = if is_sel {
            if dark { rgb(0xFFFFFF) } else { rgb(0x0F172A) }
        } else {
            if dark { rgb(0x94A3B8) } else { rgb(0x64748B) }
        };
        let hover_bg = if !is_sel {
            if dark { rgba(0xFFFFFF10) } else { rgba(0x00000008) }
        } else {
            sel_bg
        };

        let item = div()
            .id(ElementId::named_usize(name, idx))
            .h(px(30.0))
            .px(px(14.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(sel_bg)
            .border_1()
            .border_color(sel_border)
            .text_size(px(13.0))
            .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
            .text_color(sel_fg)
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .child(label)
            .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                on_change(item_val.clone(), view, cx);
            }));

        container = container.child(item);
    }

    container.into_any_element()
}
