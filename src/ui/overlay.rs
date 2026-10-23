//! Overlay helpers: backdrop, caption drag spacers, profile picker, and hover marks.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, IntoElement,
    Window, canvas, div, px, rgba,
};

use crate::icons;
use crate::session::Profile;
use crate::theme::Theme;
use crate::view::AtlasView;
use crate::workspace::{profile_title, session_icon};
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS, OVERLAY_OUT_MS};

pub const HOVER_MARK_NEW_TAB: u64 = 0;
pub const HOVER_MARK_WINDOW: u64 = 10; // + window control index

pub fn set_hover_mark(view: &mut AtlasView, mark: u64, hovered: bool, cx: &mut Context<AtlasView>) {
    if hovered {
        if view.hover_mark != Some(mark) {
            view.hover_mark = Some(mark);
            cx.notify();
        }
    } else if view.hover_mark == Some(mark) {
        view.hover_mark = None;
        cx.notify();
    }
}

/// Full-screen click-catcher painted under an overlay panel; fades in with
/// the panel, dims the chrome a touch and dismisses the overlay on any click.
pub fn overlay_backdrop(
    id: usize,
    closing: bool,
    on_click: impl Fn(&mut AtlasView, &mut Window, &mut Context<AtlasView>) + 'static,
    cx: &mut Context<AtlasView>,
) -> AnyElement {
    div()
        .id(ElementId::named_usize("overlay-backdrop", id))
        .absolute()
        .inset_0()
        .bg(rgba(0x00000059))
        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
            on_click(view, window, cx);
        }))
        .with_animation(
            ElementId::named_usize("backdrop-anim", id * 2 + closing as usize),
            Animation::new(Duration::from_millis(if closing {
                OVERLAY_OUT_MS
            } else {
                OVERLAY_IN_MS
            }))
            .with_easing(ease_apple_snappy),
            move |el, delta| el.opacity(if closing { 1.0 - delta } else { delta }),
        )
        .into_any_element()
}

/// A flexible spacer in the titlebar that reports its painted bounds to the
/// window subclass as drag-band slot `slot` (see `rounded_corners`).
pub fn caption_spacer(slot: usize) -> impl IntoElement {
    div()
        .id(ElementId::named_usize("caption-spacer", slot))
        .flex_1()
        .h(px(36.0))
        .relative()
        .child(
            canvas(
                move |_bounds, _window, _cx| (),
                move |bounds, _, _window, _cx| {
                    crate::rounded_corners::set_caption_rect(
                        slot,
                        f32::from(bounds.origin.x),
                        f32::from(bounds.origin.y),
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    );
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// Profile picker behind the workspace pill, anchored under the top bar.
pub fn profile_menu(theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
    let wsl_label = crate::wsl::default_distro()
        .map(|distro| format!("{} (WSL)", distro.title))
        .unwrap_or_else(|| "WSL".to_string());
    let entries: [(String, Profile); 5] = [
        ("PowerShell".to_string(), Profile::PowerShell),
        (wsl_label, Profile::Wsl),
        ("OpenCode".to_string(), Profile::OpenCode),
        ("Command Prompt".to_string(), Profile::Cmd),
        ("Git Bash".to_string(), Profile::GitBash),
    ];
    let item_hover = theme.card_hover;
    let mut menu = div()
        .absolute()
        .top(px(40.0))
        .left(px(8.0))
        .w(px(220.0))
        .flex()
        .flex_col()
        .p(px(6.0))
        .rounded(px(8.0))
        .bg(theme.elevated_bg)
        .border_1()
        .border_color(theme.elevated_border)
        .shadow_lg();
    for (index, (label, profile)) in entries.into_iter().enumerate() {
        let icon = session_icon(&profile_title(profile), profile);
        menu = menu.child(
            div()
                .id(ElementId::named_usize("profile-item", index))
                .h(px(30.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .rounded(px(6.0))
                .text_size(px(12.5))
                .text_color(theme.chrome_fg_active)
                .hover(move |style| style.bg(item_hover))
                .child(icons::icon(icon).size(px(13.0)).text_color(theme.chrome_fg))
                .child(label)
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.new_tab(profile, cx);
                    view.dismiss_profile(cx);
                    window.focus(&view.focus);
                })),
        );
    }
    menu.with_animation(
        ElementId::named_usize("profile-anim", closing as usize),
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
