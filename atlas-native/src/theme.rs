//! Atlas cream theme — warm, paper-like palette (one source of truth).
//!
//! All colors live here: chrome, ANSI palette, cursor. Nothing else in the
//! app hardcodes a color.

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
    /// Titlebar / chrome background (slightly darker than the terminal).
    pub chrome_bg: Rgba,
    /// Chrome text (inactive).
    pub chrome_fg: Rgba,
    /// Chrome text (active / hovered).
    pub chrome_fg_active: Rgba,
    /// 1px separators.
    pub hairline: Rgba,
    /// Active tab pill background.
    pub tab_active_bg: Rgba,
    /// Tab hover background.
    pub tab_hover_bg: Rgba,
    /// ANSI palette 0..16.
    pub ansi: [Rgba; 16],
}

impl Theme {
    pub fn cream() -> Self {
        Self {
            bg: rgb(0xFAF3E8),
            fg: rgb(0x3E3833),
            cursor: rgb(0x9A4A3A),
            chrome_bg: rgb(0xF4ECDD),
            chrome_fg: rgb(0x9A8F80),
            chrome_fg_active: rgb(0x3E3833),
            hairline: rgba(0x3E383314),
            tab_active_bg: rgba(0xFFFFFF99),
            tab_hover_bg: rgba(0x3E383310),
            ansi: [
                rgb(0x4A4440), // 0 black
                rgb(0xC14B3F), // 1 red
                rgb(0x5E8C4A), // 2 green
                rgb(0xB5883A), // 3 yellow
                rgb(0x3F6EA8), // 4 blue
                rgb(0x9A5BA8), // 5 magenta
                rgb(0x3E8E8E), // 6 cyan
                rgb(0x8E867C), // 7 white
                rgb(0x6B635A), // 8 bright black
                rgb(0xE05B4E), // 9 bright red
                rgb(0x6FA855), // 10 bright green
                rgb(0xD4A245), // 11 bright yellow
                rgb(0x4E85C9), // 12 bright blue
                rgb(0xB472C4), // 13 bright magenta
                rgb(0x4FA8A8), // 14 bright cyan
                rgb(0x2F2A25), // 15 bright white
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
