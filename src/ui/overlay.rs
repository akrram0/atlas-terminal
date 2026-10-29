//! Overlay helpers: backdrop, caption drag spacers, profile picker, and hover marks.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement,
    Window, canvas, div, px, rgba,
};

use crate::icons;
use crate::session::Profile;
use crate::theme::Theme;
use crate::view::AtlasView;
use crate::workspace::{profile_title, session_icon};
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS, OVERLAY_OUT_MS};

pub const HOVER_MARK_CAPSULE_PLUS: u64 = 1;
pub const HOVER_MARK_CAPSULE_CHEVRON: u64 = 2;
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

/// Unified Launch & Action Menu anchored under the left-side ( + │ ▾ ) capsule.
pub fn profile_menu(theme: &Theme, closing: bool, cx: &mut Context<AtlasView>) -> AnyElement {
    let wsl_label = crate::wsl::default_distro()
        .map(|distro| format!("{} (WSL)", distro.title))
        .unwrap_or_else(|| "WSL".to_string());
    let entries: [(String, Profile); 5] = [
        ("PowerShell".to_string(), Profile::PowerShell),
        (wsl_label, Profile::Wsl),
        ("Command Prompt".to_string(), Profile::Cmd),
        ("Git Bash".to_string(), Profile::GitBash),
        ("OpenCode".to_string(), Profile::OpenCode),
    ];
    let item_hover = theme.card_hover;
    let mut menu = div()
        .id(ElementId::named_usize("launch-menu-dropdown", 0))
        .absolute()
        .top(px(46.0))
        .left(px(12.0))
        .w(px(260.0))
        .flex()
        .flex_col()
        .p(px(8.0))
        .gap(px(2.0))
        .rounded(px(12.0))
        .bg(theme.elevated_bg)
        .border_1()
        .border_color(theme.elevated_border)
        .shadow_xl();

    for (index, (label, profile)) in entries.into_iter().enumerate() {
        let icon = session_icon(&profile_title(profile), profile);
        menu = menu.child(
            div()
                .id(ElementId::named_usize("profile-item", index))
                .h(px(36.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .rounded(px(7.0))
                .cursor_pointer()
                .hover(move |style| style.bg(item_hover))
                .child(icons::icon(icon).size(px(16.0)).text_color(theme.chrome_fg))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(theme.chrome_fg_active)
                        .child(label),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.new_tab(profile, cx);
                    view.dismiss_profile(cx);
                    window.focus(&view.focus);
                })),
        );
    }

    // Divider
    menu = menu.child(
        div()
            .h(px(1.0))
            .my(px(4.0))
            .bg(theme.elevated_border),
    );

    // Settings item (Ctrl+,)
    menu = menu.child(
        div()
            .id(ElementId::named_usize("menu-item-settings", 0))
            .h(px(36.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(7.0))
            .cursor_pointer()
            .hover(move |style| style.bg(item_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(icons::icon(icons::SETTINGS).size(px(16.0)).text_color(theme.chrome_fg))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(theme.chrome_fg_active)
                            .child("Settings"),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.chrome_fg)
                    .child("Ctrl+,"),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.dismiss_profile(cx);
                view.settings_tab_open = true;
                view.settings_open = true;
                view.settings_tab = crate::config::SettingsTab::General;
                view.settings_scroll_pos = 0.0;
                view.settings_scroll_target = 0.0;
                view.settings_scroll_handle.set_offset(gpui::Point::default());
                window.focus(&view.settings_focus);
                cx.notify();
            })),
    );

    // Check for updates item
    menu = menu.child(
        div()
            .id(ElementId::named_usize("menu-item-updates", 0))
            .h(px(36.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .rounded(px(7.0))
            .cursor_pointer()
            .hover(move |style| style.bg(item_hover))
            .child(icons::icon(icons::REFRESH).size(px(16.0)).text_color(theme.chrome_fg))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(theme.chrome_fg_active)
                    .child("Check for Updates"),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                view.dismiss_profile(cx);
                view.check_updates_manual(cx);
                view.toggle_update_panel(cx);
            })),
    );

    // Command palette item (Ctrl+K)
    menu = menu.child(
        div()
            .id(ElementId::named_usize("menu-item-palette", 0))
            .h(px(30.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(move |style| style.bg(item_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(icons::icon(icons::SEARCH).size(px(13.0)).text_color(theme.chrome_fg))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child("Command Palette"),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.chrome_fg)
                    .child("Ctrl+K"),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.dismiss_profile(cx);
                view.open_palette(window, cx);
            })),
    );

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
