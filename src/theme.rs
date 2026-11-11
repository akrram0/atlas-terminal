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
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp_color(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: lerp_f32(a.r, b.r, t),
        g: lerp_f32(a.g, b.g, t),
        b: lerp_f32(a.b, b.b, t),
        a: lerp_f32(a.a, b.a, t),
    }
}

impl Theme {
    /// Smoothly interpolate between two themes across all tokens for zero-flash crossfade.
    pub fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut ansi = [from.ansi[0]; 16];
        for i in 0..16 {
            ansi[i] = lerp_color(from.ansi[i], to.ansi[i], t);
        }
        Self {
            bg: lerp_color(from.bg, to.bg, t),
            fg: lerp_color(from.fg, to.fg, t),
            cursor: lerp_color(from.cursor, to.cursor, t),
            panel_bg: lerp_color(from.panel_bg, to.panel_bg, t),
            elevated_bg: lerp_color(from.elevated_bg, to.elevated_bg, t),
            elevated_border: lerp_color(from.elevated_border, to.elevated_border, t),
            chrome_fg: lerp_color(from.chrome_fg, to.chrome_fg, t),
            chrome_fg_active: lerp_color(from.chrome_fg_active, to.chrome_fg_active, t),
            faint_fg: lerp_color(from.faint_fg, to.faint_fg, t),
            hairline: lerp_color(from.hairline, to.hairline, t),
            card_hover: lerp_color(from.card_hover, to.card_hover, t),
            card_active: lerp_color(from.card_active, to.card_active, t),
            accent: lerp_color(from.accent, to.accent, t),
            good: lerp_color(from.good, to.good, t),
            sheet_border: lerp_color(from.sheet_border, to.sheet_border, t),
            soft_bg: lerp_color(from.soft_bg, to.soft_bg, t),
            ansi,
        }
    }

    /// Returns true if this is a light theme based on background luminance (Rec. 709).
    #[inline]
    pub fn is_light(&self) -> bool {
        let lum = 0.2126 * self.bg.r + 0.7152 * self.bg.g + 0.0722 * self.bg.b;
        lum > 0.5
    }

    /// Apple macOS Light: pristine paper sheet (#FFFFFF) over System Gray chrome (#F2F2F7),
    /// deep carbon ink (#1D1D1F), terracotta rust accent (#C05A28), and a high-contrast
    /// ANSI palette tuned for crisp legibility in light mode.
    pub fn cream() -> Self {
        Self {
            bg: rgb(0xFFFFFF),
            fg: rgb(0x1D1D1F),
            cursor: rgb(0xC05A28),
            panel_bg: rgb(0xF2F2F7),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD1D1D6),
            chrome_fg: rgb(0x6E6E73),
            chrome_fg_active: rgb(0x1D1D1F),
            faint_fg: rgb(0x86868B),
            hairline: rgb(0xE5E5EA),
            card_hover: rgb(0xE8E8ED),
            card_active: rgba(0xC05A281E),
            accent: rgb(0xC05A28),
            good: rgb(0x1A7F37),
            sheet_border: rgb(0xD1D1D6),
            soft_bg: rgb(0xECECF0),
            ansi: [
                rgb(0x1D1D1F), // 0 black
                rgb(0xCF222E), // 1 red
                rgb(0x1A7F37), // 2 green (crisp high-contrast prompt)
                rgb(0x9A6700), // 3 yellow / amber
                rgb(0x0969DA), // 4 blue
                rgb(0x8250DF), // 5 magenta / purple
                rgb(0x0E7490), // 6 cyan / teal
                rgb(0x6E7781), // 7 white / light gray
                rgb(0x57606A), // 8 bright black / slate
                rgb(0xA40E26), // 9 bright red
                rgb(0x116329), // 10 bright green
                rgb(0x7D4E00), // 11 bright yellow
                rgb(0x0550AE), // 12 bright blue
                rgb(0x6639BA), // 13 bright magenta
                rgb(0x0A5C64), // 14 bright cyan
                rgb(0x24292F), // 15 bright white / ink
            ],
        }
    }

    /// Cohesive dark surface: #12141C outer bar, #1C1F2A sheet/tab pill,
    /// #8F96A8 muted text, #E6E9F0 active text.
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x1C1F2A),
            fg: rgb(0xE6E9F0),
            cursor: rgb(0xD08A5C),
            panel_bg: rgb(0x12141C),
            elevated_bg: rgb(0x1C1F2A),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x8F96A8),
            chrome_fg_active: rgb(0xE6E9F0),
            faint_fg: rgb(0x6C7486),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0xE08A6226),
            accent: rgb(0xE08A62),
            good: rgb(0x9BAF7C),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x12141C), // 0 black
                rgb(0xD06055), // 1 red
                rgb(0x9BAF7C), // 2 green
                rgb(0xD9A755), // 3 yellow
                rgb(0x82A7D0), // 4 blue
                rgb(0xC98A9A), // 5 magenta
                rgb(0x7DB8B0), // 6 cyan
                rgb(0xE6E9F0), // 7 white
                rgb(0x6C7486), // 8 bright black
                rgb(0xE07A6E), // 9 bright red
                rgb(0xB3C795), // 10 bright green
                rgb(0xEBC070), // 11 bright yellow
                rgb(0x9BBDDE), // 12 bright blue
                rgb(0xDBA6B2), // 13 bright magenta
                rgb(0x93CBC3), // 14 bright cyan
                rgb(0xF2F4F8), // 15 bright white
            ],
        }
    }

    /// Classic CRT terminal: pure-black sheet (#000000), neutral light-gray
    /// ink (#CCCCCC), white cursor, electric cyan (#00FFFF) accents — the
    /// look of a stock xterm/Windows console with a `1;36` prompt.
    pub fn classic() -> Self {
        Self {
            bg: rgb(0x000000),
            fg: rgb(0xCCCCCC),
            cursor: rgb(0xFFFFFF),
            panel_bg: rgb(0x0A0A0A),
            elevated_bg: rgb(0x1A1A1A),
            elevated_border: rgba(0xFFFFFF1A),
            chrome_fg: rgb(0x8A8A8A),
            chrome_fg_active: rgb(0xFFFFFF),
            faint_fg: rgb(0x666666),
            hairline: rgba(0xFFFFFF14),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x00FFFF1E),
            accent: rgb(0x00FFFF),
            good: rgb(0x0DBC79),
            sheet_border: rgba(0xFFFFFF18),
            soft_bg: rgba(0xFFFFFF10),
            ansi: [
                rgb(0x0C0C0C), // 0 black
                rgb(0xCD3131), // 1 red
                rgb(0x0DBC79), // 2 green
                rgb(0xE5E510), // 3 yellow
                rgb(0x2472C8), // 4 blue
                rgb(0xBC3FBC), // 5 magenta
                rgb(0x00FFFF), // 6 cyan — bold `1;36` prompt paths
                rgb(0xCCCCCC), // 7 white — `1;37` hostname
                rgb(0x808080), // 8 bright black — `90` timestamps
                rgb(0xF14C4C), // 9 bright red
                rgb(0x23D18B), // 10 bright green
                rgb(0xF5F543), // 11 bright yellow
                rgb(0x3B8EEA), // 12 bright blue
                rgb(0xD670D6), // 13 bright magenta
                rgb(0x00FFFF), // 14 bright cyan
                rgb(0xFFFFFF), // 15 bright white
            ],
        }
    }

    /// Classic flipped for daylight: crisp white sheet (#FFFFFF), near-black
    /// ink, classic blue (#0969DA) accent, and a high-contrast ANSI set.
    pub fn classic_light() -> Self {
        Self {
            bg: rgb(0xFFFFFF),
            fg: rgb(0x1F1F1F),
            cursor: rgb(0x0969DA),
            panel_bg: rgb(0xF2F2F4),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD8D8D8),
            chrome_fg: rgb(0x6B6B6B),
            chrome_fg_active: rgb(0x1F1F1F),
            faint_fg: rgb(0x8F8F8F),
            hairline: rgb(0xE8E8E8),
            card_hover: rgb(0xE8E8EB),
            card_active: rgba(0x0969DA1A),
            accent: rgb(0x0969DA),
            good: rgb(0x13A10E),
            sheet_border: rgb(0xD8D8D8),
            soft_bg: rgb(0xECECEF),
            ansi: [
                rgb(0x1F1F1F), rgb(0xC50F1F), rgb(0x13A10E), rgb(0xC19C00),
                rgb(0x0969DA), rgb(0x881798), rgb(0x0E7490), rgb(0x6E7781),
                rgb(0x555555), rgb(0xE74856), rgb(0x16C60C), rgb(0xD9A406),
                rgb(0x3B8EEA), rgb(0xB4009E), rgb(0x1295A3), rgb(0x000000),
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

#[cfg(windows)]
pub fn is_system_light_mode() -> bool {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            hkey: isize,
            sub_key: *const u16,
            options: u32,
            sam_desired: u32,
            result: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: isize,
            value_name: *const u16,
            reserved: *mut u32,
            type_: *mut u32,
            data: *mut u8,
            cb_data: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }
    const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let val_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    unsafe {
        let mut key: isize = 0;
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) == 0 {
            let mut val: u32 = 0;
            let mut size: u32 = std::mem::size_of::<u32>() as u32;
            let mut val_type: u32 = 0;
            let res = RegQueryValueExW(
                key,
                val_name.as_ptr(),
                std::ptr::null_mut(),
                &mut val_type,
                &mut val as *mut u32 as *mut u8,
                &mut size,
            );
            let _ = RegCloseKey(key);
            if res == 0 {
                return val != 0;
            }
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_system_light_mode() -> bool {
    false
}

impl Theme {
    pub fn tokyo_night_dark() -> Self {
        Self {
            bg: rgb(0x1A1B26),
            fg: rgb(0xC0CAF5),
            cursor: rgb(0x7AA2F7),
            panel_bg: rgb(0x16161E),
            elevated_bg: rgb(0x24283B),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x7982A9),
            chrome_fg_active: rgb(0xC0CAF5),
            faint_fg: rgb(0x565F89),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x7AA2F726),
            accent: rgb(0x7AA2F7),
            good: rgb(0x9ECE6A),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x15161E), rgb(0xF7768E), rgb(0x9ECE6A), rgb(0xE0AF68),
                rgb(0x7AA2F7), rgb(0xBB9AF7), rgb(0x7DCFFF), rgb(0xA9B1D6),
                rgb(0x414868), rgb(0xF7768E), rgb(0x9ECE6A), rgb(0xE0AF68),
                rgb(0x7AA2F7), rgb(0xBB9AF7), rgb(0x7DCFFF), rgb(0xC0CAF5),
            ],
        }
    }

    pub fn tokyo_night_light() -> Self {
        Self {
            bg: rgb(0xF7F8FC),
            fg: rgb(0x343B58),
            cursor: rgb(0x2E7DE9),
            panel_bg: rgb(0xEBEDF5),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD5D8E8),
            chrome_fg: rgb(0x6172B0),
            chrome_fg_active: rgb(0x343B58),
            faint_fg: rgb(0x8990B3),
            hairline: rgb(0xE5E8F4),
            card_hover: rgb(0xE1E3EB),
            card_active: rgba(0x2E7DE91E),
            accent: rgb(0x2E7DE9),
            good: rgb(0x387038),
            sheet_border: rgb(0xD5D8E8),
            soft_bg: rgb(0xE6E8F0),
            ansi: [
                rgb(0x343B58), rgb(0x8C4351), rgb(0x387038), rgb(0x8F5E15),
                rgb(0x2E7DE9), rgb(0x7847BD), rgb(0x166775), rgb(0x68707F),
                rgb(0x4F5B66), rgb(0xB15C6B), rgb(0x448544), rgb(0x9D6A1F),
                rgb(0x3B8BF2), rgb(0x8C5ECF), rgb(0x217E8F), rgb(0x1D202F),
            ],
        }
    }

    pub fn catppuccin_dark() -> Self {
        Self {
            bg: rgb(0x24273A),
            fg: rgb(0xCAD3F5),
            cursor: rgb(0xF5A97F),
            panel_bg: rgb(0x1E2030),
            elevated_bg: rgb(0x363A4F),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x8087A2),
            chrome_fg_active: rgb(0xCAD3F5),
            faint_fg: rgb(0x5B6078),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0xF5A97F26),
            accent: rgb(0xF5A97F),
            good: rgb(0xA6DA95),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x494D64), rgb(0xED8796), rgb(0xA6DA95), rgb(0xEED49F),
                rgb(0x8AADF4), rgb(0xF5BDE6), rgb(0x8BD5CA), rgb(0xB8C0E0),
                rgb(0x5B6078), rgb(0xED8796), rgb(0xA6DA95), rgb(0xEED49F),
                rgb(0x8AADF4), rgb(0xF5BDE6), rgb(0x8BD5CA), rgb(0xCAD3F5),
            ],
        }
    }

    pub fn catppuccin_light() -> Self {
        Self {
            bg: rgb(0xFAFAFC),
            fg: rgb(0x4C4F69),
            cursor: rgb(0xEA76CB),
            panel_bg: rgb(0xEFF1F5),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xCCD0DA),
            chrome_fg: rgb(0x7C7F93),
            chrome_fg_active: rgb(0x4C4F69),
            faint_fg: rgb(0x9CA0B0),
            hairline: rgb(0xDCE0E8),
            card_hover: rgb(0xE5E7EB),
            card_active: rgba(0x8839EF1E),
            accent: rgb(0x8839EF),
            good: rgb(0x40A02B),
            sheet_border: rgb(0xCCD0DA),
            soft_bg: rgb(0xEAECF0),
            ansi: [
                rgb(0x4C4F69), rgb(0xD20F39), rgb(0x40A02B), rgb(0xDF8E1D),
                rgb(0x1E66F5), rgb(0x8839EF), rgb(0x179299), rgb(0x9CA0B0),
                rgb(0x6C6F85), rgb(0xE64553), rgb(0x348022), rgb(0xB87010),
                rgb(0x2A72F8), rgb(0x9B4EFA), rgb(0x147E84), rgb(0x2A2B3D),
            ],
        }
    }

    pub fn nord_dark() -> Self {
        Self {
            bg: rgb(0x2E3440),
            fg: rgb(0xECEFF4),
            cursor: rgb(0x88C0D0),
            panel_bg: rgb(0x242933),
            elevated_bg: rgb(0x3B4252),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x9099AB),
            chrome_fg_active: rgb(0xECEFF4),
            faint_fg: rgb(0x4C566A),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x88C0D026),
            accent: rgb(0x88C0D0),
            good: rgb(0xA3BE8C),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x3B4252), rgb(0xBF616A), rgb(0xA3BE8C), rgb(0xEBCB8B),
                rgb(0x81A1C1), rgb(0xB48EAD), rgb(0x88C0D0), rgb(0xE5E9F0),
                rgb(0x4C566A), rgb(0xBF616A), rgb(0xA3BE8C), rgb(0xEBCB8B),
                rgb(0x81A1C1), rgb(0xB48EAD), rgb(0x8FBCBB), rgb(0xECEFF4),
            ],
        }
    }

    pub fn nord_light() -> Self {
        Self {
            bg: rgb(0xF8FAFC),
            fg: rgb(0x2E3440),
            cursor: rgb(0x5E81AC),
            panel_bg: rgb(0xEDF1F7),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD8DEE9),
            chrome_fg: rgb(0x4C566A),
            chrome_fg_active: rgb(0x2E3440),
            faint_fg: rgb(0x7B88A1),
            hairline: rgb(0xE5E9F0),
            card_hover: rgb(0xE3E7EE),
            card_active: rgba(0x5E81AC1E),
            accent: rgb(0x5E81AC),
            good: rgb(0x4E7A36),
            sheet_border: rgb(0xD8DEE9),
            soft_bg: rgb(0xE8ECF2),
            ansi: [
                rgb(0x2E3440), rgb(0x9E3B44), rgb(0x4E7A36), rgb(0x9F7C2C),
                rgb(0x5E81AC), rgb(0x84557A), rgb(0x497F87), rgb(0x768297),
                rgb(0x4C566A), rgb(0xB34852), rgb(0x5C8E41), rgb(0xB28C33),
                rgb(0x6E93C0), rgb(0x98668E), rgb(0x55929B), rgb(0x1E222A),
            ],
        }
    }

    pub fn github_light() -> Self {
        Self {
            bg: rgb(0xFFFFFF),
            fg: rgb(0x24292F),
            cursor: rgb(0x0969DA),
            panel_bg: rgb(0xF6F8FA),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD0D7DE),
            chrome_fg: rgb(0x57606A),
            chrome_fg_active: rgb(0x24292F),
            faint_fg: rgb(0x8C959F),
            hairline: rgb(0xAFB8C1),
            card_hover: rgb(0xECEEF0),
            card_active: rgba(0x0969DA1A),
            accent: rgb(0x0969DA),
            good: rgb(0x1A7F37),
            sheet_border: rgb(0xD0D7DE),
            soft_bg: rgb(0xF0F2F5),
            ansi: [
                rgb(0x24292F), rgb(0xCF222E), rgb(0x1A7F37), rgb(0x9A6700),
                rgb(0x0969DA), rgb(0x8250DF), rgb(0x1B7C83), rgb(0x6E7781),
                rgb(0x57606A), rgb(0xA40E26), rgb(0x116329), rgb(0x633C01),
                rgb(0x0550AE), rgb(0x5A32A3), rgb(0x095D63), rgb(0x24292F),
            ],
        }
    }

    pub fn github_dark() -> Self {
        Self {
            bg: rgb(0x0D1117),
            fg: rgb(0xC9D1D9),
            cursor: rgb(0x58A6FF),
            panel_bg: rgb(0x010409),
            elevated_bg: rgb(0x161B22),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x8B949E),
            chrome_fg_active: rgb(0xF0F6FC),
            faint_fg: rgb(0x484F58),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x58A6FF26),
            accent: rgb(0x58A6FF),
            good: rgb(0x3FB950),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x484F58), rgb(0xFF7B72), rgb(0x3FB950), rgb(0xD29922),
                rgb(0x58A6FF), rgb(0xBC8CFF), rgb(0x39C5CF), rgb(0xB1BAC4),
                rgb(0x6E7681), rgb(0xFFA198), rgb(0x56D364), rgb(0xE3B341),
                rgb(0x79C0FF), rgb(0xD2A8FF), rgb(0x56D4DD), rgb(0xF0F6FC),
            ],
        }
    }

    pub fn one_light() -> Self {
        Self {
            bg: rgb(0xFAFAFA),
            fg: rgb(0x383A42),
            cursor: rgb(0x4078F2),
            panel_bg: rgb(0xF0F0F0),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xDBDBDC),
            chrome_fg: rgb(0x696C77),
            chrome_fg_active: rgb(0x383A42),
            faint_fg: rgb(0xA0A1A7),
            hairline: rgb(0xEAEAEB),
            card_hover: rgb(0xE6E6E6),
            card_active: rgba(0x4078F21A),
            accent: rgb(0x4078F2),
            good: rgb(0x50A14F),
            sheet_border: rgb(0xDBDBDC),
            soft_bg: rgb(0xEBEBEB),
            ansi: [
                rgb(0x383A42), rgb(0xE45649), rgb(0x50A14F), rgb(0xC18401),
                rgb(0x4078F2), rgb(0xA626A4), rgb(0x0184BC), rgb(0xA0A1A7),
                rgb(0x696C77), rgb(0xE05243), rgb(0x469545), rgb(0xA87200),
                rgb(0x346CD8), rgb(0x982196), rgb(0x0172A3), rgb(0x202227),
            ],
        }
    }

    pub fn one_dark() -> Self {
        Self {
            bg: rgb(0x282C34),
            fg: rgb(0xABB2BF),
            cursor: rgb(0x61AFEF),
            panel_bg: rgb(0x21252B),
            elevated_bg: rgb(0x2C313A),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x828997),
            chrome_fg_active: rgb(0xABB2BF),
            faint_fg: rgb(0x5C6370),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x61AFEF26),
            accent: rgb(0x61AFEF),
            good: rgb(0x98C379),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x3F4451), rgb(0xE06C75), rgb(0x98C379), rgb(0xE5C07B),
                rgb(0x61AFEF), rgb(0xC678DD), rgb(0x56B6C2), rgb(0xABB2BF),
                rgb(0x4B5263), rgb(0xBE5046), rgb(0x7EB35B), rgb(0xD19A66),
                rgb(0x4D9DE0), rgb(0xB25EC9), rgb(0x439EA9), rgb(0xE6EDF3),
            ],
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            bg: rgb(0xFDF6E3),
            fg: rgb(0x586E75),
            cursor: rgb(0x268BD2),
            panel_bg: rgb(0xEEE8D5),
            elevated_bg: rgb(0xFFFAF0),
            elevated_border: rgb(0xDACFBB),
            chrome_fg: rgb(0x657B83),
            chrome_fg_active: rgb(0x073642),
            faint_fg: rgb(0x93A1A1),
            hairline: rgb(0xE8E0CE),
            card_hover: rgb(0xE5DFC9),
            card_active: rgba(0x268BD21A),
            accent: rgb(0x268BD2),
            good: rgb(0x859900),
            sheet_border: rgb(0xDACFBB),
            soft_bg: rgb(0xEAE4D0),
            ansi: [
                rgb(0x073642), rgb(0xDC322F), rgb(0x859900), rgb(0xB58900),
                rgb(0x268BD2), rgb(0xD33682), rgb(0x2AA198), rgb(0x93A1A1),
                rgb(0x657B83), rgb(0xCB4B16), rgb(0x718200), rgb(0x987300),
                rgb(0x1F70A8), rgb(0xB8286E), rgb(0x207E77), rgb(0x002B36),
            ],
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            bg: rgb(0x002B36),
            fg: rgb(0x839496),
            cursor: rgb(0x268BD2),
            panel_bg: rgb(0x073642),
            elevated_bg: rgb(0x0A3F4E),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x657B83),
            chrome_fg_active: rgb(0x93A1A1),
            faint_fg: rgb(0x586E75),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0x268BD226),
            accent: rgb(0x268BD2),
            good: rgb(0x859900),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x073642), rgb(0xDC322F), rgb(0x859900), rgb(0xB58900),
                rgb(0x268BD2), rgb(0xD33682), rgb(0x2AA198), rgb(0x93A1A1),
                rgb(0x657B83), rgb(0xCB4B16), rgb(0x718200), rgb(0x987300),
                rgb(0x1F70A8), rgb(0xB8286E), rgb(0x207E77), rgb(0xFDF6E3),
            ],
        }
    }

    /// Gruvbox Dark: retro groove warm earthy palette designed for extreme eye comfort.
    pub fn gruvbox_dark() -> Self {
        Self {
            bg: rgb(0x282828),
            fg: rgb(0xEBDBB2),
            cursor: rgb(0xFE8019),
            panel_bg: rgb(0x1D2021),
            elevated_bg: rgb(0x32302F),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0xA89984),
            chrome_fg_active: rgb(0xEBDBB2),
            faint_fg: rgb(0x928374),
            hairline: rgba(0xFFFFFF12),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0xFE801926),
            accent: rgb(0xFE8019),
            good: rgb(0xB8BB26),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x282828), rgb(0xCC241D), rgb(0x98971A), rgb(0xD79921),
                rgb(0x458588), rgb(0xB16286), rgb(0x689D6A), rgb(0xA89984),
                rgb(0x928374), rgb(0xFB4934), rgb(0xB8BB26), rgb(0xFABD2F),
                rgb(0x83A598), rgb(0xD3869B), rgb(0x8EC07C), rgb(0xEBDBB2),
            ],
        }
    }

    /// Gruvbox Light: warm vintage cream paper sheet with high-contrast espresso ink.
    pub fn gruvbox_light() -> Self {
        Self {
            bg: rgb(0xFBF1C7),
            fg: rgb(0x3C3836),
            cursor: rgb(0xAF3A03),
            panel_bg: rgb(0xEBDBB2),
            elevated_bg: rgb(0xF2E5BC),
            elevated_border: rgb(0xD5C4A1),
            chrome_fg: rgb(0x7C6F64),
            chrome_fg_active: rgb(0x3C3836),
            faint_fg: rgb(0x928374),
            hairline: rgb(0xD5C4A1),
            card_hover: rgb(0xEBDBB2),
            card_active: rgba(0xAF3A031E),
            accent: rgb(0xAF3A03),
            good: rgb(0x79740E),
            sheet_border: rgb(0xD5C4A1),
            soft_bg: rgb(0xEBDBB2),
            ansi: [
                rgb(0x3C3836), rgb(0xCC241D), rgb(0x79740E), rgb(0xB57614),
                rgb(0x076678), rgb(0x8F3F71), rgb(0x427B58), rgb(0x7C6F64),
                rgb(0x928374), rgb(0x9D0006), rgb(0x79740E), rgb(0xB57614),
                rgb(0x076678), rgb(0x8F3F71), rgb(0x427B58), rgb(0x282828),
            ],
        }
    }

    /// Barbie Dark: synthwave cyber-pink dream on deep nocturnal amethyst velvet.
    pub fn barbie_dark() -> Self {
        Self {
            bg: rgb(0x1A0B2E),
            fg: rgb(0xFCE7F3),
            cursor: rgb(0xFF2A85),
            panel_bg: rgb(0x120622),
            elevated_bg: rgb(0x2A1647),
            elevated_border: rgba(0xFF2A8538),
            chrome_fg: rgb(0xC084FC),
            chrome_fg_active: rgb(0xFCE7F3),
            faint_fg: rgb(0x81689D),
            hairline: rgba(0xFF2A8526),
            card_hover: rgba(0xFF2A851A),
            card_active: rgba(0xFF2A8533),
            accent: rgb(0xFF2A85),
            good: rgb(0x00F5D4),
            sheet_border: rgba(0xFF2A852E),
            soft_bg: rgba(0xFF2A8514),
            ansi: [
                rgb(0x1A0B2E), rgb(0xFF2A85), rgb(0x00F5D4), rgb(0xFEE440),
                rgb(0x7B2CBF), rgb(0xF72585), rgb(0x4CC9F0), rgb(0xFCE7F3),
                rgb(0x5A3870), rgb(0xFF5C9D), rgb(0x52FCDA), rgb(0xFFE970),
                rgb(0x9D4EDD), rgb(0xFF66B2), rgb(0x72DDF7), rgb(0xFFFFFF),
            ],
        }
    }

    /// Barbie Light: soft pastel dreamhouse blush paper with deep berry ink & hot magenta accent.
    pub fn barbie_light() -> Self {
        Self {
            bg: rgb(0xFFF0F5),
            fg: rgb(0x4A154B),
            cursor: rgb(0xFF1493),
            panel_bg: rgb(0xFCE4EC),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xF8BBD0),
            chrome_fg: rgb(0x880E4F),
            chrome_fg_active: rgb(0x4A154B),
            faint_fg: rgb(0xAD1457),
            hairline: rgb(0xF8BBD0),
            card_hover: rgb(0xF8BBD0),
            card_active: rgba(0xFF14931E),
            accent: rgb(0xFF1493),
            good: rgb(0x2E7D32),
            sheet_border: rgb(0xF8BBD0),
            soft_bg: rgb(0xFCE4EC),
            ansi: [
                rgb(0x4A154B), rgb(0xD81B60), rgb(0x2E7D32), rgb(0xC2185B),
                rgb(0x8E24AA), rgb(0xFF1493), rgb(0x00897B), rgb(0x880E4F),
                rgb(0xAD1457), rgb(0xE91E63), rgb(0x43A047), rgb(0xD81B60),
                rgb(0xAB47BC), rgb(0xFF4081), rgb(0x26A69A), rgb(0x1A031B),
            ],
        }
    }

    /// Osaka Jade: lush deep obsidian jade cyberpunk with emerald and sage highlights.
    pub fn osaka_jade() -> Self {
        Self {
            bg: rgb(0x0C1816),
            fg: rgb(0xE2F1ED),
            cursor: rgb(0x10B981),
            panel_bg: rgb(0x08100F),
            elevated_bg: rgb(0x152A26),
            elevated_border: rgba(0x10B98133),
            chrome_fg: rgb(0x6EE7B7),
            chrome_fg_active: rgb(0xE2F1ED),
            faint_fg: rgb(0x3E6B62),
            hairline: rgba(0x10B9811F),
            card_hover: rgba(0x10B9811A),
            card_active: rgba(0x10B98133),
            accent: rgb(0x10B981),
            good: rgb(0x34D399),
            sheet_border: rgba(0x10B98126),
            soft_bg: rgba(0x10B98114),
            ansi: [
                rgb(0x0C1816), rgb(0xE06C75), rgb(0x10B981), rgb(0xE5C07B),
                rgb(0x2DD4BF), rgb(0xA78BFA), rgb(0x14B8A6), rgb(0xCCDEDA),
                rgb(0x3E6B62), rgb(0xF87171), rgb(0x34D399), rgb(0xFBBF24),
                rgb(0x5EEAD4), rgb(0xC4B5FD), rgb(0x2DD4BF), rgb(0xF0FDF4),
            ],
        }
    }

    /// Osaka Light: soothing pale mint paper with deep forest emerald ink.
    pub fn osaka_light() -> Self {
        Self {
            bg: rgb(0xF0FDF4),
            fg: rgb(0x064E3B),
            cursor: rgb(0x059669),
            panel_bg: rgb(0xDCFCE7),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xA7F3D0),
            chrome_fg: rgb(0x047857),
            chrome_fg_active: rgb(0x064E3B),
            faint_fg: rgb(0x6EE7B7),
            hairline: rgb(0xA7F3D0),
            card_hover: rgb(0xDCFCE7),
            card_active: rgba(0x0596691E),
            accent: rgb(0x059669),
            good: rgb(0x047857),
            sheet_border: rgb(0xA7F3D0),
            soft_bg: rgb(0xDCFCE7),
            ansi: [
                rgb(0x064E3B), rgb(0xDC2626), rgb(0x059669), rgb(0xD97706),
                rgb(0x0D9488), rgb(0x7C3AED), rgb(0x0284C7), rgb(0x34D399),
                rgb(0x047857), rgb(0xEF4444), rgb(0x10B981), rgb(0xF59E0B),
                rgb(0x14B8A6), rgb(0x8B5CF6), rgb(0x0EA5E9), rgb(0x022C22),
            ],
        }
    }

    /// Rosé Pine: soothing natural all-pine dark palette engineered for minimum eye fatigue.
    pub fn rose_pine() -> Self {
        Self {
            bg: rgb(0x191724),
            fg: rgb(0xE0DEF4),
            cursor: rgb(0xEB6F92),
            panel_bg: rgb(0x14121F),
            elevated_bg: rgb(0x26233A),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x908CAA),
            chrome_fg_active: rgb(0xE0DEF4),
            faint_fg: rgb(0x6E6A86),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0xEB6F9226),
            accent: rgb(0xEB6F92),
            good: rgb(0x9CCFD8),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x26233A), rgb(0xEB6F92), rgb(0x31748F), rgb(0xF6C177),
                rgb(0x9CCFD8), rgb(0xC4A7E7), rgb(0xEBBCBA), rgb(0xE0DEF4),
                rgb(0x6E6A86), rgb(0xEB6F92), rgb(0x31748F), rgb(0xF6C177),
                rgb(0x9CCFD8), rgb(0xC4A7E7), rgb(0xEBBCBA), rgb(0xE0DEF4),
            ],
        }
    }

    /// Rosé Pine Dawn: dusty iris and rose palette over soft linen morning surface.
    pub fn rose_pine_dawn() -> Self {
        Self {
            bg: rgb(0xFAF4ED),
            fg: rgb(0x575279),
            cursor: rgb(0xB4637A),
            panel_bg: rgb(0xF2E9DE),
            elevated_bg: rgb(0xFFFAF3),
            elevated_border: rgb(0xCECACD),
            chrome_fg: rgb(0x797593),
            chrome_fg_active: rgb(0x575279),
            faint_fg: rgb(0x9893A5),
            hairline: rgb(0xE5DFD5),
            card_hover: rgb(0xE8DFD3),
            card_active: rgba(0xB4637A1E),
            accent: rgb(0xB4637A),
            good: rgb(0x286983),
            sheet_border: rgb(0xCECACD),
            soft_bg: rgb(0xF2E9DE),
            ansi: [
                rgb(0x575279), rgb(0xB4637A), rgb(0x286983), rgb(0xEA9D34),
                rgb(0x56949F), rgb(0x907AA9), rgb(0xD7827E), rgb(0x797593),
                rgb(0x9893A5), rgb(0xB4637A), rgb(0x286983), rgb(0xEA9D34),
                rgb(0x56949F), rgb(0x907AA9), rgb(0xD7827E), rgb(0x26233A),
            ],
        }
    }

    /// Dracula: legendary dark palette with purple, pink and cyan highlights.
    pub fn dracula() -> Self {
        Self {
            bg: rgb(0x282A36),
            fg: rgb(0xF8F8F2),
            cursor: rgb(0xFF79C6),
            panel_bg: rgb(0x21222C),
            elevated_bg: rgb(0x343746),
            elevated_border: rgba(0xFFFFFF18),
            chrome_fg: rgb(0x6272A4),
            chrome_fg_active: rgb(0xF8F8F2),
            faint_fg: rgb(0x6272A4),
            hairline: rgba(0xFFFFFF10),
            card_hover: rgba(0xFFFFFF10),
            card_active: rgba(0xBD93F926),
            accent: rgb(0xBD93F9),
            good: rgb(0x50FA7B),
            sheet_border: rgba(0xFFFFFF14),
            soft_bg: rgba(0xFFFFFF0F),
            ansi: [
                rgb(0x21222C), rgb(0xFF5555), rgb(0x50FA7B), rgb(0xF1FA8C),
                rgb(0xBD93F9), rgb(0xFF79C6), rgb(0x8BE9FD), rgb(0xF8F8F2),
                rgb(0x6272A4), rgb(0xFF6E6E), rgb(0x69FF94), rgb(0xFFFFA5),
                rgb(0xD6ACFF), rgb(0xFF92DF), rgb(0xA4FFFF), rgb(0xFFFFFF),
            ],
        }
    }

    /// Dracula Light: clean high-contrast cream light mode with Dracula violet accents.
    pub fn dracula_light() -> Self {
        Self {
            bg: rgb(0xF8F8F2),
            fg: rgb(0x282A36),
            cursor: rgb(0xBD93F9),
            panel_bg: rgb(0xECECE5),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgb(0xD5D5CE),
            chrome_fg: rgb(0x6272A4),
            chrome_fg_active: rgb(0x282A36),
            faint_fg: rgb(0x9EA4B8),
            hairline: rgb(0xD5D5CE),
            card_hover: rgb(0xECECE5),
            card_active: rgba(0xBD93F91E),
            accent: rgb(0xBD93F9),
            good: rgb(0x279646),
            sheet_border: rgb(0xD5D5CE),
            soft_bg: rgb(0xECECE5),
            ansi: [
                rgb(0x282A36), rgb(0xD13636), rgb(0x279646), rgb(0xB39B00),
                rgb(0x7C4DFF), rgb(0xC2185B), rgb(0x00838F), rgb(0x6272A4),
                rgb(0x9EA4B8), rgb(0xE53935), rgb(0x388E3C), rgb(0xC0CA33),
                rgb(0x8E24AA), rgb(0xE91E63), rgb(0x00ACC1), rgb(0x1E1F29),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeModeSupport {
    Both,
    DarkOnly,
    LightOnly,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub dark: Theme,
    pub light: Theme,
    pub is_custom: bool,
    pub theme_mode: ThemeModeSupport,
}

impl Preset {
    /// Legacy theme reference for code expecting `preset.theme`.
    #[allow(dead_code)]
    pub fn theme(&self) -> &Theme {
        &self.dark
    }

    pub fn supports_dark(&self) -> bool {
        self.theme_mode != ThemeModeSupport::LightOnly
    }

    pub fn supports_light(&self) -> bool {
        self.theme_mode != ThemeModeSupport::DarkOnly
    }

    pub fn dark_name(&self) -> &str {
        match self.id.as_str() {
            "catppuccin" => "Catppuccin Mocha",
            "gruvbox" => "Gruvbox Dark",
            "barbie" => "Barbie Dark",
            "osaka_jade" => "Osaka Jade",
            "dracula" => "Dracula",
            "rose_pine" => "Rosé Pine",
            "solarized" => "Solarized Dark",
            "github" => "GitHub Dark",
            "tokyo_night" => "Tokyo Night",
            "nord" => "Nord Dark",
            "classic" => "Classic Black",
            "atlas" => "Atlas Dark",
            "one" => "One Dark",
            _ => &self.name,
        }
    }

    pub fn light_name(&self) -> &str {
        match self.id.as_str() {
            "solarized" => "Solarized Light",
            "barbie" => "Barbie Light",
            "gruvbox" => "Gruvbox Light",
            "rose_pine" => "Rosé Pine Dawn",
            "catppuccin" => "Catppuccin Latte",
            "osaka_jade" => "Osaka Mint",
            "dracula" => "Dracula Light",
            "github" => "GitHub Light",
            "classic" => "Classic Light",
            "atlas" => "macOS Cream",
            "nord" => "Nord Light",
            "tokyo_night" => "Tokyo Night Day",
            "one" => "One Light",
            _ => &self.name,
        }
    }
}

pub fn parse_hex(hex: &str) -> Option<Rgba> {
    let s = hex.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)))
    } else if s.len() == 8 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        let a = u8::from_str_radix(&s[6..8], 16).ok()?;
        Some(Rgba {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        })
    } else if s.len() == 3 {
        let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
        Some(rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)))
    } else {
        None
    }
}

pub fn to_hex(c: Rgba) -> String {
    let r = (c.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (c.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (c.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    if (c.a - 1.0).abs() < 0.005 {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    } else {
        let a = (c.a.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }
}

pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomThemeFile {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub fg: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,

    // Windows Terminal / iTerm color schemes compatibility
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub foreground: Option<String>,
    #[serde(default, rename = "cursorColor")]
    pub cursor_color: Option<String>,
    #[serde(default, rename = "selectionBackground")]
    pub selection_background: Option<String>,

    #[serde(default)]
    pub panel_bg: Option<String>,
    #[serde(default)]
    pub elevated_bg: Option<String>,
    #[serde(default)]
    pub elevated_border: Option<String>,
    #[serde(default)]
    pub chrome_fg: Option<String>,
    #[serde(default)]
    pub chrome_fg_active: Option<String>,
    #[serde(default)]
    pub faint_fg: Option<String>,
    #[serde(default)]
    pub hairline: Option<String>,
    #[serde(default)]
    pub card_hover: Option<String>,
    #[serde(default)]
    pub card_active: Option<String>,
    #[serde(default)]
    pub good: Option<String>,
    #[serde(default)]
    pub sheet_border: Option<String>,
    #[serde(default)]
    pub soft_bg: Option<String>,

    #[serde(default)]
    pub ansi: Option<Vec<String>>,

    #[serde(default)]
    pub black: Option<String>,
    #[serde(default)]
    pub red: Option<String>,
    #[serde(default)]
    pub green: Option<String>,
    #[serde(default)]
    pub yellow: Option<String>,
    #[serde(default)]
    pub blue: Option<String>,
    #[serde(default)]
    pub purple: Option<String>,
    #[serde(default)]
    pub cyan: Option<String>,
    #[serde(default)]
    pub white: Option<String>,
    #[serde(default, rename = "brightBlack")]
    pub bright_black: Option<String>,
    #[serde(default, rename = "brightRed")]
    pub bright_red: Option<String>,
    #[serde(default, rename = "brightGreen")]
    pub bright_green: Option<String>,
    #[serde(default, rename = "brightYellow")]
    pub bright_yellow: Option<String>,
    #[serde(default, rename = "brightBlue")]
    pub bright_blue: Option<String>,
    #[serde(default, rename = "brightPurple")]
    pub bright_purple: Option<String>,
    #[serde(default, rename = "brightCyan")]
    pub bright_cyan: Option<String>,
    #[serde(default, rename = "brightWhite")]
    pub bright_white: Option<String>,
}

impl CustomThemeFile {
    pub fn to_preset(&self) -> Preset {
        let name = self.name.clone();
        let id = self.id.clone().unwrap_or_else(|| slugify(&name));
        let desc = self.description.clone().unwrap_or_else(|| format!("User imported custom theme: {}", name));

        let bg = self.bg.as_deref()
            .or(self.background.as_deref())
            .and_then(parse_hex)
            .unwrap_or(rgb(0x1A1B26));
        let fg = self.fg.as_deref()
            .or(self.foreground.as_deref())
            .and_then(parse_hex)
            .unwrap_or(rgb(0xC0CAF5));
        let cursor = self.cursor.as_deref()
            .or(self.cursor_color.as_deref())
            .and_then(parse_hex)
            .unwrap_or(fg);
        let accent = self.accent.as_deref()
            .or(self.cursor_color.as_deref())
            .or(self.selection_background.as_deref())
            .and_then(parse_hex)
            .unwrap_or(cursor);

        let lum = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
        let is_light = lum > 0.5;

        let mut ansi = [fg; 16];
        let fallback_ansi = if is_light {
            Theme::solarized_light().ansi
        } else {
            Theme::catppuccin_dark().ansi
        };
        ansi.copy_from_slice(&fallback_ansi);

        if let Some(list) = &self.ansi {
            for (i, hex_str) in list.iter().take(16).enumerate() {
                if let Some(c) = parse_hex(hex_str) {
                    ansi[i] = c;
                }
            }
        }
        let wt_colors = [
            (&self.black, 0), (&self.red, 1), (&self.green, 2), (&self.yellow, 3),
            (&self.blue, 4), (&self.purple, 5), (&self.cyan, 6), (&self.white, 7),
            (&self.bright_black, 8), (&self.bright_red, 9), (&self.bright_green, 10), (&self.bright_yellow, 11),
            (&self.bright_blue, 12), (&self.bright_purple, 13), (&self.bright_cyan, 14), (&self.bright_white, 15),
        ];
        for (field, idx) in wt_colors {
            if let Some(c) = field.as_deref().and_then(parse_hex) {
                ansi[idx] = c;
            }
        }

        let panel_bg = self.panel_bg.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xF2F2F7) } else { rgb(0x12141C) }
        });
        let elevated_bg = self.elevated_bg.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xFFFFFF) } else { rgb(0x1C1F2A) }
        });
        let elevated_border = self.elevated_border.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xD1D1D6) } else { rgba(0xFFFFFF18) }
        });
        let chrome_fg = self.chrome_fg.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0x6E6E73) } else { rgb(0x8F96A8) }
        });
        let chrome_fg_active = self.chrome_fg_active.as_deref().and_then(parse_hex).unwrap_or(fg);
        let faint_fg = self.faint_fg.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0x86868B) } else { rgb(0x6C7486) }
        });
        let hairline = self.hairline.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xE5E5EA) } else { rgba(0xFFFFFF10) }
        });
        let card_hover = self.card_hover.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xE8E8ED) } else { rgba(0xFFFFFF10) }
        });
        let card_active = self.card_active.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            Rgba { a: 0.18, ..accent }
        });
        let good = self.good.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0x1A7F37) } else { rgb(0x9BAF7C) }
        });
        let sheet_border = self.sheet_border.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xD1D1D6) } else { rgba(0xFFFFFF14) }
        });
        let soft_bg = self.soft_bg.as_deref().and_then(parse_hex).unwrap_or_else(|| {
            if is_light { rgb(0xECECF0) } else { rgba(0xFFFFFF0F) }
        });

        let primary_theme = Theme {
            bg,
            fg,
            cursor,
            panel_bg,
            elevated_bg,
            elevated_border,
            chrome_fg,
            chrome_fg_active,
            faint_fg,
            hairline,
            card_hover,
            card_active,
            accent,
            good,
            sheet_border,
            soft_bg,
            ansi,
        };

        let theme_mode = match self.mode.as_deref() {
            Some(m) if m.eq_ignore_ascii_case("dark") => ThemeModeSupport::DarkOnly,
            Some(m) if m.eq_ignore_ascii_case("light") => ThemeModeSupport::LightOnly,
            Some(m) if m.eq_ignore_ascii_case("both") => ThemeModeSupport::Both,
            _ => {
                if is_light {
                    ThemeModeSupport::LightOnly
                } else {
                    ThemeModeSupport::DarkOnly
                }
            }
        };

        let (dark, light) = if is_light {
            let mut dark_comp = Theme::catppuccin_dark();
            dark_comp.cursor = cursor;
            dark_comp.accent = accent;
            (dark_comp, primary_theme)
        } else {
            let mut light_comp = Theme::solarized_light();
            light_comp.cursor = cursor;
            light_comp.accent = accent;
            (primary_theme, light_comp)
        };

        Preset {
            id,
            name,
            description: desc,
            dark,
            light,
            is_custom: true,
            theme_mode,
        }
    }

    pub fn from_preset(preset: &Preset) -> Self {
        let theme = &preset.dark;
        let mut ansi_vec = Vec::with_capacity(16);
        for c in &theme.ansi {
            ansi_vec.push(to_hex(*c));
        }
        Self {
            name: preset.name.clone(),
            id: Some(preset.id.clone()),
            mode: Some("dark".to_string()),
            description: Some(preset.description.clone()),
            bg: Some(to_hex(theme.bg)),
            fg: Some(to_hex(theme.fg)),
            cursor: Some(to_hex(theme.cursor)),
            accent: Some(to_hex(theme.accent)),
            background: None,
            foreground: None,
            cursor_color: None,
            selection_background: None,
            panel_bg: Some(to_hex(theme.panel_bg)),
            elevated_bg: Some(to_hex(theme.elevated_bg)),
            elevated_border: Some(to_hex(theme.elevated_border)),
            chrome_fg: Some(to_hex(theme.chrome_fg)),
            chrome_fg_active: Some(to_hex(theme.chrome_fg_active)),
            faint_fg: Some(to_hex(theme.faint_fg)),
            hairline: Some(to_hex(theme.hairline)),
            card_hover: Some(to_hex(theme.card_hover)),
            card_active: Some(to_hex(theme.card_active)),
            good: Some(to_hex(theme.good)),
            sheet_border: Some(to_hex(theme.sheet_border)),
            soft_bg: Some(to_hex(theme.soft_bg)),
            ansi: Some(ansi_vec),
            black: None, red: None, green: None, yellow: None,
            blue: None, purple: None, cyan: None, white: None,
            bright_black: None, bright_red: None, bright_green: None, bright_yellow: None,
            bright_blue: None, bright_purple: None, bright_cyan: None, bright_white: None,
        }
    }
}

pub struct ThemeRegistry {
    presets: Vec<Preset>,
}

impl ThemeRegistry {
    pub fn standard() -> Self {
        let mut registry = Self {
            presets: vec![
                Preset {
                    id: "catppuccin".to_string(),
                    name: "Catppuccin".to_string(),
                    description: "Soothing pastel palette with gentle lavender and peach accents.".to_string(),
                    dark: Theme::catppuccin_dark(),
                    light: Theme::catppuccin_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "gruvbox".to_string(),
                    name: "Gruvbox".to_string(),
                    description: "Iconic retro earthy groove with warm contrast and maximum eye comfort.".to_string(),
                    dark: Theme::gruvbox_dark(),
                    light: Theme::gruvbox_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "barbie".to_string(),
                    name: "Barbie".to_string(),
                    description: "Synthwave cyber-pink dreamhouse with vibrant neon highlights & pastel paper.".to_string(),
                    dark: Theme::barbie_dark(),
                    light: Theme::barbie_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "osaka_jade".to_string(),
                    name: "Osaka Jade".to_string(),
                    description: "Lush deep night jade cyberpunk with emerald, sage and mint serenity.".to_string(),
                    dark: Theme::osaka_jade(),
                    light: Theme::osaka_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "dracula".to_string(),
                    name: "Dracula".to_string(),
                    description: "Legendary dark palette with purple, pink, and electric cyan highlights.".to_string(),
                    dark: Theme::dracula(),
                    light: Theme::dracula_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "rose_pine".to_string(),
                    name: "Rosé Pine".to_string(),
                    description: "All-natural pine warmth engineered for zero eye fatigue in dark and light.".to_string(),
                    dark: Theme::rose_pine(),
                    light: Theme::rose_pine_dawn(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "solarized".to_string(),
                    name: "Solarized".to_string(),
                    description: "Warm ivory linen with Ethan Schoonover's iconic precision color science.".to_string(),
                    dark: Theme::solarized_dark(),
                    light: Theme::solarized_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "github".to_string(),
                    name: "GitHub".to_string(),
                    description: "Pristine white paper, GitHub blue accents, and high-contrast developer syntax.".to_string(),
                    dark: Theme::github_dark(),
                    light: Theme::github_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "tokyo_night".to_string(),
                    name: "Tokyo Night".to_string(),
                    description: "Modern neon palette inspired by the glowing lights of downtown Tokyo.".to_string(),
                    dark: Theme::tokyo_night_dark(),
                    light: Theme::tokyo_night_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "nord".to_string(),
                    name: "Nord".to_string(),
                    description: "Arctic, north-bluish palette engineered for optimal contrast and clean readability.".to_string(),
                    dark: Theme::nord_dark(),
                    light: Theme::nord_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "classic".to_string(),
                    name: "Classic".to_string(),
                    description: "Pure black sheet, neutral gray ink, and electric cyan — the stock terminal look.".to_string(),
                    dark: Theme::classic(),
                    light: Theme::classic_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "atlas".to_string(),
                    name: "Atlas Default".to_string(),
                    description: "Apple macOS Light paper sheet & sleek matte dark mode with terracotta accents.".to_string(),
                    dark: Theme::dark(),
                    light: Theme::cream(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
                Preset {
                    id: "one".to_string(),
                    name: "One Light / Dark".to_string(),
                    description: "Clean modern aesthetic with Atom violet and balanced syntax tones.".to_string(),
                    dark: Theme::one_dark(),
                    light: Theme::one_light(),
                    is_custom: false,
                    theme_mode: ThemeModeSupport::Both,
                },
            ],
        };
        registry.load_custom_themes();
        registry
    }

    pub fn custom_themes_dir() -> PathBuf {
        crate::user_data_dir().join("themes")
    }

    /// Load user-installed custom themes from `~/.config/atlas/themes/`.
    pub fn load_custom_themes(&mut self) {
        let themes_dir = Self::custom_themes_dir();
        if !themes_dir.exists() {
            if let Ok(()) = std::fs::create_dir_all(&themes_dir) {
                let readme = r#"# Atlas Terminal Custom Themes

Drop any `.json` theme file into this directory to immediately make it available in Atlas Terminal!

## Supported Formats
1. **Atlas Native Theme** (see `example.json`)
2. **Windows Terminal Color Schemes** (any `.json` scheme from windowsterminalthemes.dev or Microsoft Terminal settings)

## Commands
- `atlas theme list`          List all installed themes
- `atlas theme set <name>`    Apply theme by name or ID
- `atlas theme install <file>` Install theme file
- `atlas theme export <name>` Export a theme to JSON
- `atlas theme dir`           Open this themes folder
"#;
                let _ = std::fs::write(themes_dir.join("README.md"), readme);

                let example = CustomThemeFile {
                    name: "Neon Cyberpunk".to_string(),
                    id: Some("neon_cyberpunk".to_string()),
                    mode: Some("dark".to_string()),
                    description: Some("Electric cyberpunk theme with luminous teal and magenta highlights".to_string()),
                    bg: Some("#0B0C10".to_string()),
                    fg: Some("#C5C6C7".to_string()),
                    cursor: Some("#66FCF1".to_string()),
                    accent: Some("#45A29E".to_string()),
                    background: None,
                    foreground: None,
                    cursor_color: None,
                    selection_background: None,
                    panel_bg: Some("#07080A".to_string()),
                    elevated_bg: Some("#14181F".to_string()),
                    elevated_border: Some("#45A29E40".to_string()),
                    chrome_fg: Some("#8A9BA8".to_string()),
                    chrome_fg_active: Some("#C5C6C7".to_string()),
                    faint_fg: Some("#455A64".to_string()),
                    hairline: Some("#45A29E26".to_string()),
                    card_hover: Some("#45A29E1A".to_string()),
                    card_active: Some("#45A29E33".to_string()),
                    good: Some("#66FCF1".to_string()),
                    sheet_border: Some("#45A29E33".to_string()),
                    soft_bg: Some("#45A29E14".to_string()),
                    ansi: Some(vec![
                        "#0B0C10".to_string(), "#FF0055".to_string(), "#45A29E".to_string(), "#FFE600".to_string(),
                        "#1F2833".to_string(), "#C5C6C7".to_string(), "#66FCF1".to_string(), "#FFFFFF".to_string(),
                        "#45A29E".to_string(), "#FF3377".to_string(), "#66FCF1".to_string(), "#FFEB33".to_string(),
                        "#1F2833".to_string(), "#C5C6C7".to_string(), "#66FCF1".to_string(), "#FFFFFF".to_string(),
                    ]),
                    black: None, red: None, green: None, yellow: None,
                    blue: None, purple: None, cyan: None, white: None,
                    bright_black: None, bright_red: None, bright_green: None, bright_yellow: None,
                    bright_blue: None, bright_purple: None, bright_cyan: None, bright_white: None,
                };
                if let Ok(json) = serde_json::to_string_pretty(&example) {
                    let _ = std::fs::write(themes_dir.join("example.json"), json);
                }
            }
            return;
        }

        if let Ok(entries) = std::fs::read_dir(&themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(custom) = serde_json::from_str::<CustomThemeFile>(&content) {
                            let preset = custom.to_preset();
                            if let Some(pos) = self.presets.iter().position(|p| p.id.eq_ignore_ascii_case(&preset.id)) {
                                self.presets[pos] = preset;
                            } else {
                                self.presets.push(preset);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn preset(&self, index: usize) -> &Preset {
        &self.presets[index.min(self.presets.len() - 1)]
    }

    pub fn find_by_name_or_id(&self, key: &str) -> Option<usize> {
        let key_lower = key.to_ascii_lowercase();
        // Support legacy mappings: "dark" -> atlas, "paper" -> atlas
        if key_lower == "dark" || key_lower == "paper" || key_lower == "cream" {
            return self.presets.iter().position(|p| p.id == "atlas");
        }
        if key_lower == "barbie_light" {
            return self.presets.iter().position(|p| p.id == "barbie");
        }
        if key_lower == "gruvbox_light" {
            return self.presets.iter().position(|p| p.id == "gruvbox");
        }
        if key_lower == "rose_pine_dawn" {
            return self.presets.iter().position(|p| p.id == "rose_pine");
        }
        if key_lower == "osaka_light" || key_lower == "osaka_mint" {
            return self.presets.iter().position(|p| p.id == "osaka_jade");
        }
        if key_lower == "dracula_light" {
            return self.presets.iter().position(|p| p.id == "dracula");
        }
        self.presets.iter().position(|p| {
            p.id.eq_ignore_ascii_case(key)
                || p.name.eq_ignore_ascii_case(key)
                || p.dark_name().eq_ignore_ascii_case(key)
                || p.light_name().eq_ignore_ascii_case(key)
                || key_lower.contains(&p.id.to_ascii_lowercase())
        })
    }

    pub fn resolve(&self, preset_idx: usize, mode: &str) -> (Theme, bool) {
        let preset = self.preset(preset_idx);
        let is_light = match mode.to_ascii_lowercase().as_str() {
            "light" => true,
            "dark" => false,
            _ => is_system_light_mode(),
        };
        if is_light {
            (preset.light.clone(), false)
        } else {
            (preset.dark.clone(), true)
        }
    }

    pub fn reload(&mut self) {
        *self = Self::standard();
    }

    #[allow(dead_code)]
    pub fn next(&self, index: usize) -> usize {
        (index + 1) % self.presets.len()
    }
}

pub fn custom_themes_dir() -> PathBuf {
    ThemeRegistry::custom_themes_dir()
}

/// Deletes a custom theme `.json` file from `~/.config/atlas/themes/`.
/// Matches by theme ID, name, or filename stem.
pub fn delete_custom_theme(theme_id: &str) -> bool {
    let themes_dir = custom_themes_dir();
    if !themes_dir.exists() {
        return false;
    }
    let norm_id = theme_id.to_ascii_lowercase().replace(['-', ' '], "_");
    let mut deleted = false;

    if let Ok(entries) = std::fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let norm_file_stem = file_name.to_ascii_lowercase().replace(['-', ' '], "_");

                // 1. Direct filename stem match
                if norm_file_stem == norm_id || file_name.eq_ignore_ascii_case(theme_id) {
                    if std::fs::remove_file(&path).is_ok() {
                        deleted = true;
                    }
                    continue;
                }

                // 2. Parse file and match internal id or name
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(custom) = serde_json::from_str::<CustomThemeFile>(&content) {
                        let custom_id = custom.id.as_deref().unwrap_or("");
                        let custom_slug = slugify(&custom.name);
                        if custom_id.eq_ignore_ascii_case(theme_id)
                            || custom_slug == norm_id
                            || custom.name.eq_ignore_ascii_case(theme_id)
                        {
                            if std::fs::remove_file(&path).is_ok() {
                                deleted = true;
                            }
                        }
                    }
                }
            }
        }
    }

    deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn is(color: Rgba, r: u32, g: u32, b: u32) -> bool {
        near(color.r, r as f32 / 255.0)
            && near(color.g, g as f32 / 255.0)
            && near(color.b, b as f32 / 255.0)
            && near(color.a, 1.0)
    }

    #[test]
    fn classic_matches_reference_prompt_palette() {
        let t = Theme::classic();
        assert!(is(t.bg, 0x00, 0x00, 0x00));
        assert!(is(t.fg, 0xCC, 0xCC, 0xCC));
        assert!(is(t.cursor, 0xFF, 0xFF, 0xFF));
        assert!(is(t.ansi[0], 0x0C, 0x0C, 0x0C));
        assert!(is(t.ansi[6], 0x00, 0xFF, 0xFF));
        assert!(is(t.ansi[7], 0xCC, 0xCC, 0xCC));
        assert!(is(t.ansi[8], 0x80, 0x80, 0x80));
        assert!(is(t.ansi[14], 0x00, 0xFF, 0xFF));
        assert!(is(t.ansi[15], 0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn classic_is_registered() {
        let registry = ThemeRegistry::standard();
        let idx = registry.find_by_name_or_id("classic").expect("classic preset");
        assert_eq!(registry.preset(idx).name, "Classic");
        assert!(registry.len() >= 13);
    }

    #[test]
    fn test_all_light_themes_have_solid_hover_and_soft_bg() {
        let registry = ThemeRegistry::standard();
        for i in 0..registry.len() {
            let preset = registry.preset(i);
            assert!(preset.light.is_light(), "{} light theme must be light", preset.name);
            assert!(!preset.dark.is_light(), "{} dark theme must be dark", preset.name);

            // Light themes must have 100% solid, pre-blended card_hover and soft_bg
            // to avoid premultiplied black haloing and eroded border anti-aliasing.
            assert_eq!(preset.light.card_hover.a, 1.0, "{} card_hover must be opaque", preset.name);
            assert_eq!(preset.light.soft_bg.a, 1.0, "{} soft_bg must be opaque", preset.name);
            assert_eq!(preset.light.hairline.a, 1.0, "{} hairline must be opaque", preset.name);
            assert_eq!(preset.light.elevated_border.a, 1.0, "{} elevated_border must be opaque", preset.name);
        }
    }

    #[test]
    fn github_light_is_pristine_white() {
        let gh = Theme::github_light();
        assert!(is(gh.bg, 0xFF, 0xFF, 0xFF));
        assert!(is(gh.fg, 0x24, 0x29, 0x2F));
    }

    #[test]
    fn test_preset_names_dark_and_light() {
        let registry = ThemeRegistry::standard();
        for i in 0..registry.len() {
            let preset = registry.preset(i);
            assert!(!preset.dark_name().is_empty());
            assert!(!preset.light_name().is_empty());
        }
        let atlas_idx = registry.find_by_name_or_id("atlas").expect("atlas preset");
        let atlas = registry.preset(atlas_idx);
        assert_eq!(atlas.dark_name(), "Atlas Dark");
        assert_eq!(atlas.light_name(), "macOS Cream");
    }

    #[test]
    fn test_new_aesthetic_themes_registered() {
        let registry = ThemeRegistry::standard();
        assert!(registry.find_by_name_or_id("gruvbox").is_some(), "gruvbox must exist");
        assert!(registry.find_by_name_or_id("barbie").is_some(), "barbie must exist");
        assert!(registry.find_by_name_or_id("osaka_jade").is_some(), "osaka_jade must exist");
        assert!(registry.find_by_name_or_id("dracula").is_some(), "dracula must exist");
        assert!(registry.find_by_name_or_id("rose_pine").is_some(), "rose_pine must exist");
    }

    #[test]
    fn test_custom_theme_roundtrip_and_windows_terminal_compat() {
        let wt_json = r##"{
            "name": "Campbell",
            "background": "#0C0C0C",
            "foreground": "#CCCCCC",
            "cursorColor": "#FFFFFF",
            "black": "#0C0C0C",
            "red": "#C50F1F",
            "green": "#13A10E",
            "yellow": "#C19C00",
            "blue": "#0037DA",
            "purple": "#881798",
            "cyan": "#3A96DD",
            "white": "#CCCCCC",
            "brightBlack": "#767676",
            "brightRed": "#E74856",
            "brightGreen": "#16C60C",
            "brightYellow": "#F9F1A5",
            "brightBlue": "#3B78FF",
            "brightPurple": "#B4009E",
            "brightCyan": "#61D6D6",
            "brightWhite": "#F2F2F2"
        }"##;

        let custom: CustomThemeFile = serde_json::from_str(wt_json).expect("valid wt theme");
        let preset = custom.to_preset();
        assert_eq!(preset.name, "Campbell");
        assert_eq!(preset.id, "campbell");
        assert!(!preset.dark.is_light());
        assert_eq!(preset.dark.ansi[1], rgb(0xC50F1F));
        assert_eq!(preset.dark.ansi[2], rgb(0x13A10E));
    }

    #[test]
    fn test_delete_custom_theme_nonexistent() {
        assert!(!delete_custom_theme("nonexistent_theme_xyz123"));
    }
}
