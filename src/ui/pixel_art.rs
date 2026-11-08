//! Top Bar Pixel Art & Geometric Shaders System
//!
//! Provides Nothing-OS / Teenage Engineering inspired mathematical dot-matrix shaders,
//! native animated `.gif` and horizontal `.png` sprite sheet decoding via the `image` crate,
//! a standalone configuration file (`~/.config/atlas/pixel_art.json`),
//! and GPU-accelerated nearest-neighbor GPUI canvas rendering.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use gpui::prelude::*;
use gpui::{canvas, fill, point, px, rgba, size, Bounds, IntoElement, Rgba};
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

pub const GRID_WIDTH: usize = 24;
pub const GRID_HEIGHT: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetCategory {
    Geometric,
    #[allow(dead_code)]
    Sprites,
}

impl PresetCategory {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Geometric => "geometric",
            Self::Sprites => "sprites",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Geometric => "Matrix Shaders",
            Self::Sprites => "GIFs & Sprites",
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

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "glyph_equalizer",
        name: "Glyph Equalizer",
        category: PresetCategory::Geometric,
        desc: "8-band studio spectrum analyzer with floating peak-hold LEDs.",
    },
    Preset {
        id: "matrix_stream",
        name: "Matrix Stream",
        category: PresetCategory::Geometric,
        desc: "Digital rain columns with 5-step smooth phosphor alpha trails.",
    },
    Preset {
        id: "orbital_pulse",
        name: "Orbital Pulse",
        category: PresetCategory::Geometric,
        desc: "Concentric radar sonar sweep rings radiating on a telemetry grid.",
    },
    Preset {
        id: "sine_wave",
        name: "Sine Wave",
        category: PresetCategory::Geometric,
        desc: "Dual intertwined sinusoidal dot waves with phase interference nodes.",
    },
    Preset {
        id: "network_heartbeat",
        name: "Network Heartbeat",
        category: PresetCategory::Geometric,
        desc: "Server switch blade telemetry matrix with rhythmic packet bursts.",
    },
    Preset {
        id: "minimal_coffee",
        name: "Minimal Coffee",
        category: PresetCategory::Geometric,
        desc: "Monochrome 1-bit cup silhouette with rising mathematical steam wisps.",
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
    "glyph_equalizer".to_string()
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

/// Standalone config file path: `~/.config/atlas/pixel_art.json`
pub fn pixel_art_config_path() -> PathBuf {
    crate::user_data_dir().join("pixel_art.json")
}

/// Primary directory for user imported `.gif` and `.png` sprites: `~/.config/atlas/pixel_sprites/`
pub fn pixel_sprites_dir() -> PathBuf {
    crate::user_data_dir().join("pixel_sprites")
}

/// Legacy / secondary sprites directory: `%APPDATA%\AtlasTerminal\pixel_sprites\`
pub fn legacy_pixel_sprites_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("AtlasTerminal").join("pixel_sprites"))
}

/// Assets sprites directory: `assets/pixel_art/`
pub fn assets_pixel_sprites_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("assets").join("pixel_art");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("assets").join("pixel_art")
}

impl PixelArtConfig {
    pub fn load() -> Self {
        let modern_path = pixel_art_config_path();

        // 1. Check standard ~/.config/atlas/pixel_art.json
        if modern_path.exists() {
            if let Ok(content) = fs::read_to_string(&modern_path) {
                if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                    return cfg;
                }
            }
        }

        // 2. Seamless automatic migration from legacy %APPDATA%\AtlasTerminal\pixel_art.json
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let legacy_path = PathBuf::from(appdata).join("AtlasTerminal").join("pixel_art.json");
            if legacy_path.exists() {
                if let Ok(content) = fs::read_to_string(&legacy_path) {
                    if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                        cfg.save();
                        let _ = fs::remove_file(&legacy_path);
                        return cfg;
                    }
                }
            }
        }

        // 3. Fallback: create modern default
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

// ---------------------------------------------------------------------------
// Native GIF & PNG Sprite Sheet Decoder & Cache
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct DecodedFrame {
    pub delay_ms: u32,
    pub pixels: Vec<(u8, u8, Rgba)>,
}

#[derive(Clone)]
pub struct DecodedSpriteAnimation {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub frames: Vec<DecodedFrame>,
    pub total_duration_ms: u64,
}

static SPRITE_CACHE: RwLock<Option<(std::time::Instant, HashMap<String, Arc<DecodedSpriteAnimation>>)>> =
    RwLock::new(None);

pub fn invalidate_sprite_cache() {
    if let Ok(mut lock) = SPRITE_CACHE.write() {
        *lock = None;
    }
}

/// Decodes an animated `.gif` using the `image` crate with nearest-neighbor downscaling
fn decode_gif(id: String, name: String, path: PathBuf, bytes: &[u8]) -> Option<DecodedSpriteAnimation> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).ok()?;
    let raw_frames = decoder.into_frames().collect_frames().ok()?;
    if raw_frames.is_empty() {
        return None;
    }

    let mut frames = Vec::with_capacity(raw_frames.len());
    let mut total_duration_ms = 0u64;

    for frame in raw_frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 || numer == 0 {
            100 // default 10 FPS
        } else {
            (numer / denom).clamp(16, 2000)
        };
        total_duration_ms += delay_ms as u64;

        let buffer = frame.into_buffer();
        let orig_w = buffer.width();
        let orig_h = buffer.height();

        // Scale to 24x24 nearest-neighbor if larger
        let scaled = if orig_w > 32 || orig_h > 32 {
            image::imageops::resize(&buffer, 24, 24, image::imageops::FilterType::Nearest)
        } else {
            buffer
        };

        let w = scaled.width();
        let h = scaled.height();
        let mut pixels = Vec::with_capacity((w * h) as usize);

        for y in 0..h {
            for x in 0..w {
                let p = scaled.get_pixel(x, y);
                if p.0[3] > 10 {
                    pixels.push((
                        x as u8,
                        y as u8,
                        rgba(
                            ((p.0[0] as u32) << 24)
                                | ((p.0[1] as u32) << 16)
                                | ((p.0[2] as u32) << 8)
                                | (p.0[3] as u32),
                        ),
                    ));
                }
            }
        }

        frames.push(DecodedFrame { delay_ms, pixels });
    }

    Some(DecodedSpriteAnimation {
        id,
        name,
        path,
        frames,
        total_duration_ms: total_duration_ms.max(1),
    })
}

/// Decodes a `.png` file (supporting horizontal sprite sheets or single frames)
fn decode_png(id: String, name: String, path: PathBuf, bytes: &[u8]) -> Option<DecodedSpriteAnimation> {
    let dynamic = image::load_from_memory(bytes).ok()?;
    let rgba_img = dynamic.to_rgba8();
    let width = rgba_img.width();
    let height = rgba_img.height();
    if width == 0 || height == 0 {
        return None;
    }

    let mut frames = Vec::new();
    let delay_ms = 120u32; // ~8.3 FPS for sprite sheets

    // Horizontal sprite sheet: width > height and evenly divisible
    if width > height && height > 0 && width % height == 0 {
        let num_frames = width / height;
        let frame_size = height;

        for i in 0..num_frames {
            let sub_view = image::imageops::crop_imm(&rgba_img, i * frame_size, 0, frame_size, frame_size).to_image();
            let scaled = if frame_size > 32 {
                image::imageops::resize(&sub_view, 24, 24, image::imageops::FilterType::Nearest)
            } else {
                sub_view
            };

            let w = scaled.width();
            let h = scaled.height();
            let mut pixels = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let p = scaled.get_pixel(x, y);
                    if p.0[3] > 10 {
                        pixels.push((
                            x as u8,
                            y as u8,
                            rgba(
                                ((p.0[0] as u32) << 24)
                                    | ((p.0[1] as u32) << 16)
                                    | ((p.0[2] as u32) << 8)
                                    | (p.0[3] as u32),
                            ),
                        ));
                    }
                }
            }
            frames.push(DecodedFrame { delay_ms, pixels });
        }
    } else {
        let scaled = if width > 32 || height > 32 {
            image::imageops::resize(&rgba_img, 24, 24, image::imageops::FilterType::Nearest)
        } else {
            rgba_img
        };

        let w = scaled.width();
        let h = scaled.height();
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let p = scaled.get_pixel(x, y);
                if p.0[3] > 10 {
                    pixels.push((
                        x as u8,
                        y as u8,
                        rgba(
                            ((p.0[0] as u32) << 24)
                                | ((p.0[1] as u32) << 16)
                                | ((p.0[2] as u32) << 8)
                                | (p.0[3] as u32),
                        ),
                    ));
                }
            }
        }
        frames.push(DecodedFrame {
            delay_ms: 1000,
            pixels,
        });
    }

    let total_duration_ms = (frames.len() as u32 * delay_ms) as u64;
    Some(DecodedSpriteAnimation {
        id,
        name,
        path,
        frames,
        total_duration_ms: total_duration_ms.max(1),
    })
}

fn scan_dir_for_sprites(dir: &Path, map: &mut HashMap<String, Arc<DecodedSpriteAnimation>>) {
    if !dir.exists() {
        let _ = fs::create_dir_all(dir);
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext != "gif" && ext != "png" {
                continue;
            }

            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("sprite").to_string();
            let id = format!("sprite:{}", stem);

            // Avoid re-decoding if already loaded
            if map.contains_key(&id) {
                continue;
            }

            if let Ok(bytes) = fs::read(&path) {
                let human_name = stem
                    .replace('_', " ")
                    .replace('-', " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                let decoded = match ext.as_str() {
                    "gif" => decode_gif(id.clone(), human_name, path.clone(), &bytes),
                    "png" => decode_png(id.clone(), human_name, path.clone(), &bytes),
                    _ => None,
                };

                if let Some(anim) = decoded {
                    map.insert(id, Arc::new(anim));
                }
            }
        }
    }
}

/// Loads and caches all available sprite animations across user and assets directories
pub fn load_all_sprites() -> HashMap<String, Arc<DecodedSpriteAnimation>> {
    if let Ok(guard) = SPRITE_CACHE.read() {
        if let Some((instant, ref map)) = *guard {
            if instant.elapsed().as_millis() < 1500 {
                return map.clone();
            }
        }
    }

    let mut map = HashMap::new();

    // 1. Primary ~/.config/atlas/pixel_sprites/
    scan_dir_for_sprites(&pixel_sprites_dir(), &mut map);

    // 2. Legacy %APPDATA%\AtlasTerminal\pixel_sprites\
    if let Some(legacy) = legacy_pixel_sprites_dir() {
        scan_dir_for_sprites(&legacy, &mut map);
    }

    // 3. Project assets/pixel_art/
    scan_dir_for_sprites(&assets_pixel_sprites_dir(), &mut map);

    if let Ok(mut guard) = SPRITE_CACHE.write() {
        *guard = Some((std::time::Instant::now(), map.clone()));
    }

    map
}

/// Spawns a native file picker dialog on Windows (non-blocking when called inside `smol::unblock`)
pub fn pick_sprite_file() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let ps_script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dlg = New-Object System.Windows.Forms.OpenFileDialog
            $dlg.Title = 'Select Pixel Art GIF or PNG Sprite Sheet'
            $dlg.Filter = 'Pixel Art Files (*.gif;*.png)|*.gif;*.png|All Files (*.*)|*.*'
            if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                Write-Output $dlg.FileName
            }
        "#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
            .output()
            .ok()?;
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() && Path::new(&path_str).exists() {
            return Some(PathBuf::from(path_str));
        }
    }
    None
}

/// Imports a chosen GIF or PNG file into `~/.config/atlas/pixel_sprites/`
pub fn import_sprite_from_path(src: &Path) -> Option<String> {
    let dest_dir = pixel_sprites_dir();
    let _ = fs::create_dir_all(&dest_dir);
    let file_name = src.file_name()?;
    let dest_path = dest_dir.join(file_name);
    let _ = fs::copy(src, &dest_path);

    if let Some(legacy) = legacy_pixel_sprites_dir() {
        let _ = fs::create_dir_all(&legacy);
        let _ = fs::copy(src, legacy.join(file_name));
    }

    invalidate_sprite_cache();

    let stem = dest_path.file_stem()?.to_string_lossy().to_string();
    Some(format!("sprite:{}", stem))
}

// ---------------------------------------------------------------------------
// Color Mode Mapper
// ---------------------------------------------------------------------------

fn map_color(original: Rgba, color_mode: &str, theme_accent: Rgba, is_dark: bool) -> Rgba {
    match color_mode {
        "monochrome" => {
            let lum = 0.299 * original.r + 0.587 * original.g + 0.114 * original.b;
            if is_dark {
                let v = (lum * 0.8 + 0.2).clamp(0.0, 1.0);
                Rgba {
                    r: v,
                    g: v,
                    b: v,
                    a: original.a,
                }
            } else {
                let v = (1.0 - lum * 0.85).clamp(0.0, 1.0);
                Rgba {
                    r: v,
                    g: v,
                    b: v,
                    a: original.a,
                }
            }
        }
        "theme_accent" => {
            let lum = 0.299 * original.r + 0.587 * original.g + 0.114 * original.b;
            let factor = (lum * 0.75 + 0.25).clamp(0.0, 1.0);
            Rgba {
                r: (theme_accent.r * factor).clamp(0.0, 1.0),
                g: (theme_accent.g * factor).clamp(0.0, 1.0),
                b: (theme_accent.b * factor).clamp(0.0, 1.0),
                a: original.a,
            }
        }
        _ => original,
    }
}

// ---------------------------------------------------------------------------
// High-Craft Geometric / Nothing-OS Shaders & Sprite Renderer
// ---------------------------------------------------------------------------

/// Generates the list of active pixel points `(x, y, Rgba)` for the current tick
pub fn render_frame(
    preset_id: &str,
    tick: u64,
    color_mode: &str,
    theme_accent: Rgba,
    is_dark: bool,
) -> Vec<(u8, u8, Rgba)> {
    let mut pixels = Vec::with_capacity(160);

    // 1. Check if matching an imported GIF / PNG sprite
    let sprites = load_all_sprites();
    if let Some(anim) = sprites.get(preset_id) {
        if !anim.frames.is_empty() {
            // Select frame based on natural frame delay
            let elapsed_ms = (tick * 100) % anim.total_duration_ms;
            let mut acc_ms = 0u64;
            let mut frame_idx = 0;
            for (idx, f) in anim.frames.iter().enumerate() {
                acc_ms += f.delay_ms as u64;
                if elapsed_ms < acc_ms {
                    frame_idx = idx;
                    break;
                }
            }

            let frame = &anim.frames[frame_idx];
            for &(x, y, col) in &frame.pixels {
                let final_col = map_color(col, color_mode, theme_accent, is_dark);
                pixels.push((x, y, final_col));
            }
            return pixels;
        }
    }

    // 2. High-Craft Geometric Nothing-OS Shaders
    match preset_id {
        "glyph_equalizer" => {
            let t = (tick as f32) * 0.12;
            let col_xs = [2u8, 5, 8, 11, 14, 17, 20, 23];
            let max_bars = 12;
            let y_base = 20u8;

            for (b_idx, &cx) in col_xs.iter().enumerate() {
                let b = b_idx as f32;
                let f1 = (t * (1.1 + b * 0.22) + b * 0.9).sin();
                let f2 = (t * (1.8 + b * 0.15) - b * 1.4).cos();
                let raw_h = (f1 * 0.6 + f2 * 0.4 + 1.0) * 0.5;
                let level = (raw_h * (max_bars as f32)).round().clamp(1.0, max_bars as f32) as u8;

                let peak_phase = (t * 0.8 + b * 0.7).sin();
                let peak_level = (level + 2 + if peak_phase > 0.3 { 1 } else { 0 }).min(max_bars + 2);

                for dy in 0..level {
                    let y = y_base.saturating_sub(dy);
                    let alpha = 0.55 + (dy as f32 / max_bars as f32) * 0.45;
                    let col = map_color(rgba(0xFFB020FF), color_mode, theme_accent, is_dark);
                    pixels.push((cx, y, Rgba { a: col.a * alpha, ..col }));
                    if cx + 1 < 24 {
                        pixels.push((cx + 1, y, Rgba { a: col.a * alpha * 0.85, ..col }));
                    }
                }

                let peak_y = y_base.saturating_sub(peak_level);
                let peak_col = map_color(rgba(0xFFFFFFEE), color_mode, theme_accent, is_dark);
                pixels.push((cx, peak_y, peak_col));
                if cx + 1 < 24 {
                    pixels.push((cx + 1, peak_y, Rgba { a: peak_col.a * 0.8, ..peak_col }));
                }
            }
        }

        "matrix_stream" => {
            let t = (tick as f32) * 0.35;
            let stream_cols = [2u8, 6, 10, 14, 18, 22];

            for (i, &col_x) in stream_cols.iter().enumerate() {
                let speed = 0.9 + (i as f32 * 0.28);
                let offset = i as f32 * 6.7;
                let head_y = (((t * speed + offset) as i32) % 32) - 6;

                let head_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);
                if head_y >= 0 && head_y < 24 {
                    pixels.push((col_x, head_y as u8, head_col));
                }

                let alphas = [0.85, 0.65, 0.45, 0.25, 0.10];
                let trail_base = rgba(0x22C55EFF);
                let trail_col = map_color(trail_base, color_mode, theme_accent, is_dark);

                for (step, &a) in alphas.iter().enumerate() {
                    let trail_y = head_y - (step as i32 + 1);
                    if trail_y >= 0 && trail_y < 24 {
                        pixels.push((col_x, trail_y as u8, Rgba { a: trail_col.a * a, ..trail_col }));
                    }
                }
            }
        }

        "orbital_pulse" => {
            let t = (tick as f32) * 0.14;
            let center_x = 11.5f32;
            let center_y = 11.5f32;
            let max_r = 10.5f32;

            let r = (t * 4.5) % (max_r + 2.0);
            let fade = (1.0 - (r / max_r)).clamp(0.0, 1.0);
            let ring_col = map_color(rgba(0x38BDF8FF), color_mode, theme_accent, is_dark);

            if r > 1.0 && r <= max_r {
                for deg in (0..360).step_by(12) {
                    let rad = (deg as f32) * std::f32::consts::PI / 180.0;
                    let px = (center_x + rad.cos() * r).round() as i32;
                    let py = (center_y + rad.sin() * r).round() as i32;
                    if px >= 0 && px < 24 && py >= 0 && py < 24 {
                        pixels.push((px as u8, py as u8, Rgba { a: ring_col.a * fade * 0.9, ..ring_col }));
                    }
                }
            }

            let grid_col = map_color(rgba(0x64748B44), color_mode, theme_accent, is_dark);
            for &gx in &[3, 11, 12, 20] {
                for &gy in &[3, 11, 12, 20] {
                    if (gx == 11 || gx == 12) && (gy == 11 || gy == 12) {
                        let c_col = map_color(rgba(0x38BDF8EE), color_mode, theme_accent, is_dark);
                        pixels.push((gx as u8, gy as u8, c_col));
                    } else {
                        pixels.push((gx as u8, gy as u8, grid_col));
                    }
                }
            }

            let blip_angle = t * 2.2;
            let blip_r = 6.0f32;
            let bx = (center_x + blip_angle.cos() * blip_r).round() as i32;
            let by = (center_y + blip_angle.sin() * blip_r).round() as i32;
            if bx >= 0 && bx < 24 && by >= 0 && by < 24 {
                let blip_col = map_color(rgba(0x00F0FFEE), color_mode, theme_accent, is_dark);
                pixels.push((bx as u8, by as u8, blip_col));
            }
        }

        "sine_wave" => {
            let t = (tick as f32) * 0.15;
            let wave1_col = map_color(rgba(0x38BDF8FF), color_mode, theme_accent, is_dark);
            let wave2_col = map_color(rgba(0xF43F5EFF), color_mode, theme_accent, is_dark);
            let inter_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);

            for x in 2..=21 {
                let xf = x as f32;
                let y1 = (11.5 + (t * 1.6 + xf * 0.42).sin() * 5.8).round() as i32;
                let y2 = (11.5 + (t * 1.6 - xf * 0.42 + std::f32::consts::PI).sin() * 5.8).round() as i32;

                let is_intersect = (y1 - y2).abs() <= 1;

                if y1 >= 0 && y1 < 24 {
                    if is_intersect {
                        pixels.push((x as u8, y1 as u8, inter_col));
                    } else {
                        pixels.push((x as u8, y1 as u8, wave1_col));
                    }
                }

                if y2 >= 0 && y2 < 24 && !is_intersect {
                    pixels.push((x as u8, y2 as u8, wave2_col));
                }
            }
        }

        "network_heartbeat" => {
            let t = (tick as f32) * 0.18;
            let rack_bg = map_color(rgba(0x33415555), color_mode, theme_accent, is_dark);
            let active_led = map_color(rgba(0x22C55EFF), color_mode, theme_accent, is_dark);
            let port_glow = map_color(rgba(0x06B6D4FF), color_mode, theme_accent, is_dark);

            let blade_ys = [5u8, 9, 13, 17];
            for (b_idx, &by) in blade_ys.iter().enumerate() {
                let b = b_idx as f32;
                pixels.push((2, by, rack_bg));
                pixels.push((3, by, rack_bg));

                for p in 0..6 {
                    let px = 6 + p * 2;
                    let led_pulse = (t * 2.5 + b * 1.3 + (p as f32) * 0.7).sin();
                    if led_pulse > 0.25 {
                        pixels.push((px, by, active_led));
                    } else {
                        pixels.push((px, by, Rgba { a: active_led.a * 0.15, ..active_led }));
                    }
                }

                let link_on = (t * 0.8 + b).sin() > -0.5;
                let link_col = if link_on { port_glow } else { Rgba { a: port_glow.a * 0.15, ..port_glow } };
                pixels.push((19, by, link_col));
                pixels.push((21, by, link_col));
            }
        }

        "minimal_coffee" => {
            let t = (tick as f32) * 0.12;
            let cup_col = map_color(rgba(0xEDE8DCFF), color_mode, theme_accent, is_dark);
            let steam_col = map_color(rgba(0xE2E8F0AA), color_mode, theme_accent, is_dark);

            // Saucer line
            for x in 4..=19 {
                pixels.push((x, 21, cup_col));
            }

            // Cup bottom & walls
            for x in 8..=15 {
                pixels.push((x, 19, cup_col));
            }
            pixels.push((7, 18, cup_col));
            pixels.push((16, 18, cup_col));

            for y in 12..=17 {
                pixels.push((6, y, cup_col));
                pixels.push((17, y, cup_col));
            }

            for x in 6..=17 {
                pixels.push((x, 11, cup_col));
            }

            // Loop handle
            pixels.push((18, 13, cup_col));
            pixels.push((19, 13, cup_col));
            pixels.push((20, 14, cup_col));
            pixels.push((20, 15, cup_col));
            pixels.push((19, 16, cup_col));
            pixels.push((18, 16, cup_col));

            // Rising mathematical steam wisps
            for step in 0..9 {
                let y = 10u8.saturating_sub(step);
                let prog = step as f32 / 9.0;
                let alpha = (1.0 - prog) * 0.85;

                let wisp1_x = (9.5 + (t * 1.8 - (step as f32) * 0.7).sin() * 1.8).round() as i32;
                if wisp1_x >= 0 && wisp1_x < 24 {
                    pixels.push((wisp1_x as u8, y, Rgba { a: steam_col.a * alpha, ..steam_col }));
                }

                let wisp2_x = (13.5 + (t * 1.8 - (step as f32) * 0.7 + std::f32::consts::PI * 0.5).cos() * 1.8).round() as i32;
                if wisp2_x >= 0 && wisp2_x < 24 {
                    pixels.push((wisp2_x as u8, y, Rgba { a: steam_col.a * alpha, ..steam_col }));
                }
            }
        }

        _ => {
            // Default fallback: glyph equalizer
            return render_frame("glyph_equalizer", tick, color_mode, theme_accent, is_dark);
        }
    }

    pixels
}

/// Renders a native GPUI Canvas element with rounded LED dots and razor-sharp nearest-neighbor scaling
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
    let is_sprite = preset_id.starts_with("sprite:");
    canvas(
        move |_bounds, _window, _cx| (),
        move |bounds, _, window, _cx| {
            let pixels = render_frame(&preset_id, tick, &color_mode, accent, is_dark);
            let pixel_w = bounds.size.width / (GRID_WIDTH as f32);
            let pixel_h = bounds.size.height / (GRID_HEIGHT as f32);

            // Subtle LED micro-rounding for geometric shaders, sharp nearest-neighbor for sprites
            let radius = if is_sprite {
                px(0.0)
            } else if target_width >= 48.0 {
                px(1.5)
            } else {
                px(0.5)
            };

            for (px_x, px_y, col) in pixels {
                let p_bounds = Bounds {
                    origin: point(
                        bounds.origin.x + px(px_x as f32 * f32::from(pixel_w)),
                        bounds.origin.y + px(px_y as f32 * f32::from(pixel_h)),
                    ),
                    size: size(pixel_w, pixel_h),
                };
                window.paint_quad(fill(p_bounds, col).corner_radii(radius));
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
        assert_eq!(def.active_preset, "glyph_equalizer");
        assert_eq!(def.fps, 10);
        assert!(def.speed_boost_on_activity);
        assert_eq!(def.color_mode, "original");
        assert!(def.custom_sprite_path.is_none());
    }

    #[test]
    fn test_all_geometric_presets_render_within_24x24() {
        let test_ticks = [0, 1, 5, 10, 25, 60];
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

    #[test]
    fn test_glyph_equalizer_bands_and_peak() {
        let pixels = render_frame("glyph_equalizer", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 16, "Equalizer must produce rich active LED columns");
    }

    #[test]
    fn test_minimal_coffee_clean_saucer_and_steam() {
        let pixels = render_frame("minimal_coffee", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 20, "Minimal coffee must render silhouette and steam");
        // Saucer on row 21
        let has_saucer = pixels.iter().any(|&(_, y, _)| y == 21);
        assert!(has_saucer, "Minimal coffee must have saucer baseline");
    }

    #[test]
    fn test_png_sprite_sheet_decode() {
        let mut img = image::RgbaImage::new(48, 24);
        img.put_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(34, 10, image::Rgba([0, 255, 0, 255]));

        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode png");

        let decoded = decode_png(
            "test_sheet".to_string(),
            "Test Sheet".to_string(),
            PathBuf::from("test.png"),
            &bytes,
        );
        assert!(decoded.is_some());
        let anim = decoded.unwrap();
        assert_eq!(anim.frames.len(), 2, "48x24 must slice into 2 frames of 24x24");
        assert_eq!(anim.frames[0].pixels.len(), 1);
        assert_eq!(anim.frames[1].pixels.len(), 1);
    }
}
