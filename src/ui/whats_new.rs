//! "What's New in Atlas Terminal" Release Notes & Onboarding Modal.
//!
//! Provides zero-latency, embedded visual feature highlights for new releases,
//! with Apple-style glassmorphism styling, clean vector iconography,
//! dedicated Settings tab integration, 1-key keyboard dismissal, and persistent user preferences.

use gpui::prelude::*;
use gpui::{
    div, px, rgb, rgba, AnyElement, ClickEvent, Context, ElementId, FontWeight, IntoElement,
    Rgba,
};

use crate::icons;
use crate::theme::Theme;
use crate::view::AtlasView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureHighlight {
    pub category: &'static str,
    pub icon: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub badge: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseHighlight {
    pub version: &'static str,
    pub codename: &'static str,
    pub date: &'static str,
    pub highlights: &'static [FeatureHighlight],
}

pub const CURRENT_RELEASE: ReleaseHighlight = ReleaseHighlight {
    version: "0.2.44",
    codename: "OpenCode Binary Launcher & Self-Healing Shim Repair",
    date: "November 2026",
    highlights: &[
        FeatureHighlight {
            category: "OpenCode & CLI",
            icon: icons::OPENCODE_MARK,
            title: "OpenCode Auto-Sync & Shim Self-Healing",
            description: "Repaired fragile pnpm global hash path shims: Atlas now automatically resolves, syncs, and launches the direct native opencode.exe binary and auto-heals broken opencode.ps1 and opencode.cmd wrapper scripts.",
            badge: Some("Fixed"),
        },
        FeatureHighlight {
            category: "Input & TUI",
            icon: icons::OPENCODE_MARK,
            title: "Full Escape Key Passthrough to OpenCode & PTY",
            description: "Fixed unconditional escape interception in handle_key: pressing Esc now cleanly forwards 0x1b to active shell PTY and interactive TUI sessions (OpenCode, Vim, Nano) while seamlessly preserving overlay dismissal.",
            badge: Some("Input"),
        },
        FeatureHighlight {
            category: "Input & Keystrokes",
            icon: icons::KEYBOARD,
            title: "Robust Key Code Normalization",
            description: "Added dual matching for Esc/Escape, Enter/Return/NumpadEnter, Arrow keys, and case-insensitive control chords across both input router and keymap translator.",
            badge: Some("Core Input"),
        },
        FeatureHighlight {
            category: "Settings & UX",
            icon: icons::REFRESH,
            title: "Interactive Settings Scrollbar & Paging",
            description: "Full mouse drag-to-scroll, track click paging, PageUp/PageDown/Home/End keyboard navigation, and high-contrast Apple glass styling (Issue #7).",
            badge: Some("Issue #7 Fixed"),
        },
        FeatureHighlight {
            category: "Settings Hub",
            icon: icons::SLIDERS,
            title: "Settings Strip Reordered & Default General Tab",
            description: "Moved What's New to the end of the sub-nav strip so General remains the primary settings landing page, with instant Esc keycap close.",
            badge: Some("UX Polish"),
        },
        FeatureHighlight {
            category: "Canvas & Scrolling",
            icon: icons::REFRESH,
            title: "Interactive Visible Scrollbar & Touchpad Dragging",
            description: "Full mouse and touchpad drag-to-scroll, track click paging, configurable visibility (Always / Auto-Hide / Hidden), and Ctrl+Shift+Up/Down line scrolling (Issue #6).",
            badge: Some("Issue #6 Fixed"),
        },
        FeatureHighlight {
            category: "Release Hub",
            icon: icons::SPARKLE,
            title: "What's New Release Hub & Dedicated Settings Tab",
            description: "Review update highlights anytime directly in Settings or via the centered Apple glass upgrade modal, with instant 1-key Esc dismissal.",
            badge: Some("New Hub"),
        },
        FeatureHighlight {
            category: "Performance",
            icon: icons::REFRESH,
            title: "Zero-Lag Pixel Art Engine",
            description: "Eliminated UI thread filesystem polling with zero-I/O in-memory caching. Settings open smoothly at an instant 60 FPS.",
            badge: Some("0ms I/O"),
        },
        FeatureHighlight {
            category: "Companions",
            icon: icons::PACKAGE,
            title: "5 Bundled Animated Pixel Companions",
            description: "White Cat, Playful Penguin, Electric Fox, Shadow Bat, and Dark Knight are now official bundled presets in your top bar.",
            badge: Some("5 Presets"),
        },
        FeatureHighlight {
            category: "Themes",
            icon: icons::SUN,
            title: "Aesthetic Pastel Themes & Mode Segregation",
            description: "Added Sakura Blossom, Lavender Haze, and Peach Sorbet. Atlas Ragnarok, Black Metal, and Neon Cyberpunk are now built-in, with strict dark/light mode separation.",
            badge: Some("3 New Themes"),
        },
        FeatureHighlight {
            category: "Command Palette",
            icon: icons::SEARCH,
            title: "Live Theme Preview (Ctrl+K)",
            description: "Navigate theme list in Ctrl+K with Up/Down arrow keys for instant live preview before committing with Enter.",
            badge: Some("Live Preview"),
        },
        FeatureHighlight {
            category: "Terminal Engine",
            icon: icons::TERMINAL,
            title: "Diagnostic Wavy Undercurls",
            description: "Native wavy squiggly underlines for LSP compiler diagnostics with dedicated colors in Neovim, Helix, and modern shells.",
            badge: Some("LSP Undercurls"),
        },
        FeatureHighlight {
            category: "Tiling",
            icon: icons::SPLIT_COLUMNS,
            title: "1:1 Precision Nested Split Resizing",
            description: "Dividers now track cursor movement with 1:1 precision across nested split pane hierarchies, with widened 8px grab hitboxes.",
            badge: Some("Precision"),
        },
        FeatureHighlight {
            category: "UI Polish",
            icon: icons::SETTINGS,
            title: "Refined Esc Keycap & Theme Cards Layout",
            description: "Crisp vector Close icon with Apple-style <kbd>Esc</kbd> keycap badge, plus multi-row theme cards with text-overflow ellipsis protection.",
            badge: Some("UI Polish"),
        },
    ],
};

#[allow(dead_code)]
pub const RELEASES: &[ReleaseHighlight] = &[CURRENT_RELEASE];

pub fn latest_release() -> &'static ReleaseHighlight {
    &CURRENT_RELEASE
}

impl AtlasView {
    /// Render the centered Apple glass "What's New" modal dialog.
    pub fn render_whats_new_modal(
        &self,
        theme: &Theme,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let release = latest_release();
        let item_hover = theme.card_hover;
        let checkbox_active = self.app_settings.show_whats_new_on_update;

        // Centered flex layout overlay
        div()
            .id(ElementId::named_usize("whats-new-overlay-center", 0))
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("whats-new-modal-sheet", 0))
                    .w(px(540.0))
                    .max_h(px(600.0))
                    .flex()
                    .flex_col()
                    .rounded(px(14.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .overflow_hidden()
                    // Modal Header
                    .child(
                        div()
                            .p(px(18.0))
                            .pb(px(14.0))
                            .flex()
                            .flex_row()
                            .items_start()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .w(px(38.0))
                                            .h(px(38.0))
                                            .rounded(px(10.0))
                                            .bg(theme.soft_bg)
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                icons::icon(icons::ATLAS_MARK)
                                                    .size(px(20.0))
                                                    .text_color(theme.accent),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(15.5))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme.chrome_fg_active)
                                                            .child("What's New in Atlas"),
                                                    )
                                                    .child(
                                                        div()
                                                            .px(px(6.0))
                                                            .py(px(1.5))
                                                            .rounded(px(4.0))
                                                            .bg(Rgba {
                                                                a: 0.15,
                                                                ..theme.accent
                                                            })
                                                            .text_size(px(11.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme.accent)
                                                            .child(format!("v{}", release.version)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(theme.chrome_fg)
                                                    .child(format!(
                                                        "{} · {}",
                                                        release.codename, release.date
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id(ElementId::named_usize("whats-new-close-btn", 0))
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_color(theme.faint_fg)
                                    .hover(move |s| {
                                        s.bg(item_hover).text_color(theme.chrome_fg_active)
                                    })
                                    .child(
                                        icons::icon(icons::CLOSE)
                                            .size(px(12.5))
                                            .text_color(theme.faint_fg),
                                    )
                                    .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                        view.close_whats_new(window, cx);
                                    })),
                            ),
                    )
                    // Hairline divider
                    .child(div().h(px(1.0)).bg(theme.hairline))
                    // Scrollable Feature Cards
                    .child(
                        div()
                            .id(ElementId::named_usize("whats-new-feature-list", 0))
                            .flex_1()
                            .overflow_y_scroll()
                            .p(px(18.0))
                            .py(px(14.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .children(release.highlights.iter().enumerate().map(
                                |(idx, feat)| {
                                    div()
                                        .id(ElementId::named_usize("whats-new-item", idx))
                                        .flex()
                                        .flex_row()
                                        .gap(px(12.0))
                                        .p(px(10.0))
                                        .rounded(px(8.0))
                                        .bg(if is_dark {
                                            rgba(0x00000030)
                                        } else {
                                            rgba(0xFFFFFF70)
                                        })
                                        .border_1()
                                        .border_color(theme.hairline)
                                        .child(
                                            div()
                                                .w(px(32.0))
                                                .h(px(32.0))
                                                .flex_none()
                                                .rounded(px(6.0))
                                                .bg(theme.soft_bg)
                                                .border_1()
                                                .border_color(theme.hairline)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    icons::icon(feat.icon)
                                                        .size(px(15.0))
                                                        .text_color(theme.accent),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.chrome_fg_active)
                                                                .child(feat.title),
                                                        )
                                                        .children(feat.badge.map(|b| {
                                                            div()
                                                                .px(px(5.0))
                                                                .py(px(1.0))
                                                                .rounded(px(3.0))
                                                                .text_size(px(9.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .bg(Rgba {
                                                                    a: 0.12,
                                                                    ..theme.accent
                                                                })
                                                                .text_color(theme.accent)
                                                                .child(b)
                                                        })),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .text_color(theme.chrome_fg)
                                                        .line_height(px(15.0))
                                                        .child(feat.description),
                                                ),
                                        )
                                },
                            )),
                    )
                    // Hairline divider
                    .child(div().h(px(1.0)).bg(theme.hairline))
                    // Modal Footer
                    .child(
                        div()
                            .p(px(14.0))
                            .px(px(18.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            // Left: Checkbox "Show after future updates"
                            .child(
                                div()
                                    .id(ElementId::named_usize("whats-new-auto-toggle", 0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .rounded(px(3.0))
                                            .border_1()
                                            .border_color(if checkbox_active {
                                                theme.accent
                                            } else {
                                                theme.hairline
                                            })
                                            .bg(if checkbox_active {
                                                theme.accent
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .children(if checkbox_active {
                                                Some(
                                                    icons::icon(icons::CHECK)
                                                        .size(px(10.0))
                                                        .text_color(rgb(0xFFFFFF)),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(theme.chrome_fg)
                                            .child("Show after future updates"),
                                    )
                                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                        view.app_settings.show_whats_new_on_update =
                                            !view.app_settings.show_whats_new_on_update;
                                        crate::config::save_app_settings(&view.app_settings);
                                        cx.notify();
                                    })),
                            )
                            // Right: Action buttons (GitHub release link + Got it)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("whats-new-gh-link", 0))
                                            .h(px(28.0))
                                            .px(px(10.0))
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .bg(theme.soft_bg)
                                            .hover(move |s| {
                                                s.bg(item_hover).border_color(theme.accent)
                                            })
                                            .cursor_pointer()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child(
                                                icons::icon(icons::LINK)
                                                    .size(px(11.0))
                                                    .text_color(theme.chrome_fg),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.chrome_fg)
                                                    .child("Full Changelog ↗"),
                                            )
                                            .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
                                                crate::url::open_url(
                                                    "https://github.com/akrram0/atlas-terminal/releases",
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("whats-new-got-it-btn", 0))
                                            .h(px(28.0))
                                            .px(px(14.0))
                                            .rounded(px(6.0))
                                            .bg(theme.accent)
                                            .hover(|s| s.opacity(0.90))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xFFFFFF))
                                                    .child("Start Typing"),
                                            )
                                            .on_click(cx.listener(
                                                |view, _: &ClickEvent, window, cx| {
                                                    view.close_whats_new(window, cx);
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Render the full-width "What's New" settings tab view.
    pub fn render_whats_new_settings_page(
        &self,
        theme: &Theme,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let release = latest_release();
        let item_hover = theme.card_hover;
        let checkbox_active = self.app_settings.show_whats_new_on_update;

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Hero Banner Card
            .child(
                div()
                    .rounded(px(10.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(14.0))
                                    .child(
                                        div()
                                            .w(px(44.0))
                                            .h(px(44.0))
                                            .rounded(px(10.0))
                                            .bg(theme.soft_bg)
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                icons::icon(icons::ATLAS_MARK)
                                                    .size(px(24.0))
                                                    .text_color(theme.accent),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(17.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme.chrome_fg_active)
                                                            .child("Atlas Terminal"),
                                                    )
                                                    .child(
                                                        div()
                                                            .px(px(7.0))
                                                            .py(px(2.0))
                                                            .rounded(px(4.0))
                                                            .bg(Rgba {
                                                                a: 0.15,
                                                                ..theme.accent
                                                            })
                                                            .text_size(px(12.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme.accent)
                                                            .child(format!("v{}", release.version)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .text_color(theme.chrome_fg)
                                                    .child(format!(
                                                        "{} · Released {}",
                                                        release.codename, release.date
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("whats-new-page-modal-btn", 0))
                                            .h(px(28.0))
                                            .px(px(12.0))
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(theme.hairline)
                                            .bg(theme.soft_bg)
                                            .hover(move |s| s.bg(item_hover).border_color(theme.accent))
                                            .cursor_pointer()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(5.0))
                                            .child(
                                                icons::icon(icons::WINDOW_NEW)
                                                    .size(px(12.0))
                                                    .text_color(theme.chrome_fg),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.chrome_fg)
                                                    .child("Open As Modal"),
                                            )
                                            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.open_whats_new(cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::named_usize("whats-new-page-gh-link", 0))
                                            .h(px(28.0))
                                            .px(px(12.0))
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(theme.accent)
                                            .bg(Rgba { a: 0.12, ..theme.accent })
                                            .hover(move |s| s.bg(Rgba { a: 0.22, ..theme.accent }))
                                            .cursor_pointer()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(5.0))
                                            .child(
                                                icons::icon(icons::LINK)
                                                    .size(px(11.0))
                                                    .text_color(theme.accent),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.accent)
                                                    .child("GitHub Releases ↗"),
                                            )
                                            .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
                                                crate::url::open_url(
                                                    "https://github.com/akrram0/atlas-terminal/releases",
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .pt(px(10.0))
                            .border_t_1()
                            .border_color(theme.hairline)
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .id(ElementId::named_usize("whats-new-page-toggle", 0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .rounded(px(3.0))
                                            .border_1()
                                            .border_color(if checkbox_active {
                                                theme.accent
                                            } else {
                                                theme.hairline
                                            })
                                            .bg(if checkbox_active {
                                                theme.accent
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .children(if checkbox_active {
                                                Some(
                                                    icons::icon(icons::CHECK)
                                                        .size(px(10.0))
                                                        .text_color(rgb(0xFFFFFF)),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme.chrome_fg)
                                            .child("Automatically show release notes on launch after updating"),
                                    )
                                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                        view.app_settings.show_whats_new_on_update =
                                            !view.app_settings.show_whats_new_on_update;
                                        crate::config::save_app_settings(&view.app_settings);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Feature Highlights Grid
            .child(
                div()
                    .rounded(px(10.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .p(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .children(release.highlights.iter().enumerate().map(|(idx, feat)| {
                        div()
                            .id(ElementId::named_usize("whats-new-page-item", idx))
                            .flex()
                            .flex_row()
                            .gap(px(14.0))
                            .p(px(12.0))
                            .rounded(px(8.0))
                            .bg(if is_dark {
                                rgba(0x00000030)
                            } else {
                                rgba(0xFFFFFF70)
                            })
                            .border_1()
                            .border_color(theme.hairline)
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .flex_none()
                                    .rounded(px(8.0))
                                    .bg(theme.soft_bg)
                                    .border_1()
                                    .border_color(theme.hairline)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        icons::icon(feat.icon)
                                            .size(px(17.0))
                                            .text_color(theme.accent),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(px(13.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.chrome_fg_active)
                                                    .child(feat.title),
                                            )
                                            .children(feat.badge.map(|b| {
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.5))
                                                    .rounded(px(4.0))
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .bg(Rgba {
                                                        a: 0.12,
                                                        ..theme.accent
                                                    })
                                                    .text_color(theme.accent)
                                                    .child(b)
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme.chrome_fg)
                                            .line_height(px(16.0))
                                            .child(feat.description),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_release_metadata_valid() {
        let release = latest_release();
        assert_eq!(release.version, "0.2.44");
        assert!(!release.codename.is_empty());
        assert!(!release.date.is_empty());
        assert!(release.highlights.len() >= 4);

        for h in release.highlights {
            assert!(!h.title.is_empty());
            assert!(!h.description.is_empty());
            assert!(!h.category.is_empty());
            assert!(h.icon.starts_with("icons/"));
        }
    }
}
