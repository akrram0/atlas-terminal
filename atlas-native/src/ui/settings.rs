//! Application settings UI: appearance, shortcuts, profiles, and general configuration.

use std::time::Duration;

use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, KeyDownEvent, Rgba, div, px, relative, rgb, rgba,
};

use crate::config::{
    action_display_name, default_shortcuts, format_keystroke_chord,
    pretty_shortcut, save_app_settings, SettingsTab, ACTION_CLEAR_SCROLLBACK, ACTION_CLOSE_TAB,
    ACTION_COPY, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE, ACTION_FULLSCREEN,
    ACTION_NEW_TAB, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_PALETTE, ACTION_PASTE,
    ACTION_PREV_TAB, ACTION_SPLIT_HORIZONTAL, ACTION_SPLIT_VERTICAL, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT,
};
use crate::icons;
use crate::theme::Theme;
use crate::updater::{self, format_bytes};
use crate::view::{AtlasView, UpdateDownloadState};
use super::animation::{ease_apple_snappy, BODY_IN_MS};
use super::controls::{pill_button, stepper, toggle_pill};

impl AtlasView {
    pub fn sync_and_save_settings(&mut self) {
        let cursor_shape = match self.cursor_style {
            None => "auto".to_string(),
            Some(CursorShape::Block) => "block".to_string(),
            Some(CursorShape::Beam) => "bar".to_string(),
            Some(CursorShape::Underline) => "underline".to_string(),
            Some(CursorShape::HollowBlock) => "hollow".to_string(),
            Some(CursorShape::Hidden) => "hidden".to_string(),
        };
        self.app_settings.theme_mode = self.theme_mode.clone();
        self.app_settings.theme = self.registry.preset(self.theme_id).id.to_string();
        self.app_settings.font_family = self.font_family.clone();
        self.app_settings.font_size = self.font_size;
        self.app_settings.line_height = self.line_height;
        self.app_settings.corner_radius = self.corner_radius;
        self.app_settings.opacity = self.opacity;
        self.app_settings.blur_enabled = self.blur_enabled;
        self.app_settings.cursor_shape = cursor_shape;
        self.app_settings.cursor_blink = self.cursor_blink;
        self.app_settings.auto_check_updates = self.updater_settings.auto_check_updates;
        self.app_settings.update_check_interval_mins = self.updater_settings.update_check_interval_mins;
        self.app_settings.last_check_time = self.updater_settings.last_check_time;
        self.app_settings.etag = self.updater_settings.etag.clone();
        self.app_settings.last_seen_version = self.updater_settings.last_seen_version.clone();
        self.app_settings.skipped_version = self.updater_settings.skipped_version.clone();

        let (_, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        crate::rounded_corners::set_dark_mode(is_dark);
        crate::rounded_corners::set_blur(self.blur_enabled);
        save_app_settings(&self.app_settings);
    }

    pub fn set_font_family(&mut self, family: &str, cx: &mut Context<Self>) {
        self.font_family = family.to_string();
        self.font = crate::view::resolve_terminal_font(&self.font_family);
        self.sync_and_save_settings();
        cx.notify();
    }

    pub fn settings_page(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let lum = 0.2126 * theme.elevated_bg.r + 0.7152 * theme.elevated_bg.g + 0.0722 * theme.elevated_bg.b;
        let is_dark = lum < 0.45;
        let sc_title = if is_dark { rgb(0xFFFFFF) } else { rgb(0x0F172A) };
        let sc_section = theme.accent;
        let sc_label = if is_dark { rgb(0xE2E8F0) } else { rgb(0x1E293B) };
        let sc_desc = if is_dark { rgb(0x94A3B8) } else { rgb(0x475569) };
        let sc_back_bg = if is_dark { rgba(0xFFFFFF0F) } else { rgba(0x0000000C) };
        let sc_back_border = if is_dark { rgba(0xFFFFFF20) } else { rgba(0x00000018) };
        let sc_back_fg = if is_dark { rgb(0xE2E8F0) } else { rgb(0x1E293B) };
        let sc_nav_idle_fg = if is_dark { rgb(0x94A3B8) } else { rgb(0x64748B) };
        let sc_nav_active_fg = if is_dark { rgb(0xFFFFFF) } else { rgb(0x0F172A) };
        let sc_nav_active_bg = if is_dark { rgba(0xFFFFFF18) } else { rgba(0x00000012) };

        let card = || {
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(16.0))
                .rounded(px(10.0))
                .bg(theme.elevated_bg)
                .border_1()
                .border_color(theme.elevated_border)
                .w(relative(1.0))
                .min_w(px(0.0))
        };
        let section = |label: &'static str| -> AnyElement {
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(sc_section)
                .child(label)
                .into_any_element()
        };
        let row_label = |label: &'static str| -> AnyElement {
            div()
                .w(px(110.0))
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(sc_label)
                .child(label)
                .into_any_element()
        };

        // --- Left Sidebar (150px) ---
        let nav_items = [
            (SettingsTab::General, "General", icons::TERMINAL),
            (SettingsTab::Appearance, "Appearance", icons::SLIDERS),
            (SettingsTab::Shortcuts, "Shortcuts", icons::KEYBOARD),
        ];

        let mut sidebar = div()
            .id(ElementId::named_usize("settings-sidebar-scroll", 0))
            .w(px(150.0))
            .flex_shrink_0()
            .h(relative(1.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.hairline)
            .p(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .mb(px(10.0))
                    .child(icons::icon(icons::SETTINGS).size(px(16.0)).text_color(theme.accent))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(sc_title)
                            .child("Settings"),
                    ),
            );

        for (nav_idx, &(tab, label, icon)) in nav_items.iter().enumerate() {
            let is_active = self.settings_tab == tab;
            let item_fg = if is_active {
                sc_nav_active_fg
            } else {
                sc_nav_idle_fg
            };
            let item_bg = if is_active {
                sc_nav_active_bg
            } else {
                rgba(0x00000000)
            };
            let hover_bg = theme.card_hover;
            sidebar = sidebar.child(
                div()
                    .id(ElementId::named_usize("settings-nav-item", nav_idx))
                    .h(px(32.0))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .rounded(px(6.0))
                    .bg(item_bg)
                    .text_color(item_fg)
                    .hover(move |s| s.bg(hover_bg))
                    .child(icons::icon(icon).size(px(14.0)).text_color(item_fg))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(if is_active {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(label),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.settings_tab = tab;
                        view.settings_scroll_pos = 0.0;
                        view.settings_scroll_target = 0.0;
                        view.settings_scroll_handle.set_offset(gpui::Point::default());
                        cx.notify();
                    })),
            );
        }


        // --- Appearance Tab ---
        let fonts = [
            "Cascadia Code",
            "Consolas",
            "Fira Code",
            "JetBrains Mono",
            "Cascadia Mono",
        ];
        let mut font_family_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Font family"));
        for (idx, &family) in fonts.iter().enumerate() {
            let is_active = self.font_family.eq_ignore_ascii_case(family);
            let fam_str = family.to_string();
            font_family_row = font_family_row.child(pill_button(
                fam_str.clone(),
                100 + idx,
                is_active,
                theme,
                move |view, cx| {
                    view.set_font_family(&fam_str, cx);
                },
                cx,
            ));
        }

        let font_size_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Font size"))
            .child(stepper(
                110,
                format!("{} pt", self.font_size as i32),
                theme,
                |view, cx| {
                    view.font_size = (view.font_size - 1.0).max(10.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.font_size = (view.font_size + 1.0).min(24.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let line_height_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Line height"))
            .child(stepper(
                111,
                format!("{:.2}", self.line_height),
                theme,
                |view, cx| {
                    view.line_height =
                        ((view.line_height - 0.05) * 100.0).round() / 100.0;
                    view.line_height = view.line_height.max(1.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.line_height =
                        ((view.line_height + 0.05) * 100.0).round() / 100.0;
                    view.line_height = view.line_height.min(1.8);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let corner_radius_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Corner radius"))
            .child(stepper(
                112,
                format!("{} px", self.corner_radius as i32),
                theme,
                |view, cx| {
                    view.corner_radius = (view.corner_radius - 1.0).max(0.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.corner_radius = (view.corner_radius + 1.0).min(20.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let opacity_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Window opacity"))
            .child(stepper(
                113,
                format!("{}%", (self.opacity * 100.0).round() as i32),
                theme,
                |view, cx| {
                    view.opacity =
                        ((view.opacity - 0.05) * 100.0).round() / 100.0;
                    view.opacity = view.opacity.max(0.5);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                |view, cx| {
                    view.opacity =
                        ((view.opacity + 0.05) * 100.0).round() / 100.0;
                    view.opacity = view.opacity.min(1.0);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let blur_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Windows 11 Acrylic blur"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("System backdrop acrylic blur (Windows 11 DWM)"),
                    ),
            )
            .child(toggle_pill(
                114,
                self.blur_enabled,
                theme,
                |view, cx| {
                    view.blur_enabled = !view.blur_enabled;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let mode_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Color mode"))
            .child(pill_button(
                "Auto (System)".to_string(),
                118,
                self.theme_mode == "auto",
                theme,
                |view, cx| view.set_theme_mode("auto", cx),
                cx,
            ))
            .child(pill_button(
                "Dark".to_string(),
                119,
                self.theme_mode == "dark",
                theme,
                |view, cx| view.set_theme_mode("dark", cx),
                cx,
            ))
            .child(pill_button(
                "Warm Light".to_string(),
                120,
                self.theme_mode == "light",
                theme,
                |view, cx| view.set_theme_mode("light", cx),
                cx,
            ));

        let mut theme_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Theme preset"));
        for index in 0..self.registry.len() {
            let is_sel = self.theme_id == index;
            theme_row = theme_row.child(pill_button(
                self.registry.preset(index).name.to_string(),
                121 + index,
                is_sel,
                theme,
                move |view, cx| view.set_theme(index, cx),
                cx,
            ));
        }

        let theme_desc_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(div().w(px(110.0)).flex_shrink_0())
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(sc_desc)
                    .child(self.registry.preset(self.theme_id).description),
            );

        let palette_swatch = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Active colors"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .items_center()
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.bg)
                            .border_1()
                            .border_color(theme.elevated_border),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.panel_bg)
                            .border_1()
                            .border_color(theme.elevated_border),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.elevated_bg)
                            .border_1()
                            .border_color(theme.elevated_border),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.accent),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.fg),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(5.0))
                            .bg(theme.chrome_fg),
                    ),
            );

        let cursor_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Cursor style"))
            .child(pill_button(
                "Auto".to_string(),
                130,
                self.cursor_style.is_none(),
                theme,
                |view, cx| {
                    view.cursor_style = None;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Block".to_string(),
                131,
                self.cursor_style == Some(CursorShape::Block),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Block);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Bar".to_string(),
                132,
                self.cursor_style == Some(CursorShape::Beam),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Beam);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ))
            .child(pill_button(
                "Underline".to_string(),
                133,
                self.cursor_style == Some(CursorShape::Underline),
                theme,
                |view, cx| {
                    view.cursor_style = Some(CursorShape::Underline);
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let cursor_blink_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Cursor blinking"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Animate cursor breathing and blinking when idle"),
                    ),
            )
            .child(toggle_pill(
                134,
                self.cursor_blink,
                theme,
                |view, cx| {
                    view.cursor_blink = !view.cursor_blink;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let appearance_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(sc_desc)
                    .mb(px(4.0))
                    .child("Color theme, typography, window geometry, and terminal caret"),
            )
            .child(
                card()
                    .child(section("THEME & COLOR SCHEME"))
                    .child(mode_row)
                    .child(theme_row)
                    .child(theme_desc_row)
                    .child(palette_swatch),
            )
            .child(
                card()
                    .child(section("FONT & TYPOGRAPHY"))
                    .child(font_family_row)
                    .child(font_size_row)
                    .child(line_height_row),
            )
            .child(
                card()
                    .child(section("WINDOW & BACKDROP"))
                    .child(corner_radius_row)
                    .child(opacity_row)
                    .child(blur_row),
            )
            .child(
                card()
                    .child(section("TERMINAL CARET"))
                    .child(cursor_row)
                    .child(cursor_blink_row),
            );

        // --- Shortcuts Tab ---
        let def_map = default_shortcuts();

        let render_shortcut_row = |idx: usize, action: &'static str| -> AnyElement {
            let current_chord = self.get_shortcut(action);
            let default_chord = def_map.get(action).cloned().unwrap_or_default();
            let is_recording = self.recording_shortcut.as_deref() == Some(action);
            let is_custom = current_chord != default_chord;

            let conflict = self
                .app_settings
                .shortcuts
                .iter()
                .find(|(act, ch)| {
                    act.as_str() != action && ch.as_str() == current_chord.as_str()
                })
                .map(|(act, _)| action_display_name(act));

            let action_str = action.to_string();
            let def_str = default_chord.clone();

            let mut row = div()
                .id(ElementId::named_usize("shortcut-row", idx))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .py(px(4.0));

            let mut left_col = div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(sc_label)
                        .child(action_display_name(action)),
                );

            if let Some(other) = conflict {
                left_col = left_col.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0xE6A700))
                        .child(format!("Conflict with {other}")),
                );
            }
            row = row.child(left_col);

            let keycap_text = if is_recording {
                "Press key combo…".to_string()
            } else {
                pretty_shortcut(&current_chord)
            };

            let keycap_bg = if is_recording {
                Rgba {
                    a: 0.22,
                    ..theme.accent
                }
            } else if is_dark {
                rgba(0xFFFFFF12)
            } else {
                rgba(0x0000000A)
            };
            let keycap_border = if is_recording {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF22)
            } else {
                rgba(0x00000018)
            };
            let keycap_fg = if is_recording {
                theme.accent
            } else if is_dark {
                rgb(0xF1F5F9)
            } else {
                rgb(0x0F172A)
            };

            let mut right_col = div().flex().flex_row().items_center().gap(px(6.0));

            let act_clone = action_str.clone();
            let keycap_btn = div()
                .id(ElementId::named_usize("shortcut-keycap", idx))
                .h(px(26.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(keycap_bg)
                .border_1()
                .border_color(keycap_border)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(keycap_fg)
                .hover(move |s| s.opacity(0.85))
                .child(keycap_text)
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.recording_shortcut = Some(act_clone.clone());
                    cx.notify();
                }));
            right_col = right_col.child(keycap_btn);

            if is_custom {
                let reset_btn = div()
                    .id(ElementId::named_usize("shortcut-reset", idx))
                    .h(px(26.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .text_size(px(11.0))
                    .text_color(sc_desc)
                    .hover(move |s| {
                        s.bg(theme.card_hover).text_color(sc_label)
                    })
                    .child("Reset")
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.app_settings
                            .shortcuts
                            .insert(action_str.clone(), def_str.clone());
                        view.sync_and_save_settings();
                        cx.notify();
                    }));
                right_col = right_col.child(reset_btn);
            }

            row.child(right_col).into_any_element()
        };

        let tab_window_actions = [
            ACTION_NEW_TAB,
            ACTION_CLOSE_TAB,
            ACTION_NEXT_TAB,
            ACTION_PREV_TAB,
            ACTION_PALETTE,
            ACTION_OPEN_SETTINGS,
            ACTION_FULLSCREEN,
        ];
        let mut tab_window_card = card().child(section("TABS & WINDOWS"));
        for (i, &act) in tab_window_actions.iter().enumerate() {
            tab_window_card = tab_window_card.child(render_shortcut_row(i, act));
        }

        let split_pane_actions = [
            ACTION_SPLIT_VERTICAL,
            ACTION_SPLIT_HORIZONTAL,
            ACTION_FOCUS_NEXT_PANE,
            ACTION_FOCUS_PREV_PANE,
        ];
        let mut split_pane_card = card().child(section("SPLITS & PANES"));
        for (i, &act) in split_pane_actions.iter().enumerate() {
            split_pane_card = split_pane_card.child(render_shortcut_row(100 + i, act));
        }

        let terminal_selection_actions = [
            ACTION_COPY,
            ACTION_PASTE,
            ACTION_CLEAR_SCROLLBACK,
            ACTION_ZOOM_IN,
            ACTION_ZOOM_OUT,
        ];
        let mut terminal_selection_card = card().child(section("TERMINAL & SELECTION"));
        for (i, &act) in terminal_selection_actions.iter().enumerate() {
            terminal_selection_card = terminal_selection_card.child(render_shortcut_row(200 + i, act));
        }

        let shortcuts_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .mb(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(sc_desc)
                            .child("Click a binding to rebind. Press Esc to cancel."),
                    )
                    .child(pill_button(
                        "Reset All".to_string(),
                        299,
                        false,
                        theme,
                        |view, cx| {
                            view.app_settings.shortcuts = default_shortcuts();
                            view.sync_and_save_settings();
                            cx.notify();
                        },
                        cx,
                    )),
            )
            .child(tab_window_card)
            .child(split_pane_card)
            .child(terminal_selection_card);

        // --- General Tab ---
        let shells = ["PowerShell", "WSL", "OpenCode", "Command Prompt", "Git Bash"];
        let mut default_shell_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Default shell"));
        for (idx, &shell_name) in shells.iter().enumerate() {
            let is_sel = self.app_settings.default_shell.eq_ignore_ascii_case(shell_name);
            let s_str = shell_name.to_string();
            default_shell_row = default_shell_row.child(pill_button(
                s_str.clone(),
                300 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.default_shell = s_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let shell_desc_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(div().w(px(110.0)).flex_shrink_0())
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(sc_desc)
                    .child(if self.app_settings.default_shell.eq_ignore_ascii_case("opencode") {
                        "Launches the OpenCode AI agent CLI in the tab workspace"
                    } else if self.app_settings.default_shell.eq_ignore_ascii_case("wsl") {
                        "Launches the default Linux distribution via Windows Subsystem for Linux"
                    } else {
                        "Default shell profile spawned when opening new tabs and panes"
                    }),
            );

        let start_dirs = [("home", "User home (~)"), ("current", "Current directory")];
        let mut start_dir_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Start directory"));
        for (idx, &(dir_key, dir_label)) in start_dirs.iter().enumerate() {
            let is_sel = self.app_settings.start_directory == dir_key;
            let k_str = dir_key.to_string();
            start_dir_row = start_dir_row.child(pill_button(
                dir_label.to_string(),
                310 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.start_directory = k_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let scroll_opts = [
            (5000, "5,000"),
            (10000, "10,000"),
            (20000, "20,000"),
            (50000, "50,000"),
        ];
        let mut scroll_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Scrollback"));
        for (idx, &(lines, lbl)) in scroll_opts.iter().enumerate() {
            let is_sel = self.app_settings.scrollback_lines == lines;
            scroll_row = scroll_row.child(pill_button(
                lbl.to_string(),
                320 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.scrollback_lines = lines;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let startup_opts = [
            ("restore_session", "Restore last session"),
            ("new_tab", "Open new tab"),
        ];
        let mut startup_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Startup mode"));
        for (idx, &(mode_key, mode_lbl)) in startup_opts.iter().enumerate() {
            let is_sel = self.app_settings.startup_mode == mode_key;
            let m_str = mode_key.to_string();
            startup_row = startup_row.child(pill_button(
                mode_lbl.to_string(),
                330 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.app_settings.startup_mode = m_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

        let confirm_close_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Confirm before closing active process"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Ask before closing tabs with running commands"),
                    ),
            )
            .child(toggle_pill(
                340,
                self.app_settings.confirm_close_process,
                theme,
                |view, cx| {
                    view.app_settings.confirm_close_process =
                        !view.app_settings.confirm_close_process;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let copy_on_select_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Copy on select"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Automatically copy selected terminal text to clipboard"),
                    ),
            )
            .child(toggle_pill(
                341,
                self.app_settings.copy_on_select,
                theme,
                |view, cx| {
                    view.app_settings.copy_on_select = !view.app_settings.copy_on_select;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let is_auto_check = self.updater_settings.auto_check_updates;
        let auto_check_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Check for updates automatically"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Background check every interval and on focus; silent when offline"),
                    ),
            )
            .child(toggle_pill(
                350,
                is_auto_check,
                theme,
                |view, cx| {
                    view.updater_settings.auto_check_updates =
                        !view.updater_settings.auto_check_updates;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let mut interval_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(row_label("Interval"));
        for (idx, &(mins, label)) in updater::INTERVAL_OPTIONS.iter().enumerate() {
            let is_sel = self.updater_settings.update_check_interval_mins == mins;
            interval_row = interval_row.child(pill_button(
                label.to_string(),
                360 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.updater_settings.update_check_interval_mins = mins;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

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
            format!("Last checked: {last_checked_text}")
        };

        let check_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pt(px(2.0))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(sc_label)
                    .child(status_line),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .children(if let Some(ref update) = self.available_update {
                        if matches!(self.update_download_state, UpdateDownloadState::Idle) {
                            let ver = update.version.clone();
                            Some(pill_button(
                                format!("Download v{ver}"),
                                369,
                                true,
                                theme,
                                |view, cx| {
                                    view.start_download_update(cx);
                                },
                                cx,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    })
                    .child(pill_button(
                        if is_checking {
                            "Checking…".to_string()
                        } else {
                            "Check now".to_string()
                        },
                        370,
                        false,
                        theme,
                        |view, cx| {
                            view.check_updates_manual(cx);
                        },
                        cx,
                    )),
            );

        let update_status_card = match &self.update_download_state {
            UpdateDownloadState::Downloading {
                version,
                received_bytes,
                total_bytes,
                percent,
            } => {
                let pct = percent.clamp(0.0, 1.0);
                let pct_int = (pct * 100.0) as u32;
                let detail = if *total_bytes > 0 {
                    format!(
                        "{} / {} ({}%)",
                        format_bytes(*received_bytes),
                        format_bytes(*total_bytes),
                        pct_int
                    )
                } else {
                    format!("{} ({}%)", format_bytes(*received_bytes), pct_int)
                };
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .bg(theme.panel_bg)
                        .border_1()
                        .border_color(theme.hairline)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(sc_label)
                                        .child(format!("Downloading v{version}…")),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.accent)
                                        .child(detail),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(5.0))
                                .rounded(px(2.5))
                                .bg(theme.elevated_bg)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(px((200.0 * pct).max(6.0)))
                                        .rounded(px(2.5))
                                        .bg(theme.accent),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .child(pill_button(
                                    "Cancel".to_string(),
                                    371,
                                    false,
                                    theme,
                                    |view, cx| {
                                        view.cancel_download_update(cx);
                                    },
                                    cx,
                                )),
                        ),
                )
            }
            UpdateDownloadState::ReadyToRestart { version, .. } => {
                let ver = version.clone();
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .bg(Rgba {
                            a: 0.12,
                            ..theme.accent
                        })
                        .border_1()
                        .border_color(Rgba {
                            a: 0.25,
                            ..theme.accent
                        })
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(sc_label)
                                .child(format!("v{ver} is verified and ready to apply!")),
                        )
                        .child(pill_button(
                            "Restart to Apply".to_string(),
                            372,
                            true,
                            theme,
                            |view, cx| {
                                view.apply_update_and_restart(cx);
                            },
                            cx,
                        )),
                )
            }
            UpdateDownloadState::Failed { error, .. } => {
                let err = error.clone();
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .p(px(8.0))
                        .rounded(px(6.0))
                        .bg(Rgba {
                            a: 0.12,
                            ..rgb(0xE05252)
                        })
                        .border_1()
                        .border_color(Rgba {
                            a: 0.25,
                            ..rgb(0xE05252)
                        })
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(rgb(0xE05252))
                                .child(format!("Update error: {err}")),
                        )
                        .child(pill_button(
                            "Retry".to_string(),
                            373,
                            false,
                            theme,
                            |view, cx| {
                                if view.available_update.as_ref().and_then(|u| u.installer_url.as_ref()).is_none() {
                                    view.check_updates_manual(cx);
                                } else {
                                    view.start_download_update(cx);
                                }
                            },
                            cx,
                        )),
                )
            }
            _ => None,
        };

        let link_hot = theme.chrome_fg_active;
        let pkg_version = env!("CARGO_PKG_VERSION");
        let about = card()
            .child(section("ABOUT"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                icons::icon(icons::ATLAS_MARK)
                                    .size(px(18.0))
                                    .text_color(theme.accent),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(sc_title)
                                    .child(format!("Atlas Terminal {pkg_version}")),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(sc_desc)
                            .child("Native GPUI renderer · alacritty_terminal · no webview, no HTML"),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("settings-link", 0))
                            .text_size(px(11.5))
                            .text_color(theme.accent)
                            .hover(move |style| style.text_color(link_hot))
                            .child("github.com/akrram0/atlas-terminal")
                            .on_click(cx.listener(|_view, _: &ClickEvent, _window, cx| {
                                cx.open_url("https://github.com/akrram0/atlas-terminal");
                            })),
                    ),
            );

        let config_file = crate::user_data_dir().join("settings.json");
        let config_dir = crate::user_data_dir();

        let raw_config_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("settings.json (User Configuration)"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Inspect or edit raw JSON settings directly in your preferred editor"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .child(pill_button(
                        "Open File".to_string(),
                        380,
                        false,
                        theme,
                        move |_view, _cx| {
                            let path_str = config_file.to_string_lossy().to_string();
                            let _ = std::process::Command::new("cmd")
                                .args(["/c", "start", "", &path_str])
                                .spawn();
                        },
                        cx,
                    ))
                    .child(pill_button(
                        "Open Folder".to_string(),
                        381,
                        false,
                        theme,
                        move |_view, _cx| {
                            let _ = std::process::Command::new("explorer")
                                .arg(&config_dir)
                                .spawn();
                        },
                        cx,
                    )),
            );

        let reset_defaults_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child("Restore default configuration"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child("Reset theme, shortcuts, fonts, and window styling to defaults"),
                    ),
            )
            .child(pill_button(
                "Reset All Settings".to_string(),
                382,
                false,
                theme,
                |view, cx| {
                    view.app_settings = crate::config::AppSettingsFile::default();
                    view.theme_mode = view.app_settings.theme_mode.clone();
                    view.font_family = view.app_settings.font_family.clone();
                    view.font_size = view.app_settings.font_size;
                    view.line_height = view.app_settings.line_height;
                    view.corner_radius = view.app_settings.corner_radius;
                    view.opacity = view.app_settings.opacity;
                    view.blur_enabled = view.app_settings.blur_enabled;
                    view.cursor_style = None;
                    view.cursor_blink = view.app_settings.cursor_blink;
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));

        let general_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(sc_desc)
                    .mb(px(4.0))
                    .child("Shell profiles, session startup, and application updates"),
            )
            .child(
                card()
                    .child(section("SHELL & ENVIRONMENT"))
                    .child(default_shell_row)
                    .child(shell_desc_row)
                    .child(start_dir_row)
                    .child(scroll_row),
            )
            .child(
                card()
                    .child(section("USER FREEDOM & CONFIG"))
                    .child(raw_config_row)
                    .child(reset_defaults_row),
            )
            .child(
                card()
                    .child(section("SESSION & BEHAVIOR"))
                    .child(startup_row)
                    .child(confirm_close_row)
                    .child(copy_on_select_row),
            )
            .child(
                card()
                    .child(section("UPDATES"))
                    .child(auto_check_row)
                    .child(interval_row)
                    .children(update_status_card)
                    .child(check_row),
            )
            .child(about);

        let content_body = div()
            .flex()
            .flex_col()
            .child(match self.settings_tab {
                SettingsTab::General => general_view.into_any_element(),
                SettingsTab::Appearance => appearance_view.into_any_element(),
                SettingsTab::Shortcuts => shortcuts_view.into_any_element(),
            })
            .with_animation(
                ElementId::named_usize("settings-tab-pane", self.settings_tab as usize),
                Animation::new(Duration::from_millis(BODY_IN_MS)).with_easing(ease_apple_snappy),
                |el, delta| el.opacity(delta),
            )
            .into_any_element();

        let content_header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .mb(px(18.0))
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(sc_title)
                    .child(match self.settings_tab {
                        SettingsTab::General => "General",
                        SettingsTab::Appearance => "Appearance",
                        SettingsTab::Shortcuts => "Keyboard Shortcuts",
                    }),
            )
            .child(
                div()
                    .id(ElementId::named_usize("settings-top-back-btn", 0))
                    .h(px(28.0))
                    .px(px(10.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(sc_back_border)
                    .bg(sc_back_bg)
                    .hover(move |s| s.bg(theme.card_hover))
                    .text_color(sc_back_fg)
                    .child(icons::icon(icons::CHEVRON_LEFT).size(px(13.0)))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Back to Terminal (Esc)"),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                    })),
            );

        let right_pane = div()
            .id(ElementId::named_usize("settings-content-scroll", 0))
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.settings_scroll_handle)
            .p(px(16.0))
            .pb(px(80.0))
            .child(
                div()
                    .w(relative(1.0))
                    .max_w(px(680.0))
                    .min_w(px(0.0))
                    .child(content_header)
                    .child(content_body),
            );

        div()
            .id(ElementId::named_usize("settings-root", 0))
            .flex()
            .flex_row()
            .flex_1()
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .track_focus(&self.settings_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if let Some(ref action) = view.recording_shortcut.clone() {
                    if event.keystroke.key == "escape" {
                        view.recording_shortcut = None;
                        cx.notify();
                        return;
                    }
                    if let Some(chord) = format_keystroke_chord(&event.keystroke) {
                        view.app_settings.shortcuts.insert(action.clone(), chord);
                        view.recording_shortcut = None;
                        view.sync_and_save_settings();
                        cx.notify();
                        return;
                    }
                }
                if event.keystroke.key == "escape" {
                    view.settings_open = false;
                    window.focus(&view.focus);
                    cx.notify();
                    return;
                }
                if let Some(chord) = format_keystroke_chord(&event.keystroke) {
                    if chord == view.get_shortcut(ACTION_OPEN_SETTINGS) {
                        view.settings_open = false;
                        window.focus(&view.focus);
                        cx.notify();
                        return;
                    }
                }
            }))
            .child(sidebar)
            .child(right_pane)
            .into_any_element()
    }
}
