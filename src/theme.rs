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

    /// Paper: warm cream sheet (#EDE8DC) over sand chrome (#E5E0D4), warm
    /// deep-charcoal ink (#1C1A16), terracotta rust accent (#C05A28). Zed aesthetic.
    pub fn cream() -> Self {
        Self {
            bg: rgb(0xEDE8DC),
            fg: rgb(0x1C1A16),
            cursor: rgb(0xC05A28),
            panel_bg: rgb(0xE5E0D4),
            elevated_bg: rgb(0xF5F0E8),
            elevated_border: rgba(0x00000014),
            chrome_fg: rgb(0x7A7570),
            chrome_fg_active: rgb(0x1C1A16),
            faint_fg: rgb(0x9A9590),
            hairline: rgba(0x00000014),
            card_hover: rgba(0x0000000D),
            card_active: rgba(0xC05A281E),
            accent: rgb(0xC05A28),
            good: rgb(0x5F7A46),
            sheet_border: rgba(0x00000014),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x1C1A16), // 0 black
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

    /// Classic flipped for daylight: off-white sheet (#F2F2F2), near-black
    /// ink, classic blue (#2472C8) accent, and a Campbell-flavoured ANSI set
    /// with the illegible-on-white hues (yellow, cyan, white) darkened.
    pub fn classic_light() -> Self {
        Self {
            bg: rgb(0xF2F2F2),
            fg: rgb(0x1F1F1F),
            cursor: rgb(0x2472C8),
            panel_bg: rgb(0xE8E8E8),
            elevated_bg: rgb(0xFFFFFF),
            elevated_border: rgba(0x0000001A),
            chrome_fg: rgb(0x6B6B6B),
            chrome_fg_active: rgb(0x1F1F1F),
            faint_fg: rgb(0x8F8F8F),
            hairline: rgba(0x00000018),
            card_hover: rgba(0x0000000D),
            card_active: rgba(0x2472C81E),
            accent: rgb(0x2472C8),
            good: rgb(0x13A10E),
            sheet_border: rgba(0x00000018),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x0C0C0C), // 0 black
                rgb(0xC50F1F), // 1 red
                rgb(0x13A10E), // 2 green
                rgb(0xC19C00), // 3 yellow
                rgb(0x2472C8), // 4 blue
                rgb(0x881798), // 5 magenta
                rgb(0x0E7490), // 6 cyan
                rgb(0x555555), // 7 white — mid gray on the light sheet
                rgb(0x808080), // 8 bright black
                rgb(0xE74856), // 9 bright red
                rgb(0x16C60C), // 10 bright green
                rgb(0xD9A406), // 11 bright yellow
                rgb(0x3B8EEA), // 12 bright blue
                rgb(0xB4009E), // 13 bright magenta
                rgb(0x1295A3), // 14 bright cyan
                rgb(0x1F1F1F), // 15 bright white — default ink, like other light themes
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
    use std::ptr::null_mut;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            hkey: *mut core::ffi::c_void,
            sub_key: *const u16,
            options: u32,
            sam_desired: u32,
            result: *mut *mut core::ffi::c_void,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: *mut core::ffi::c_void,
            value_name: *const u16,
            reserved: *mut u32,
            type_: *mut u32,
            data: *mut u8,
            cb_data: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: *mut core::ffi::c_void) -> i32;
    }
    const HKEY_CURRENT_USER: *mut core::ffi::c_void = 0x80000001u32 as usize as *mut core::ffi::c_void;
    const KEY_READ: u32 = 0x20019;

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let val_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    unsafe {
        let mut key: *mut core::ffi::c_void = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) == 0 {
            let mut val: u32 = 0;
            let mut size: u32 = std::mem::size_of::<u32>() as u32;
            let mut val_type: u32 = 0;
            let res = RegQueryValueExW(
                key,
                val_name.as_ptr(),
                null_mut(),
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
            bg: rgb(0xE1E2E7),
            fg: rgb(0x3760BF),
            cursor: rgb(0x2E7DE9),
            panel_bg: rgb(0xD5D6DB),
            elevated_bg: rgb(0xEBEDF2),
            elevated_border: rgba(0x00000014),
            chrome_fg: rgb(0x6172B0),
            chrome_fg_active: rgb(0x3760BF),
            faint_fg: rgb(0x8990B3),
            hairline: rgba(0x00000014),
            card_hover: rgba(0x0000000D),
            card_active: rgba(0x2E7DE91E),
            accent: rgb(0x2E7DE9),
            good: rgb(0x587539),
            sheet_border: rgba(0x00000014),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x0F0F14), rgb(0x8C4351), rgb(0x485E30), rgb(0x8F5E15),
                rgb(0x2E7DE9), rgb(0x7847BD), rgb(0x166775), rgb(0x4F5B66),
                rgb(0x68707F), rgb(0xB15C6B), rgb(0x587539), rgb(0x9D6A1F),
                rgb(0x3B8BF2), rgb(0x8C5ECF), rgb(0x217E8F), rgb(0xE1E2E7),
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
            bg: rgb(0xEFF1F5),
            fg: rgb(0x4C4F69),
            cursor: rgb(0xEA76CB),
            panel_bg: rgb(0xE6E9EF),
            elevated_bg: rgb(0xDCE0E8),
            elevated_border: rgba(0x00000014),
            chrome_fg: rgb(0x7C7F93),
            chrome_fg_active: rgb(0x4C4F69),
            faint_fg: rgb(0x9CA0B0),
            hairline: rgba(0x00000014),
            card_hover: rgba(0x0000000D),
            card_active: rgba(0xEA76CB1E),
            accent: rgb(0xEA76CB),
            good: rgb(0x40A02B),
            sheet_border: rgba(0x00000014),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x5C5F77), rgb(0xD20F39), rgb(0x40A02B), rgb(0xDF8E1D),
                rgb(0x1E66F5), rgb(0xEA76CB), rgb(0x179299), rgb(0xACB0BE),
                rgb(0x6C6F85), rgb(0xD20F39), rgb(0x40A02B), rgb(0xDF8E1D),
                rgb(0x1E66F5), rgb(0xEA76CB), rgb(0x179299), rgb(0x4C4F69),
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
            bg: rgb(0xECEFF4),
            fg: rgb(0x2E3440),
            cursor: rgb(0x5E81AC),
            panel_bg: rgb(0xE5E9F0),
            elevated_bg: rgb(0xD8DEE9),
            elevated_border: rgba(0x00000014),
            chrome_fg: rgb(0x4C566A),
            chrome_fg_active: rgb(0x2E3440),
            faint_fg: rgb(0x7B88A1),
            hairline: rgba(0x00000014),
            card_hover: rgba(0x0000000D),
            card_active: rgba(0x5E81AC1E),
            accent: rgb(0x5E81AC),
            good: rgb(0x608E46),
            sheet_border: rgba(0x00000014),
            soft_bg: rgba(0x0000000F),
            ansi: [
                rgb(0x2E3440), rgb(0x9E3B44), rgb(0x608E46), rgb(0x9F7C2C),
                rgb(0x5E81AC), rgb(0x84557A), rgb(0x497F87), rgb(0x768297),
                rgb(0x4C566A), rgb(0xB34852), rgb(0x72A355), rgb(0xB28C33),
                rgb(0x6E93C0), rgb(0x98668E), rgb(0x55929B), rgb(0x2E3440),
            ],
        }
    }
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub dark: Theme,
    pub light: Theme,
}

impl Preset {
    /// Legacy theme reference for code expecting `preset.theme`.
    pub fn theme(&self) -> &Theme {
        &self.dark
    }
}

pub struct ThemeRegistry {
    presets: Vec<Preset>,
}

impl ThemeRegistry {
    pub fn standard() -> Self {
        Self {
            presets: vec![
                Preset {
                    id: "atlas",
                    name: "Atlas Default",
                    description: "Warm paper light mode & sleek matte dark mode with terracotta rust accents.",
                    dark: Theme::dark(),
                    light: Theme::cream(),
                },
                Preset {
                    id: "tokyo_night",
                    name: "Tokyo Night",
                    description: "Modern neon palette inspired by the glowing lights of downtown Tokyo.",
                    dark: Theme::tokyo_night_dark(),
                    light: Theme::tokyo_night_light(),
                },
                Preset {
                    id: "catppuccin",
                    name: "Catppuccin",
                    description: "Soothing pastel palette with gentle lavender and peach accents.",
                    dark: Theme::catppuccin_dark(),
                    light: Theme::catppuccin_light(),
                },
                Preset {
                    id: "nord",
                    name: "Nord",
                    description: "Arctic, north-bluish palette engineered for optimal contrast and clean readability.",
                    dark: Theme::nord_dark(),
                    light: Theme::nord_light(),
                },
                Preset {
                    id: "classic",
                    name: "Classic",
                    description: "Pure black sheet, neutral gray ink, and electric cyan — the stock terminal look.",
                    dark: Theme::classic(),
                    light: Theme::classic_light(),
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

    pub fn find_by_name_or_id(&self, key: &str) -> Option<usize> {
        let key_lower = key.to_ascii_lowercase();
        // Support legacy mappings: "dark" -> atlas, "paper" -> atlas
        if key_lower == "dark" || key_lower == "paper" || key_lower == "cream" {
            return Some(0);
        }
        self.presets.iter().position(|p| {
            p.id.eq_ignore_ascii_case(key)
                || p.name.eq_ignore_ascii_case(key)
                || key_lower.contains(&p.id)
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

    pub fn next(&self, index: usize) -> usize {
        (index + 1) % self.presets.len()
    }
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
        assert_eq!(registry.len(), 5);
    }
}
