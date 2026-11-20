//! "What's New in Atlas Terminal" Release Notes & Onboarding Modal.
//!
//! Provides zero-latency, embedded visual feature highlights for new releases,
//! with Apple-style glassmorphism styling, clean vector iconography,
//! dedicated Settings tab integration, 1-key keyboard dismissal, and persistent user preferences.

use gpui::prelude::*;
use gpui::{
    div, px, relative, rgb, rgba, AnyElement, ClickEvent, Context, ElementId, FontWeight,
    IntoElement, Rgba,
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
    version: "0.2.51",
    codename: "Multi-Monitor Snap Stability & PowerShell Word Navigation",
    date: "November 2026",
    highlights: &[
        FeatureHighlight {
            category: "Windows & Multi-Monitor",
            icon: icons::TERMINAL,
            title: "Multi-Monitor Maximized Snap & Move Stability",
            description: "Fixed client rectangle and swapchain coordinate clipping when moving maximized windows across multi-monitor setups with Win+Shift+Left/Right shortcuts (Issue #10).",
            badge: Some("Issue #10 Fixed"),
        },
        FeatureHighlight {
            category: "Keyboard & Shell Input",
            icon: icons::TERMINAL,
            title: "PowerShell & Terminal Word Navigation",
            description: "Standardized xterm modifier sequences for cursor keys: Ctrl+Left/Right now cleanly navigates words (BackwardWord / ForwardWord) in PowerShell 5.1/7 and readline shells (Issue #10).",
            badge: Some("Shell Input"),
        },
        FeatureHighlight {
            category: "Keyboard & Shell Input",
            icon: icons::TERMINAL,
            title: "PowerShell Line-Editing Shortcut Passthrough",
            description: "PSReadLine selection chords now reach the shell: Ctrl+Shift+Left/Right select words, Shift+Home/End select to the line edges, and Ctrl+A stays reserved for SelectAll. Scroll-to-top/bottom moved to Ctrl+Home/End (Issue #11).",
            badge: Some("Issue #11 Fixed"),
        },
        FeatureHighlight {
            category: "UI & Top Bar",
            icon: icons::SPARKLE,
            title: "Native What's New Top-Bar Tab",
            description: "Decommissioned the broken popup modal dialog in favor of a first-class top-bar tab pill with full-width responsive card layout and native scrolling.",
            badge: Some("New Tab"),
        },
        FeatureHighlight {
            category: "UI & Layout",
            icon: icons::SLIDERS,
            title: "Responsive Text Wrapping & Card Containment",
            description: "Fixed layout flex calculation across Settings and release sheets: descriptions now flow naturally onto multiple lines within card boundaries without vertical character stacking (Issue #8).",
            badge: Some("Issue #8 Fixed"),
        },
        FeatureHighlight {
            category: "Typing Effects",
            icon: icons::SPARKLE,
            title: "Tactile Letter Springing & Jostle Waves",
            description: "Physical stamp-drop squash and harmonic bounce on newly typed characters paired with an organic kinetic jostle wave radiating along neighboring text.",
            badge: Some("New"),
        },
        FeatureHighlight {
            category: "Settings & Customization",
            icon: icons::SETTINGS,
            title: "Configurable ASCII Ink Scale",
            description: "Customize the spread and intensity of procedural Bayer dither ripples and particle wakes (Compact 0.55×, Balanced 1.0×, or Expanded 1.6×).",
            badge: Some("Settings"),
        },
        FeatureHighlight {
            category: "Typing Effects",
            icon: icons::SPARKLE,
            title: "Native ASCII Ink Field Typing Flow",
            description: "Procedural 4×4 Bayer matrix dither density ramp (.,:;t%SX8#@) with expanding ripples, swimming fish particle wakes, and backspace rose dissolve beneath the cursor.",
            badge: Some("Visual"),
        },
        FeatureHighlight {
            category: "Cursor & Kinetics",
            icon: icons::PEN,
            title: "Elastic Mass Squish & Stretch Caret",
            description: "Organic mass-spring kinetics dynamically elongate the caret along its motion vector during transit and settle with snappy 45ms Apple spring damping.",
            badge: Some("Kinetics"),
        },
        FeatureHighlight {
            category: "Cursor & Trails",
            icon: icons::SPARKLE,
            title: "Ghost Comet Trails & Glow Dynamics",
            description: "Optional decaying 3-step ghost trails with quadratic alpha decay, zero-idle battery sleep state, and micro-rounded Apple corners.",
            badge: Some("Kinetics"),
        },
        FeatureHighlight {
            category: "Input & Modal TUI",
            icon: icons::TERMINAL,
            title: "Smooth Geometric Shape Morphing",
            description: "Continuous 60ms Hermite interpolation between Block, Beam (Bar), and Underline cursor shapes during Vim, OpenCode, and shell mode changes.",
            badge: Some("TUI"),
        },
        FeatureHighlight {
            category: "Rendering Stability",
            icon: icons::REFRESH,
            title: "Line-Wrap Discontinuity Guards & Hybrid Inversion",
            description: "Instant snap eliminates 80-column screen-wide streaks on line wraps, while hybrid inversion preserves underlying text without strobing or flickering.",
            badge: Some("Core"),
        },
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
    /// Render the full What's New tab sheet (when open as a native top-bar tab).
    pub fn render_whats_new_tab_page(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_dark = !theme.is_light();
        let sc_title = theme.chrome_fg_active;
        let sc_desc = theme.chrome_fg;

        let content_header = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .mb(px(20.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(sc_title)
                            .child("Release Highlights"),
                    )
                    .child(
                        div()
                            .id(ElementId::named_usize("whats-new-tab-close-pill", 0))
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(6.0))
                            .bg(theme.soft_bg)
                            .border_1()
                            .border_color(theme.hairline)
                            .hover(move |s| s.bg(theme.card_hover))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                icons::icon(icons::CLOSE)
                                    .size(px(12.0))
                                    .text_color(theme.faint_fg),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.chrome_fg)
                                    .child("Close (Esc)"),
                            )
                            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                view.close_whats_new(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(sc_desc)
                    .child("Explore new capabilities, performance upgrades, and release highlights in Atlas Terminal."),
            );

        div()
            .id(ElementId::named_usize("whats-new-tab-scroll", 0))
            .w(relative(1.0))
            .h(relative(1.0))
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .p(px(28.0))
            .pb(px(88.0))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(relative(1.0))
                    .max_w(px(880.0))
                    .min_w(px(0.0))
                    .child(content_header)
                    .child(self.render_whats_new_settings_page(theme, is_dark, cx)),
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
        let checkbox_active = self.app_settings.show_whats_new_on_update;

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Hero Banner Card
            .child(
                div()
                    .w(relative(1.0))
                    .min_w(px(0.0))
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
                            .w(relative(1.0))
                            .min_w(px(0.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(14.0))
                                    .child(
                                        div()
                                            .w(px(44.0))
                                            .h(px(44.0))
                                            .flex_none()
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
                                            .flex_1()
                                            .min_w(px(0.0))
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
                                                            .flex_none()
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
                                    .flex_none()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
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
                    .w(relative(1.0))
                    .min_w(px(0.0))
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
                            .w(relative(1.0))
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .flex()
                            .flex_row()
                            .items_start()
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
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(13.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.chrome_fg_active)
                                                    .child(feat.title),
                                            )
                                            .children(feat.badge.map(|b| {
                                                div()
                                                    .flex_none()
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
        assert_eq!(release.version, "0.2.51");
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

    #[test]
    fn test_issue_8_highlight_entry_present_and_well_formed() {
        let release = latest_release();
        let issue_8 = release
            .highlights
            .iter()
            .find(|h| h.title.contains("Card Containment") || h.badge == Some("Issue #8 Fixed"));
        assert!(
            issue_8.is_some(),
            "Issue #8 highlight must be registered in CURRENT_RELEASE"
        );
        let feat = issue_8.unwrap();
        assert!(!feat.description.is_empty());
        assert_eq!(feat.badge, Some("Issue #8 Fixed"));
    }
}
