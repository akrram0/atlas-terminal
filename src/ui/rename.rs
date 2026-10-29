//! Tab rename dialog popover.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, Context, ElementId, FontWeight, IntoElement,
    KeyDownEvent, Keystroke, Window, div, px,
};

use crate::theme::Theme;
use crate::view::AtlasView;
use crate::workspace::{profile_title, tab_name};
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS};

impl AtlasView {
    pub fn open_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(pane) = tab.panes.get(tab.active_pane) else {
            return;
        };
        self.rename_value = tab
            .custom_title
            .clone()
            .unwrap_or_else(|| tab_name(&pane.title, &profile_title(pane.profile)));
        self.rename_open = true;
        if self.palette_open {
            self.close_palette(window, cx);
        }
        window.focus(&self.rename_focus);
        cx.notify();
    }

    pub fn close_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rename_open = false;
        self.focus_main(window);
        cx.notify();
    }

    /// Commit the rename; an empty name falls back to the shell title.
    pub fn apply_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            let name = self.rename_value.trim().to_string();
            tab.custom_title = if name.is_empty() { None } else { Some(name) };
        }
        self.save_session();
        self.close_rename(window, cx);
    }

    pub fn handle_rename_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        match ks.key.as_str() {
            "escape" => self.close_rename(window, cx),
            "enter" => self.apply_rename(window, cx),
            "backspace" => {
                if self.rename_value.pop().is_some() {
                    cx.notify();
                }
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    // Space arrives without `key_char` (gpui quirk).
                    let typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else {
                        ks.key_char
                            .as_deref()
                            .map(|text| {
                                text.chars().filter(|c| !c.is_control()).collect()
                            })
                            .unwrap_or_default()
                    };
                    if !typed.is_empty() {
                        self.rename_value.push_str(&typed);
                        cx.notify();
                    }
                }
            }
        }
    }

    pub fn rename_dialog(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut field = div()
            .h(px(40.0))
            .px(px(14.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .rounded(px(8.0))
            .bg(theme.card_hover)
            .border_1()
            .border_color(theme.elevated_border)
            .text_size(px(14.5))
            .text_color(theme.chrome_fg_active)
            .child(self.rename_value.clone());
        if self.cursor_on {
            field = field.child(div().w(px(2.0)).h(px(20.0)).rounded_full().bg(theme.accent));
        }

        div()
            .absolute()
            .top(px(120.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("rename-panel", 0))
                    .w(px(480.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .p(px(20.0))
                    .rounded(px(12.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .track_focus(&self.rename_focus)
                    .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                        let keystroke = event.keystroke.clone();
                        view.handle_rename_key(&keystroke, window, cx);
                    }))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.chrome_fg_active)
                            .child("Rename tab"),
                    )
                    .child(field)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.faint_fg)
                            .child("Enter to apply · Esc to cancel · empty restores the shell name"),
                    ),
            )
            .with_animation(
                ElementId::named_usize("rename-anim", 0),
                Animation::new(Duration::from_millis(OVERLAY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta).top(px(120.0 - 8.0 * (1.0 - delta))),
            )
            .into_any_element()
    }
}
