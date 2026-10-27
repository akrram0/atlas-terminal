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
    ACTION_COPY, ACTION_FOCUS_NEXT_PANE, ACTION_FOCUS_PREV_PANE, ACTION_FULLSCREEN,
    ACTION_NEW_TAB, ACTION_NEXT_TAB, ACTION_OPEN_SETTINGS, ACTION_PALETTE, ACTION_PASTE,
    ACTION_PREV_TAB, ACTION_SPLIT_HORIZONTAL, ACTION_SPLIT_VERTICAL, ACTION_ZOOM_IN,
    ACTION_ZOOM_OUT, ACTION_ZOOM_RESET, ACTION_SELECT_ALL, ACTION_SCROLL_PAGE_UP,
    ACTION_SCROLL_PAGE_DOWN, ACTION_SCROLL_TO_TOP, ACTION_SCROLL_TO_BOTTOM,
    ACTION_OPEN_URL_PICKER, ACTION_TOGGLE_THEME, ACTION_TOGGLE_MAXIMIZE,
};
use crate::icons;
use crate::theme::Theme;
use crate::updater::{self, format_bytes};
use crate::view::{AtlasView, UpdateDownloadState};
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
        crate::rounded_corners::set_opacity(self.opacity);
        save_app_settings(&self.app_settings);
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
                .px(px(14.0))
                .pt(px(14.0))
                .pb(px(6.0))
                .text_size(px(11.0))
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
                .px(px(14.0))
                .py(px(11.0))
                .gap(px(12.0));
            if !is_last {
                row = row.border_b_1().border_color(theme.hairline);
            }
            row.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.5))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(sc_label)
                            .child(label),
                    )
                    .children(desc.map(|d| {
                        div()
                            .text_size(px(11.0))
                            .text_color(sc_desc)
                            .child(d)
                    })),
            )
            .child(control)
            .into_any_element()
        };

        // --- Left Sidebar (160px) ---
        let nav_items = [
            (SettingsTab::General, "General", icons::TERMINAL),
            (SettingsTab::Appearance, "Appearance", icons::SLIDERS),
            (SettingsTab::Shortcuts, "Shortcuts", icons::KEYBOARD),
        ];

        let mut sidebar = div()
            .id(ElementId::named_usize("settings-sidebar-scroll", 0))
            .w(px(160.0))
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
                    .py(px(8.0))
                    .mb(px(8.0))
                    .child(icons::icon(icons::SETTINGS).size(px(16.0)).text_color(theme.accent))
                    .child(
                        div()
                            .text_size(px(13.5))
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
                            .text_size(px(12.5))
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
            118,
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
                .w(px(150.0))
                .flex()
                .flex_col()
                .rounded(px(8.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .p(px(6.0))
                .cursor_pointer()
                .hover(move |s| s.bg(theme.card_hover))
                .child(
                    // Mini Window Mockup
                    div()
                        .h(px(52.0))
                        .w(relative(1.0))
                        .rounded(px(5.0))
                        .bg(preview_theme.bg)
                        .border_1()
                        .border_color(preview_theme.elevated_border)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            // Window Header
                            div()
                                .h(px(11.0))
                                .px(px(4.0))
                                .bg(preview_theme.panel_bg)
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(3.0))
                                .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0xFF5F56)))
                                .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0xFFBD2E)))
                                .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0x27C93F))),
                        )
                        .child(
                            // Terminal Body Preview
                            div()
                                .flex_1()
                                .p(px(4.0))
                                .flex()
                                .flex_col()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_size(px(8.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(preview_theme.accent)
                                                .child("~ $"),
                                        )
                                        .child(
                                            div()
                                                .w(px(2.5))
                                                .h(px(7.0))
                                                .bg(preview_theme.cursor),
                                        ),
                                )
                                .child(
                                    // 4 Swatch Dots
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(3.0))
                                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(preview_theme.accent))
                                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(preview_theme.fg))
                                        .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(preview_theme.chrome_fg)),
                                ),
                        ),
                )
                .child(
                    // Label & Indicator
                    div()
                        .pt(px(6.0))
                        .px(px(2.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                .text_color(if is_sel { sc_title } else { sc_label })
                                .child(preset.name),
                        )
                        .children(if is_sel {
                            Some(icons::icon(icons::CHECK).size(px(12.0)).text_color(theme.accent))
                        } else {
                            None
                        }),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.set_theme(idx, cx);
                }));

            theme_grid = theme_grid.child(theme_card);
        }

        // Font Family Pills
        let fonts = [
            "Cascadia Code",
            "JetBrains Mono",
            "SF Mono",
            "Fira Code",
            "Consolas",
        ];
        let mut font_pills = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(5.0));

        for (idx, &fname) in fonts.iter().enumerate() {
            let is_sel = self.font_family.eq_ignore_ascii_case(fname);
            let s_str = fname.to_string();
            font_pills = font_pills.child(pill_button(
                s_str.clone(),
                160 + idx,
                is_sel,
                theme,
                move |view, cx| {
                    view.font_family = s_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                },
                cx,
            ));
        }

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

        // Interactive Opacity Stepper (5% steps, 15% to 100%)
        let opacity_ctrl = stepper(
            113,
            format!("{}%", (self.opacity * 100.0).round() as i32),
            theme,
            |view, cx| {
                let current = (view.opacity * 20.0).round() / 20.0;
                view.opacity = (current - 0.05).max(0.15);
                view.sync_and_save_settings();
                cx.notify();
            },
            |view, cx| {
                let current = (view.opacity * 20.0).round() / 20.0;
                view.opacity = (current + 0.05).min(1.0);
                view.sync_and_save_settings();
                cx.notify();
            },
            cx,
        );

        let corner_radius_ctrl = stepper(
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
        );

        let cursor_options = vec![
            (None, "Block".to_string()),
            (Some(CursorShape::Beam), "Bar".to_string()),
            (Some(CursorShape::Underline), "Underline".to_string()),
            (Some(CursorShape::HollowBlock), "Hollow".to_string()),
        ];
        let cursor_shape_ctrl = segmented_control(
            130,
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
                    .child(setting_row("Font Family", Some("Monospace terminal typeface"), font_pills, false))
                    .child(setting_row("Font Size", Some("Glyph rendering dimension"), font_size_ctrl, false))
                    .child(setting_row("Line Height", Some("Vertical cell proportion multiplier"), line_height_ctrl, true)),
            )
            .child(
                card()
                    .child(section_header("WINDOW & GLASS"))
                    .child(setting_row("Window Opacity", Some("Glass translucency factor (15% to 100%)"), opacity_ctrl, false))
                    .child(setting_row("Corner Radius", Some("Curvature of window container corners"), corner_radius_ctrl, true)),
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
        let shell_list = [
            ("PowerShell", "PowerShell", "Windows PowerShell & Core", icons::POWERSHELL),
            ("WSL", "WSL (Linux)", "Ubuntu / Default Linux Distro", icons::UBUNTU),
            ("OpenCode", "OpenCode AI", "Autonomous Agent CLI", icons::OPENCODE_MARK),
            ("Command Prompt", "CMD", "Classic Windows Command Prompt", icons::CMD),
            ("Git Bash", "Git Bash", "MinGW Git for Windows", icons::GIT),
        ];

        let mut shell_cards = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.0))
            .p(px(14.0));

        for (idx, &(s_id, s_title, s_desc, s_icon)) in shell_list.iter().enumerate() {
            let is_sel = self.app_settings.default_shell.eq_ignore_ascii_case(s_id);
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
                .w(px(145.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .p(px(10.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(card_border)
                .bg(card_bg)
                .cursor_pointer()
                .hover(move |s| s.bg(theme.card_hover))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(icons::icon(s_icon).size(px(16.0)).text_color(if is_sel { theme.accent } else { sc_label }))
                        .children(if is_sel {
                            Some(icons::icon(icons::CHECK).size(px(12.0)).text_color(theme.accent))
                        } else {
                            None
                        }),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                        .text_color(if is_sel { sc_title } else { sc_label })
                        .child(s_title),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(sc_desc)
                        .child(s_desc),
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.app_settings.default_shell = s_str.clone();
                    view.sync_and_save_settings();
                    cx.notify();
                }));

            shell_cards = shell_cards.child(card_el);
        }

        let startup_options = vec![
            ("restore_session".to_string(), "Restore Session".to_string()),
            ("new_tab".to_string(), "New Tab".to_string()),
        ];
        let startup_ctrl = segmented_control(
            330,
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
            310,
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
            320,
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
            360,
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

        let general_view = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                card()
                    .child(section_header("DEFAULT SHELL"))
                    .child(shell_cards),
            )
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
                    .child(section_header("SOFTWARE UPDATES"))
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
                .px(px(14.0))
                .py(px(9.0))
                .border_b_1()
                .border_color(theme.hairline);

            let mut left_col = div()
                .flex()
                .flex_col()
                .gap(px(1.5))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(sc_label)
                        .child(action_display_name(action)),
                );

            if let Some(other) = conflict {
                left_col = left_col.child(
                    div()
                        .text_size(px(10.5))
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
                .gap(px(6.0));

            if is_custom && !is_recording {
                right_col = right_col.child(
                    div()
                        .id(ElementId::named_usize("reset-shortcut", idx))
                        .text_size(px(11.0))
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
                    .h(px(24.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .bg(keycap_bg)
                    .border_1()
                    .border_color(keycap_border)
                    .text_size(px(11.5))
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
            .mb(px(16.0))
            .child(
                div()
                    .text_size(px(18.0))
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
                    .max_w(px(700.0))
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
