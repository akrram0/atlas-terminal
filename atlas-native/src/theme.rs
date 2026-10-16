//! Atlas theme — one source of truth for every colour in the app.
//!
//! The default is the dark terminal palette (near-black terminal, charcoal
//! chrome); a warm cream variant mirrors it for the light switch.

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
    /// Chrome surfaces: titlebar, sidebar, pane header, status bar.
    pub panel_bg: Rgba,
    /// Floating surfaces: new-tab button, settings popover.
    pub elevated_bg: Rgba,
    /// Hairline border for floating surfaces.
    pub elevated_border: Rgba,
    /// The titlebar search pill (darker than the bar, like the mock).
    pub search_bg: Rgba,
    /// Border of the search pill and its shortcut chip.
    pub search_border: Rgba,
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
    pub fn cream() -> Self {
        Self {
            bg: rgb(0xFAF3E8),
            fg: rgb(0x3E3833),
            cursor: rgb(0x9A4A3A),
            panel_bg: rgb(0xF4ECDD),
            elevated_bg: rgb(0xFBF7EF),
            elevated_border: rgba(0x3E38331F),
            search_bg: rgb(0xFDF9F1),
            search_border: rgba(0x3E383324),
            chrome_fg: rgb(0x9A8F80),
            chrome_fg_active: rgb(0x3E3833),
            faint_fg: rgb(0xB3A896),
            hairline: rgba(0x3E383314),
            card_hover: rgba(0x3E38330D),
            card_active: rgba(0xFFFFFFB3),
            accent: rgb(0x2F6FED),
            good: rgb(0x1A7F37),
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

    /// Default palette, matched to the reference design: blue-black terminal,
    /// slate chrome, GitHub-dark ANSI.
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x0C0F15),
            fg: rgb(0xC9D1D9),
            cursor: rgb(0xD8E1EA),
            panel_bg: rgb(0x161B22),
            elevated_bg: rgb(0x1F242C),
            elevated_border: rgb(0x2E3239),
            search_bg: rgb(0x0E1217),
            search_border: rgba(0xFFFFFF14),
            chrome_fg: rgb(0x9AA3B0),
            chrome_fg_active: rgb(0xE8EDF4),
            faint_fg: rgb(0x697380),
            hairline: rgba(0xFFFFFF0F),
            card_hover: rgb(0x1D232B),
            card_active: rgb(0x232A33),
            accent: rgb(0x4C8DF0),
            good: rgb(0x3FB950),
            ansi: [
                rgb(0x484F58), // 0 black
                rgb(0xFF7B72), // 1 red
                rgb(0x3FB950), // 2 green
                rgb(0xD29922), // 3 yellow
                rgb(0x58A6FF), // 4 blue
                rgb(0xBC8CFF), // 5 magenta
                rgb(0x39C5CF), // 6 cyan
                rgb(0xB1BAC4), // 7 white
                rgb(0x6E7681), // 8 bright black
                rgb(0xFFA198), // 9 bright red
                rgb(0x56D364), // 10 bright green
                rgb(0xE3B341), // 11 bright yellow
                rgb(0x79C0FF), // 12 bright blue
                rgb(0xD2A8FF), // 13 bright magenta
                rgb(0x56D4DD), // 14 bright cyan
                rgb(0xF0F6FC), // 15 bright white
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
