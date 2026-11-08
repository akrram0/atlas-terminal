//! Top Bar Pixel Art Companion System
//!
//! Provides procedural multi-frame pixel art animations (32x16 grid),
//! a standalone configuration file (`pixel_art.json`), and native GPU-accelerated
//! GPUI canvas rendering for both top-bar and Settings UI preview cards.

use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{
    canvas, fill, point, px, rgba, size, Bounds,
    IntoElement, Rgba,
};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

pub const GRID_WIDTH: usize = 32;
pub const GRID_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetCategory {
    Cozy,
    Cyber,
    System,
}

impl PresetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cozy => "cozy",
            Self::Cyber => "cyber",
            Self::System => "system",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Cozy => "Cozy & Chill",
            Self::Cyber => "Cyber & Hacker",
            Self::System => "System & Retro",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub category: PresetCategory,
    pub desc: &'static str,
}

impl Preset {
    #[allow(dead_code)]
    pub fn category_label(&self) -> &'static str {
        self.category.label()
    }
}

pub const PRESETS: &[Preset] = &[
    // Cozy & Chill
    Preset {
        id: "hot_coffee",
        name: "Hot Coffee",
        category: PresetCategory::Cozy,
        desc: "Warm pixel coffee mug with rising animated steam wisps.",
    },
    Preset {
        id: "campfire",
        name: "Campfire",
        category: PresetCategory::Cozy,
        desc: "Flickering warm pixel flames with dancing embers and sparks.",
    },
    Preset {
        id: "cassette_tape",
        name: "Cassette Tape",
        category: PresetCategory::Cozy,
        desc: "Retro cassette chassis with continuously rotating spools.",
    },
    // Cyber & Hacker
    Preset {
        id: "matrix_rain",
        name: "Matrix Rain",
        category: PresetCategory::Cyber,
        desc: "Cascading digital green/cyan data drops falling down.",
    },
    Preset {
        id: "packet_radar",
        name: "Packet Radar",
        category: PresetCategory::Cyber,
        desc: "Cyber radar sweep with periodic packet blips and network LEDs.",
    },
    Preset {
        id: "cyber_glitch",
        name: "Cyber Glitch",
        category: PresetCategory::Cyber,
        desc: "Minimal hacker skull glyph with periodic horizontal scanline glitches.",
    },
    // System & Retro
    Preset {
        id: "server_rack",
        name: "Server Rack",
        category: PresetCategory::System,
        desc: "Mini server blade with dynamically flickering read/write LEDs.",
    },
    Preset {
        id: "dot_wave",
        name: "Dot Wave",
        category: PresetCategory::System,
        desc: "Smooth sinusoidal dot-matrix waveform inspired by oscilloscopes.",
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelArtConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_preset")]
    pub active_preset: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_true")]
    pub speed_boost_on_activity: bool,
    #[serde(default = "default_color_mode")]
    pub color_mode: String,
    #[serde(default)]
    pub custom_sprite_path: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_preset() -> String {
    "hot_coffee".to_string()
}

fn default_fps() -> u32 {
    10
}

fn default_color_mode() -> String {
    "original".to_string()
}

impl Default for PixelArtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            active_preset: default_preset(),
            fps: default_fps(),
            speed_boost_on_activity: true,
            color_mode: default_color_mode(),
            custom_sprite_path: None,
        }
    }
}

/// Standalone config file path: `%APPDATA%\AtlasTerminal\pixel_art.json`
pub fn pixel_art_config_path() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dir = PathBuf::from(appdata).join("AtlasTerminal");
        return dir.join("pixel_art.json");
    }
    crate::user_data_dir().join("pixel_art.json")
}

impl PixelArtConfig {
    pub fn load() -> Self {
        let path = pixel_art_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                    return cfg;
                }
            }
        }

        // Also check if modern user_data_dir has one
        let alt_path = crate::user_data_dir().join("pixel_art.json");
        if alt_path.exists() {
            if let Ok(content) = fs::read_to_string(&alt_path) {
                if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                    return cfg;
                }
            }
        }

        let def = Self::default();
        def.save();
        def
    }

    pub fn save(&self) {
        let path = pixel_art_config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }
}

/// Procedural pixel art frame generator.
/// Generates a list of `(x, y, Rgba)` active pixels in the 32x16 grid coordinate space.
pub fn render_frame(
    preset_id: &str,
    tick: u64,
    color_mode: &str,
    theme_accent: Rgba,
    is_dark: bool,
) -> Vec<(u8, u8, Rgba)> {
    let t = tick as f32;
    let mut pixels = Vec::with_capacity(96);

    // Color mapper based on color_mode: "original", "theme_accent", "monochrome"
    let map_color = |r: u8, g: u8, b: u8, a: f32| -> Rgba {
        match color_mode {
            "monochrome" => {
                let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
                if is_dark {
                    let v = (lum * 0.8 + 0.2).clamp(0.0, 1.0);
                    rgba(
                        ((v * 255.0) as u32) << 24
                            | ((v * 255.0) as u32) << 16
                            | ((v * 255.0) as u32) << 8
                            | ((a * 255.0) as u32),
                    )
                } else {
                    let v = (1.0 - lum * 0.85).clamp(0.0, 1.0);
                    rgba(
                        ((v * 255.0) as u32) << 24
                            | ((v * 255.0) as u32) << 16
                            | ((v * 255.0) as u32) << 8
                            | ((a * 255.0) as u32),
                    )
                }
            }
            "theme_accent" => {
                let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
                let factor = (lum * 0.7 + 0.3).clamp(0.0, 1.0);
                rgba(
                    (((theme_accent.r * factor * 255.0) as u32) << 24)
                        | (((theme_accent.g * factor * 255.0) as u32) << 16)
                        | (((theme_accent.b * factor * 255.0) as u32) << 8)
                        | (((a * 255.0) as u32)),
                )
            }
            _ => rgba(
                ((r as u32) << 24)
                    | ((g as u32) << 16)
                    | ((b as u32) << 8)
                    | ((a * 255.0) as u32),
            ),
        }
    };

    match preset_id {
        "hot_coffee" => {
            // Mug body: cols 10..21, rows 8..15
            let mug_col = map_color(0xC0, 0x5A, 0x28, 1.0);
            for x in 10..=20 {
                for y in 8..=14 {
                    pixels.push((x, y, mug_col));
                }
            }
            // Handle: cols 21..=23, rows 9..=13 (hollow inside)
            for y in 9..=13 {
                pixels.push((21, y, mug_col));
                pixels.push((23, y, mug_col));
            }
            pixels.push((22, 9, mug_col));
            pixels.push((22, 13, mug_col));

            // Coffee surface
            let coffee_col = map_color(0x6A, 0x3B, 0x18, 1.0);
            for x in 11..=19 {
                pixels.push((x, 8, coffee_col));
            }

            // Steam wisps (rising sine waves)
            let steam_col = map_color(0xE8, 0xE4, 0xDC, 0.85);
            let s1 = (t * 0.4).sin() * 2.0;
            let s2 = (t * 0.4).cos() * 2.0;
            let y1 = (6.0 - ((t * 0.8) as i32 % 6) as f32).max(1.0) as u8;
            let y2 = (6.0 - (((t * 0.8) as i32 + 3) % 6) as f32).max(1.0) as u8;

            let x1 = ((13.0 + s1).round() as i32).clamp(10, 20) as u8;
            let x2 = ((17.0 + s2).round() as i32).clamp(10, 20) as u8;

            pixels.push((x1, y1, steam_col));
            if y1 + 1 < 8 {
                pixels.push((x1, y1 + 1, steam_col));
            }
            pixels.push((x2, y2, steam_col));
            if y2 + 1 < 8 {
                pixels.push((x2, y2 + 1, steam_col));
            }
        }
        "campfire" => {
            // Logs at base: cols 9..23, rows 13..15
            let log_col = map_color(0x5A, 0x35, 0x1A, 1.0);
            for x in 9..=22 {
                pixels.push((x, 14, log_col));
            }
            for x in 11..=20 {
                pixels.push((x, 13, log_col));
            }

            // Flickering fire core
            let flicker = (t * 1.5).sin();
            let fh = (6.0 + flicker * 2.2).clamp(3.0, 9.0) as u8;

            // Outer flame (red/orange)
            let flame_outer = map_color(0xE0, 0x5A, 0x2B, 1.0);
            for x in 13..=18 {
                let start_y = 14u8.saturating_sub(fh);
                for y in start_y..14 {
                    pixels.push((x, y, flame_outer));
                }
            }

            // Inner flame (bright yellow)
            let flame_inner = map_color(0xFF, 0xD2, 0x3F, 1.0);
            let inner_h = fh.saturating_sub(2).max(2);
            for x in 14..=17 {
                let start_y = 14u8.saturating_sub(inner_h);
                for y in start_y..14 {
                    pixels.push((x, y, flame_inner));
                }
            }

            // Rising sparks / dancing embers
            let spark_col = map_color(0xFF, 0xEE, 0x55, 0.9);
            let spk_y = (8.0 - ((t * 1.2) as i32 % 8) as f32).max(1.0) as u8;
            let spk_x = ((15.0 + (t * 2.0).sin() * 3.0).round() as i32).clamp(10, 22) as u8;
            pixels.push((spk_x, spk_y, spark_col));
        }
        "cassette_tape" => {
            // Outer cassette body: cols 6..26, rows 4..14
            let body_col = map_color(0x2A, 0x2A, 0x28, 1.0);
            for x in 6..=25 {
                for y in 4..=13 {
                    pixels.push((x, y, body_col));
                }
            }

            // Label window: cols 9..23, rows 6..12
            let label_col = map_color(0xE8, 0xE4, 0xDC, 1.0);
            for x in 9..=22 {
                for y in 6..=11 {
                    pixels.push((x, y, label_col));
                }
            }

            // Left spool & Right spool (dark reels)
            let spool_col = map_color(0x18, 0x18, 0x16, 1.0);
            for x in 11..=13 {
                for y in 8..=10 {
                    pixels.push((x, y, spool_col));
                }
            }
            for x in 18..=20 {
                for y in 8..=10 {
                    pixels.push((x, y, spool_col));
                }
            }

            // Rotating spool teeth
            let tooth_col = map_color(0xFF, 0xFF, 0xFF, 1.0);
            let phase = (tick / 2) % 4;
            match phase {
                0 => {
                    pixels.push((12, 8, tooth_col));
                    pixels.push((19, 8, tooth_col));
                }
                1 => {
                    pixels.push((13, 9, tooth_col));
                    pixels.push((20, 9, tooth_col));
                }
                2 => {
                    pixels.push((12, 10, tooth_col));
                    pixels.push((19, 10, tooth_col));
                }
                _ => {
                    pixels.push((11, 9, tooth_col));
                    pixels.push((18, 9, tooth_col));
                }
            }
        }
        "matrix_rain" => {
            // Columns falling down: 4, 8, 12, 16, 20, 24, 28
            let cols: [u8; 7] = [4, 8, 12, 16, 20, 24, 28];
            for (i, &col_x) in cols.iter().enumerate() {
                let speed = ((i % 3) as f32 + 1.0) * 0.8;
                let head_y = (((t * speed + (i as f32 * 5.0)) as i32) % 22) - 3;

                // Bright leading drop
                let lead_col = map_color(0xA3, 0xFF, 0xD6, 1.0);
                if head_y >= 0 && head_y < 16 {
                    pixels.push((col_x, head_y as u8, lead_col));
                }
                if head_y + 1 >= 0 && head_y + 1 < 16 {
                    pixels.push((col_x, (head_y + 1) as u8, lead_col));
                }

                // Medium trail
                let trail_col = map_color(0x00, 0xCC, 0x66, 0.75);
                if head_y - 1 >= 0 && head_y - 1 < 16 {
                    pixels.push((col_x, (head_y - 1) as u8, trail_col));
                }
                if head_y - 2 >= 0 && head_y - 2 < 16 {
                    pixels.push((col_x, (head_y - 2) as u8, trail_col));
                }

                // Faint trail
                let faint_col = map_color(0x00, 0x55, 0x22, 0.4);
                if head_y - 3 >= 0 && head_y - 3 < 16 {
                    pixels.push((col_x, (head_y - 3) as u8, faint_col));
                }
                if head_y - 4 >= 0 && head_y - 4 < 16 {
                    pixels.push((col_x, (head_y - 4) as u8, faint_col));
                }
            }
        }
        "packet_radar" => {
            let cx = 16.0f32;
            let cy = 8.0f32;
            let r = 6.0f32;

            // Radar circle perimeter
            let circle_col = map_color(0x0E, 0x4D, 0x3E, 0.6);
            for deg in (0..360).step_by(24) {
                let rad = (deg as f32) * PI / 180.0;
                let px = (cx + rad.cos() * r).round() as u8;
                let py = (cy + rad.sin() * r).round() as u8;
                if px < 32 && py < 16 {
                    pixels.push((px, py, circle_col));
                }
            }

            // Radar crosshairs
            let cross_col = map_color(0x14, 0x38, 0x2C, 0.4);
            for x in (cx - r) as u8..=(cx + r) as u8 {
                if x < 32 {
                    pixels.push((x, cy as u8, cross_col));
                }
            }
            for y in (cy - r) as u8..=(cy + r) as u8 {
                if y < 16 {
                    pixels.push((cx as u8, y, cross_col));
                }
            }

            // Rotating sweep line
            let sweep_angle = (t * 0.28) % (2.0 * PI);
            let sweep_col = map_color(0x00, 0xFF, 0xB2, 1.0);
            for step in 0..=6 {
                let dist = step as f32;
                let sx = ((cx + sweep_angle.cos() * dist).round() as i32).clamp(0, 31) as u8;
                let sy = ((cy + sweep_angle.sin() * dist).round() as i32).clamp(0, 15) as u8;
                pixels.push((sx, sy, sweep_col));
            }

            // Pulsing target blip
            if (tick / 3) % 4 != 0 {
                let blip_col = map_color(0xFF, 0x4D, 0x4D, 1.0);
                pixels.push((19, 5, blip_col));
                pixels.push((20, 5, blip_col));
            }
        }
        "cyber_glitch" => {
            let is_glitch = (t * 3.5).sin() > 0.85;
            let shift: i32 = if is_glitch {
                ((t * 10.0).sin() * 2.5).round() as i32
            } else {
                0
            };

            let skull_col = if is_glitch {
                map_color(0xFF, 0x33, 0x66, 1.0)
            } else {
                map_color(0xE8, 0xE4, 0xDC, 1.0)
            };

            // Cranium: x: 11..21, y: 3..9
            for x in 11..=20 {
                let sx = ((x as i32) + shift).clamp(0, 31) as u8;
                for y in 3..=8 {
                    // Skip eye sockets at (13..=14, 5..=6) and (17..=18, 5..=6)
                    let in_left_eye = (13..=14).contains(&x) && (5..=6).contains(&y);
                    let in_right_eye = (17..=18).contains(&x) && (5..=6).contains(&y);
                    if !in_left_eye && !in_right_eye {
                        pixels.push((sx, y, skull_col));
                    }
                }
            }

            // Cheek / jaw: x: 13..19, y: 9..13
            for x in 13..=18 {
                let sx = ((x as i32) - shift).clamp(0, 31) as u8;
                for y in 9..=12 {
                    pixels.push((sx, y, skull_col));
                }
            }

            // Glitch horizontal scanline bar
            if is_glitch {
                let scan_col = map_color(0x00, 0xF0, 0xFF, 0.9);
                for x in 5..=26 {
                    pixels.push((x, 7, scan_col));
                }
            }
        }
        "server_rack" => {
            // 3 Server blades at rows 3..6, 7..10, 11..14
            for row in 0..3 {
                let y_base = 3 + row * 4;
                let blade_col = map_color(0x1C, 0x1C, 0x1A, 1.0);
                for x in 6..=25 {
                    for dy in 0..3 {
                        pixels.push((x, y_base + dy, blade_col));
                    }
                }

                // Front ventilation slot
                let vent_col = map_color(0x2A, 0x2A, 0x28, 1.0);
                for x in 7..=18 {
                    pixels.push((x, y_base + 1, vent_col));
                }

                // Blinking activity LEDs
                let led1_on = ((t * (2.1 + row as f32 * 1.3)).sin()) > 0.0;
                let led2_on = ((t * (3.2 + row as f32 * 0.9)).cos()) > 0.2;

                let led1_col = if led1_on {
                    map_color(0x00, 0xE6, 0x76, 1.0)
                } else {
                    map_color(0x11, 0x33, 0x22, 0.4)
                };
                let led2_col = if led2_on {
                    map_color(0x33, 0xB5, 0xE5, 1.0)
                } else {
                    map_color(0x11, 0x22, 0x33, 0.4)
                };

                pixels.push((21, y_base + 1, led1_col));
                pixels.push((23, y_base + 1, led2_col));
            }
        }
        "dot_wave" => {
            // Smooth sinusoidal dot-matrix oscilloscope wave
            let wave_col = map_color(0x38, 0xBD, 0xF8, 1.0);
            let shadow_col = map_color(0x38, 0xBD, 0xF8, 0.25);

            for i in 0..20 {
                let x = 6 + i;
                let wave_y = ((8.0 + (t * 0.5 + (i as f32 * 0.42)).sin() * 4.5).round() as i32)
                    .clamp(1, 14) as u8;
                pixels.push((x, wave_y, wave_col));
                if wave_y + 1 < 16 {
                    pixels.push((x, wave_y + 1, shadow_col));
                }
            }
        }
        _ => {
            // Fallback: simple pulsing dot
            let col = map_color(0xE0, 0x8A, 0x62, 1.0);
            pixels.push((16, 8, col));
        }
    }

    pixels
}

/// Renders a native GPUI Canvas element of the animated pixel companion
pub fn render_pixel_canvas(
    preset_id: String,
    tick: u64,
    color_mode: String,
    theme: Theme,
    is_dark: bool,
    target_width: f32,
    target_height: f32,
) -> impl IntoElement {
    let accent = theme.accent;
    canvas(
        move |_bounds, _window, _cx| (),
        move |bounds, _, window, _cx| {
            let pixels = render_frame(&preset_id, tick, &color_mode, accent, is_dark);
            let pixel_w = bounds.size.width / (GRID_WIDTH as f32);
            let pixel_h = bounds.size.height / (GRID_HEIGHT as f32);

            for (px_x, px_y, col) in pixels {
                let p_bounds = Bounds {
                    origin: point(
                        bounds.origin.x + px(px_x as f32 * f32::from(pixel_w)),
                        bounds.origin.y + px(px_y as f32 * f32::from(pixel_h)),
                    ),
                    size: size(pixel_w, pixel_h),
                };
                window.paint_quad(fill(p_bounds, col));
            }
        },
    )
    .w(px(target_width))
    .h(px(target_height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_art_defaults() {
        let def = PixelArtConfig::default();
        assert!(def.enabled);
        assert_eq!(def.active_preset, "hot_coffee");
        assert_eq!(def.fps, 10);
        assert!(def.speed_boost_on_activity);
        assert_eq!(def.color_mode, "original");
        assert!(def.custom_sprite_path.is_none());
    }

    #[test]
    fn test_pixel_art_json_roundtrip() {
        let cfg = PixelArtConfig {
            enabled: false,
            active_preset: "matrix_rain".to_string(),
            fps: 16,
            speed_boost_on_activity: false,
            color_mode: "monochrome".to_string(),
            custom_sprite_path: Some("C:\\custom.json".to_string()),
        };

        let json = serde_json::to_string(&cfg).expect("serialize");
        let parsed: PixelArtConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.enabled, false);
        assert_eq!(parsed.active_preset, "matrix_rain");
        assert_eq!(parsed.fps, 16);
        assert_eq!(parsed.speed_boost_on_activity, false);
        assert_eq!(parsed.color_mode, "monochrome");
        assert_eq!(parsed.custom_sprite_path, Some("C:\\custom.json".to_string()));
    }

    #[test]
    fn test_all_presets_render_within_grid() {
        let test_ticks = [0, 1, 5, 10, 25, 60, 120];
        let accent = rgba(0xE08A62FF);

        for p in PRESETS {
            for &tick in &test_ticks {
                for &mode in &["original", "theme_accent", "monochrome"] {
                    for &is_dark in &[true, false] {
                        let pixels = render_frame(p.id, tick, mode, accent, is_dark);
                        assert!(!pixels.is_empty(), "Preset '{}' produced empty frame at tick {}", p.id, tick);
                        for &(x, y, _) in &pixels {
                            assert!(
                                (x as usize) < GRID_WIDTH && (y as usize) < GRID_HEIGHT,
                                "Preset '{}' out of bounds: ({}, {}) at tick {}",
                                p.id, x, y, tick
                            );
                        }
                    }
                }
            }
        }
    }
}
