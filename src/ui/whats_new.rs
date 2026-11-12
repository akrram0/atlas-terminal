//! "What's New in Atlas Terminal" Release Notes & Onboarding Modal.
//!
//! Provides zero-latency, embedded visual feature highlights for new releases,
//! with Apple-style glassmorphism styling, clean vector iconography,
//! 1-key keyboard dismissal, and persistent user preferences.

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
    version: "0.2.39",
    codename: "Velocity & Aesthetics",
    date: "November 2026",
    highlights: &[
        FeatureHighlight {
            category: "Performance",
            icon: icons::SPARKLE,
            title: "Zero-Lag Pixel Art Engine",
            description: "Eliminated UI thread filesystem polling with zero-I/O in-memory caching. Settings open smoothly at 60 FPS.",
            badge: Some("0ms I/O"),
        },
        FeatureHighlight {
            category: "Companions",
            icon: icons::PACKAGE,
            title: "Official Animated Pixel Companions",
            description: "White Cat, Playful Penguin, Electric Fox, Shadow Bat, and Dark Knight are now official bundled presets in your top bar.",
            badge: Some("5 Presets"),
        },
        FeatureHighlight {
            category: "Themes",
            icon: icons::SUN,
            title: "Aesthetic Pastel Themes & Mode Segregation",
            description: "Added Sakura Blossom, Lavender Haze, and Peach Sorbet. Atlas Ragnarok and Black Metal are now built-in, with strict dark/light mode separation.",
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
            description: "Native wavy squiggly underlines for LSP errors and compiler diagnostics in Neovim, Helix, and modern shells, with per-diagnostic colors.",
            badge: Some("LSP Undercurls"),
        },
        FeatureHighlight {
            category: "Tiling",
            icon: icons::SPLIT_COLUMNS,
            title: "1:1 Precision Nested Split Resizing",
            description: "Dividers now track cursor movement with 1:1 precision across nested split pane hierarchies, with widened 8px grab hitboxes.",
            badge: Some("Precision"),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_release_metadata_valid() {
        let release = latest_release();
        assert_eq!(release.version, "0.2.39");
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
