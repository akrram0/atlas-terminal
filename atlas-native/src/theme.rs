//! Atlas theme — one source of truth for every colour in the app.
//!
//! `ThemeRegistry` holds the shipped presets: "Dark" — a neutral matte
//! charcoal sheet (#141413) under a just-lighter chrome (#1C1C1A) — is the
//! boot default, "Warm Paper" the cream alternative (#FAF6EE over #EFECE4).
//! Ctrl+Shift+T cycles the presets; the settings page lists them.
//!
//! Programs can re-theme their own cells at runtime through standard OSC 4 /
//! OSC 10/11/12 sequences (`\e]11;#101010`): alacritty_terminal routes those
//! into the terminal's override table (`Term::colors()`), which `resolve`,
//! `defaults` and `color_at` consult.

use alacritty_terminal::term::color::{Colors, COUNT};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui::{rgb, rgba, Rgba};

#[derive(Clone)]
pub struct Theme {
    /// Terminal background.
    pub bg: Rgba,
    /// Default foreground.
    pub fg: Rgba,
    /// Cursor color.
    pub cursor: Rgba,
    /// Chrome surfaces: the top bar.
    pub panel_bg: Rgba,
    /// Floating surfaces: pill tabs, menus, the workspace button.
    pub elevated_bg: Rgba,
    /// Hairline border for floating surfaces.
    pub elevated_border: Rgba,
    /// Chrome text (inactive).
    pub chrome_fg: Rgba,
    /// Chrome text (active / hovered).
    pub chrome_fg_active: Rgba,
    /// Faint text: section labels, placeholders, keyboard chips.
    pub faint_fg: Rgba,
    /// 1px separators.
    pub hairline: Rgba,
    /// Hovered list-item background.
    pub card_hover: Rgba,
    /// Active / selected list-item background.
    pub card_active: Rgba,
    /// Brand accent (active indicators).
    pub accent: Rgba,
    /// Success green (connected / running dots).
    pub good: Rgba,
    /// Hairline border of the floating terminal sheet.
    pub sheet_border: Rgba,
    /// Soft translucent fill for quiet pills (the workspace button).
    pub soft_bg: Rgba,
    /// ANSI palette 0..16.
    pub ansi: [Rgba; 16],
}

impl Theme {
    /// Warm Paper: cream sheet (#FAF6EE) over sand chrome (#EFECE4), warm
    /// deep-charcoal ink, terracotta accent.
    pub fn cream() -> Self {
        Self {
            bg: rgb(0xFAF6EE),
            fg: rgb(0x2C2A25),
            cursor: rgb(0x9A6032),
            panel_bg: rgb(0xEFECE4),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgba(0x0000000F),
            chrome_fg: rgb(0x6B6860),
            chrome_fg_active: rgb(0x2C2A25),
            faint_fg: rgb(0x8A8378),
            hairline: rgba(0x0000000F),
            card_hover: rgba(0x00000008),
            card_active: rgba(0xD4724E1A),
            accent: rgb(0xD4724E),
            good: rgb(0x5F7A46),
            sheet_border: rgb(0xE2DEC9),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x2C2A25), // 0 black
                rgb(0xB8332A), // 1 red
                rgb(0x5F7A46), // 2 green
                rgb(0x8F6824), // 3 yellow
                rgb(0x3F6C9E), // 4 blue
                rgb(0xA25B69), // 5 magenta
                rgb(0x3E7A74), // 6 cyan
                rgb(0xB3AC9C), // 7 white
                rgb(0x78736A), // 8 bright black
                rgb(0xD14B3F), // 9 bright red
                rgb(0x6F9052), // 10 bright green
                rgb(0xB8882E), // 11 bright yellow
                rgb(0x5586BC), // 12 bright blue
                rgb(0xBF7381), // 13 bright magenta
                rgb(0x4E968E), // 14 bright cyan
                rgb(0xE8E2D4), // 15 bright white
            ],
        }
    }

    /// Neutral matte charcoal: the chrome is #141413, the unified sheet (pane
    /// header + terminal canvas) #1C1C1A with a #2A2A27 hairline, warm ivory
    /// ink, terracotta accent. The registry's default preset.
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x1C1C1A),
            fg: rgb(0xE8E4DC),
            cursor: rgb(0xD08A5C),
            panel_bg: rgb(0x141413),
            elevated_bg: rgb(0x242422),
            elevated_border: rgba(0xFFFFFF14),
            chrome_fg: rgb(0x9C978C),
            chrome_fg_active: rgb(0xF0EFEA),
            faint_fg: rgb(0x7A7468),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF0A),
            card_active: rgba(0xE08A6226),
            accent: rgb(0xE08A62),
            good: rgb(0x9BAF7C),
            sheet_border: rgb(0x2A2A27),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x262320), // 0 black
                rgb(0xD06055), // 1 red
                rgb(0x9BAF7C), // 2 green
                rgb(0xD9A755), // 3 yellow
                rgb(0x82A7D0), // 4 blue
                rgb(0xC98A9A), // 5 magenta
                rgb(0x7DB8B0), // 6 cyan
                rgb(0xD8D2C6), // 7 white
                rgb(0x7A7468), // 8 bright black
                rgb(0xE07A6E), // 9 bright red
                rgb(0xB3C795), // 10 bright green
                rgb(0xEBC070), // 11 bright yellow
                rgb(0x9BBDDE), // 12 bright blue
                rgb(0xDBA6B2), // 13 bright magenta
                rgb(0x93CBC3), // 14 bright cyan
                rgb(0xF2EEE6), // 15 bright white
            ],
        }
    }

    /// Resolve an alacritty cell color into a concrete RGBA.
    ///
    /// `over` is the terminal's OSC override table (`Term::colors()`): OSC 4
    /// palette writes and OSC 10/11/12 dynamic-color writes (`\e]10;…`) land
    /// there, so a program can re-theme its own cells exactly like in xterm.
    pub fn resolve(&self, color: Color, dim: bool, over: &Colors) -> Rgba {
        let index = match color {
            Color::Named(named) => Some(named as usize),
            Color::Indexed(index) => Some(index as usize),
            Color::Spec(_) => None,
        };
        let out = match index.and_then(|index| over[index]) {
            Some(rgb_) => from_rgb(rgb_),
            None => match color {
                Color::Named(named) => self.named(named),
                Color::Indexed(index) => self.indexed(index),
                Color::Spec(rgb_) => from_rgb(rgb_),
            },
        };
        if dim {
            self.dimmed(out)
        } else {
            out
        }
    }

    /// Theme defaults after OSC overrides: (background, foreground, cursor).
    /// A program that sets `\e]11;#101010` shifts the sheet itself, and the
    /// renderer's default-background comparisons must follow.
    pub fn defaults(&self, over: &Colors) -> (Rgba, Rgba, Rgba) {
        let pick = |index: usize, base: Rgba| over[index].map(from_rgb).unwrap_or(base);
        (
            pick(NamedColor::Background as usize, self.bg),
            pick(NamedColor::Foreground as usize, self.fg),
            pick(NamedColor::Cursor as usize, self.cursor),
        )
    }

    /// Concrete color for an OSC 4/10/11/12 report index: 0-255 palette,
    /// 256 foreground, 257 background, 258 cursor. The override table wins,
    /// so a querying program is told what is actually on screen.
    pub fn color_at(&self, index: usize, over: &Colors) -> Rgba {
        if index < COUNT {
            if let Some(rgb_) = over[index] {
                return from_rgb(rgb_);
            }
        }
        match index {
            0..=15 => self.ansi[index],
            16..=255 => self.indexed(index as u8),
            257 => self.bg,
            258 => self.cursor,
            _ => self.fg,
        }
    }

    fn named(&self, named: NamedColor) -> Rgba {
        match named {
            NamedColor::Black => self.ansi[0],
            NamedColor::Red => self.ansi[1],
            NamedColor::Green => self.ansi[2],
            NamedColor::Yellow => self.ansi[3],
            NamedColor::Blue => self.ansi[4],
            NamedColor::Magenta => self.ansi[5],
            NamedColor::Cyan => self.ansi[6],
            NamedColor::White => self.ansi[7],
            NamedColor::BrightBlack => self.ansi[8],
            NamedColor::BrightRed => self.ansi[9],
            NamedColor::BrightGreen => self.ansi[10],
            NamedColor::BrightYellow => self.ansi[11],
            NamedColor::BrightBlue => self.ansi[12],
            NamedColor::BrightMagenta => self.ansi[13],
            NamedColor::BrightCyan => self.ansi[14],
            NamedColor::BrightWhite => self.ansi[15],
            NamedColor::DimBlack => self.dimmed(self.ansi[0]),
            NamedColor::DimRed => self.dimmed(self.ansi[1]),
            NamedColor::DimGreen => self.dimmed(self.ansi[2]),
            NamedColor::DimYellow => self.dimmed(self.ansi[3]),
            NamedColor::DimBlue => self.dimmed(self.ansi[4]),
            NamedColor::DimMagenta => self.dimmed(self.ansi[5]),
            NamedColor::DimCyan => self.dimmed(self.ansi[6]),
            NamedColor::DimWhite => self.dimmed(self.ansi[7]),
            NamedColor::Foreground | NamedColor::BrightForeground => self.fg,
            NamedColor::DimForeground => self.dimmed(self.fg),
            NamedColor::Background => self.bg,
            NamedColor::Cursor => self.cursor,
        }
    }

    fn dimmed(&self, color: Rgba) -> Rgba {
        // Dimming scales the channels, not the alpha: translucent text over a
        // colored cell background would lie about both colors.
        Rgba {
            r: color.r * 0.6,
            g: color.g * 0.6,
            b: color.b * 0.6,
            a: color.a,
        }
    }

    /// xterm 256-color palette: 0-15 ANSI, 16-231 6x6x6 cube, 232-255 grayscale.
    fn indexed(&self, index: u8) -> Rgba {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let index = index as u32 - 16;
                let steps = [index / 36, (index % 36) / 6, index % 6];
                let channel = |step: u32| {
                    if step == 0 {
                        0.0
                    } else {
                        (55 + step * 40) as f32 / 255.0
                    }
                };
                Rgba {
                    r: channel(steps[0]),
                    g: channel(steps[1]),
                    b: channel(steps[2]),
                    a: 1.0,
                }
            }
            _ => {
                let v = (8 + (index as u32 - 232) * 10) as f32 / 255.0;
                Rgba { r: v, g: v, b: v, a: 1.0 }
            }
        }
    }
}

/// vte 8-bit RGB to paint-ready RGBA.
fn from_rgb(rgb_: Rgb) -> Rgba {
    Rgba {
        r: rgb_.r as f32 / 255.0,
        g: rgb_.g as f32 / 255.0,
        b: rgb_.b as f32 / 255.0,
        a: 1.0,
    }
}

/// The named theme presets (settings pills + Ctrl+Shift+T cycle). Index 0 is
/// the boot default.
pub struct ThemeRegistry {
    presets: Vec<Preset>,
}

pub struct Preset {
    /// Label shown on the settings pill.
    pub name: &'static str,
    pub theme: Theme,
}

impl ThemeRegistry {
    /// The shipped presets: neutral matte charcoal first (default), Warm
    /// Paper second.
    pub fn standard() -> Self {
        Self {
            presets: vec![
                Preset {
                    name: "Dark",
                    theme: Theme::dark(),
                },
                Preset {
                    name: "Warm Paper",
                    theme: Theme::cream(),
                },
            ],
        }
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    pub fn preset(&self, index: usize) -> &Preset {
        &self.presets[index.min(self.presets.len() - 1)]
    }

    /// Next preset in the cycle.
    pub fn next(&self, index: usize) -> usize {
        (index + 1) % self.presets.len()
    }
}
