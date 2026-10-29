//! Application settings UI: appearance, shortcuts, profiles, and general configuration.
//! Designed with macOS / Apple SF design language, pixel-perfect metric geometry,
//! and native vector controls.

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
    ACTION_COPY, ACTION_FIND, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE, ACTION_FULLSCREEN,
    ACTION_NEW_TAB, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_PALETTE, ACTION_PASTE,
    ACTION_PREV_TAB, ACTION_SPLIT_HORIZONTAL, ACTION_SPLIT_VERTICAL, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT, ACTION_ZOOM_RESET, ACTION_SELECT_ALL, ACTION_SCROLL_PAGE_UP,
    ACTION_SCROLL_PAGE_DOWN, ACTION_SCROLL_TO_TOP, ACTION_SCROLL_TO_BOTTOM,
    ACTION_OPEN_URL_PICKER, ACTION_TOGGLE_THEME, ACTION_TOGGLE_MAXIMIZE, ACTION_TOGGLE_SIDEBAR,
};
use crate::icons;
use crate::theme::Theme;
use crate::view::AtlasView;
use super::animation::{ease_apple_snappy, BODY_IN_MS};
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
        self.app_settings.theme = self.registry.preset(self.theme_id).id.to_string();
        self.app_settings.font_family = self.font_family.clone();
        self.app_settings.font_size = self.font_size;
        self.app_settings.line_height = self.line_height;
        self.app_settings.corner_radius = self.corner_radius;
        self.app_settings.opacity = self.opacity;
        self.app_settings.blur_enabled = self.opacity < 0.999;
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
    }

    pub fn reload_settings_from_disk(&mut self, cx: &mut Context<Self>) {
        let loaded = crate::config::load_app_settings();
        self.theme_mode = loaded.theme_mode.clone();
        self.font_family = loaded.font_family.clone();
        self.font_size = loaded.font_size;
        self.line_height = loaded.line_height;
        self.opacity = if loaded.blur_enabled || loaded.opacity < 0.999 {
            crate::view::normalize_opacity(loaded.opacity)
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
        if let Some(idx) = self.registry.find_by_name_or_id(&loaded.theme) {
            self.set_theme(idx, cx);
        }
        for tab in &mut self.tabs {
            for pane in &mut tab.panes {
                pane.cursor_pos = None;
            }
        }
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
            .child(control)
            .into_any_element()
        };

        // --- Left Sidebar (220px) ---
        let nav_items = [
            (SettingsTab::General, "General", icons::TERMINAL),
            (SettingsTab::Appearance, "Appearance", icons::SLIDERS),
            (SettingsTab::Shortcuts, "Shortcuts", icons::KEYBOARD),
        ];

        let mut sidebar = div()
            .id(ElementId::named_usize("settings-sidebar-scroll", 0))
            .w(px(220.0))
            .flex_shrink_0()
            .h(relative(1.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .border_r_1()
            .border_color(theme.hairline)
            .p(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(12.0))
                    .py(px(12.0))
                    .mb(px(12.0))
                    .child(icons::icon(icons::SETTINGS).size(px(19.0)).text_color(theme.accent))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(sc_title)
                            .child("Preferences"),
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
                    .h(px(40.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .rounded(px(8.0))
                    .bg(item_bg)
                    .text_color(item_fg)
                    .hover(move |s| s.bg(hover_bg))
                    .child(icons::icon(icon).size(px(17.5)).text_color(item_fg))
                    .child(
                        div()
                            .text_size(px(14.0))
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

        // Visual Theme Cards Grid
        let mut theme_grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0))
            .p(px(14.0));

        for idx in 0..self.registry.len() {
            let is_sel = self.theme_id == idx;
            let preset = self.registry.preset(idx);
            let p_dark = &preset.dark;
            let p_light = &preset.light;
            let preview_theme = if self.theme_mode == "light" { p_light } else { p_dark };

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
                .id(ElementId::named_usize("theme-card", idx))
                .w(px(180.0))
                .flex()
                .flex_col()
                .rounded(px(10.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .p(px(8.0))
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
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                .text_color(if is_sel { sc_title } else { sc_label })
                                .child(preset.name),
                        )
                        .children(if is_sel {
                            Some(icons::icon(icons::CHECK).size(px(14.0)).text_color(theme.accent))
                        } else {
                            None
                        }),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.set_theme(idx, cx);
                }));

            theme_grid = theme_grid.child(theme_card);
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

        // Blur Background Switch
        let is_blur = self.opacity < 0.999;
        let blur_switch = toggle_pill(
            113,
            is_blur,
            theme,
            |view, cx| {
                if view.opacity < 0.999 {
                    view.opacity = 1.0;
                    view.app_settings.blur_enabled = false;
                } else {
                    let target = if view.app_settings.opacity < 0.999 && view.app_settings.opacity >= 0.15 {
                        view.app_settings.opacity
                    } else {
                        0.85
                    };
                    view.opacity = target;
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
        ];
        let tab_layout_ctrl = segmented_control(
            "tab-layout-ctrl",
            tab_layout_options,
            &self.app_settings.tab_bar_position,
            theme,
            |pos, view, cx| {
                view.app_settings.tab_bar_position = pos;
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

        let appearance_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("COLOR THEME"))
                    .child(setting_row("Appearance Mode", Some("System automatic or forced dark/light"), color_mode_ctrl, false))
                    .child(theme_grid),
            )
            .child(
                card()
                    .child(section_header("TYPOGRAPHY"))
                    .child(setting_row("Active Typeface", Some("Typeface loaded from settings.json"), font_badge, false))
                    .child(setting_row("Font Size", Some("Glyph rendering dimension"), font_size_ctrl, false))
                    .child(setting_row("Line Height", Some("Vertical cell proportion multiplier"), line_height_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("BACKGROUND & BLUR"))
                    .child(setting_row("Blur Background", Some("Fluent acrylic translucency (adjust % in settings.json)"), blur_switch, true)),
            )
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
                    .child(section_header("TERMINAL CARET"))
                    .child(setting_row("Caret Shape", Some("Visual style of input focus cursor"), cursor_shape_ctrl, false))
                    .child(setting_row("Breathing Animation", Some("Smooth cursor pulse when idle"), cursor_blink_ctrl, true)),
            );

        // ==========================================
        // --- Tab 2: General Tab ---
        // ==========================================

        // Visual Shell Selector Cards
        let (wsl_title, wsl_desc, wsl_icon, has_wsl) = match crate::wsl::default_distro() {
            Some(distro) => (
                format!("{} (WSL)", distro.title),
                format!("WSL 2 • {}", distro.name),
                distro.icon,
                true,
            ),
            None => (
                "WSL (Linux)".to_string(),
                "You don't have WSL".to_string(),
                icons::LINUX,
                false,
            ),
        };

        let shell_list = vec![
            ("PowerShell", "PowerShell".to_string(), "Windows PowerShell & Core".to_string(), icons::POWERSHELL, true),
            ("WSL", wsl_title, wsl_desc, wsl_icon, has_wsl),
            ("OpenCode", "OpenCode AI".to_string(), "Autonomous Agent CLI".to_string(), icons::OPENCODE_MARK, true),
            ("Command Prompt", "CMD".to_string(), "Classic Windows Command Prompt".to_string(), icons::CMD, true),
            ("Git Bash", "Git Bash".to_string(), "MinGW Git for Windows".to_string(), icons::GIT, true),
        ];

        let mut shell_cards = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.0))
            .p(px(14.0));

        for (idx, (s_id, s_title, s_desc, s_icon, is_available)) in shell_list.into_iter().enumerate() {
            let is_sel = self.app_settings.default_shell.eq_ignore_ascii_case(s_id) && is_available;
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

        let general_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("DEFAULT SHELL"))
                    .child(shell_cards),
            )
            .child({
                let mut config_card = card()
                    .child(section_header("CONFIGURATION & PROMPT FILES"))
                    .child(setting_row("Atlas Config (settings.json)", Some("Font family, opacity %, custom themes, and keybindings"), edit_settings_btn, false))
                    .child(setting_row("PowerShell Profile ($PROFILE)", Some("Custom prompt name, Oh-My-Posh / Starship prompt, startup script"), edit_profile_btn, false));
                if let Some(btn) = edit_fastfetch_btn {
                    config_card = config_card.child(setting_row("Fastfetch Config (config.jsonc)", Some("Logo image, system specs, modules, and inline layout"), btn, false));
                }
                config_card
                    .child(setting_row("Atlas Data Folder", Some("Session histories, logs, and portable overrides (~/.config/atlas)"), reveal_dir_btn, false))
                    .child(setting_row("Reload Settings", Some("Apply manual edits from settings.json immediately"), reload_settings_btn, true))
            })
            .child(
                card()
                    .child(section_header("STARTUP & WORKSPACE"))
                    .child(setting_row("Startup Mode", Some("Restore previous workspace state or open clean tab"), startup_ctrl, false))
                    .child(setting_row("Initial Directory", Some("Starting working location for new shells"), start_dir_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("TERMINAL BEHAVIOR"))
                    .child(setting_row("Scrollback History", Some("Maximum stored buffer lines per pane"), scroll_ctrl, false))
                    .child(setting_row("Copy on Select", Some("Automatically copy selected terminal text to clipboard"), copy_on_select_ctrl, false))
                    .child(setting_row("Confirm Process Exit", Some("Warn before closing tabs running foreground processes"), confirm_close_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("DEVELOPER & EDITOR"))
                    .child(setting_row("Default Code Editor", Some("Application launched on Ctrl+Click file paths with line & col coordinates"), editor_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("BACKGROUND NOTIFICATIONS"))
                    .child(setting_row("Notify on Long Commands", Some("Send native Windows notification when a task finishes while Atlas is unfocused"), notify_ctrl, false))
                    .child(setting_row("Alert Threshold", Some("Minimum running duration before triggering an alert"), threshold_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("SOFTWARE UPDATES"))
                    .child(setting_row("Update Channel", Some("Official milestone releases or early rolling cloud builds"), channel_ctrl, false))
                    .child(setting_row("Automatic Checks", Some("Background release inspection on focus"), auto_check_ctrl, false))
                    .child(setting_row("Check Frequency", Some("Interval between update scans"), interval_ctrl, false))
                    .child(setting_row("Manual Check", Some("Poll GitHub release channels immediately"), check_updates_btn, true)),
            )
            .child(
                card()
                    .child(section_header("RESET CONFIGURATION"))
                    .child(setting_row("Factory Settings", Some("Revert all appearance, shortcuts, and behavior options"), reset_defaults_btn, true)),
            );

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
            ACTION_NEW_TAB,
            ACTION_CLOSE_TAB,
            ACTION_NEXT_TAB,
            ACTION_PREV_TAB,
            ACTION_TOGGLE_SIDEBAR,
            ACTION_OPEN_SETTINGS,
            ACTION_PALETTE,
            ACTION_FULLSCREEN,
            ACTION_TOGGLE_THEME,
        ];
        let mut tab_window_card = card().child(section_header("TABS & WINDOW"));
        for (i, &act) in tab_window_actions.iter().enumerate() {
            tab_window_card = tab_window_card.child(render_shortcut_row(100 + i, act));
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
            .child(split_pane_card)
            .child(scroll_card)
            .child(text_card)
            .child(zoom_card);

        // ==========================================
        // --- Body / Content Framing ---
        // ==========================================

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
            .mb(px(22.0))
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(sc_title)
                    .child(match self.settings_tab {
                        SettingsTab::General => "General Preferences",
                        SettingsTab::Appearance => "Appearance & Themes",
                        SettingsTab::Shortcuts => "Keyboard Shortcuts",
                    }),
            )
            .child(
                div()
                    .id(ElementId::named_usize("settings-top-back-btn", 0))
                    .h(px(36.0))
                    .px(px(16.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(sc_back_border)
                    .bg(sc_back_bg)
                    .hover(move |s| s.bg(theme.card_hover))
                    .text_color(sc_back_fg)
                    .child(icons::icon(icons::CHEVRON_LEFT).size(px(16.0)))
                    .child(
                        div()
                            .text_size(px(13.5))
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
            .p(px(32.0))
            .pb(px(88.0))
            .child(
                div()
                    .w(relative(1.0))
                    .max_w(px(880.0))
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
