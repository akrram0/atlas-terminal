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
}

#[allow(dead_code)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub dark: Theme,
    pub light: Theme,
}

#[allow(dead_code)]
impl Preset {
    /// Legacy theme reference for code expecting `preset.theme`.
    pub fn theme(&self) -> &Theme {
        &self.dark
    }

    pub fn dark_name(&self) -> &'static str {
        match self.id {
            "atlas" => "Atlas Dark",
            "github" => "GitHub Dark",
            "one" => "One Dark",
            "solarized" => "Solarized Dark",
            "tokyo_night" => "Tokyo Night",
            "catppuccin" => "Catppuccin Macchiato",
            "nord" => "Nord Dark",
            "classic" => "Classic Black",
            _ => self.name,
        }
    }

    pub fn light_name(&self) -> &'static str {
        match self.id {
            "atlas" => "macOS Cream",
            "github" => "GitHub Light",
            "one" => "One Light",
            "solarized" => "Solarized Light",
            "tokyo_night" => "Tokyo Night Day",
            "catppuccin" => "Catppuccin Latte",
            "nord" => "Nord Light",
            "classic" => "Classic Light",
            _ => self.name,
        }
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
                    description: "Apple macOS Light paper sheet & sleek matte dark mode with terracotta accents.",
                    dark: Theme::dark(),
                    light: Theme::cream(),
                },
                Preset {
                    id: "github",
                    name: "GitHub",
                    description: "Pristine white paper, GitHub blue accents, and high-contrast developer syntax.",
                    dark: Theme::github_dark(),
                    light: Theme::github_light(),
                },
                Preset {
                    id: "one",
                    name: "One Light / Dark",
                    description: "Clean modern aesthetic with Atom violet and balanced syntax tones.",
                    dark: Theme::one_dark(),
                    light: Theme::one_light(),
                },
                Preset {
                    id: "solarized",
                    name: "Solarized",
                    description: "Warm ivory linen with Ethan Schoonover's iconic precision color science.",
                    dark: Theme::solarized_dark(),
                    light: Theme::solarized_light(),
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

    #[allow(dead_code)]
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
        assert_eq!(registry.len(), 8);
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
        let atlas = registry.preset(0);
        assert_eq!(atlas.dark_name(), "Atlas Dark");
        assert_eq!(atlas.light_name(), "macOS Cream");
    }
}
