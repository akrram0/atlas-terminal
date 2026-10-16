//! Atlas theme — one source of truth for every colour in the app.
//!
//! The default is "Warm Paper": a cream terminal sheet floating over a sand
//! chrome, with an editorial warm ANSI palette. "Warm Dark" mirrors it with
//! a matte-stone chrome and a warm charcoal sheet (Ctrl+Shift+T switches).

use alacritty_terminal::vte::ansi::{Color, NamedColor};
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

    /// Warm Dark: warm charcoal sheet (#1E1C19) under a matte-stone chrome
    /// (#181816), warm ivory ink, terracotta accent.
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x1E1C19),
            fg: rgb(0xE8E4DC),
            cursor: rgb(0xD08A5C),
            panel_bg: rgb(0x181816),
            elevated_bg: rgb(0x262624),
            elevated_border: rgba(0xFFFFFF14),
            chrome_fg: rgb(0x9C978C),
            chrome_fg_active: rgb(0xE8E4DC),
            faint_fg: rgb(0x7A7468),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF0A),
            card_active: rgba(0xE08A6226),
            accent: rgb(0xE08A62),
            good: rgb(0x9BAF7C),
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
    pub fn resolve(&self, color: Color, dim: bool) -> Rgba {
        let out = match color {
            Color::Named(named) => self.named(named),
            Color::Indexed(index) => self.indexed(index),
            Color::Spec(rgb_) => Rgba {
                r: rgb_.r as f32 / 255.0,
                g: rgb_.g as f32 / 255.0,
                b: rgb_.b as f32 / 255.0,
                a: 1.0,
            },
        };
        if dim {
            self.dimmed(out)
        } else {
            out
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
