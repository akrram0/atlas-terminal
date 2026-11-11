//! Application settings UI: appearance, shortcuts, profiles, and general configuration.
//! Designed with macOS / Apple SF design language, pixel-perfect metric geometry,
//! and native vector controls.

use alacritty_terminal::vte::ansi::CursorShape;
use gpui::prelude::*;
use gpui::{
    ScrollDelta, ScrollWheelEvent,
    AnyElement, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, KeyDownEvent, Rgba, div, px, relative, rgb, rgba,
};

use crate::config::{
    action_display_name, default_shortcuts, format_keystroke_chord,
    pretty_shortcut, save_app_settings, SettingsTab, ACTION_NEW_WINDOW, ACTION_CLEAR_SCROLLBACK, ACTION_CLOSE_TAB,
    ACTION_COPY, ACTION_FIND, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE, ACTION_FULLSCREEN,
    ACTION_NEW_TAB, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_PALETTE, ACTION_PASTE,
    ACTION_PREV_TAB, ACTION_SPLIT_HORIZONTAL, ACTION_SPLIT_VERTICAL, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT, ACTION_ZOOM_RESET, ACTION_SELECT_ALL, ACTION_SCROLL_PAGE_UP,
    ACTION_SCROLL_PAGE_DOWN, ACTION_SCROLL_TO_TOP, ACTION_SCROLL_TO_BOTTOM,
    ACTION_OPEN_URL_PICKER, ACTION_NEXT_THEME, ACTION_PREV_THEME, ACTION_TOGGLE_DARK_LIGHT,
    ACTION_TOGGLE_MAXIMIZE, ACTION_TOGGLE_SIDEBAR,
    ACTION_PROFILE_MENU, ACTION_NEW_TAB_WSL, ACTION_NEW_TAB_POWERSHELL, ACTION_NEW_TAB_CMD,
    ACTION_NEW_TAB_GITBASH, ACTION_NEW_TAB_OPENCODE,
};
use crate::icons;
use crate::motion::Motion;
use crate::theme::Theme;
use crate::view::AtlasView;
use super::controls::{pill_button, segmented_control, stepper, toggle_pill};

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
        self.app_settings.theme_dark = self.registry.preset(self.theme_dark_id).id.to_string();
        self.app_settings.theme_light = self.registry.preset(self.theme_light_id).id.to_string();
        self.app_settings.theme = self.registry.preset(self.theme_id).id.to_string();
        self.app_settings.font_family = self.font_family.clone();
        self.app_settings.font_size = self.font_size;
        self.app_settings.line_height = self.line_height;
        self.app_settings.corner_radius = self.corner_radius;
        if self.app_settings.blur_enabled {
            self.app_settings.opacity = self.opacity.clamp(0.01, 0.99);
        }
        self.app_settings.last_settings_tab = self.settings_tab.as_str().to_string();
        self.app_settings.cursor_shape = cursor_shape;
        self.app_settings.cursor_blink = self.cursor_blink;
        self.app_settings.auto_check_updates = self.updater_settings.auto_check_updates;
        self.app_settings.update_check_interval_mins = self.updater_settings.update_check_interval_mins;
        self.app_settings.update_channel = self.updater_settings.channel.as_str().to_string();
        self.app_settings.last_check_time = self.updater_settings.last_check_time;
        self.app_settings.etag = self.updater_settings.etag.clone();
        self.app_settings.last_seen_version = self.updater_settings.last_seen_version.clone();
        self.app_settings.skipped_version = self.updater_settings.skipped_version.clone();

        let (_, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        crate::rounded_corners::set_dark_mode(is_dark);
        crate::rounded_corners::set_opacity(self.opacity);
        save_app_settings(&self.app_settings);
        self.last_settings_mtime = std::fs::metadata(crate::user_data_dir().join("settings.json")).and_then(|m| m.modified()).ok();
    }

    pub fn reload_settings_from_disk(&mut self, cx: &mut Context<Self>) {
        let loaded = crate::config::load_app_settings();
        self.last_settings_mtime = std::fs::metadata(crate::user_data_dir().join("settings.json")).and_then(|m| m.modified()).ok();
        self.theme_mode = loaded.theme_mode.clone();
        self.theme_dark_id = self.registry.find_by_name_or_id(&loaded.theme_dark).unwrap_or(0);
        self.theme_light_id = self.registry.find_by_name_or_id(&loaded.theme_light).unwrap_or(3);
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        self.theme_id = if is_light { self.theme_light_id } else { self.theme_dark_id };
        let (target, is_dark) = self.registry.resolve(self.theme_id, &self.theme_mode);
        self.theme = target;
        crate::rounded_corners::set_dark_mode(is_dark);
        self.font_family = loaded.font_family.clone();
        self.font_size = loaded.font_size;
        self.line_height = loaded.line_height;
        let pref_op = crate::view::normalize_opacity(loaded.opacity);
        self.app_settings.opacity = pref_op;
        self.app_settings.blur_enabled = loaded.blur_enabled;
        self.opacity = if loaded.blur_enabled {
            pref_op
        } else {
            1.0
        };
        self.cursor_blink = loaded.cursor_blink;
        self.cursor_style = match loaded.cursor_shape.to_lowercase().as_str() {
            "block" => Some(CursorShape::Block),
            "beam" | "bar" => Some(CursorShape::Beam),
            "underline" => Some(CursorShape::Underline),
            "hollow" => Some(CursorShape::HollowBlock),
            _ => None,
        };
        crate::rounded_corners::set_opacity(self.opacity);
        self.updater_settings.channel = match loaded.update_channel.as_str() {
            "preview" => crate::updater::UpdateChannel::Preview,
            _ => crate::updater::UpdateChannel::Stable,
        };
        self.app_settings = loaded;
        cx.notify();
    }

    pub fn settings_page(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let lum = 0.2126 * theme.elevated_bg.r + 0.7152 * theme.elevated_bg.g + 0.0722 * theme.elevated_bg.b;
        let is_dark = lum < 0.45;
        let sc_title = if is_dark { rgb(0xFFFFFF) } else { rgb(0x0F172A) };
        let sc_section = theme.accent;
        let sc_label = if is_dark { rgb(0xE2E8F0) } else { rgb(0x1E293B) };
        let sc_desc = if is_dark { rgb(0x94A3B8) } else { rgb(0x64748B) };
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
                .rounded(px(10.0))
                .bg(theme.elevated_bg)
                .border_1()
                .border_color(theme.elevated_border)
                .w(relative(1.0))
                .min_w(px(0.0))
                .overflow_hidden()
        };

        let section_header = |label: &'static str| -> AnyElement {
            div()
                .px(px(20.0))
                .pt(px(18.0))
                .pb(px(10.0))
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(sc_section)
                .child(label)
                .into_any_element()
        };

        let setting_row = |label: &'static str, desc: Option<&'static str>, control: AnyElement, is_last: bool| -> AnyElement {
            let mut row = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px(px(20.0))
                .py(px(16.0))
                .gap(px(18.0));
            if !is_last {
                row = row.border_b_1().border_color(theme.hairline);
            }
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child(label),
                    )
                    .children(desc.map(|d| {
                        div()
                            .text_size(px(12.5))
                            .text_color(sc_desc)
                            .child(d)
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .child(control)
            )
            .into_any_element()
        };

                // --- Horizontal Category Strip (Apple-Style Full-Width 2-Tier Header) ---
        let nav_items = [
            (SettingsTab::General, "General", icons::TERMINAL),
            (SettingsTab::Appearance, "Appearance", icons::SUN),
            (SettingsTab::Typography, "Typography", icons::PEN),
            (SettingsTab::Scrolling, "Scrolling", icons::REFRESH),
            (SettingsTab::Window, "Window", icons::SLIDERS),
            (SettingsTab::Tools, "Tools", icons::SATELLITE),
            (SettingsTab::PixelArt, "Pixel Art", icons::SPARKLE),
            (SettingsTab::Shortcuts, "Shortcuts", icons::KEYBOARD),
            (SettingsTab::Updates, "Updates", icons::CLOUD_DOWNLOAD),
        ];

        let active_tab_title = match self.settings_tab {
            SettingsTab::General => "General Preferences",
            SettingsTab::Appearance => "Appearance & Themes",
            SettingsTab::Typography => "Typography & Font Scale",
            SettingsTab::Scrolling => "Scrolling & Physics",
            SettingsTab::Window => "Window & Glass Backdrop",
            SettingsTab::Tools => "Satellites & Protocols",
            SettingsTab::PixelArt => "Pixel Art Companion",
            SettingsTab::Shortcuts => "Keyboard Shortcuts",
            SettingsTab::Updates => "Updates & Release Channels",
        };

        let mut top_nav_pills = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .flex_shrink_0()
            .relative()
            .left(px(-self.settings_nav_scroll));

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
            let item_border = if is_active {
                theme.elevated_border
            } else {
                rgba(0x00000000)
            };
            let hover_bg = theme.card_hover;
            top_nav_pills = top_nav_pills.child(
                div()
                    .id(ElementId::named_usize("settings-top-nav-item", nav_idx))
                    .h(px(28.0))
                    .px(px(9.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .rounded(px(6.0))
                    .bg(item_bg)
                    .border_1()
                    .border_color(item_border)
                    .text_color(item_fg)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg).text_color(sc_nav_active_fg))
                    .active(move |s| s.opacity(0.80))
                    .child(icons::icon(icon).size(px(13.5)).text_color(if is_active { theme.accent } else { item_fg }))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(if is_active {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(label),
                    )
                    .children(if tab == SettingsTab::PixelArt {
                        Some(
                            div()
                                .px(px(4.0))
                                .py(px(0.5))
                                .rounded(px(3.0))
                                .bg(if is_active { theme.accent } else { rgba(0xEF444424) })
                                .text_size(px(8.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_active { rgb(0xFFFFFF) } else { theme.accent })
                                .child("BETA"),
                        )
                    } else {
                        None
                    })
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.settings_tab = tab;
                        view.settings_scroll_pos = 0.0;
                        view.settings_scroll_target = 0.0;
                        view.settings_scroll_handle.set_offset(gpui::Point::default());
                        view.settings_search_query.clear();
                        view.settings_search_active = false;
                        // STABLE POSITIONING: Never shift or re-rank buttons on click!
                        view.sync_and_save_settings();
                        cx.notify();
                    }))
                    .stagger_in(ElementId::named_usize("settings-nav-stagger", nav_idx), nav_idx),
            );
        }

        let nav_scroll_area = div()
            .id(ElementId::named_usize("settings-top-nav-scroll-area", 0))
            .w(relative(1.0))
            .h(px(32.0))
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                let delta = match event.delta {
                    ScrollDelta::Lines(d) => {
                        if d.x.abs() > 0.01 { d.x * 30.0 } else { d.y * 30.0 }
                    }
                    ScrollDelta::Pixels(d) => {
                        let px_x = f32::from(d.x);
                        let px_y = f32::from(d.y);
                        if px_x.abs() > 0.01 { px_x } else { px_y }
                    }
                };
                view.settings_nav_scroll = (view.settings_nav_scroll - delta).clamp(0.0, 300.0);
                cx.notify();
            }))
            .child(top_nav_pills);

        let is_searching = !self.settings_search_query.trim().is_empty();
        let sc_search_bg = if is_dark { rgba(0xFFFFFF0D) } else { rgba(0x00000008) };
        let sc_search_border = if is_dark { rgba(0xFFFFFF18) } else { rgba(0x00000014) };

        let search_bar = div()
            .id(ElementId::named_usize("settings-top-search", 0))
            .h(px(28.0))
            .w(if self.settings_search_active { px(200.0) } else { px(150.0) })
            .px(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .rounded(px(6.0))
            .bg(if self.settings_search_active { Rgba { a: 0.12, ..theme.accent } } else { sc_search_bg })
            .border_1()
            .border_color(if self.settings_search_active { theme.accent } else { sc_search_border })
            .cursor_pointer()
            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                view.settings_search_active = true;
                cx.notify();
            }))
            .child(icons::icon(icons::SEARCH).size(px(13.0)).text_color(if self.settings_search_active { theme.accent } else { sc_desc }))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .text_color(if self.settings_search_query.is_empty() { sc_desc } else { sc_label })
                    .overflow_hidden()
                    .child(if self.settings_search_query.is_empty() {
                        "Search settings...".to_string()
                    } else {
                        self.settings_search_query.clone()
                    })
            )
            .children(if is_searching {
                Some(
                    div()
                        .id(ElementId::named_usize("settings-clear-search", 0))
                        .cursor_pointer()
                        .p(px(2.0))
                        .text_color(sc_desc)
                        .hover(move |s| s.text_color(theme.chrome_fg_active))
                        .child(icons::icon(icons::CLOSE).size(px(12.0)))
                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                            view.settings_search_query.clear();
                            view.settings_search_active = false;
                            cx.notify();
                        }))
                )
            } else {
                None
            });

        let back_btn = div()
            .id(ElementId::named_usize("settings-top-back-btn", 0))
            .h(px(28.0))
            .px(px(9.0))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(sc_back_border)
            .bg(sc_back_bg)
            .cursor_pointer()
            .hover(move |s| s.bg(theme.card_hover))
            .active(move |s| s.opacity(0.80))
            .text_color(sc_back_fg)
            .child(icons::icon(icons::CLOSE).size(px(11.0)))
            .child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Esc"),
            )
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.settings_open = false;
                view.settings_search_query.clear();
                view.settings_search_active = false;
                window.focus(&view.focus);
                cx.notify();
            }));

        let top_nav = div()
            .id(ElementId::named_usize("settings-top-nav", 0))
            .flex_shrink_0()
            .w(relative(1.0))
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(theme.hairline)
            // Tier 1: Title, Active Category, Search Bar & Close Button
            .child(
                div()
                    .h(px(42.0))
                    .px(px(16.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(icons::icon(icons::SETTINGS).size(px(15.0)).text_color(theme.accent))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(sc_title)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(sc_desc)
                                    .child("•"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(sc_desc)
                                    .child(active_tab_title),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(search_bar)
                            .child(back_btn),
                    ),
            )
            // Tier 2: Category Navigation Strip (Spans full width!)
            .child(
                div()
                    .h(px(36.0))
                    .px(px(12.0))
                    .bg(if is_dark { rgba(0x00000028) } else { rgba(0x00000006) })
                    .border_t_1()
                    .border_color(theme.hairline)
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(nav_scroll_area),
            );

        // ==========================================
        // --- Tab 1: Appearance Tab ---
        // ==========================================

        // Color Mode (Segmented Control)
        let mode_options = vec![
            ("auto".to_string(), "Auto".to_string()),
            ("dark".to_string(), "Dark".to_string()),
            ("light".to_string(), "Light".to_string()),
        ];
        let color_mode_ctrl = segmented_control(
            "color-mode-ctrl",
            mode_options,
            &self.theme_mode,
            theme,
            |mode, view, cx| view.set_theme_mode(&mode, cx),
            cx,
        );

        // Visual Dark Themes Grid
        let mut dark_theme_grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0))
            .p(px(14.0));

        for idx in 0..self.registry.len() {
            let preset = self.registry.preset(idx);
            if !preset.supports_dark() {
                continue;
            }
            let is_sel = self.theme_dark_id == idx;
            let preview_theme = &preset.dark;

            let card_border = if is_sel {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_sel {
                Rgba { a: 0.12, ..theme.accent }
            } else {
                rgba(0x00000000)
            };

            let theme_card = div()
                .id(ElementId::named_usize("dark-theme-card", idx))
                .w(px(195.0))
                .flex()
                .flex_col()
                .rounded(px(10.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .p(px(8.0))
                .overflow_hidden()
                .cursor_pointer()
                .hover(move |s| s.bg(theme.card_hover))
                .child(
                    // Mini Window Mockup
                    div()
                        .h(px(72.0))
                        .w(relative(1.0))
                        .rounded(px(6.0))
                        .bg(preview_theme.bg)
                        .border_1()
                        .border_color(preview_theme.elevated_border)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            // Window Header
                            div()
                                .h(px(14.0))
                                .px(px(6.0))
                                .bg(preview_theme.panel_bg)
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0xFF5F56)))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0xFFBD2E)))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0x27C93F))),
                        )
                        .child(
                            // Terminal Body Preview
                            div()
                                .flex_1()
                                .p(px(6.0))
                                .flex()
                                .flex_col()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(3.0))
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(preview_theme.accent)
                                                .child("~ $"),
                                        )
                                        .child(
                                            div()
                                                .w(px(3.5))
                                                .h(px(9.0))
                                                .bg(preview_theme.cursor),
                                        ),
                                )
                                .child(
                                    // 3 Swatch Dots
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(4.0))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.accent))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.fg))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.chrome_fg)),
                                ),
                        ),
                )
                .child(
                    // Label & Indicator
                    div()
                        .pt(px(8.0))
                        .px(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            // Row 1: Title and checkmark
                            div()
                                .w(relative(1.0))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .text_size(px(13.0))
                                        .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                        .text_color(if is_sel { sc_title } else { sc_label })
                                        .child(preset.dark_name().to_string()),
                                )
                                .children(if is_sel {
                                    Some(icons::icon(icons::CHECK).size(px(14.0)).text_color(theme.accent))
                                } else {
                                    None
                                }),
                        )
                        .children(if preset.is_custom {
                            let del_id = preset.id.clone();
                            let del_name = preset.name.clone();
                            Some(
                                // Row 2: Custom badge & Delete button
                                div()
                                    .w(relative(1.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .pt(px(2.0))
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("dark-custom-badge", idx))
                                            .px(px(5.0))
                                            .py(px(1.5))
                                            .rounded(px(4.0))
                                            .bg(theme.soft_bg)
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                                            .cursor_pointer()
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.accent)
                                            .child("Custom")
                                            .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
                                                let dir = crate::theme::custom_themes_dir();
                                                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("dark-custom-del-btn", idx))
                                            .px(px(5.0))
                                            .py(px(1.5))
                                            .rounded(px(4.0))
                                            .bg(if is_dark { rgba(0xFF3B3018) } else { rgba(0xFF3B3012) })
                                            .border_1()
                                            .border_color(if is_dark { rgba(0xFF453A35) } else { rgba(0xFF3B3028) })
                                            .hover(|s| s.bg(rgba(0xFF3B3033)).border_color(rgba(0xFF453A80)))
                                            .cursor_pointer()
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(0xFF453A))
                                            .child("Delete")
                                            .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                                let _ = crate::theme::delete_custom_theme(&del_id);
                                                view.registry.reload();
                                                let def_pos = view.registry.find_by_name_or_id("atlas").unwrap_or(0);
                                                if view.theme_dark_id >= view.registry.len()
                                                    || view.registry.preset(view.theme_dark_id).id == del_id
                                                {
                                                    view.set_dark_theme(def_pos, cx);
                                                }
                                                if view.theme_light_id >= view.registry.len()
                                                    || view.registry.preset(view.theme_light_id).id == del_id
                                                {
                                                    view.set_light_theme(def_pos, cx);
                                                }
                                                view.show_update_toast(
                                                    format!("Deleted custom theme '{}'", del_name),
                                                    true,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            )
                        } else {
                            None
                        }),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    if idx < view.registry.len() {
                        view.set_dark_theme(idx, cx);
                    }
                }));

            dark_theme_grid = dark_theme_grid.child(theme_card);
        }

        // Visual Light Themes Grid
        let mut light_theme_grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0))
            .p(px(14.0));

        for idx in 0..self.registry.len() {
            let preset = self.registry.preset(idx);
            if !preset.supports_light() {
                continue;
            }
            let is_sel = self.theme_light_id == idx;
            let preview_theme = &preset.light;

            let card_border = if is_sel {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_sel {
                Rgba { a: 0.12, ..theme.accent }
            } else {
                rgba(0x00000000)
            };

            let theme_card = div()
                .id(ElementId::named_usize("light-theme-card", idx))
                .w(px(195.0))
                .flex()
                .flex_col()
                .rounded(px(10.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .p(px(8.0))
                .overflow_hidden()
                .cursor_pointer()
                .hover(move |s| s.bg(theme.card_hover))
                .child(
                    // Mini Window Mockup
                    div()
                        .h(px(72.0))
                        .w(relative(1.0))
                        .rounded(px(6.0))
                        .bg(preview_theme.bg)
                        .border_1()
                        .border_color(preview_theme.elevated_border)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            // Window Header
                            div()
                                .h(px(14.0))
                                .px(px(6.0))
                                .bg(preview_theme.panel_bg)
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0xFF5F56)))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0xFFBD2E)))
                                .child(div().w(px(4.5)).h(px(4.5)).rounded_full().bg(rgb(0x27C93F))),
                        )
                        .child(
                            // Terminal Body Preview
                            div()
                                .flex_1()
                                .p(px(6.0))
                                .flex()
                                .flex_col()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(3.0))
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(preview_theme.accent)
                                                .child("~ $"),
                                        )
                                        .child(
                                            div()
                                                .w(px(3.5))
                                                .h(px(9.0))
                                                .bg(preview_theme.cursor),
                                        ),
                                )
                                .child(
                                    // 3 Swatch Dots
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(4.0))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.accent))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.fg))
                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(preview_theme.chrome_fg)),
                                ),
                        ),
                )
                .child(
                    // Label & Indicator
                    div()
                        .pt(px(8.0))
                        .px(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            // Row 1: Title and checkmark
                            div()
                                .w(relative(1.0))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .text_size(px(13.0))
                                        .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                        .text_color(if is_sel { sc_title } else { sc_label })
                                        .child(preset.light_name().to_string()),
                                )
                                .children(if is_sel {
                                    Some(icons::icon(icons::CHECK).size(px(14.0)).text_color(theme.accent))
                                } else {
                                    None
                                }),
                        )
                        .children(if preset.is_custom {
                            let del_id = preset.id.clone();
                            let del_name = preset.name.clone();
                            Some(
                                // Row 2: Custom badge & Delete button
                                div()
                                    .w(relative(1.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .pt(px(2.0))
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("light-custom-badge", idx))
                                            .px(px(5.0))
                                            .py(px(1.5))
                                            .rounded(px(4.0))
                                            .bg(theme.soft_bg)
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                                            .cursor_pointer()
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.accent)
                                            .child("Custom")
                                            .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
                                                let dir = crate::theme::custom_themes_dir();
                                                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("light-custom-del-btn", idx))
                                            .px(px(5.0))
                                            .py(px(1.5))
                                            .rounded(px(4.0))
                                            .bg(if is_dark { rgba(0xFF3B3018) } else { rgba(0xFF3B3012) })
                                            .border_1()
                                            .border_color(if is_dark { rgba(0xFF453A35) } else { rgba(0xFF3B3028) })
                                            .hover(|s| s.bg(rgba(0xFF3B3033)).border_color(rgba(0xFF453A80)))
                                            .cursor_pointer()
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(0xFF453A))
                                            .child("Delete")
                                            .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                                let _ = crate::theme::delete_custom_theme(&del_id);
                                                view.registry.reload();
                                                let def_pos = view.registry.find_by_name_or_id("atlas").unwrap_or(0);
                                                if view.theme_light_id >= view.registry.len()
                                                    || view.registry.preset(view.theme_light_id).id == del_id
                                                {
                                                    view.set_light_theme(def_pos, cx);
                                                }
                                                if view.theme_dark_id >= view.registry.len()
                                                    || view.registry.preset(view.theme_dark_id).id == del_id
                                                {
                                                    view.set_dark_theme(def_pos, cx);
                                                }
                                                view.show_update_toast(
                                                    format!("Deleted custom theme '{}'", del_name),
                                                    true,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            )
                        } else {
                            None
                        }),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    if idx < view.registry.len() {
                        view.set_light_theme(idx, cx);
                    }
                }));

            light_theme_grid = light_theme_grid.child(theme_card);
        }

        // Active font display badge (user customizes font in settings.json)
        let font_badge = div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .bg(if is_dark { rgba(0xFFFFFF12) } else { rgba(0x0000000A) })
            .border_1()
            .border_color(if is_dark { rgba(0xFFFFFF20) } else { rgba(0x00000015) })
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(sc_title)
            .child(self.font_family.clone())
            .into_any_element();

        let font_size_ctrl = stepper(
            110,
            format!("{} pt", self.font_size as i32),
            theme,
            |view, cx| {
                view.adjust_font_size(view.font_size - 1.0, cx);
            },
            |view, cx| {
                view.adjust_font_size(view.font_size + 1.0, cx);
            },
            cx,
        );

        let line_height_ctrl = stepper(
            111,
            format!("{:.2}", self.line_height),
            theme,
            |view, cx| {
                view.line_height = ((view.line_height - 0.05) * 100.0).round() / 100.0;
                view.line_height = view.line_height.max(1.0);
                view.sync_and_save_settings();
                cx.notify();
            },
            |view, cx| {
                view.line_height = ((view.line_height + 0.05) * 100.0).round() / 100.0;
                view.line_height = view.line_height.min(1.8);
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

                // Opacity Stepper: 0% to 100% (Preference memory)
        let effective_pct = (if self.app_settings.blur_enabled {
            self.opacity
        } else {
            self.app_settings.opacity
        } * 100.0).round() as i32;

        let opacity_ctrl = stepper(
            112,
            format!("{}%", effective_pct),
            theme,
            |view, cx| {
                let current_pct = (if view.app_settings.blur_enabled {
                    view.opacity
                } else {
                    view.app_settings.opacity
                } * 100.0).round() as i32;
                let next_pct = if current_pct <= 5 {
                    (current_pct - 1).max(1)
                } else {
                    current_pct - 5
                };
                let new_op = next_pct as f32 / 100.0;
                view.app_settings.opacity = new_op.clamp(0.01, 0.99);
                view.app_settings.blur_enabled = true;
                view.opacity = new_op;
                view.sync_and_save_settings();
                cx.notify();
            },
            |view, cx| {
                let current_pct = (if view.app_settings.blur_enabled {
                    view.opacity
                } else {
                    view.app_settings.opacity
                } * 100.0).round() as i32;
                let next_pct = if current_pct < 5 {
                    current_pct + 1
                } else {
                    (current_pct + 5).min(99)
                };
                let new_op = next_pct as f32 / 100.0;
                view.app_settings.opacity = new_op.clamp(0.01, 0.99);
                view.app_settings.blur_enabled = true;
                view.opacity = new_op;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        // Blur Background Switch (Fix for Bug 1)
        let is_blur = self.app_settings.blur_enabled;
        let blur_switch = toggle_pill(
            113,
            is_blur,
            theme,
            |view, cx| {
                if view.app_settings.blur_enabled {
                    view.opacity = 1.0;
                    view.app_settings.blur_enabled = false;
                } else {
                    let target = if view.app_settings.opacity < 0.999 && view.app_settings.opacity >= 0.01 {
                        view.app_settings.opacity
                    } else {
                        0.67
                    };
                    view.opacity = target;
                    view.app_settings.opacity = target;
                    view.app_settings.blur_enabled = true;
                }
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let tab_layout_options = vec![
            ("top".to_string(), "Top Bar".to_string()),
            ("sidebar".to_string(), "Sidebar".to_string()),
            ("borderless".to_string(), "Borderless".to_string()),
        ];
        let tab_layout_ctrl = segmented_control(
            "tab-layout-ctrl",
            tab_layout_options,
            &self.app_settings.tab_bar_position,
            theme,
            |pos, view, cx| {
                view.app_settings.tab_bar_position = pos.clone();
                if pos == "sidebar" {
                    view.sidebar_mode = crate::view::sidebar::SidebarMode::Expanded;
                    view.sidebar_open = true;
                } else if pos == "borderless" {
                    view.sidebar_mode = crate::view::sidebar::SidebarMode::Hidden;
                    view.sidebar_open = false;
                }
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let cursor_options = vec![
            (None, "Block".to_string()),
            (Some(CursorShape::Beam), "Bar".to_string()),
            (Some(CursorShape::Underline), "Underline".to_string()),
            (Some(CursorShape::HollowBlock), "Hollow".to_string()),
        ];
        let cursor_shape_ctrl = segmented_control(
            "cursor-shape-ctrl",
            cursor_options,
            &self.cursor_style,
            theme,
            |shape, view, cx| {
                view.cursor_style = shape;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let cursor_blink_ctrl = toggle_pill(
            134,
            self.cursor_blink,
            theme,
            |view, cx| {
                view.cursor_blink = !view.cursor_blink;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        // Tiling Gap Stepper: 0px to 16px in 2px increments
        let gap_val = self.app_settings.tiling_gap.round() as i32;
        let tiling_gap_ctrl = stepper(
            115,
            format!("{}px", gap_val),
            theme,
            |view, cx| {
                let cur = view.app_settings.tiling_gap.round() as i32;
                let next = (cur - 2).clamp(0, 16);
                view.app_settings.tiling_gap = next as f32;
                view.sync_and_save_settings();
                cx.notify();
            },
            |view, cx| {
                let cur = view.app_settings.tiling_gap.round() as i32;
                let next = (cur + 2).clamp(0, 16);
                view.app_settings.tiling_gap = next as f32;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        // Scroll Speed Multiplier Stepper / Segmented Control
        let speed_str = if (self.app_settings.scroll_speed - 0.6).abs() < 0.15 {
            "0.6".to_string()
        } else if (self.app_settings.scroll_speed - 1.5).abs() < 0.15 {
            "1.5".to_string()
        } else {
            "1.0".to_string()
        };
        let speed_options = vec![
            ("0.6".to_string(), "Precision (0.6x)".to_string()),
            ("1.0".to_string(), "Balanced (1.0x)".to_string()),
            ("1.5".to_string(), "Fast (1.5x)".to_string()),
        ];
        let scroll_speed_ctrl = segmented_control(
            "scroll-speed-ctrl",
            speed_options,
            &speed_str,
            theme,
            |speed_val, view, cx| {
                view.app_settings.scroll_speed = speed_val.parse::<f32>().unwrap_or(1.0);
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let scroll_to_bottom_ctrl = toggle_pill(
            161,
            self.app_settings.scroll_to_bottom_on_input,
            theme,
            |view, cx| {
                view.app_settings.scroll_to_bottom_on_input = !view.app_settings.scroll_to_bottom_on_input;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let startup_banner_ctrl = toggle_pill(
            159,
            self.app_settings.show_startup_banner,
            theme,
            |view, cx| {
                view.app_settings.show_startup_banner = !view.app_settings.show_startup_banner;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );


        let appearance_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("COLOR MODE"))
                    .child(setting_row("Theme Mode", Some("System automatic sync or manual light/dark override"), color_mode_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("DARK THEMES"))
                    .child(dark_theme_grid),
            )
            .child(
                card()
                    .child(section_header("LIGHT THEMES"))
                    .child(light_theme_grid),
            )
            .child(
                card()
                    .child(section_header("CUSTOM THEMES"))
                    .child(setting_row(
                        "Themes Directory",
                        Some("Drop any .json theme file into ~/.config/atlas/themes or import via 'atlas theme install'"),
                        div()
                            .id(ElementId::named_usize("open-themes-folder-btn", 0))
                            .h(px(28.0))
                            .px(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .rounded(px(6.0))
                            .bg(theme.soft_bg)
                            .border_1()
                            .border_color(theme.elevated_border)
                            .cursor_pointer()
                            .hover(move |s| s.bg(theme.card_hover))
                            .child(icons::icon(icons::FOLDER).size(px(14.0)).text_color(theme.chrome_fg_active))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.chrome_fg_active)
                                    .child("Open Themes Folder"),
                            )
                            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                let path = crate::user_data_dir().join("themes");
                                open_folder(&path);
                                view.registry = crate::theme::ThemeRegistry::standard();
                                cx.notify();
                            }))
                            .into_any_element(),
                        true,
                    )),
            )
            .child(
                card()
                    .child(section_header("BACKGROUND & BLUR"))
                    .child(setting_row("Blur Background", Some("Fluent acrylic translucency (toggles desktop blur)"), blur_switch, false))
                    .child(setting_row(
                        "Window Opacity",
                        Some(if self.app_settings.blur_enabled {
                            "Sheet opacity (0% = fully translucent glass, 100% = solid)"
                        } else {
                            "Sheet opacity (Preference preserved — Blur currently off)"
                        }),
                        opacity_ctrl,
                        true,
                    )),
            );

        let typography_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("TYPOGRAPHY"))
                    .child(setting_row("Active Typeface", Some("Typeface loaded from settings.json"), font_badge, false))
                    .child(setting_row("Font Size", Some("Glyph rendering dimension"), font_size_ctrl, false))
                    .child(setting_row("Line Height", Some("Vertical cell proportion multiplier"), line_height_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("TERMINAL CARET"))
                    .child(setting_row("Caret Shape", Some("Visual style of input focus cursor"), cursor_shape_ctrl, false))
                    .child(setting_row("Breathing Animation", Some("Smooth cursor pulse when idle"), cursor_blink_ctrl, true)),
            );





        let editor_options = vec![
            ("auto".to_string(), "Auto".to_string()),
            ("code".to_string(), "VS Code".to_string()),
            ("cursor".to_string(), "Cursor".to_string()),
            ("zed".to_string(), "Zed".to_string()),
            ("system".to_string(), "Default".to_string()),
        ];
        let editor_ctrl = segmented_control(
            "default-editor-ctrl",
            editor_options,
            &self.app_settings.default_editor,
            theme,
            |editor, view, cx| {
                view.app_settings.default_editor = editor;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );


        let scroll_options = vec![
            (5000, "5,000".to_string()),
            (10000, "10,000".to_string()),
            (20000, "20,000".to_string()),
            (50000, "50,000".to_string()),
        ];
        let scroll_ctrl = segmented_control(
            "scrollback-ctrl",
            scroll_options,
            &self.app_settings.scrollback_lines,
            theme,
            |lines, view, cx| {
                view.app_settings.scrollback_lines = lines;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let copy_on_select_ctrl = toggle_pill(
            341,
            self.app_settings.copy_on_select,
            theme,
            |view, cx| {
                view.app_settings.copy_on_select = !view.app_settings.copy_on_select;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );


        let window_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("NAVIGATION & TABS"))
                    .child(setting_row(
                        "Tab Bar Layout",
                        Some("Choose between top horizontal bar or left vertical sidebar"),
                        tab_layout_ctrl,
                        true,
                    )),
            )
            .child(
                card()
                    .child(section_header("TILING & SPLITS"))
                    .child(setting_row(
                        "Split Window Gaps",
                        Some("Hyprland-style pane gaps (0px = seamless full-bleed, 6px = default)"),
                        tiling_gap_ctrl,
                        true,
                    )),
            )
            .child(
                card()
                    .child(section_header("DEVELOPER & EDITOR"))
                    .child(setting_row("Default Code Editor", Some("Application launched on Ctrl+Click file paths with line & col coordinates"), editor_ctrl, true)),
            );

        let scrolling_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("SCROLL SENSITIVITY & PHYSICS"))
                    .child(setting_row("Scroll Speed Multiplier", Some("Controls lines scrolled per mouse wheel notch. Balanced (1.0x) eliminates runaway speed"), scroll_speed_ctrl, false))
                    .child(setting_row("Scrollback History", Some("Maximum stored buffer lines per pane"), scroll_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("SELECTION & BUFFER"))
                    .child(setting_row("Copy on Select", Some("Automatically copy selected terminal text to clipboard"), copy_on_select_ctrl, false))
                    .child(setting_row("Scroll to Bottom on Key Input", Some("Immediately restore prompt view when typing while scrolled up in buffer"), scroll_to_bottom_ctrl, true)),
            );

        // ==========================================
        // --- Tab 2: General Tab ---
        // ==========================================

        // Visual Shell Selector Cards
        let mut shell_list: Vec<(String, String, String, &'static str, bool)> = Vec::new();

        for s in &self.user_profile.shells {
            let (desc, icon) = match s.id.as_str() {
                "pwsh" => ("PowerShell Core 7+", icons::POWERSHELL),
                "powershell" => ("Windows PowerShell 5.1", icons::POWERSHELL),
                "cmd" => ("Command Prompt", icons::CMD),
                "gitbash" => ("MinGW Git for Windows", icons::GIT),
                _ => ("System Shell", icons::TERMINAL),
            };
            shell_list.push((s.id.clone(), s.title.clone(), desc.to_string(), icon, true));
        }
        if shell_list.is_empty() {
            shell_list.push(("powershell".to_string(), "PowerShell".to_string(), "Windows PowerShell".to_string(), icons::POWERSHELL, true));
            shell_list.push(("cmd".to_string(), "CMD".to_string(), "Classic Command Prompt".to_string(), icons::CMD, true));
        }

        for d in &self.user_profile.wsl_distros {
            let s_id = format!("wsl:{}", d.name);
            let s_title = format!("{} (WSL)", d.title);
            let s_desc = format!("WSL {} • {}", d.version, d.name);
            let s_icon = crate::wsl::icon_for_name(&d.name);
            shell_list.push((s_id, s_title, s_desc, s_icon, true));
        }
        if self.user_profile.wsl_distros.is_empty() {
            shell_list.push(("wsl".to_string(), "WSL (Linux)".to_string(), "No WSL distributions detected".to_string(), icons::LINUX, false));
        }

        for t in &self.user_profile.tools {
            if t.name == "opencode" {
                shell_list.push(("opencode".to_string(), "OpenCode AI".to_string(), "Autonomous Agent CLI".to_string(), icons::OPENCODE_MARK, true));
            } else if t.name == "claude" {
                shell_list.push(("claude".to_string(), "Claude Code".to_string(), "Anthropic Claude Code CLI".to_string(), icons::CLAUDE_MARK, true));
            }
        }

        let mut shell_cards = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.0))
            .p(px(14.0));

        for (idx, (s_id, s_title, s_desc, s_icon, is_available)) in shell_list.into_iter().enumerate() {
            let is_sel = (self.app_settings.default_shell.eq_ignore_ascii_case(&s_id)
                || self.user_profile.default_shell.eq_ignore_ascii_case(&s_id)
                || (s_id == "powershell" && self.user_profile.default_shell.eq_ignore_ascii_case("PowerShell"))
                || (s_id == "cmd" && self.user_profile.default_shell.eq_ignore_ascii_case("Command Prompt")))
                && is_available;
            let s_str = s_id.to_string();
            let card_border = if is_sel {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_sel {
                Rgba { a: 0.12, ..theme.accent }
            } else {
                rgba(0x00000000)
            };

            let card_el = div()
                .id(ElementId::named_usize("shell-card", idx))
                .w(px(165.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .p(px(12.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .cursor_pointer()
                .hover(move |s| if is_available { s.bg(theme.card_hover) } else { s })
                .opacity(if is_available { 1.0 } else { 0.60 })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(icons::icon(s_icon).size(px(19.0)).text_color(if is_sel { theme.accent } else { sc_label }))
                        .children(if is_sel {
                            Some(icons::icon(icons::CHECK).size(px(14.0)).text_color(theme.accent))
                        } else {
                            None
                        }),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                        .text_color(if is_sel { sc_title } else { sc_label })
                        .child(s_title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(sc_desc)
                        .child(s_desc),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    if is_available {
                        view.user_profile.default_shell = s_str.clone();
                        view.user_profile.save();
                        view.app_settings.default_shell = s_str.clone();
                        view.sync_and_save_settings();
                        cx.notify();
                    }
                }));

            shell_cards = shell_cards.child(card_el);
        }

        let startup_options = vec![
            ("restore_session".to_string(), "Restore Session".to_string()),
            ("new_tab".to_string(), "New Tab".to_string()),
        ];
        let startup_ctrl = segmented_control(
            "startup-ctrl",
            startup_options,
            &self.app_settings.startup_mode,
            theme,
            |mode, view, cx| {
                view.app_settings.startup_mode = mode;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let start_dir_options = vec![
            ("home".to_string(), "Home (~)".to_string()),
            ("current".to_string(), "Current Working Dir".to_string()),
        ];
        let start_dir_ctrl = segmented_control(
            "start-dir-ctrl",
            start_dir_options,
            &self.app_settings.start_directory,
            theme,
            |dir, view, cx| {
                view.app_settings.start_directory = dir;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let confirm_close_ctrl = toggle_pill(
            340,
            self.app_settings.confirm_close_process,
            theme,
            |view, cx| {
                view.app_settings.confirm_close_process = !view.app_settings.confirm_close_process;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let notify_ctrl = toggle_pill(
            345,
            self.app_settings.notify_on_complete,
            theme,
            |view, cx| {
                view.app_settings.notify_on_complete = !view.app_settings.notify_on_complete;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let threshold_options = vec![
            (5, "5s".to_string()),
            (10, "10s".to_string()),
            (30, "30s".to_string()),
            (60, "60s".to_string()),
        ];
        let threshold_ctrl = segmented_control(
            "alert-threshold-ctrl",
            threshold_options,
            &self.app_settings.notify_threshold_secs,
            theme,
            |secs, view, cx| {
                view.app_settings.notify_threshold_secs = secs;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let channel_options = vec![
            (crate::updater::UpdateChannel::Stable, "Stable (Official)".to_string()),
            (crate::updater::UpdateChannel::Preview, "Preview (Rolling Cloud)".to_string()),
        ];
        let channel_ctrl = segmented_control(
            "update-channel-ctrl",
            channel_options,
            &self.updater_settings.channel,
            theme,
            |channel, view, cx| {
                if view.updater_settings.channel != channel {
                    view.updater_settings.channel = channel;
                    view.updater_settings.etag = None;
                    view.available_update = None;
                    view.update_panel_open = false;
                    view.manual_status_message = None;
                    view.sync_and_save_settings();
                    view.trigger_update_check(true, cx);
                    cx.notify();
                }
            },
            cx,
        );

        let auto_check_ctrl = toggle_pill(
            350,
            self.updater_settings.auto_check_updates,
            theme,
            |view, cx| {
                view.updater_settings.auto_check_updates = !view.updater_settings.auto_check_updates;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let interval_options = vec![
            (15, "15m".to_string()),
            (60, "1h".to_string()),
            (240, "4h".to_string()),
            (720, "12h".to_string()),
            (1440, "24h".to_string()),
        ];
        let interval_ctrl = segmented_control(
            "check-interval-ctrl",
            interval_options,
            &self.updater_settings.update_check_interval_mins,
            theme,
            |mins, view, cx| {
                view.updater_settings.update_check_interval_mins = mins;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let check_updates_btn = pill_button(
            if self.is_checking_updates { "Checking…".to_string() } else { "Check for Updates".to_string() },
            370,
            false,
            theme,
            |view, cx| {
                view.check_updates_manual(cx);
            },
            cx,
        );

        let reset_defaults_btn = pill_button(
            "Reset All to Defaults".to_string(),
            380,
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
                view.cursor_style = None;
                view.cursor_blink = view.app_settings.cursor_blink;
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let edit_settings_btn = pill_button(
            "Edit settings.json".to_string(),
            381,
            false,
            theme,
            |view, _cx| {
                open_file(&crate::user_data_dir().join("settings.json"), &view.app_settings.default_editor);
            },
            cx,
        );

        let edit_profile_btn = pill_button(
            "Edit $PROFILE".to_string(),
            382,
            false,
            theme,
            |view, _cx| {
                open_file(&powershell_profile_path(), &view.app_settings.default_editor);
            },
            cx,
        );

        let reveal_dir_btn = pill_button(
            "Reveal in Explorer".to_string(),
            383,
            false,
            theme,
            |_view, _cx| {
                open_folder(&crate::user_data_dir());
            },
            cx,
        );

        let reload_settings_btn = pill_button(
            "Reload from Disk".to_string(),
            384,
            false,
            theme,
            |view, cx| {
                view.reload_settings_from_disk(cx);
            },
            cx,
        );

        let edit_fastfetch_btn = fastfetch_config_path().map(|ff_path| {
            pill_button(
                "Edit config.jsonc".to_string(),
                385,
                false,
                theme,
                move |view, _cx| {
                    open_fastfetch_config(&ff_path, &view.app_settings.default_editor);
                },
                cx,
            )
        });

        let edit_profile_json_btn = pill_button(
            "Edit profile.json".to_string(),
            386,
            false,
            theme,
            |view, _cx| {
                open_file(&crate::discovery::profile_path(), &view.app_settings.default_editor);
            },
            cx,
        );

        let rescan_shells_btn = pill_button(
            "Rescan Environment".to_string(),
            387,
            false,
            theme,
            |view, cx| {
                view.rescan_environment(cx);
            },
            cx,
        );

        let general_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .pr(px(14.0))
                            .child(section_header("DEFAULT SHELL & ENVIRONMENT"))
                            .child(rescan_shells_btn),
                    )
                    .child(shell_cards),
            )
            .child(
                card()
                    .child(section_header("STARTUP & WORKSPACE"))
                    .child(setting_row("Startup Mode", Some("Restore previous workspace state or open clean tab"), startup_ctrl, false))
                    .child(setting_row("Initial Directory", Some("Starting working location for new shells"), start_dir_ctrl, false))
                    .child(setting_row("Startup Fastfetch Banner", Some("Display zero-latency hardware specs card automatically when opening the terminal"), startup_banner_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("SAFETY & NOTIFICATIONS"))
                    .child(setting_row("Confirm Process Exit", Some("Warn before closing tabs running foreground processes"), confirm_close_ctrl, false))
                    .child(setting_row("Notify on Long Commands", Some("Send native Windows notification when a task finishes while Atlas is unfocused"), notify_ctrl, false))
                    .child(setting_row("Alert Threshold", Some("Minimum running duration before triggering an alert"), threshold_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("RESET CONFIGURATION"))
                    .child(setting_row("Factory Settings", Some("Revert all appearance, shortcuts, and behavior options"), reset_defaults_btn, true)),
            );

        let updates_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("SOFTWARE UPDATES"))
                    .child(setting_row(
                        "Installed Version",
                        Some("Currently running Atlas Terminal build"),
                        div()
                            .h(px(28.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.0))
                            .bg(Rgba { a: 0.12, ..theme.accent })
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.accent)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .into_any_element(),
                        false,
                    ))
                    .child(setting_row("Update Channel", Some("Official milestone releases or early rolling cloud builds"), channel_ctrl, false))
                    .child(setting_row("Automatic Checks", Some("Background release inspection on focus"), auto_check_ctrl, false))
                    .child(setting_row("Check Frequency", Some("Interval between update scans"), interval_ctrl, false))
                    .child(setting_row("Manual Check", Some("Poll GitHub release channels immediately"), check_updates_btn, true)),
            )
            .child({
                let mut config_card = card()
                    .child(section_header("CONFIGURATION & PROMPT FILES"))
                    .child(setting_row("Atlas Config (settings.json)", Some("Font family, opacity %, custom themes, and keybindings"), edit_settings_btn, false))
                    .child(setting_row("Atlas Environment Profile (profile.json)", Some("Detected shells, WSL distros, and dev/AI CLI tool paths (%APPDATA%\\AtlasTerminal)"), edit_profile_json_btn, false))
                    .child(setting_row("PowerShell Profile ($PROFILE)", Some("Custom prompt name, Oh-My-Posh / Starship prompt, startup script"), edit_profile_btn, false));
                if let Some(btn) = edit_fastfetch_btn {
                    config_card = config_card.child(setting_row("Fastfetch Config (config.jsonc)", Some("Logo image, system specs, modules, and inline layout"), btn, false));
                }
                config_card
                    .child(setting_row("Atlas Data Folder", Some("Session histories, logs, and portable overrides (~/.config/atlas)"), reveal_dir_btn, false))
                    .child(setting_row("Reload Settings", Some("Apply manual edits from settings.json immediately"), reload_settings_btn, true))
            });

        // ==========================================
        // --- Tab 3: Shortcuts Tab ---
        // ==========================================

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
                .px(px(18.0))
                .py(px(14.0))
                .border_b_1()
                .border_color(theme.hairline);

            let mut left_col = div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(14.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(sc_label)
                        .child(action_display_name(action)),
                );

            if let Some(other) = conflict {
                left_col = left_col.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgb(0xE6A700))
                        .child(format!("Conflict with {other}")),
                );
            }
            row = row.child(left_col);

            let keycap_text = if is_recording {
                "Press keys… (Esc to cancel)".to_string()
            } else {
                pretty_shortcut(&current_chord)
            };

            let keycap_bg = if is_recording {
                Rgba { a: 0.22, ..theme.accent }
            } else if is_custom {
                Rgba { a: 0.12, ..theme.accent }
            } else if is_dark {
                rgba(0xFFFFFF12)
            } else {
                rgba(0x0000000C)
            };

            let keycap_border = if is_recording {
                theme.accent
            } else if is_custom {
                Rgba { a: 0.35, ..theme.accent }
            } else if is_dark {
                rgba(0xFFFFFF1E)
            } else {
                rgba(0x00000014)
            };

            let keycap_fg = if is_recording {
                theme.accent
            } else if is_custom {
                theme.accent
            } else {
                sc_label
            };

            let act_clone = action_str.clone();

            let mut right_col = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0));

            if is_custom && !is_recording {
                right_col = right_col.child(
                    div()
                        .id(ElementId::named_usize("reset-shortcut", idx))
                        .text_size(px(12.5))
                        .text_color(sc_desc)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(sc_label))
                        .child("Reset")
                        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            view.app_settings.shortcuts.insert(action_str.clone(), def_str.clone());
                            view.sync_and_save_settings();
                            cx.notify();
                        })),
                );
            }

            right_col = right_col.child(
                div()
                    .id(ElementId::named_usize("keycap-btn", idx))
                    .h(px(30.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .bg(keycap_bg)
                    .border_1()
                    .border_color(keycap_border)
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(keycap_fg)
                    .cursor_pointer()
                    .hover(move |s| s.opacity(0.85))
                    .child(keycap_text)
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.recording_shortcut = Some(act_clone.clone());
                        cx.notify();
                    })),
            );

            row.child(right_col).into_any_element()
        };

        let tab_window_actions = [
            ACTION_NEW_WINDOW,
            ACTION_NEW_TAB,
            ACTION_CLOSE_TAB,
            ACTION_NEXT_TAB,
            ACTION_PREV_TAB,
            ACTION_TOGGLE_SIDEBAR,
            ACTION_OPEN_SETTINGS,
            ACTION_PALETTE,
            ACTION_FULLSCREEN,
            ACTION_NEXT_THEME,
            ACTION_PREV_THEME,
            ACTION_TOGGLE_DARK_LIGHT,
        ];
        let mut tab_window_card = card().child(section_header("TABS & WINDOW"));
        for (i, &act) in tab_window_actions.iter().enumerate() {
            tab_window_card = tab_window_card.child(render_shortcut_row(100 + i, act));
        }

        let shell_actions = [
            ACTION_PROFILE_MENU,
            ACTION_NEW_TAB_WSL,
            ACTION_NEW_TAB_POWERSHELL,
            ACTION_NEW_TAB_CMD,
            ACTION_NEW_TAB_GITBASH,
            ACTION_NEW_TAB_OPENCODE,
        ];
        let mut shell_card = card().child(section_header("SHELLS & PROFILES (QUICK LAUNCH)"));
        for (i, &act) in shell_actions.iter().enumerate() {
            shell_card = shell_card.child(render_shortcut_row(150 + i, act));
        }

        let ref_keycap_bg = if is_dark {
            rgba(0xFFFFFF12)
        } else {
            rgba(0x0000000C)
        };
        let ref_keycap_border = if is_dark {
            rgba(0xFFFFFF1E)
        } else {
            rgba(0x00000014)
        };

        let render_reference_row = |desc: &'static str, keycaps: &'static str| {
            div()
                .h(px(40.0))
                .px(px(16.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(theme.hairline)
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(sc_label)
                        .child(desc),
                )
                .child(
                    div()
                        .h(px(26.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .bg(ref_keycap_bg)
                        .border_1()
                        .border_color(ref_keycap_border)
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(sc_label)
                        .child(keycaps),
                )
        };

        let mut built_in_card = card().child(section_header("BUILT-IN SHORTCUTS & LINUX TILING"));
        let built_in_list: [(&'static str, &'static str); 17] = [
            ("New Window", "Ctrl+Shift+N"),
            ("New Tab (Default Shell)", "Ctrl+T"),
            ("Close Tab / Pane", "Alt+W / Ctrl+W"),
            ("New WSL (Linux) Tab", "Ctrl+Shift+2"),
            ("New PowerShell Tab", "Ctrl+Shift+1"),
            ("Command Palette / Shell Switcher", "Ctrl+K / Ctrl+Shift+P"),
            ("Cycle Theme Preset (Forward)", "Alt+T / Ctrl+Shift+T"),
            ("Cycle Theme Preset (Backward)", "Alt+Shift+T"),
            ("Toggle Dark / Light Mode", "Ctrl+Shift+L"),
            ("Tiling: Split Vertical (Columns)", "Alt+Enter"),
            ("Tiling: Split Horizontal (Rows)", "Alt+D"),
            ("Tiling: Toggle Layout (Cols ↔ Rows)", "Alt+Space"),
            ("Tiling: Spatial Navigate Panes", "Alt+Left / Right / Up / Down (or H,J,K,L)"),
            ("Tiling: Resize Active Pane (5%)", "Alt+Shift+Arrows (or drag divider)"),
            ("Tiling: Zoom / Maximize Pane", "Alt+Z"),
            ("Next / Previous Tab", "Ctrl+Tab / Ctrl+Shift+Tab"),
            ("Find / Search in Buffer", "Ctrl+F"),
        ];
        for (label, keycap) in built_in_list {
            built_in_card = built_in_card.child(render_reference_row(label, keycap));
        }

        let split_actions = [
            ACTION_SPLIT_HORIZONTAL,
            ACTION_SPLIT_VERTICAL,
            ACTION_FOCUS_NEXT_PANE,
            ACTION_FOCUS_PREV_PANE,
            ACTION_TOGGLE_MAXIMIZE,
        ];
        let mut split_pane_card = card().child(section_header("SPLIT PANES"));
        for (i, &act) in split_actions.iter().enumerate() {
            split_pane_card = split_pane_card.child(render_shortcut_row(200 + i, act));
        }

        let scroll_actions = [
            ACTION_SCROLL_PAGE_UP,
            ACTION_SCROLL_PAGE_DOWN,
            ACTION_SCROLL_TO_TOP,
            ACTION_SCROLL_TO_BOTTOM,
            ACTION_CLEAR_SCROLLBACK,
        ];
        let mut scroll_card = card().child(section_header("SCROLLING & BUFFER"));
        for (i, &act) in scroll_actions.iter().enumerate() {
            scroll_card = scroll_card.child(render_shortcut_row(300 + i, act));
        }

        let text_actions = [
            ACTION_FIND,
            ACTION_COPY,
            ACTION_PASTE,
            ACTION_SELECT_ALL,
        ];
        let mut text_card = card().child(section_header("TEXT & CLIPBOARD"));
        for (i, &act) in text_actions.iter().enumerate() {
            text_card = text_card.child(render_shortcut_row(400 + i, act));
        }

        let zoom_actions = [
            ACTION_ZOOM_IN,
            ACTION_ZOOM_OUT,
            ACTION_ZOOM_RESET,
            ACTION_OPEN_URL_PICKER,
        ];
        let mut zoom_card = card().child(section_header("VIEWPORT & ZOOM"));
        for (i, &act) in zoom_actions.iter().enumerate() {
            zoom_card = zoom_card.child(render_shortcut_row(500 + i, act));
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
                            .text_size(px(11.5))
                            .text_color(sc_desc)
                            .child("Click any key binding to record a new key combination. Press Esc to cancel."),
                    )
                    .child(pill_button(
                        "Reset All Shortcuts".to_string(),
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
            .child(shell_card)
            .child(built_in_card)
            .child(split_pane_card)
            .child(scroll_card)
            .child(text_card)
            .child(zoom_card);

        // ==========================================
        // --- Tab 4: Tools & Satellites Tab ---
        // ==========================================

        let mut tool_action_row = |cmd: &'static str, id: usize| -> AnyElement {
            let cmd_text = cmd.to_string();
            let cmd_copy = cmd.to_string();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .id(ElementId::named_usize("tool-badge-btn", id))
                        .h(px(26.0))
                        .px(px(9.0))
                        .max_w(px(260.0))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .bg(if is_dark {
                            Rgba { r: 0.16, g: 0.18, b: 0.22, a: 0.9 }
                        } else {
                            Rgba { r: 0.88, g: 0.90, b: 0.94, a: 0.9 }
                        })
                        .border_1()
                        .border_color(theme.hairline)
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.accent)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .child(
                            div()
                                .truncate()
                                .child(cmd)
                        )
                        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd_copy.clone()));
                            view.show_update_toast(format!("Copied '{cmd_copy}' to clipboard"), true, cx);
                        }))
                )
                .child(
                    div()
                        .flex_none()
                        .child(pill_button(
                            "Run in Tab".to_string(),
                            id,
                            false,
                            theme,
                            move |view, cx| {
                                view.settings_open = false;
                                view.write_to_shell(format!("{cmd_text}\r").as_bytes());
                                cx.notify();
                            },
                            cx,
                        ))
                )
                .into_any_element()
        };

        let status_badge = div()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.0))
            .bg(Rgba { a: 0.12, ..theme.accent })
            .text_size(px(12.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.accent)
            .child(format!("Embedded (v{})", env!("CARGO_PKG_VERSION")))
            .into_any_element();

        let terminal_integ_badge = div()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.0))
            .bg(if is_dark {
                Rgba { r: 0.12, g: 0.28, b: 0.16, a: 0.3 }
            } else {
                Rgba { r: 0.82, g: 0.94, b: 0.85, a: 0.8 }
            })
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if is_dark {
                Rgba { r: 0.45, g: 0.85, b: 0.55, a: 1.0 }
            } else {
                Rgba { r: 0.15, g: 0.60, b: 0.25, a: 1.0 }
            })
            .child("ConPTY Attached")
            .into_any_element();

        let tools_overview_card = card()
            .child(section_header("CLI ARCHITECTURE & RUNTIME"))
            .child(setting_row(
                "Satellite Status",
                Some("Zero-overhead companion tools compiled directly into the core Atlas binary"),
                status_badge,
                false,
            ))
            .child(setting_row(
                "Console Attachment",
                Some("Auto-attaches to parent console (PowerShell, CMD, Git Bash, WSL)"),
                terminal_integ_badge,
                false,
            ))
            .child(setting_row(
                "Global Invocation Syntax",
                Some("Execute subterminal tools with 'atlas <command>' or 'atlas-terminal <command>'"),
                tool_action_row("atlas tools", 700),
                true,
            ));

        let system_tools_card = card()
            .child(section_header("SYSTEM TOOLS"))
            .child(setting_row(
                "System & Architecture Specs",
                Some("Detailed system, GPU backend, memory & build specifications"),
                tool_action_row("atlas -v", 701),
                false,
            ))
            .child(setting_row(
                "Environment & Toolchains",
                Some("Audit discovered shells, WSL distros, and dev CLI toolchains"),
                tool_action_row("atlas info", 702),
                false,
            ))
            .child(setting_row(
                "Theme Swatch Manager",
                Some("Inspect theme palettes, switch active themes, and toggle dark/light"),
                tool_action_row("atlas theme list", 703),
                false,
            ))
            .child(setting_row(
                "Direct Settings Access",
                Some("Inspect, query, modify, or edit settings.json directly from terminal"),
                tool_action_row("atlas config path", 704),
                false,
            ))
            .child(setting_row(
                "Smart Directory Memory (jump, z)",
                Some("Frecency path tracker & ghost-text autosuggestion engine for instant folder jumps"),
                tool_action_row("atlas jump --list", 715),
                false,
            ))
            .child(setting_row(
                "System & Process Top Snapshot",
                Some("Ultra-lightweight inline system RAM gauge and top process memory card"),
                tool_action_row("atlas top", 716),
                false,
            ))
            .child(setting_row(
                "Dynamic Island HUD Bridge",
                Some("Trigger macOS-style notifications, progress bars, and status pulses via OSC 1338"),
                tool_action_row("atlas hud notify \"Atlas\" \"Task completed!\"", 705),
                false,
            ))
            .child(setting_row(
                "Throughput & Color Benchmark",
                Some("Measure PTY write throughput and verify 24-bit truecolor rendering"),
                tool_action_row("atlas bench", 706),
                true,
            ));

        let dev_tools_card = card()
            .child(section_header("DEVELOPER PRODUCTIVITY TOOLS"))
            .child(setting_row(
                "Port Inspector & Killer",
                Some("Inspect listening TCP ports or kill processes instantly (atlas port kill <port>)"),
                tool_action_row("atlas port", 707),
                false,
            ))
            .child(setting_row(
                "Offline JWT Debugger",
                Some("Inspect JSON Web Tokens with offline safety and expiration countdown"),
                tool_action_row("atlas jwt --help", 708),
                false,
            ))
            .child(setting_row(
                "Terminal QR Generator",
                Some("Generate terminal QR codes for Wi-Fi, auth tokens, or mobile handoff"),
                tool_action_row("atlas qr \"https://github.com/akrram0/atlas-terminal\"", 709),
                false,
            ))
            .child(setting_row(
                "JSON Colorizer & Query",
                Some("Syntax-highlighted pretty printer with dot-notation field queries"),
                tool_action_row("atlas json --help", 710),
                false,
            ))
            .child(setting_row(
                "Instant HTTP Static Server",
                Some("Zero-dependency static file server with auto-MIME and dark Apple directory UI"),
                tool_action_row("atlas serve 8000", 717),
                false,
            ))
            .child(setting_row(
                "Crypto & Checksum Hasher",
                Some("Fast SHA-256/MD5 for Winget manifests, plus Base64 & URL encoder/decoder"),
                tool_action_row("atlas hash --help", 718),
                false,
            ))
            .child(setting_row(
                "Terminal Clipboard Sink/Source",
                Some("Pipe command output to clipboard (cmd | atlas clip) or paste directly to stdout"),
                tool_action_row("atlas clip paste", 719),
                false,
            ))
            .child(setting_row(
                "Terminal Side-by-Side Diff",
                Some("Fast line-by-line diff viewer with line numbers and syntax highlighting"),
                tool_action_row("atlas diff --help", 711),
                true,
            ));

        let visual_tools_card = card()
            .child(section_header("VISUAL SATELLITES"))
            .child(setting_row(
                "Inline Image Satellite",
                Some("GPU-accelerated OSC 1337 image rendering with 24-bit half-block fallback"),
                tool_action_row("atlas img --help", 712),
                false,
            ))
            .child(setting_row(
                "Screen Token Picker",
                Some("Extract URLs, Git commit hashes, paths, and IPs with 1-shot key hints"),
                tool_action_row("atlas pick --help", 713),
                false,
            ))
            .child(setting_row(
                "Terminal AI Companion",
                Some("Command explanations, error diagnostic advisor, and CLI assistance"),
                tool_action_row("atlas ai ask \"how to list git branches\"", 714),
                true,
            ));

        let net_tools_card = card()
            .child(section_header("NETWORKING & CONNECTIVITY TOOLS"))
            .child(setting_row(
                "Network Connectivity Hub",
                Some("Overview of network status, cloud connectivity health & quick utilities"),
                tool_action_row("atlas net", 720),
                false,
            ))
            .child(setting_row(
                "Fast Latency Ping",
                Some("TCP connection handshake & ping latency with jitter statistics and sparkline"),
                tool_action_row("atlas ping 1.1.1.1", 721),
                false,
            ))
            .child(setting_row(
                "IPv4 & IPv6 DNS Resolver",
                Some("High-performance DNS record resolution with latency benchmarks"),
                tool_action_row("atlas dns github.com", 722),
                false,
            ))
            .child(setting_row(
                "Local & Public IP Inspector",
                Some("Inspect local network adapter IPs and query public IP, ISP & geolocation"),
                tool_action_row("atlas ip", 723),
                false,
            ))
            .child(setting_row(
                "HTTP Request Inspector",
                Some("Inspect HTTP response status codes, timing metrics, and response bodies"),
                tool_action_row("atlas http https://api.github.com", 724),
                true,
            ));

        let security_privacy_card = card()
            .child(section_header("SECURITY & DATA PRIVACY PROTOCOLS"))
            .child(setting_row(
                "Strict Localhost-First Sandboxing",
                Some("atlas serve binds to 127.0.0.1 by default. Local network exposure (0.0.0.0) requires explicit '--lan' opt-in."),
                div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(4.0))
                    .bg(rgba(0x10B98124))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x10B981))
                    .child("STRICT LOCALHOST")
                    .into_any_element(),
                false,
            ))
            .child(setting_row(
                "Directory Traversal & Secrets Shield",
                Some("Automatic blocking of path traversal, .env, .git, SSH keys, certificates, and hidden credentials."),
                div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(4.0))
                    .bg(rgba(0x10B98124))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x10B981))
                    .child("CANONICAL SHIELD")
                    .into_any_element(),
                false,
            ))
            .child(setting_row(
                "Privacy-Safe IP Redaction",
                Some("Redact public IP octets and physical city/region for screen sharing or streaming with 'atlas ip --mask'."),
                tool_action_row("atlas ip -m", 725),
                false,
            ))
            .child(setting_row(
                "Zero Cloud Telemetry Guarantee",
                Some("Atlas Terminal and satellites transmit zero keystrokes, commands, or telemetry. 100% offline-first."),
                div()
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(4.0))
                    .bg(rgba(0x38BDF824))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x38BDF8))
                    .child("ZERO TELEMETRY")
                    .into_any_element(),
                true,
            ));

        let tools_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(tools_overview_card)
            .child(security_privacy_card)
            .child(system_tools_card)
            .child(dev_tools_card)
            .child(net_tools_card)
            .child(visual_tools_card);

        // ==========================================
        // --- Tab 7: Pixel Art & Companion ---
        // ==========================================
        let is_pixel_enabled = self.pixel_art.enabled;
        let pixel_master_ctrl = toggle_pill(
            180,
            is_pixel_enabled,
            theme,
            |view, cx| {
                view.pixel_art.enabled = !view.pixel_art.enabled;
                view.pixel_art.save();
                cx.notify();
            },
            cx,
        );

        let pixel_master_card = card()
            .child(section_header("TOP BAR COMPANION"))
            .child(setting_row(
                "Enable Pixel Art Companion",
                Some("Renders a live animated pixel companion widget in the top header bar beside Search"),
                pixel_master_ctrl.into_any_element(),
                false,
            ))
            .child(setting_row(
                "Standalone Configuration File",
                Some("Decoupled persistent JSON configuration for pixel art settings"),
                div()
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(5.0))
                    .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x00000008) })
                    .border_1()
                    .border_color(if is_dark { rgba(0xFFFFFF18) } else { rgba(0x00000012) })
                    .text_size(px(11.5))
                    .text_color(sc_desc)
                    .child(crate::ui::pixel_art::pixel_art_config_path().display().to_string())
                    .into_any_element(),
                false,
            ))
            .child(setting_row(
                "Custom Sprites Directory",
                Some("Handcraft your own 24x24 companions in ~/.config/atlas/pixel_art/"),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id(ElementId::named_usize("pixel-import-btn", 0))
                            .px(px(11.0))
                            .py(px(4.0))
                            .rounded(px(5.0))
                            .bg(theme.accent)
                            .hover(move |s| s.opacity(0.85))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xFFFFFF))
                            .child("Import GIF / Sprite")
                            .on_click(cx.listener(|_view, _: &ClickEvent, _window, cx| {
                                cx.spawn(async move |this, cx| {
                                    let picked = smol::unblock(crate::ui::pixel_art::pick_sprite_file).await;
                                    if let Some(path) = picked {
                                        if let Some(preset_id) = crate::ui::pixel_art::import_sprite_from_path(&path) {
                                            let _ = this.update(cx, |view, cx| {
                                                view.pixel_art.active_preset = preset_id;
                                                view.pixel_art.save();
                                                cx.notify();
                                            });
                                        }
                                    }
                                }).detach();
                            })),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("pixel-open-folder-btn", 0))
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(5.0))
                            .bg(if is_dark { rgba(0xFFFFFF12) } else { rgba(0x0000000C) })
                            .border_1()
                            .border_color(theme.hairline)
                            .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(sc_title)
                            .child("Open Folder")
                            .on_click(cx.listener(|_, _: &ClickEvent, _window, _| {
                                open_folder(&crate::ui::pixel_art::custom_sprites_dir());
                            })),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("pixel-new-template-btn", 0))
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(5.0))
                            .bg(if is_dark { rgba(0xFFFFFF12) } else { rgba(0x0000000C) })
                            .border_1()
                            .border_color(theme.hairline)
                            .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.accent)
                            .child("+ New Template")
                            .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
                                let _ = crate::ui::pixel_art::create_custom_template("my_companion");
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
                true,
            ));

        let cat_filters: &[(&'static str, &'static str)] = &[
            ("all", "All"),
            ("cozy", "Cozy & Chill"),
            ("system", "System & Retro"),
            ("custom", "Custom & GIFs"),
        ];

        let mut cat_pills_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(20.0))
            .pt(px(12.0))
            .pb(px(8.0));

        for (cat_idx, &(cat_id, cat_label)) in cat_filters.iter().enumerate() {
            let is_sel = self.pixel_art_category_filter == cat_id;
            let pill_bg = if is_sel {
                if is_dark { rgba(0xFFFFFF1E) } else { rgba(0x00000018) }
            } else {
                rgba(0x00000000)
            };
            let pill_fg = if is_sel { sc_title } else { sc_desc };
            let pill_border = if is_sel { theme.accent } else { theme.hairline };

            cat_pills_row = cat_pills_row.child(
                div()
                    .id(ElementId::named_usize("pixel-cat-filter-pill", cat_idx))
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(6.0))
                    .bg(pill_bg)
                    .border_1()
                    .border_color(pill_border)
                    .text_size(px(12.0))
                    .font_weight(if is_sel { FontWeight::BOLD } else { FontWeight::MEDIUM })
                    .text_color(pill_fg)
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme.card_hover))
                    .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                        view.pixel_art_category_filter = cat_id;
                        cx.notify();
                    }))
                    .child(cat_label),
            );
        }

        let mut preset_grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(12.0))
            .p(px(20.0));

        for (p_idx, preset) in crate::ui::pixel_art::PRESETS.iter().enumerate() {
            if self.pixel_art_category_filter != "all"
                && preset.category.as_str() != self.pixel_art_category_filter
            {
                continue;
            }

            let is_active = self.pixel_art.active_preset == preset.id;
            let card_border = if is_active {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_active {
                Rgba { a: 0.12, ..theme.accent }
            } else if is_dark {
                rgba(0x00000040)
            } else {
                rgba(0xFFFFFF60)
            };

            let preview_canvas = crate::ui::pixel_art::render_pixel_canvas(
                preset.id.to_string(),
                self.pixel_art_frame,
                self.pixel_art.color_mode.clone(),
                theme.clone(),
                is_dark,
                64.0,
                64.0,
            );

            let pid = preset.id;
            let card_el = div()
                .id(ElementId::named_usize("pixel-preset-card", p_idx))
                .w(px(220.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(card_bg)
                .border_1()
                .border_color(card_border)
                .cursor_pointer()
                .hover(move |s| s.border_color(theme.accent))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.pixel_art.active_preset = pid.to_string();
                    view.pixel_art.save();
                    cx.notify();
                }))
                .child(
                    div()
                        .w(relative(1.0))
                        .h(px(76.0))
                        .rounded(px(5.0))
                        .bg(if is_dark { rgba(0x00000080) } else { rgba(0x00000012) })
                        .border_1()
                        .border_color(theme.hairline)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(preview_canvas),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(sc_title)
                                .child(preset.name),
                        )
                        .children(if is_active {
                            Some(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(theme.accent)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xFFFFFF))
                                    .child("ACTIVE"),
                            )
                        } else {
                            Some(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x0000000A) })
                                    .text_size(px(10.0))
                                    .text_color(sc_desc)
                                    .child(preset.category.label()),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(sc_desc)
                        .line_height(px(15.0))
                        .child(preset.desc),
                );

            preset_grid = preset_grid.child(card_el);
        }

        let sprite_entries: Vec<(String, std::sync::Arc<crate::ui::pixel_art::DecodedSpriteAnimation>)> =
            if self.pixel_art_category_filter == "all" || self.pixel_art_category_filter == "custom" {
                let all_sprites = crate::ui::pixel_art::load_all_sprites();
                let mut entries: Vec<_> = all_sprites.into_iter().collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
            } else {
                Vec::new()
            };
        for (s_idx, (sprite_id, sprite)) in sprite_entries.iter().enumerate() {

            let is_active = self.pixel_art.active_preset == *sprite_id;
            let card_border = if is_active {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_active {
                Rgba { a: 0.12, ..theme.accent }
            } else if is_dark {
                rgba(0x00000040)
            } else {
                rgba(0xFFFFFF60)
            };

            let preview_canvas = crate::ui::pixel_art::render_pixel_canvas(
                sprite_id.clone(),
                self.pixel_art_frame,
                self.pixel_art.color_mode.clone(),
                theme.clone(),
                is_dark,
                64.0,
                64.0,
            );

            let sid = sprite_id.clone();
            let card_el = div()
                .id(ElementId::named_usize("pixel-sprite-card", s_idx))
                .w(px(220.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(card_bg)
                .border_1()
                .border_color(card_border)
                .cursor_pointer()
                .hover(move |s| s.border_color(theme.accent))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.pixel_art.active_preset = sid.clone();
                    view.pixel_art.save();
                    cx.notify();
                }))
                .child(
                    div()
                        .w(relative(1.0))
                        .h(px(76.0))
                        .rounded(px(5.0))
                        .bg(if is_dark { rgba(0x00000080) } else { rgba(0x00000012) })
                        .border_1()
                        .border_color(theme.hairline)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(preview_canvas),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(sc_title)
                                .child(sprite.name.clone()),
                        )
                        .children(if is_active {
                            Some(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(theme.accent)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xFFFFFF))
                                    .child("ACTIVE"),
                            )
                        } else {
                            Some(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x0000000A) })
                                    .text_size(px(10.0))
                                    .text_color(theme.accent)
                                    .child(format!("GIF ({}f, {}x{})", sprite.num_frames, sprite.width, sprite.height)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.5))
                                .text_color(sc_desc)
                                .line_height(px(15.0))
                                .overflow_hidden()
                                .child(
                                    sprite
                                        .path
                                        .file_name()
                                        .and_then(|f| f.to_str())
                                        .unwrap_or("Custom sprite")
                                        .to_string(),
                                ),
                        )
                        .child({
                            let del_id = sprite_id.clone();
                            div()
                                .id(ElementId::named_usize("pixel-del-sprite-btn", s_idx))
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x0000000A) })
                                .border_1()
                                .border_color(theme.hairline)
                                .hover(move |s| s.bg(rgba(0xEF444428)).border_color(rgba(0xEF444480)).text_color(rgb(0xEF4444)))
                                .cursor_pointer()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(sc_desc)
                                .child("Delete")
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    let _ = crate::ui::pixel_art::delete_sprite(&del_id);
                                    if view.pixel_art.active_preset == del_id {
                                        view.pixel_art.active_preset = "hot_coffee".to_string();
                                        view.pixel_art.save();
                                    }
                                    cx.notify();
                                }))
                        }),
                );

            preset_grid = preset_grid.child(card_el);
        }

        let custom_sprites = if self.pixel_art_category_filter == "all" || self.pixel_art_category_filter == "custom" {
            crate::ui::pixel_art::load_custom_sprites()
        } else {
            Vec::new()
        };
        for (c_idx, custom) in custom_sprites.iter().enumerate() {
            if self.pixel_art_category_filter != "all"
                && self.pixel_art_category_filter != "custom"
                && custom.category != self.pixel_art_category_filter
            {
                continue;
            }

            let is_active = self.pixel_art.active_preset == custom.id;
            let card_border = if is_active {
                theme.accent
            } else if is_dark {
                rgba(0xFFFFFF14)
            } else {
                rgba(0x00000010)
            };
            let card_bg = if is_active {
                Rgba { a: 0.12, ..theme.accent }
            } else if is_dark {
                rgba(0x00000040)
            } else {
                rgba(0xFFFFFF60)
            };

            let preview_canvas = crate::ui::pixel_art::render_pixel_canvas(
                custom.id.clone(),
                self.pixel_art_frame,
                self.pixel_art.color_mode.clone(),
                theme.clone(),
                is_dark,
                64.0,
                64.0,
            );

            let cid = custom.id.clone();
            let card_el = div()
                .id(ElementId::named_usize("pixel-custom-card", c_idx))
                .w(px(220.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(card_bg)
                .border_1()
                .border_color(card_border)
                .cursor_pointer()
                .hover(move |s| s.border_color(theme.accent))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.pixel_art.active_preset = cid.clone();
                    view.pixel_art.save();
                    cx.notify();
                }))
                .child(
                    div()
                        .w(relative(1.0))
                        .h(px(76.0))
                        .rounded(px(5.0))
                        .bg(if is_dark { rgba(0x00000080) } else { rgba(0x00000012) })
                        .border_1()
                        .border_color(theme.hairline)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(preview_canvas),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(sc_title)
                                .child(custom.name.clone()),
                        )
                        .child(if is_active {
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(theme.accent)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xFFFFFF))
                                .child("ACTIVE")
                                .into_any_element()
                        } else {
                            div()
                                .id(ElementId::named_usize("pixel-custom-badge", c_idx))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x0000000A) })
                                .border_1()
                                .border_color(theme.hairline)
                                .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                                .cursor_pointer()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child("CUSTOM")
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    let dir = crate::ui::pixel_art::custom_sprites_dir();
                                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                    view.pixel_art_category_filter = "custom";
                                    cx.notify();
                                }))
                                .into_any_element()
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.5))
                                .text_color(sc_desc)
                                .line_height(px(15.0))
                                .overflow_hidden()
                                .child(custom.desc.clone()),
                        )
                        .child({
                            let del_id = custom.id.clone();
                            div()
                                .id(ElementId::named_usize("pixel-del-custom-btn", c_idx))
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(if is_dark { rgba(0xFFFFFF10) } else { rgba(0x0000000A) })
                                .border_1()
                                .border_color(theme.hairline)
                                .hover(move |s| s.bg(rgba(0xEF444428)).border_color(rgba(0xEF444480)).text_color(rgb(0xEF4444)))
                                .cursor_pointer()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(sc_desc)
                                .child("Delete")
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    let _ = crate::ui::pixel_art::delete_sprite(&del_id);
                                    if view.pixel_art.active_preset == del_id {
                                        view.pixel_art.active_preset = "hot_coffee".to_string();
                                        view.pixel_art.save();
                                    }
                                    cx.notify();
                                }))
                        }),
                );

            preset_grid = preset_grid.child(card_el);
        }

        let fps_options = vec![
            (0u32, "Static (Still)".to_string()),
            (2u32, "Calm (2 FPS)".to_string()),
            (4u32, "Smooth (4 FPS)".to_string()),
            (8u32, "Dynamic (8 FPS)".to_string()),
        ];
        let fps_ctrl = segmented_control(
            "pixel-fps-ctrl",
            fps_options,
            &self.pixel_art.fps,
            theme,
            |fps, view, cx| {
                view.pixel_art.fps = fps;
                view.pixel_art.save();
                cx.notify();
            },
            cx,
        );

        let color_mode_options = vec![
            ("original".to_string(), "Original".to_string()),
            ("theme_accent".to_string(), "Theme Accent".to_string()),
            ("monochrome".to_string(), "Monochrome".to_string()),
        ];
        let color_mode_ctrl = segmented_control(
            "pixel-color-mode-ctrl",
            color_mode_options,
            &self.pixel_art.color_mode,
            theme,
            |mode, view, cx| {
                view.pixel_art.color_mode = mode;
                view.pixel_art.save();
                cx.notify();
            },
            cx,
        );

        let is_boost = self.pixel_art.speed_boost_on_activity;
        let boost_ctrl = toggle_pill(
            181,
            is_boost,
            theme,
            |view, cx| {
                view.pixel_art.speed_boost_on_activity = !view.pixel_art.speed_boost_on_activity;
                view.pixel_art.save();
                cx.notify();
            },
            cx,
        );

        let pixel_controls_card = card()
            .child(section_header("KINEMATICS & COLOR TUNING"))
            .child(setting_row(
                "Animation Frame Rate",
                Some("Static (still icon), Calm (2 FPS), Smooth (4 FPS), or Dynamic (8 FPS)"),
                fps_ctrl.into_any_element(),
                false,
            ))
            .child(setting_row(
                "Color Mode",
                Some("Original truecolor, dynamic Theme Accent matching, or Nothing-OS Monochrome"),
                color_mode_ctrl.into_any_element(),
                false,
            ))
            .child(setting_row(
                "Speed Boost on Activity",
                Some("Temporarily double frame rate (2x) when typing or running commands"),
                boost_ctrl.into_any_element(),
                true,
            ));

        let pixel_art_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(pixel_master_card)
            .child(
                card()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(section_header("ANIMATED PRESET LIBRARY"))
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(if is_dark { rgba(0xFFFFFF12) } else { rgba(0x0000000C) })
                                    .border_1()
                                    .border_color(theme.hairline)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.accent)
                                    .child("BETA"),
                            ),
                    )
                    .child(cat_pills_row)
                    .child(preset_grid),
            )
            .child(pixel_controls_card);

        // ==========================================
        // --- Body / Content Framing ---
        // ==========================================

                let is_searching = !self.settings_search_query.trim().is_empty();
        let search_query_lower = self.settings_search_query.trim().to_lowercase();

        let content_body: AnyElement = if is_searching {
            let searchable_items: &[(&'static str, &'static str, SettingsTab, &'static str)] = &[
                ("Theme Mode", "Appearance & Blur", SettingsTab::Appearance, "Color Mode: Auto sync, Dark, or Light override"),
                ("Dark Themes & Presets", "Appearance & Blur", SettingsTab::Appearance, "Tokyo Night, Gruvbox, Catppuccin, Nord, Classic"),
                ("Light Themes & Presets", "Appearance & Blur", SettingsTab::Appearance, "Solarized Light, Warm Paper, Classic Light"),
                ("Blur Background", "Appearance & Blur", SettingsTab::Appearance, "Fluent Windows 11 DWM acrylic glassmorphism backdrop"),
                ("Window Opacity", "Appearance & Blur", SettingsTab::Appearance, "Sheet opacity percentage with saved preference memory"),
                ("Active Typeface", "Font & Caret", SettingsTab::Typography, "Hardware-accelerated monospace font"),
                ("Font Size & Line Height", "Font & Caret", SettingsTab::Typography, "Glyph dimension and vertical spacing proportions"),
                ("Caret Shape & Cursor Breathing", "Font & Caret", SettingsTab::Typography, "Beam, block, underline cursor styles and sine pulse"),
                ("Scroll Speed Multiplier", "Scrolling & Mouse", SettingsTab::Scrolling, "Tame scroll velocity: Precision 0.6x, Balanced 1.0x, Fast 1.5x"),
                ("Scrollback History", "Scrolling & Mouse", SettingsTab::Scrolling, "Max lines preserved in RAM per tab"),
                ("Copy on Select", "Scrolling & Mouse", SettingsTab::Scrolling, "Automatically copy highlighted terminal text to clipboard"),
                ("Scroll to Bottom on Key Input", "Scrolling & Mouse", SettingsTab::Scrolling, "Immediately restore prompt view when typing while scrolled up"),
                ("Tab Bar Layout", "Window & Splits", SettingsTab::Window, "Top horizontal bar, vertical sidebar, or borderless"),
                ("Split Window Gaps", "Window & Splits", SettingsTab::Window, "Spacing between split terminal panes"),
                ("Default Code Editor", "Window & Splits", SettingsTab::Window, "Application launched on Ctrl+Click file paths"),
                ("Default Shell", "General", SettingsTab::General, "PowerShell, WSL, or CMD startup profile"),
                ("Initial Directory", "General", SettingsTab::General, "Starting directory for freshly opened shells"),
                ("Startup Fastfetch Banner", "General", SettingsTab::General, "Hardware specs card automatically displayed on startup"),
                ("Confirm Process Exit", "General", SettingsTab::General, "Warn before closing tabs running foreground processes"),
                ("Notify on Long Commands", "General", SettingsTab::General, "Trigger notification island when a background task finishes"),
                ("Atlas Jump (z)", "Tools & Satellites", SettingsTab::Tools, "Frecency directory memory & ghost-text inline path suggestions"),
                ("Atlas Serve (http)", "Tools & Satellites", SettingsTab::Tools, "Zero-dependency static HTTP file server with QR handoff"),
                ("Atlas Hash (crypto)", "Tools & Satellites", SettingsTab::Tools, "Fast SHA-256 and MD5 checksums, Base64 & URL encoding"),
                ("Atlas Clip (copy)", "Tools & Satellites", SettingsTab::Tools, "Terminal clipboard manager & pipe sink/source (cmd | atlas clip)"),
                ("Atlas Top (proc)", "Tools & Satellites", SettingsTab::Tools, "Inline CPU, RAM, and process memory gauge card"),
                ("Atlas Port Inspector", "Tools & Satellites", SettingsTab::Tools, "Inspect listening ports and terminate blocking dev servers"),
                ("Atlas QR Code Generator", "Tools & Satellites", SettingsTab::Tools, "Generate ISO-compliant terminal QR codes for mobile handoff"),
                ("Atlas JWT Debugger", "Tools & Satellites", SettingsTab::Tools, "Offline, privacy-safe JSON Web Token claims and expiration inspector"),
                ("Atlas JSON Colorizer (jq)", "Tools & Satellites", SettingsTab::Tools, "Syntax-highlighted pretty printer with dot-notation field queries"),
                ("Atlas Diff Viewer", "Tools & Satellites", SettingsTab::Tools, "Terminal side-by-side diff viewer with line numbers and syntax"),
                ("Atlas Image Viewer (img)", "Tools & Satellites", SettingsTab::Tools, "GPU-accelerated OSC 1337 image rendering with half-block fallback"),
                ("Atlas Token Picker (hints)", "Tools & Satellites", SettingsTab::Tools, "Extract URLs, Git commit hashes, paths, and IPs with key hints"),
                ("Atlas AI Companion", "Tools & Satellites", SettingsTab::Tools, "Command explanations, error diagnostic advisor, and CLI assistance"),
                ("Atlas Dynamic Island (hud)", "Tools & Satellites", SettingsTab::Tools, "Trigger macOS-style notifications and status pulses via OSC 1338"),
                ("Pixel Art Companion", "Pixel Art & Companion", SettingsTab::PixelArt, "Top bar animated companion widget"),
                ("Pixel Art Presets", "Pixel Art & Companion", SettingsTab::PixelArt, "Hot Coffee, Campfire, Tux, Cassette, Cat, Prompt, Imported GIFs"),
                ("Animation Frame Rate (FPS)", "Pixel Art & Companion", SettingsTab::PixelArt, "Continuous loop speed (Static, 2, 4, 8 FPS)"),
                ("Pixel Art Color Mode", "Pixel Art & Companion", SettingsTab::PixelArt, "Original truecolor, Theme Accent, or Monochrome"),
                ("Pixel Art Activity Boost", "Pixel Art & Companion", SettingsTab::PixelArt, "Double animation cadence during active terminal typing"),
                ("Atlas Network Hub", "Networking & Satellites", SettingsTab::Tools, "Global connectivity health, public IP, and quick network tests"),
                ("Atlas Ping (net)", "Networking & Satellites", SettingsTab::Tools, "Fast TCP latency ping with jitter statistics and visual sparkline"),
                ("Atlas DNS (net)", "Networking & Satellites", SettingsTab::Tools, "IPv4 & IPv6 DNS record resolution with timing benchmarks"),
                ("Atlas IP (net)", "Networking & Satellites", SettingsTab::Tools, "Local network adapters and public IP/location inspector"),
                ("Atlas HTTP (net)", "Networking & Satellites", SettingsTab::Tools, "Inspect HTTP status codes, timing metrics, and payload"),
                ("Atlas Benchmark", "Tools & Satellites", SettingsTab::Tools, "Measure PTY throughput and 24-bit truecolor rendering"),
                ("Atlas Theme Manager", "Tools & Satellites", SettingsTab::Tools, "Inspect palettes, switch themes, or export custom themes"),
                ("Atlas Environment Info", "Tools & Satellites", SettingsTab::Tools, "Audit shells, WSL distros, and detected developer toolchains"),
                ("Atlas System Specs (-v)", "Tools & Satellites", SettingsTab::Tools, "Rich system, GPU hardware backend, and runtime specifications card"),
                ("Atlas Config Editor", "Tools & Satellites", SettingsTab::Tools, "Inspect, query, modify, or edit settings.json from terminal"),
                ("Keyboard Shortcuts Map", "Shortcuts", SettingsTab::Shortcuts, "Customize keystroke chords or review navigation bindings"),
                ("Software Updates", "Updates & Config", SettingsTab::Updates, "Release channels, automated checks, and manual update poll"),
                ("Configuration Files", "Updates & Config", SettingsTab::Updates, "settings.json, profile.json, PowerShell $PROFILE"),
            ];

            let matches: Vec<_> = searchable_items.iter().filter(|(name, cat, _, desc)| {
                name.to_lowercase().contains(&search_query_lower)
                    || cat.to_lowercase().contains(&search_query_lower)
                    || desc.to_lowercase().contains(&search_query_lower)
            }).collect();

            if matches.is_empty() {
                div()
                    .p(px(32.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(sc_label)
                            .child(format!("No settings matching \"{}\"", self.settings_search_query))
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(sc_desc)
                            .child("Try searching for 'blur', 'scroll', 'font', 'theme', or 'shell'.")
                    )
                    .into_any_element()
            } else {
                let mut results_list = div().flex().flex_col().gap(px(8.0));
                for (idx, (name, cat, tab, desc)) in matches.iter().enumerate() {
                    let target_tab = *tab;
                    let result_card = div()
                        .id(ElementId::named_usize("settings-search-result", idx))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .p(px(16.0))
                        .rounded(px(8.0))
                        .bg(theme.elevated_bg)
                        .border_1()
                        .border_color(theme.elevated_border)
                        .cursor_pointer()
                        .hover(move |s| s.bg(theme.card_hover).border_color(theme.accent))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(sc_label)
                                                .child(*name)
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(4.0))
                                                .bg(theme.soft_bg)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.accent)
                                                .child(*cat)
                                        )
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(sc_desc)
                                        .child(*desc)
                                )
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .text_size(px(12.0))
                                .text_color(theme.accent)
                                .child("Go to setting →")
                        )
                        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            view.settings_tab = target_tab;
                            view.settings_search_query.clear();
                            view.settings_search_active = false;
                            view.settings_scroll_pos = 0.0;
                            view.settings_scroll_target = 0.0;
                            view.settings_scroll_handle.set_offset(gpui::Point::default());
                            view.sync_and_save_settings();
                            cx.notify();
                        }));
                    results_list = results_list.child(result_card);
                }
                results_list.into_any_element()
            }
        } else {
            div()
                .flex()
                .flex_col()
                .child(match self.settings_tab {
                    SettingsTab::General => general_view.into_any_element(),
                    SettingsTab::Appearance => appearance_view.into_any_element(),
                    SettingsTab::Typography => typography_view.into_any_element(),
                    SettingsTab::Scrolling => scrolling_view.into_any_element(),
                    SettingsTab::Window => window_view.into_any_element(),
                    SettingsTab::Tools => tools_view.into_any_element(),
                    SettingsTab::PixelArt => pixel_art_view.into_any_element(),
                    SettingsTab::Shortcuts => shortcuts_view.into_any_element(),
                    SettingsTab::Updates => updates_view.into_any_element(),
                })
                .slide_up(ElementId::named_usize("settings-tab-pane", self.settings_tab as usize))
                .into_any_element()
        };

        let content_header = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .mb(px(22.0))
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(sc_title)
                    .child(if is_searching {
                        format!("Search Results for \"{}\"", self.settings_search_query)
                    } else {
                        match self.settings_tab {
                            SettingsTab::General => "General Preferences".to_string(),
                            SettingsTab::Appearance => "Appearance & Blur".to_string(),
                            SettingsTab::Typography => "Font & Caret".to_string(),
                            SettingsTab::Scrolling => "Scrolling & Mouse".to_string(),
                            SettingsTab::Window => "Window & Splits".to_string(),
                            SettingsTab::Tools => "Tools & Satellites".to_string(),
                            SettingsTab::PixelArt => "Pixel Art Companion [BETA]".to_string(),
                            SettingsTab::Shortcuts => "Keyboard Shortcuts".to_string(),
                            SettingsTab::Updates => "Updates & Config".to_string(),
                        }
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(sc_desc)
                    .child(if is_searching {
                        "Click any matching item to jump directly to its category.".to_string()
                    } else {
                        match self.settings_tab {
                            SettingsTab::General => "Configure default shell, startup working directory, and execution safety.".to_string(),
                            SettingsTab::Appearance => "Fluent acrylic glass translucency, color mode, and curated terminal themes.".to_string(),
                            SettingsTab::Typography => "Monospace typeface rendering, vertical cell line height, and cursor dynamics.".to_string(),
                            SettingsTab::Scrolling => "Control scroll velocity, eliminate runaway acceleration, and manage clipboard buffers.".to_string(),
                            SettingsTab::Window => "Tab bar position, tiling split gaps, and window framing.".to_string(),
                            SettingsTab::Tools => "Zero-dependency developer companions and CLI satellites.".to_string(),
                            SettingsTab::PixelArt => "Minimalist pixel companions and artist-made animated GIF / PNG sprites.".to_string(),
                            SettingsTab::Shortcuts => "Review native navigation keystrokes or record custom keybindings.".to_string(),
                            SettingsTab::Updates => "Software release channels, background update checks, and configuration files.".to_string(),
                        }
                    }),
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
            .p(px(28.0))
            .pb(px(88.0))
            .child(
                div()
                    .w(relative(1.0))
                    .max_w(px(880.0))
                    .min_w(px(0.0))
                    .mx_auto()
                    .child(content_header)
                    .child(content_body),
            );

        div()
            .id(ElementId::named_usize("settings-root", 0))
            .flex()
            .flex_col()
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
            .child(top_nav)
            .child(right_pane)
            .fade_in("settings-root-motion")
            .into_any_element()
    }
}

pub fn open_file(path: &std::path::Path, editor_pref: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !path.exists() {
        let _ = std::fs::write(path, "");
    }
    crate::url::open_in_editor(&path.to_string_lossy(), editor_pref, None);
}

pub fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}

pub fn powershell_profile_path() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let p = std::path::PathBuf::from(home);
        let core_ps = p.join("Documents").join("PowerShell").join("Microsoft.PowerShell_profile.ps1");
        if core_ps.exists() {
            return core_ps;
        }
        let win_ps = p.join("Documents").join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1");
        if win_ps.exists() {
            return win_ps;
        }
        // If neither exists yet, default to modern PowerShell (pwsh) directory if it exists, else WindowsPowerShell
        let ps_core_dir = p.join("Documents").join("PowerShell");
        if ps_core_dir.is_dir() {
            core_ps
        } else {
            win_ps
        }
    } else {
        std::path::PathBuf::from("Microsoft.PowerShell_profile.ps1")
    }
}

pub fn open_fastfetch_config(path: &std::path::Path, editor_pref: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !path.exists() {
        let default_config = r#"{
    "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
    "logo": {
        "type": "iterm",
        "width": 18,
        "height": 9,
        "padding": {
            "top": 1,
            "left": 2,
            "right": 3
        }
    },
    "modules": [
        "title",
        "separator",
        "os",
        "host",
        "kernel",
        "uptime",
        "terminal",
        "display",
        "cpu",
        "gpu",
        "memory"
    ]
}
"#;
        let _ = std::fs::write(path, default_config);
    }
    open_file(path, editor_pref);
}

pub fn is_fastfetch_installed() -> bool {
    // 1. Check if config directory/file already exists
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let base = std::path::PathBuf::from(home).join(".config").join("fastfetch");
        if base.join("config.jsonc").exists() || base.join("config.json").exists() {
            return true;
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let base = std::path::PathBuf::from(appdata).join("fastfetch");
        if base.join("config.jsonc").exists() || base.join("config.json").exists() {
            return true;
        }
    }

    // 2. Check PATH environment variable
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join("fastfetch.exe").exists() || dir.join("fastfetch").exists() {
                return true;
            }
        }
    }

    // 3. Check Windows package managers (WinGet, Scoop, Chocolatey, LocalAppData)
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local_p = std::path::PathBuf::from(local);
            if local_p.join("Programs").join("fastfetch").join("fastfetch.exe").exists() {
                return true;
            }
            let winget_dir = local_p.join("Microsoft").join("WinGet").join("Packages");
            if let Ok(entries) = std::fs::read_dir(winget_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("fastfetch") && entry.path().join("fastfetch.exe").exists() {
                        return true;
                    }
                }
            }
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let home_p = std::path::PathBuf::from(home);
            if home_p.join("scoop").join("apps").join("fastfetch").join("current").join("fastfetch.exe").exists() {
                return true;
            }
        }
        if std::path::Path::new("C:\\ProgramData\\chocolatey\\bin\\fastfetch.exe").exists() {
            return true;
        }
    }

    // 4. Check Unix standard binary paths
    #[cfg(not(windows))]
    {
        if std::path::Path::new("/usr/bin/fastfetch").exists()
            || std::path::Path::new("/usr/local/bin/fastfetch").exists()
        {
            return true;
        }
    }

    false
}

pub fn fastfetch_config_path() -> Option<std::path::PathBuf> {
    // 1. Check ~/.config/fastfetch/config.jsonc or config.json
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let base = std::path::PathBuf::from(home).join(".config").join("fastfetch");
        let jsonc = base.join("config.jsonc");
        if jsonc.exists() {
            return Some(jsonc);
        }
        let json = base.join("config.json");
        if json.exists() {
            return Some(json);
        }
    }

    // 2. Check %APPDATA%/fastfetch/config.jsonc or config.json
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let base = std::path::PathBuf::from(appdata).join("fastfetch");
        let jsonc = base.join("config.jsonc");
        if jsonc.exists() {
            return Some(jsonc);
        }
        let json = base.join("config.json");
        if json.exists() {
            return Some(json);
        }
    }

    // 3. If config doesn't exist yet, check if fastfetch binary is installed
    if is_fastfetch_installed() {
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            return Some(std::path::PathBuf::from(home).join(".config").join("fastfetch").join("config.jsonc"));
        }
    }

    None
}
