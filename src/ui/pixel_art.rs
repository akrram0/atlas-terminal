//! Top Bar Pixel Art & Geometric Shaders System
//!
//! Provides Nothing-OS / Teenage Engineering inspired mathematical dot-matrix shaders (20x7 chunky LEDs),
//! native animated `.gif` and horizontal `.png` sprite sheet decoding via the `image` crate,
//! unlit background matrix LED rendering with high-contrast active bloom,
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

pub const GRID_WIDTH: usize = 20;
pub const GRID_HEIGHT: usize = 7;

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
    #[serde(default = "default_size")]
    pub size: String,
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

fn default_size() -> String {
    "wide".to_string()
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
            size: default_size(),
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
    pub width: u32,
    pub height: u32,
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

        // Scale to 24px tall nearest-neighbor if larger
        let (target_w, target_h) = if orig_h > 26 {
            let ratio = orig_w as f32 / orig_h as f32;
            (((24.0 * ratio).round() as u32).max(1), 24u32)
        } else {
            (orig_w, orig_h)
        };

        let scaled = if target_w != orig_w || target_h != orig_h {
            image::imageops::resize(&buffer, target_w, target_h, image::imageops::FilterType::Nearest)
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

        frames.push(DecodedFrame {
            width: w,
            height: h,
            delay_ms,
            pixels,
        });
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
            let scaled = if frame_size > 26 {
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
            frames.push(DecodedFrame {
                width: w,
                height: h,
                delay_ms,
                pixels,
            });
        }
    } else {
        let scaled = if height > 26 {
            let ratio = width as f32 / height as f32;
            let target_w = ((24.0 * ratio).round() as u32).max(1);
            image::imageops::resize(&rgba_img, target_w, 24, image::imageops::FilterType::Nearest)
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
            width: w,
            height: h,
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
// High-Craft Geometric / Nothing-OS Shaders & Sprite Renderer (20x7 LED Grid)
// ---------------------------------------------------------------------------

/// Returns `(grid_w, grid_h, pixels)` for the active preset
pub fn render_frame(
    preset_id: &str,
    tick: u64,
    color_mode: &str,
    theme_accent: Rgba,
    is_dark: bool,
) -> (usize, usize, Vec<(u8, u8, Rgba)>) {
    let mut pixels = Vec::with_capacity(96);

    // 1. Check if matching an imported GIF / PNG sprite
    let sprites = load_all_sprites();
    if let Some(anim) = sprites.get(preset_id) {
        if !anim.frames.is_empty() {
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
            return (frame.width as usize, frame.height as usize, pixels);
        }
    }

    // 2. High-Craft Geometric Nothing-OS Shaders (Chunky 20x7 LED Grid)
    match preset_id {
        "glyph_equalizer" => {
            let t = (tick as f32) * 0.16;
            let col_bands = 10;
            let y_base = 6u8; // bottom row in 0..=6

            for b in 0..col_bands {
                let bf = b as f32;
                let f1 = (t * (1.1 + bf * 0.22) + bf * 0.85).sin();
                let f2 = (t * (1.8 + bf * 0.16) - bf * 1.3).cos();
                let raw_h = (f1 * 0.65 + f2 * 0.35 + 1.0) * 0.5;
                let level = (raw_h * 5.0).round().clamp(1.0, 5.0) as u8;

                let peak_phase = (t * 0.9 + bf * 0.7).sin();
                let peak_level = (level + 1 + if peak_phase > 0.35 { 1 } else { 0 }).min(6);

                let x_left = (b * 2) as u8;
                let x_right = x_left + 1;

                // Draw LED bar levels
                for dy in 0..level {
                    let y = y_base.saturating_sub(dy);
                    let alpha = 0.75 + (dy as f32 / 6.0) * 0.25;
                    let base_col = rgba(0xFFB020FF);
                    let col = map_color(base_col, color_mode, theme_accent, is_dark);
                    let c = Rgba { a: col.a * alpha, ..col };
                    pixels.push((x_left, y, c));
                    pixels.push((x_right, y, c));
                }

                // Peak-hold floating LED dot
                let peak_y = y_base.saturating_sub(peak_level);
                let peak_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);
                pixels.push((x_left, peak_y, peak_col));
                pixels.push((x_right, peak_y, peak_col));
            }
        }

        "matrix_stream" => {
            let t = (tick as f32) * 0.32;
            let stream_cols = [1u8, 4, 7, 10, 13, 16, 19];

            for (i, &col_x) in stream_cols.iter().enumerate() {
                let speed = 0.8 + (i as f32 * 0.25);
                let offset = i as f32 * 3.4;
                let head_y = (((t * speed + offset) as i32) % 13) - 3;

                // Bright leading LED
                let head_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);
                if head_y >= 0 && head_y < 7 {
                    pixels.push((col_x, head_y as u8, head_col));
                }

                // 3-step alpha phosphor trail
                let alphas = [0.85, 0.50, 0.22];
                let trail_base = rgba(0x22C55EFF);
                let trail_col = map_color(trail_base, color_mode, theme_accent, is_dark);

                for (step, &a) in alphas.iter().enumerate() {
                    let trail_y = head_y - (step as i32 + 1);
                    if trail_y >= 0 && trail_y < 7 {
                        pixels.push((col_x, trail_y as u8, Rgba { a: trail_col.a * a, ..trail_col }));
                    }
                }
            }
        }

        "orbital_pulse" => {
            let t = (tick as f32) * 0.16;
            let center_x = 9.5f32;
            let center_y = 3.0f32;
            let max_r = 9.0f32;

            let r_x = (t * 3.6) % (max_r + 2.0);
            let r_y = r_x / 2.3;
            let fade = (1.0 - (r_x / max_r)).clamp(0.0, 1.0);
            let ring_col = map_color(rgba(0x38BDF8FF), color_mode, theme_accent, is_dark);

            if r_x > 0.8 && r_x <= max_r {
                for deg in (0..360).step_by(18) {
                    let rad = (deg as f32) * std::f32::consts::PI / 180.0;
                    let px = (center_x + rad.cos() * r_x).round() as i32;
                    let py = (center_y + rad.sin() * r_y).round() as i32;
                    if px >= 0 && px < 20 && py >= 0 && py < 7 {
                        pixels.push((px as u8, py as u8, Rgba { a: ring_col.a * fade, ..ring_col }));
                    }
                }
            }

            // Central beacon & sweep blip
            let beacon_col = map_color(rgba(0x00F0FFEE), color_mode, theme_accent, is_dark);
            pixels.push((9, 3, beacon_col));
            pixels.push((10, 3, beacon_col));

            let blip_angle = t * 2.2;
            let blip_x = (center_x + blip_angle.cos() * 5.0).round() as i32;
            let blip_y = (center_y + blip_angle.sin() * 2.2).round() as i32;
            if blip_x >= 0 && blip_x < 20 && blip_y >= 0 && blip_y < 7 {
                let blip_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);
                pixels.push((blip_x as u8, blip_y as u8, blip_col));
            }
        }

        "sine_wave" => {
            let t = (tick as f32) * 0.16;
            let wave1_col = map_color(rgba(0x38BDF8FF), color_mode, theme_accent, is_dark);
            let wave2_col = map_color(rgba(0xF43F5EFF), color_mode, theme_accent, is_dark);
            let inter_col = map_color(rgba(0xFFFFFFFF), color_mode, theme_accent, is_dark);

            for x in 0..20 {
                let xf = x as f32;
                let y1 = (3.0 + (t * 1.5 + xf * 0.44).sin() * 2.6).round().clamp(0.0, 6.0) as u8;
                let y2 = (3.0 + (t * 1.5 - xf * 0.44 + std::f32::consts::PI).sin() * 2.6).round().clamp(0.0, 6.0) as u8;

                if y1 == y2 {
                    pixels.push((x as u8, y1, inter_col));
                } else {
                    pixels.push((x as u8, y1, wave1_col));
                    pixels.push((x as u8, y2, wave2_col));
                }
            }
        }

        "network_heartbeat" => {
            let t = (tick as f32) * 0.20;
            let rack_guide = map_color(rgba(0x64748B99), color_mode, theme_accent, is_dark);
            let active_led = map_color(rgba(0x22C55EFF), color_mode, theme_accent, is_dark);
            let link_glow = map_color(rgba(0x06B6D4FF), color_mode, theme_accent, is_dark);

            let blade_ys = [1u8, 3, 5];
            for (b_idx, &by) in blade_ys.iter().enumerate() {
                let b = b_idx as f32;
                pixels.push((1, by, rack_guide));
                pixels.push((2, by, rack_guide));

                for p in 0..7 {
                    let px = 4 + p * 2;
                    let led_pulse = (t * 2.8 + b * 1.2 + (p as f32) * 0.6).sin();
                    if led_pulse > 0.15 {
                        pixels.push((px, by, active_led));
                    }
                }

                let link_pulse = (t * 0.9 + b * 1.5).sin() > -0.4;
                if link_pulse {
                    pixels.push((18, by, link_glow));
                }
            }
        }

        "minimal_coffee" => {
            let t = (tick as f32) * 0.15;
            let cup_col = map_color(rgba(0xEDE8DCFF), color_mode, theme_accent, is_dark);
            let steam_col = map_color(rgba(0xE2E8F0DD), color_mode, theme_accent, is_dark);

            // Saucer: row 6, cols 6..=14
            for x in 6..=14 {
                pixels.push((x, 6, cup_col));
            }

            // Cup body: rows 4..5, cols 8..=12
            for x in 8..=12 {
                pixels.push((x, 4, cup_col));
                pixels.push((x, 5, cup_col));
            }

            // Cup rim: row 3, cols 7..=13
            for x in 7..=13 {
                pixels.push((x, 3, cup_col));
            }

            // Handle: row 4, col 14
            pixels.push((14, 4, cup_col));

            // Rising steam: rows 0..2
            let s1_x = (9.5 + (t * 1.8).sin() * 1.5).round().clamp(7.0, 13.0) as u8;
            let s2_x = (11.5 + (t * 1.8 + 1.6).cos() * 1.5).round().clamp(7.0, 13.0) as u8;
            pixels.push((s1_x, 2, steam_col));
            pixels.push((s2_x, 1, steam_col));
            pixels.push((s1_x, 0, Rgba { a: steam_col.a * 0.6, ..steam_col }));
        }

        _ => {
            return render_frame("glyph_equalizer", tick, color_mode, theme_accent, is_dark);
        }
    }

    (GRID_WIDTH, GRID_HEIGHT, pixels)
}

/// Renders a native GPUI Canvas element with chunky LED dots, unlit matrix background, and glowing active LEDs
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
            let (grid_w, grid_h, pixels) = render_frame(&preset_id, tick, &color_mode, accent, is_dark);
            let pixel_w = bounds.size.width / (grid_w as f32);
            let pixel_h = bounds.size.height / (grid_h as f32);

            // 1. Paint unlit background matrix LED dots for geometric shaders (Nothing OS matrix look)
            if !is_sprite {
                let unlit_col = if is_dark {
                    rgba(0xFFFFFF14) // subtle 8% unlit glass LED
                } else {
                    rgba(0x00000012)
                };
                let dot_inset_x = pixel_w * 0.16;
                let dot_inset_y = pixel_h * 0.16;
                let dot_w = pixel_w * 0.68;
                let dot_h = pixel_h * 0.68;
                let dot_radius = (dot_w.min(dot_h) * 0.42).min(px(2.0));

                for gy in 0..grid_h {
                    for gx in 0..grid_w {
                        let p_bounds = Bounds {
                            origin: point(
                                bounds.origin.x + px(gx as f32 * f32::from(pixel_w)) + dot_inset_x,
                                bounds.origin.y + px(gy as f32 * f32::from(pixel_h)) + dot_inset_y,
                            ),
                            size: size(dot_w, dot_h),
                        };
                        window.paint_quad(fill(p_bounds, unlit_col).corner_radii(dot_radius));
                    }
                }
            }

            // 2. Paint active lit LEDs with subtle bloom highlight
            let dot_inset_x = if is_sprite { px(0.0) } else { pixel_w * 0.12 };
            let dot_inset_y = if is_sprite { px(0.0) } else { pixel_h * 0.12 };
            let dot_w = if is_sprite { pixel_w } else { pixel_w * 0.76 };
            let dot_h = if is_sprite { pixel_h } else { pixel_h * 0.76 };
            let dot_radius = if is_sprite {
                px(0.0)
            } else {
                (dot_w.min(dot_h) * 0.45).min(px(2.0))
            };

            for (px_x, px_y, col) in pixels {
                let px_pt = bounds.origin.x + px(px_x as f32 * f32::from(pixel_w));
                let py_pt = bounds.origin.y + px(px_y as f32 * f32::from(pixel_h));

                if !is_sprite && col.a > 0.35 {
                    // Subtle ambient bloom behind active LED
                    let halo_bounds = Bounds {
                        origin: point(px_pt, py_pt),
                        size: size(pixel_w, pixel_h),
                    };
                    window.paint_quad(
                        fill(halo_bounds, Rgba { a: col.a * 0.22, ..col }).corner_radii(dot_radius),
                    );
                }

                // Core high-contrast active LED dot
                let p_bounds = Bounds {
                    origin: point(px_pt + dot_inset_x, py_pt + dot_inset_y),
                    size: size(dot_w, dot_h),
                };
                window.paint_quad(fill(p_bounds, col).corner_radii(dot_radius));
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
        assert_eq!(def.size, "wide");
        assert_eq!(def.fps, 10);
        assert!(def.speed_boost_on_activity);
        assert_eq!(def.color_mode, "original");
        assert!(def.custom_sprite_path.is_none());
    }

    #[test]
    fn test_all_geometric_presets_render_within_20x7() {
        let test_ticks = [0, 1, 5, 10, 25, 60];
        let accent = rgba(0xE08A62FF);

        for p in PRESETS {
            for &tick in &test_ticks {
                for &mode in &["original", "theme_accent", "monochrome"] {
                    for &is_dark in &[true, false] {
                        let (gw, gh, pixels) = render_frame(p.id, tick, mode, accent, is_dark);
                        assert_eq!(gw, GRID_WIDTH);
                        assert_eq!(gh, GRID_HEIGHT);
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
        let (_, _, pixels) = render_frame("glyph_equalizer", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 16, "Equalizer must produce rich active LED columns");
    }

    #[test]
    fn test_minimal_coffee_clean_saucer_and_steam() {
        let (_, _, pixels) = render_frame("minimal_coffee", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 12, "Minimal coffee must render silhouette and steam");
        let has_saucer = pixels.iter().any(|&(_, y, _)| y == 6);
        assert!(has_saucer, "Minimal coffee must have saucer baseline on row 6");
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
