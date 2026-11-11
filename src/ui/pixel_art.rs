//! Top Bar Pixel Art Companion System
//!
//! Provides handcrafted minimalist 24x24 pixel art animations,
//! native animated `.gif` and horizontal `.png` sprite sheet decoding via the `image` crate,
//! custom JSON sprite loading from `~/.config/atlas/pixel_art/`,
//! solid contiguous square pixel GPU canvas rendering (no gaps or dot grids),
//! a standalone configuration file (`~/.config/atlas/pixel_art.json`),
//! and native GPUI canvas rendering for both top-bar and Settings UI preview cards.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Cursor;
use image::GenericImage;
use std::sync::{Arc, OnceLock, RwLock};

use gpui::prelude::*;
use gpui::{canvas, fill, point, px, rgba, size, Bounds, Corners, IntoElement, RenderImage, Rgba};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

pub const GRID_WIDTH: usize = 24;
pub const GRID_HEIGHT: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetCategory {
    Cozy,
    System,
    #[allow(dead_code)]
    Custom,
}

impl PresetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cozy => "cozy",
            Self::System => "system",
            Self::Custom => "custom",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Cozy => "Cozy & Chill",
            Self::System => "System & Retro",
            Self::Custom => "Custom Creations",
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
    Preset {
        id: "white_cat",
        name: "White Cat",
        category: PresetCategory::Cozy,
        desc: "Graceful white cat companion with gentle tail swish.",
    },
    Preset {
        id: "blue_penguin",
        name: "Playful Penguin",
        category: PresetCategory::Cozy,
        desc: "Adorable animated penguin with cheerful bouncing flippers.",
    },
    Preset {
        id: "electric_fox",
        name: "Electric Fox",
        category: PresetCategory::Cozy,
        desc: "Spirited lightning fox companion ready for quick keystrokes.",
    },
    Preset {
        id: "shadow_bat",
        name: "Shadow Bat",
        category: PresetCategory::System,
        desc: "Mysterious nocturnal shadow bat hovering quietly.",
    },
    Preset {
        id: "dark_knight",
        name: "Dark Knight",
        category: PresetCategory::System,
        desc: "Valiant knight in ornate plate armor guarding your terminal.",
    },
    Preset {
        id: "hot_coffee",
        name: "Hot Coffee",
        category: PresetCategory::Cozy,
        desc: "Minimalist ceramic mug with subtle steam wisps and saucer plate.",
    },
    Preset {
        id: "campfire",
        name: "Cozy Campfire",
        category: PresetCategory::Cozy,
        desc: "Warm minimalist flame with campfire logs and ember sparks.",
    },
    Preset {
        id: "pixel_heart",
        name: "8-Bit Heart",
        category: PresetCategory::Cozy,
        desc: "Iconic retro arcade pulsing red heart with glint gleam.",
    },
    Preset {
        id: "linux_tux",
        name: "Tux the Penguin",
        category: PresetCategory::System,
        desc: "Chubby Linux mascot with crisp white belly, orange beak & blink.",
    },
    Preset {
        id: "retro_cassette",
        name: "Retro Cassette",
        category: PresetCategory::Cozy,
        desc: "Classic audio cassette chassis with rotating spools.",
    },
    Preset {
        id: "chibi_cat",
        name: "Pixel Cat",
        category: PresetCategory::Cozy,
        desc: "Cute minimalist kitty companion with twitching ears.",
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
    0 // 0 = Static (Still Icon mode)
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

/// Unified directory for user-created custom pixel art sprites & GIFs: `~/.config/atlas/pixel_art/`
pub fn custom_sprites_dir() -> PathBuf {
    crate::user_data_dir().join("pixel_art")
}

impl PixelArtConfig {
    /// Loads from disk, creating defaults if missing
    pub fn load() -> Self {
        let path = pixel_art_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<Self>(&content) {
                    return config;
                }
            }
        }
        let def = Self::default();
        def.save();
        def
    }

    /// Saves current settings to disk atomically
    pub fn save(&self) {
        let path = pixel_art_config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp_path = path.with_extension("tmp");
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Native Animated GIF & Horizontal PNG Sprite Sheet Decoder
// ---------------------------------------------------------------------------

pub const MAX_SPRITE_DIM: u32 = 72;
pub const MAX_SPRITE_FRAMES: usize = 64;

#[derive(Clone)]
pub struct DecodedSpriteAnimation {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub num_frames: usize,
    pub render_image: Arc<RenderImage>,
}

#[derive(Clone)]
struct CachedSprite {
    mtime: std::time::SystemTime,
    file_size: u64,
    sprite: Arc<DecodedSpriteAnimation>,
}

static SPRITES_CACHE: RwLock<Option<HashMap<String, CachedSprite>>> = RwLock::new(None);
static LAST_DISK_CHECK: RwLock<Option<std::time::Instant>> = RwLock::new(None);

fn decode_gif(id: String, name: String, path: PathBuf, bytes: &[u8]) -> Option<DecodedSpriteAnimation> {
    use image::codecs::gif::GifDecoder;
    use image::AnimationDecoder;

    let decoder = GifDecoder::new(Cursor::new(bytes)).ok()?;
    let raw_frames = decoder.into_frames().collect_frames().ok()?;
    if raw_frames.is_empty() {
        return None;
    }

    let orig_w = raw_frames[0].buffer().width();
    let orig_h = raw_frames[0].buffer().height();
    if orig_w == 0 || orig_h == 0 {
        return None;
    }

    // Subsample frame count if excessive to prevent CPU & RAM ballooning
    let sampled_frames: Vec<image::Frame> = if raw_frames.len() > MAX_SPRITE_FRAMES {
        let step = (raw_frames.len() as f32) / (MAX_SPRITE_FRAMES as f32);
        let mut list = Vec::with_capacity(MAX_SPRITE_FRAMES);
        for i in 0..MAX_SPRITE_FRAMES {
            let idx = ((i as f32 * step).floor() as usize).min(raw_frames.len() - 1);
            list.push(raw_frames[idx].clone());
        }
        list
    } else {
        raw_frames
    };

    // Calculate downscaled bounds maintaining aspect ratio
    let (target_w, target_h) = if orig_w > MAX_SPRITE_DIM || orig_h > MAX_SPRITE_DIM {
        if orig_w >= orig_h {
            let h = ((orig_h as f32 / orig_w as f32) * MAX_SPRITE_DIM as f32).round() as u32;
            (MAX_SPRITE_DIM, h.max(1))
        } else {
            let w = ((orig_w as f32 / orig_h as f32) * MAX_SPRITE_DIM as f32).round() as u32;
            (w.max(1), MAX_SPRITE_DIM)
        }
    } else {
        (orig_w, orig_h)
    };

    let needs_resize = target_w != orig_w || target_h != orig_h;
    let mut final_frames = Vec::with_capacity(sampled_frames.len());

    for frame in sampled_frames {
        let mut buf = if needs_resize {
            image::imageops::resize(
                frame.buffer(),
                target_w,
                target_h,
                image::imageops::FilterType::Nearest,
            )
        } else {
            let mut b = image::RgbaImage::new(frame.buffer().width(), frame.buffer().height());
            b.copy_from(frame.buffer(), 0, 0).ok();
            b
        };

        // Convert RGBA -> BGRA in-place for GPUI's hardware GPU texture pipeline
        for pixel in buf.pixels_mut() {
            pixel.0.swap(0, 2);
        }

        final_frames.push(image::Frame::new(buf));
    }

    let num_frames = final_frames.len();
    let render_image = Arc::new(RenderImage::new(final_frames));

    Some(DecodedSpriteAnimation {
        id,
        name,
        path,
        width: target_w,
        height: target_h,
        num_frames,
        render_image,
    })
}

fn decode_png(id: String, name: String, path: PathBuf, bytes: &[u8]) -> Option<DecodedSpriteAnimation> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba_img = img.to_rgba8();
    let width = rgba_img.width();
    let height = rgba_img.height();
    if width == 0 || height == 0 {
        return None;
    }

    if width > height && width % height == 0 {
        // Horizontal sprite sheet (frame_size = height)
        let frame_size = height;
        let num_frames = (width / frame_size) as usize;
        let max_take = num_frames.min(MAX_SPRITE_FRAMES);
        let mut frames = Vec::with_capacity(max_take);

        let target_size = frame_size.min(MAX_SPRITE_DIM);
        let needs_resize = target_size != frame_size;

        for i in 0..max_take {
            let sub_view = image::imageops::crop_imm(&rgba_img, (i as u32) * frame_size, 0, frame_size, frame_size).to_image();
            let mut buf = if needs_resize {
                image::imageops::resize(&sub_view, target_size, target_size, image::imageops::FilterType::Nearest)
            } else {
                sub_view
            };
            for pixel in buf.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            frames.push(image::Frame::new(buf));
        }

        let render_image = Arc::new(RenderImage::new(frames));
        Some(DecodedSpriteAnimation {
            id,
            name,
            path,
            width: target_size,
            height: target_size,
            num_frames: max_take,
            render_image,
        })
    } else {
        // Single static PNG sprite
        let (target_w, target_h) = if width > MAX_SPRITE_DIM || height > MAX_SPRITE_DIM {
            if width >= height {
                let h = ((height as f32 / width as f32) * MAX_SPRITE_DIM as f32).round() as u32;
                (MAX_SPRITE_DIM, h.max(1))
            } else {
                let w = ((width as f32 / height as f32) * MAX_SPRITE_DIM as f32).round() as u32;
                (w.max(1), MAX_SPRITE_DIM)
            }
        } else {
            (width, height)
        };

        let mut buf = if target_w != width || target_h != height {
            image::imageops::resize(&rgba_img, target_w, target_h, image::imageops::FilterType::Nearest)
        } else {
            rgba_img
        };

        for pixel in buf.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        let render_image = Arc::new(RenderImage::new(vec![image::Frame::new(buf)]));
        Some(DecodedSpriteAnimation {
            id,
            name,
            path,
            width: target_w,
            height: target_h,
            num_frames: 1,
            render_image,
        })
    }
}

/// Ensures sprites from `~/.config/atlas/pixel_art/` are synced into memory.
/// Only reads and decodes files that are new or whose file size / mtime changed on disk.
pub fn sync_sprites_cache(force: bool) {
    if !force {
        if let Ok(guard) = LAST_DISK_CHECK.read() {
            if let Some(instant) = *guard {
                if instant.elapsed().as_secs() < 4 {
                    return;
                }
            }
        }
    }

    if let Ok(mut guard) = LAST_DISK_CHECK.write() {
        *guard = Some(std::time::Instant::now());
    }

    let dir = custom_sprites_dir();
    if !dir.exists() {
        if let Ok(mut write_guard) = SPRITES_CACHE.write() {
            *write_guard = Some(HashMap::new());
        }
        return;
    }

    let mut map = if let Ok(guard) = SPRITES_CACHE.read() {
        guard.clone().unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut current_ids = std::collections::HashSet::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext != "gif" && ext != "png" {
                continue;
            }

            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let file_size = metadata.len();

            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("sprite");
            let id = format!("sprite_{}", stem.to_lowercase().replace(' ', "_"));
            current_ids.insert(id.clone());

            // Check if already in cache and unchanged on disk
            if let Some(cached) = map.get(&id) {
                if cached.mtime == mtime && cached.file_size == file_size {
                    continue;
                }
            }

            // Read and decode changed or new file
            let name = stem.replace('_', " ").replace('-', " ");
            if let Ok(bytes) = fs::read(&path) {
                let decoded = if ext == "gif" {
                    decode_gif(id.clone(), name, path, &bytes)
                } else {
                    decode_png(id.clone(), name, path, &bytes)
                };

                if let Some(sprite) = decoded {
                    map.insert(
                        id,
                        CachedSprite {
                            mtime,
                            file_size,
                            sprite: Arc::new(sprite),
                        },
                    );
                }
            }
        }
    }

    // Retain only files that currently exist
    map.retain(|k, _| current_ids.contains(k));

    if let Ok(mut write_guard) = SPRITES_CACHE.write() {
        *write_guard = Some(map);
    }
}

const BUNDLED_WHITE_CAT: &[u8] = include_bytes!("../../assets/pixel_art/white_cat.gif");
const BUNDLED_BLUE_PENGUIN: &[u8] = include_bytes!("../../assets/pixel_art/blue_penguin.gif");
const BUNDLED_ELECTRIC_FOX: &[u8] = include_bytes!("../../assets/pixel_art/jolteon.gif");
const BUNDLED_SHADOW_BAT: &[u8] = include_bytes!("../../assets/pixel_art/shadow_bat.gif");
const BUNDLED_DARK_KNIGHT: &[u8] = include_bytes!("../../assets/pixel_art/dark_knight.gif");

static BUNDLED_SPRITES: OnceLock<HashMap<String, Arc<DecodedSpriteAnimation>>> = OnceLock::new();

pub fn is_bundled_sprite_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase().replace(['-', ' '], "_");
    let stem = lower.strip_prefix("sprite_").unwrap_or(&lower);
    matches!(
        stem,
        "white_cat"
            | "whitecat"
            | "1aej"
            | "blue_penguin"
            | "bluepenguin"
            | "2qdh"
            | "electric_fox"
            | "electricfox"
            | "ws1q"
            | "jolteon"
            | "shadow_bat"
            | "shadowbat"
            | "xosa"
            | "dark_knight"
            | "darkknight"
            | "xwi4"
            | "xwl4"
    )
}

pub fn is_procedural_preset(id: &str) -> bool {
    matches!(
        id,
        "hot_coffee"
            | "campfire"
            | "pixel_heart"
            | "beating_heart"
            | "heart"
            | "linux_tux"
            | "linux_penguin"
            | "tux"
            | "retro_cassette"
            | "cassette_tape"
            | "cassette"
            | "chibi_cat"
            | "pixel_cat"
            | "cat"
    )
}

pub fn get_bundled_sprite(id: &str) -> Option<Arc<DecodedSpriteAnimation>> {
    let norm = id.to_ascii_lowercase().replace(['-', ' '], "_");
    let norm_stem = norm.strip_prefix("sprite_").unwrap_or(&norm);

    let map = BUNDLED_SPRITES.get_or_init(|| {
        let mut map: HashMap<String, Arc<DecodedSpriteAnimation>> = HashMap::new();

        let bundled_defs: &[(&str, &str, &[u8], &[&str])] = &[
            (
                "white_cat",
                "White Cat",
                BUNDLED_WHITE_CAT,
                &["white_cat", "whitecat", "1aej", "sprite_1aej", "sprite_white_cat"],
            ),
            (
                "blue_penguin",
                "Playful Penguin",
                BUNDLED_BLUE_PENGUIN,
                &["blue_penguin", "bluepenguin", "penguin", "2qdh", "sprite_2qdh", "sprite_blue_penguin"],
            ),
            (
                "electric_fox",
                "Electric Fox",
                BUNDLED_ELECTRIC_FOX,
                &["electric_fox", "electricfox", "ws1q", "sprite_ws1q", "jolteon", "sprite_jolteon", "sprite_electric_fox"],
            ),
            (
                "shadow_bat",
                "Shadow Bat",
                BUNDLED_SHADOW_BAT,
                &["shadow_bat", "shadowbat", "xosa", "sprite_xosa", "bat", "sprite_shadow_bat"],
            ),
            (
                "dark_knight",
                "Dark Knight",
                BUNDLED_DARK_KNIGHT,
                &["dark_knight", "darkknight", "xwi4", "xwl4", "sprite_xwi4", "sprite_xwl4", "knight", "sprite_dark_knight"],
            ),
        ];

        for &(canonical_id, name, bytes, aliases) in bundled_defs {
            let path = PathBuf::from(format!("bundled://pixel_art/{}.gif", canonical_id));
            if let Some(decoded) = decode_gif(canonical_id.to_string(), name.to_string(), path, bytes) {
                let shared = Arc::new(decoded);
                for &alias in aliases {
                    map.insert(alias.to_string(), Arc::clone(&shared));
                }
                map.insert(canonical_id.to_string(), Arc::clone(&shared));
            }
        }

        map
    });

    map.get(norm_stem).or_else(|| map.get(&norm)).cloned()
}

pub fn get_decoded_sprite(id: &str) -> Option<Arc<DecodedSpriteAnimation>> {
    // 1. Check bundled GIF presets first (instant OnceLock lookup)
    if let Some(bundled) = get_bundled_sprite(id) {
        return Some(bundled);
    }

    // 2. Procedural presets are never on disk
    if is_procedural_preset(id) {
        return None;
    }

    // 3. Check existing in-memory cache
    if let Ok(read_guard) = SPRITES_CACHE.read() {
        if let Some(ref map) = *read_guard {
            if let Some(cached) = map.get(id) {
                return Some(Arc::clone(&cached.sprite));
            }
        }
    }

    // 4. If missing or cache empty, sync from disk
    sync_sprites_cache(false);

    if let Ok(read_guard) = SPRITES_CACHE.read() {
        if let Some(ref map) = *read_guard {
            if let Some(cached) = map.get(id) {
                return Some(Arc::clone(&cached.sprite));
            }
        }
    }

    None
}

/// Loads all decoded GIF & PNG sprites from `~/.config/atlas/pixel_art/`
pub fn load_all_sprites() -> HashMap<String, Arc<DecodedSpriteAnimation>> {
    sync_sprites_cache(false);
    if let Ok(read_guard) = SPRITES_CACHE.read() {
        if let Some(ref map) = *read_guard {
            return map
                .iter()
                .filter(|(k, _)| !is_bundled_sprite_key(k))
                .map(|(k, v)| (k.clone(), Arc::clone(&v.sprite)))
                .collect();
        }
    }
    HashMap::new()
}

/// Deletes an imported sprite or custom JSON file from disk and invalidates caches
pub fn delete_sprite(preset_id: &str) -> bool {
    let mut deleted = false;
    let custom_dir = custom_sprites_dir();
    let norm_id = preset_id.to_lowercase().replace(' ', "_");
    let target_stem = norm_id.strip_prefix("sprite_").unwrap_or(&norm_id);

    if let Ok(entries) = fs::read_dir(&custom_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

            // Match GIF/PNG file by stem
            if (ext == "gif" || ext == "png") && (stem == norm_id || stem == target_stem) {
                if fs::remove_file(&path).is_ok() {
                    deleted = true;
                }
                continue;
            }

            // Match JSON file by file stem or by internal sprite.id
            if ext == "json" {
                if stem == norm_id || stem == target_stem {
                    if fs::remove_file(&path).is_ok() {
                        deleted = true;
                    }
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(sprite) = serde_json::from_str::<CustomSprite>(&content) {
                        if sprite.id == norm_id || sprite.id == target_stem {
                            if fs::remove_file(&path).is_ok() {
                                deleted = true;
                            }
                        }
                    }
                }
            }
        }
    }

    if deleted {
        sync_sprites_cache(true);
        if let Ok(mut lock) = CUSTOM_SPRITES_CACHE.write() {
            *lock = None;
        }
    }

    deleted
}

/// Imports an external GIF or PNG file into `~/.config/atlas/pixel_art/` and returns preset ID
pub fn import_sprite_from_path(src: &Path) -> Option<String> {
    if !src.exists() {
        return None;
    }

    let ext = src.extension().and_then(|e| e.to_str())?.to_lowercase();
    if ext != "gif" && ext != "png" {
        return None;
    }

    let dest_dir = custom_sprites_dir();
    let _ = fs::create_dir_all(&dest_dir);

    let file_name = src.file_name()?;
    let dest_path = dest_dir.join(file_name);

    if fs::copy(src, &dest_path).is_err() {
        return None;
    }

    // Invalidate cache
    sync_sprites_cache(true);

    let stem = dest_path.file_stem().and_then(|s| s.to_str())?;
    let id = format!("sprite_{}", stem.to_lowercase().replace(' ', "_"));
    Some(id)
}

/// Opens native OS file picker to select an animated GIF or horizontal PNG sprite sheet
pub fn pick_sprite_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Import Pixel Art GIF or PNG Sprite Sheet")
        .add_filter("Pixel Art Images", &["gif", "png"])
        .pick_file()
}

// ---------------------------------------------------------------------------
// Custom JSON Sprite Engine (24x24 pixel art)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSprite {
    pub id: String,
    pub name: String,
    pub category: String,
    pub desc: String,
    pub width: usize,
    pub height: usize,
    pub palette: HashMap<char, String>,
    pub frames: Vec<Vec<String>>,
}

static CUSTOM_SPRITES_CACHE: RwLock<Option<(std::time::Instant, Vec<CustomSprite>)>> = RwLock::new(None);

pub fn ensure_custom_pixel_art_dir_and_template() {
    let dir = custom_sprites_dir();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    let readme = dir.join("README.txt");
    if !readme.exists() {
        let _ = fs::write(
            &readme,
            "Atlas Terminal - Custom Pixel Art Sprites Directory\n\n             Drop any .gif, .png, or 24x24 .json sprite file into this folder to use in your top bar!\n",
        );
    }
}

pub fn create_custom_template(name: &str) -> std::io::Result<PathBuf> {
    ensure_custom_pixel_art_dir_and_template();
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let file_path = custom_sprites_dir().join(format!("{}.json", sanitized));
    if !file_path.exists() {
        let sample = CustomSprite {
            id: sanitized.clone(),
            name: name.to_string(),
            category: "custom".to_string(),
            desc: format!("Custom 24x24 companion '{}'.", name),
            width: 24,
            height: 24,
            palette: {
                let mut map = HashMap::new();
                map.insert('C', "#38BDF8".to_string());
                map.insert('W', "#FFFFFF".to_string());
                map.insert('D', "#0284C7".to_string());
                map
            },
            frames: vec![vec![
                "........................".to_string(),
                "........................".to_string(),
                "........CCCCCCCC........".to_string(),
                "......CCCCCCCCCCCC......".to_string(),
                ".....CCCCWWCCCCWWCC.....".to_string(),
                ".....CCCCWWCCCCWWCC.....".to_string(),
                ".....CCCCCCCCCCCCCC.....".to_string(),
                ".....CCCCCCCCCCCCCC.....".to_string(),
                "......CCCCDDDDCCCC......".to_string(),
                "........CCCCCCCC........".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
                "........................".to_string(),
            ]],
        };
        if let Ok(json) = serde_json::to_string_pretty(&sample) {
            fs::write(&file_path, json)?;
        }
    }
    if let Ok(mut lock) = CUSTOM_SPRITES_CACHE.write() {
        *lock = None;
    }
    Ok(file_path)
}

pub fn load_custom_sprites() -> Vec<CustomSprite> {
    if let Ok(read_guard) = CUSTOM_SPRITES_CACHE.read() {
        if let Some((instant, ref list)) = *read_guard {
            if instant.elapsed().as_millis() < 5000 {
                return list.clone();
            }
        }
    }

    ensure_custom_pixel_art_dir_and_template();
    let dir = custom_sprites_dir();
    let mut sprites = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(sprite) = serde_json::from_str::<CustomSprite>(&content) {
                        if !sprite.frames.is_empty() {
                            // Filter out duplicates that match built-in presets or internal heart templates
                            if PRESETS.iter().any(|p| p.id == sprite.id || p.name.eq_ignore_ascii_case(&sprite.name))
                                || sprite.id == "custom_heart"
                                || sprite.id == "pixel_heart"
                                || sprite.id == "template_24x24"
                            {
                                continue;
                            }
                            sprites.push(sprite);
                        }
                    }
                }
            }
        }
    }

    if let Ok(mut write_guard) = CUSTOM_SPRITES_CACHE.write() {
        *write_guard = Some((std::time::Instant::now(), sprites.clone()));
    }

    sprites
}

pub fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(rgba(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0xFF))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(rgba(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Handcrafted Minimalist 24x24 Sprite Definitions
// ---------------------------------------------------------------------------

struct BuiltinSprite {
    palette: &'static [(char, u32)],
    frames: &'static [&'static [&'static str]],
}

// 1. Hot Coffee
const COFFEE_PALETTE: &[(char, u32)] = &[
    ('M', 0xE06C53FF), // Terracotta mug
    ('H', 0xFCA5A5FF), // Highlight rim
    ('S', 0x991B1BFF), // Mug depth
    ('C', 0x78350FFF), // Coffee crema
    ('L', 0xFDE68AFF), // Crema latte foam
    ('D', 0xCBD5E1FF), // Porcelain saucer plate
    ('W', 0xF1F5F9CC), // Rising steam wisps
];

const COFFEE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        ".........WW.............",
        "..........WW...WW.......",
        "...........WW..WW.......",
        "...........WW...WW......",
        "............WW..WW......",
        "......HHHHHHHHHH........",
        ".....MCCCCLCCCCCM.......",
        ".....MCCCCCCCCCCM.......",
        ".....MHHHHMMMMMSMMMM....",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSMMMM....",
        "......MHHHMMMMSS........",
        ".......SSSSSSSS.........",
        "....DDDDDDDDDDDDDDDD....",
        ".....DDDDDDDDDDDDDD.....",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "..........WW............",
        "...........WW..WW.......",
        "...........WW...WW......",
        "............WW..WW......",
        "............WW...WW.....",
        ".............WW..WW.....",
        "......HHHHHHHHHH........",
        ".....MCCCCLCCCCCM.......",
        ".....MCCCCCCCCCCM.......",
        ".....MHHHHMMMMMSMMMM....",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSM..MM...",
        ".....MHHHHMMMMMSMMMM....",
        "......MHHHMMMMSS........",
        ".......SSSSSSSS.........",
        "....DDDDDDDDDDDDDDDD....",
        ".....DDDDDDDDDDDDDD.....",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 2. Cozy Campfire
const CAMPFIRE_PALETTE: &[(char, u32)] = &[
    ('W', 0x451A03FF), // Log wood dark
    ('B', 0x78350FFF), // Log bark
    ('R', 0xDC2626FF), // Flame red
    ('O', 0xF97316FF), // Flame orange
    ('Y', 0xFBBF24FF), // Flame yellow core
    ('S', 0xFEF08AFF), // Embers / sparks
];

const CAMPFIRE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "........S...............",
        ".......OO...............",
        ".......YY..........S....",
        "......OYYO........OO....",
        "......OYYO........YY....",
        ".....ROYYOR......OYYO...",
        ".....ROYYOR......OYYO...",
        ".....RROYYOR....ROYYOR..",
        "......RROYYOR..RROYYOR..",
        "......RRROYYORRRROYYOR..",
        ".....RRRROYYOYROYYORRR..",
        ".....RRRRROYYYYYYORRR...",
        "....RRRRRROYYYYYYORRRR..",
        ".....BBBBBBBBBBBBBB.....",
        "...WBBBBBBBBBBBBBBBW....",
        "..WWBBBBBBBBBBBBBBBBB...",
        "..WWWWWWWWWWWWWWWWWWW...",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "...........S............",
        "..........OO............",
        "..........YY............",
        "....S....OYYO...........",
        "........ROYYOR..........",
        "........ROYYOR......S...",
        ".......RROYYORR.........",
        ".......RROYYORR..OO.....",
        "......RRROYYORR..YY.....",
        "......RRROYYORR.OYYO....",
        ".....RRRROYYOYRROYYOR...",
        ".....RRRRROYYYYYYORRR...",
        "....RRRRRROYYYYYYORRRR..",
        ".....BBBBBBBBBBBBBB.....",
        "...WBBBBBBBBBBBBBBBW....",
        "..WWBBBBBBBBBBBBBBBBB...",
        "..WWWWWWWWWWWWWWWWWWW...",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 3. 8-Bit Heart
const HEART_PALETTE: &[(char, u32)] = &[
    ('R', 0xEF4444FF), // Red
    ('D', 0x991B1BFF), // Deep shadow
    ('W', 0xFFFFFFFF), // White gleam
    ('P', 0xFCA5A5FF), // Pink gleam
];

const HEART_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "........................",
        "....RRRR....RRRR........",
        "..RRRRRRRR.RRRRRRRR.....",
        ".RRWWPPRRRRRRRRRRRRR....",
        ".RRRRRRRRRRRRRRRRRRR....",
        ".RRRRRRRRRRRRRRRRRRR....",
        "..RRRRRRRRRRRRRRRRR.....",
        "...RRRRRRRRRRRRRRR......",
        "....RRRRRRRRRRRRR.......",
        ".....RRRRRRRRRRR........",
        "......RRRRRRRRR.........",
        ".......RRRRRRR..........",
        "........RRRRR...........",
        ".........RRR............",
        "..........R.............",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "........................",
        "........................",
        "......RRRR....RRRR......",
        "....RRRRRRRR.RRRRRRRR...",
        "...RRWWPPRRRRRRRRRRRR...",
        "...RRRRRRRRRRRRRRRRRR...",
        "...RRRRRRRRRRRRRRRRRR...",
        "....RRRRRRRRRRRRRRRR....",
        ".....RRRRRRRRRRRRRR.....",
        "......RRRRRRRRRRRR......",
        ".......RRRRRRRRRR.......",
        "........RRRRRRRR........",
        ".........RRRRRR.........",
        "..........RRRR..........",
        "...........RR...........",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 4. Tux the Linux Penguin
const TUX_PALETTE: &[(char, u32)] = &[
    ('K', 0x0F172AFF), // Body black
    ('W', 0xFFFFFFFF), // Belly white
    ('O', 0xF59E0BFF), // Beak/feet orange
    ('B', 0x000000FF), // Pupil
];

const TUX_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "........KKKKKK..........",
        ".......KKKKKKKK.........",
        "......KKWWKKWWKK........",
        "......KKWBKKWBKK........",
        "......KKWWKKWWKK........",
        ".......KKKOOOKK.........",
        ".......KKKOOOKK.........",
        "......KKKKKKKKKK........",
        ".....KKKWWWWWWKKK.......",
        ".....KKWWWWWWWWKK.......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        ".....KKWWWWWWWWKK.......",
        "......KKWWWWWWKK........",
        ".......KKKKKKKK.........",
        "......OOOO..OOOO........",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "........KKKKKK..........",
        ".......KKKKKKKK.........",
        "......KKWWKKWWKK........",
        "......KKBBKKBBKK........",
        "......KKWWKKWWKK........",
        ".......KKKOOOKK.........",
        ".......KKKOOOKK.........",
        "......KKKKKKKKKK........",
        ".....KKKWWWWWWKKK.......",
        ".....KKWWWWWWWWKK.......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        ".....KKWWWWWWWWKK.......",
        "......KKWWWWWWKK........",
        ".......KKKKKKKK.........",
        "......OOOO..OOOO........",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 5. Retro Cassette Tape
const CASSETTE_PALETTE: &[(char, u32)] = &[
    ('B', 0x1E293BFF),
    ('L', 0xF1F5F9FF),
    ('W', 0x0F172AFF),
    ('R', 0x94A3B8FF),
    ('T', 0xEF4444FF),
];

const CASSETTE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
        "..BBTTTTTTTTTTTTTTTTBB..",
        "..BBWWWWWWWWWWWWWWWWBB..",
        "..BBWW.RR.WWWW.RR.WWBB..",
        "..BBWW.RR.WWWW.RR.WWBB..",
        "..BBWWWWWWWWWWWWWWWWBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "...BBBB..........BBBB...",
        "....BB............BB....",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
        "..BBTTTTTTTTTTTTTTTTBB..",
        "..BBWWWWWWWWWWWWWWWWBB..",
        "..BBWW..R.WWWW..R.WWBB..",
        "..BBWW.R..WWWW.R..WWBB..",
        "..BBWWWWWWWWWWWWWWWWBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "...BBBB..........BBBB...",
        "....BB............BB....",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 6. Pixel Cat
const CAT_PALETTE: &[(char, u32)] = &[
    ('C', 0xF8FAFCFF),
    ('E', 0x38BDF8FF),
    ('N', 0xF472B6FF),
];

const CAT_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "......CC........CC......",
        ".....CCCC......CCCC.....",
        "....CCCCCC....CCCCCC....",
        "....CCCCCCCCCCCCCCCC....",
        "....CCCCCCCCCCCCCCCC....",
        "....CCEECCCCCCCCEECC....",
        "....CCEECCCCCCCCEECC....",
        "....CCCCCCNNNNCCCCCC....",
        ".....CCCCCCCCCCCCCC.....",
        "......CCCCCCCCCCCC......",
        ".......CCCCCCCCCC.......",
        ".......CCCCCCCCCC.......",
        ".......CC......CC.......",
        "......CCCC....CCCC......",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        ".....CCC........CCC.....",
        "....CCCCC......CCCCC....",
        "....CCCCCC....CCCCCC....",
        "....CCCCCCCCCCCCCCCC....",
        "....CCCCCCCCCCCCCCCC....",
        "....CC..CCCCCCCC..CC....",
        "....CCEECCCCCCCCEECC....",
        "....CCCCCCNNNNCCCCCC....",
        ".....CCCCCCCCCCCCCC.....",
        "......CCCCCCCCCCCC......",
        ".......CCCCCCCCCC.......",
        ".......CCCCCCCCCC.......",
        ".......CC......CC.......",
        "......CCCC....CCCC......",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];


fn get_builtin_sprite(preset_id: &str) -> Option<BuiltinSprite> {
    match preset_id {
        "hot_coffee" => Some(BuiltinSprite {
            palette: COFFEE_PALETTE,
            frames: COFFEE_FRAMES,
        }),
        "campfire" => Some(BuiltinSprite {
            palette: CAMPFIRE_PALETTE,
            frames: CAMPFIRE_FRAMES,
        }),
        "pixel_heart" | "beating_heart" | "heart" => Some(BuiltinSprite {
            palette: HEART_PALETTE,
            frames: HEART_FRAMES,
        }),
        "white_cat" | "whitecat" | "1aej" | "chibi_cat" | "pixel_cat" | "cat" => Some(BuiltinSprite {
            palette: CAT_PALETTE,
            frames: CAT_FRAMES,
        }),
        "blue_penguin" | "bluepenguin" | "2qdh" | "linux_tux" | "linux_penguin" | "tux" => Some(BuiltinSprite {
            palette: TUX_PALETTE,
            frames: TUX_FRAMES,
        }),
        "electric_fox" | "electricfox" | "ws1q" | "jolteon" => Some(BuiltinSprite {
            palette: CAMPFIRE_PALETTE,
            frames: CAMPFIRE_FRAMES,
        }),
        "shadow_bat" | "shadowbat" | "xosa" => Some(BuiltinSprite {
            palette: HEART_PALETTE,
            frames: HEART_FRAMES,
        }),
        "dark_knight" | "darkknight" | "xwi4" | "xwl4" => Some(BuiltinSprite {
            palette: CASSETTE_PALETTE,
            frames: CASSETTE_FRAMES,
        }),
        "retro_cassette" | "cassette_tape" | "cassette" => Some(BuiltinSprite {
            palette: CASSETTE_PALETTE,
            frames: CASSETTE_FRAMES,
        }),
        _ => None,
    }
}

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

/// Renders a frame of active pixels in standard 24x24 coordinate space.
pub fn render_frame(
    preset_id: &str,
    tick: u64,
    color_mode: &str,
    theme_accent: Rgba,
    is_dark: bool,
) -> Vec<(u8, u8, Rgba)> {
    let mut pixels = Vec::with_capacity(160);

    // 1. Check built-in minimalist presets
    if let Some(builtin) = get_builtin_sprite(preset_id) {
        if !builtin.frames.is_empty() {
            let frame_idx = (tick as usize) % builtin.frames.len();
            let frame = builtin.frames[frame_idx];

            for (y, row) in frame.iter().enumerate() {
                if y >= GRID_HEIGHT {
                    break;
                }
                for (x, ch) in row.chars().enumerate() {
                    if x >= GRID_WIDTH || ch == '.' || ch == ' ' {
                        continue;
                    }
                    if let Some(&(_, hex)) = builtin.palette.iter().find(|(c, _)| *c == ch) {
                        let original = rgba(hex);
                        let final_col = map_color(original, color_mode, theme_accent, is_dark);
                        pixels.push((x as u8, y as u8, final_col));
                    }
                }
            }
            return pixels;
        }
    }

        // 2. Check custom JSON sprites (~/.config/atlas/pixel_art/*.json)
    let custom_sprites = load_custom_sprites();
    if let Some(custom) = custom_sprites.iter().find(|s| s.id == preset_id) {
        if !custom.frames.is_empty() {
            let frame_idx = (tick as usize) % custom.frames.len();
            let frame = &custom.frames[frame_idx];

            for (y, row) in frame.iter().enumerate() {
                if y >= GRID_HEIGHT {
                    break;
                }
                for (x, ch) in row.chars().enumerate() {
                    if x >= GRID_WIDTH || ch == '.' || ch == ' ' {
                        continue;
                    }
                    if let Some(hex_str) = custom.palette.get(&ch) {
                        if let Some(original) = parse_hex_color(hex_str) {
                            let final_col = map_color(original, color_mode, theme_accent, is_dark);
                            pixels.push((x as u8, y as u8, final_col));
                        }
                    }
                }
            }
            return pixels;
        }
    }

    // 3. Fallback to default preset
    if preset_id != "hot_coffee" {
        return render_frame("hot_coffee", tick, color_mode, theme_accent, is_dark);
    }

    pixels
}

/// Renders a native GPUI Canvas element
/// - Uses direct hardware GPU texture painting (`window.paint_image`) for imported GIFs/PNGs at full native resolution
/// - Uses solid contiguous square pixels for procedural and custom JSON pixel art
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
    let maybe_sprite = get_decoded_sprite(&preset_id);
    let pid = preset_id;
    canvas(
        move |_bounds, _window, _cx| (),
        move |bounds, _, window, _cx| {
            // 1. High-fidelity hardware GPU rendering for imported GIFs and PNG sprites
            if let Some(ref sprite) = maybe_sprite {
                let frame_idx = if sprite.num_frames > 0 {
                    (tick as usize) % sprite.num_frames
                } else {
                    0
                };
                let is_grayscale = color_mode == "monochrome";

                let bounds_w = f32::from(bounds.size.width);
                let bounds_h = f32::from(bounds.size.height);

                let aspect = if sprite.height > 0 {
                    sprite.width as f32 / sprite.height as f32
                } else {
                    1.0
                };

                let bounds_aspect = if bounds_h > 0.0 { bounds_w / bounds_h } else { 1.0 };

                let (draw_w, draw_h) = if aspect > bounds_aspect {
                    let w = bounds_w;
                    let h = (w / aspect).min(bounds_h);
                    (w, h)
                } else {
                    let h = bounds_h;
                    let w = (h * aspect).min(bounds_w);
                    (w, h)
                };

                let draw_x = f32::from(bounds.origin.x) + (bounds_w - draw_w) * 0.5;
                let draw_y = f32::from(bounds.origin.y) + (bounds_h - draw_h) * 0.5;

                let img_bounds = Bounds::new(
                    point(px(draw_x), px(draw_y)),
                    size(px(draw_w), px(draw_h)),
                );

                let _ = window.paint_image(
                    img_bounds,
                    Corners::default(),
                    sprite.render_image.clone(),
                    frame_idx,
                    is_grayscale,
                );
                return;
            }

            // 2. Solid contiguous square pixel GPU canvas for procedural presets and JSON sprites
            let pixels = render_frame(&pid, tick, &color_mode, accent, is_dark);
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
        assert_eq!(def.fps, 0); // Static by default
        assert!(def.speed_boost_on_activity);
        assert_eq!(def.color_mode, "original");
        assert!(def.custom_sprite_path.is_none());
    }

    #[test]
    fn test_all_builtin_presets_render_within_24x24() {
        let test_ticks = [0, 1, 2, 5];
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
                                "Pixel ({}, {}) out of bounds in preset '{}'",
                                x,
                                y,
                                p.id
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_clean_coffee_mug_and_saucer() {
        let pixels = render_frame("hot_coffee", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 30, "Coffee must render detailed pixels");
        let has_saucer = pixels.iter().any(|&(_, y, _)| y == 16 || y == 17);
        assert!(has_saucer, "Coffee must have saucer baseline");
    }

    #[test]
    fn test_8bit_heart_pixels() {
        let pixels = render_frame("pixel_heart", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() >= 40, "Heart must render rich 8-bit pixels");
    }

    #[test]
    fn test_bundled_gifs_load_successfully() {
        for &(id, name) in &[
            ("white_cat", "White Cat"),
            ("blue_penguin", "Playful Penguin"),
            ("electric_fox", "Electric Fox"),
            ("shadow_bat", "Shadow Bat"),
            ("dark_knight", "Dark Knight"),
        ] {
            let sprite = get_bundled_sprite(id);
            assert!(sprite.is_some(), "Bundled GIF '{}' must be decodable", id);
            let s = sprite.unwrap();
            assert_eq!(s.name, name);
            assert!(s.num_frames > 0, "Bundled GIF '{}' must have frames", id);
        }
    }

    #[test]
    fn test_bundled_aliases_resolve() {
        assert!(get_bundled_sprite("1aej").is_some());
        assert!(get_bundled_sprite("2qdh").is_some());
        assert!(get_bundled_sprite("ws1q").is_some());
        assert!(get_bundled_sprite("xosa").is_some());
        assert!(get_bundled_sprite("xwi4").is_some());
        assert!(get_bundled_sprite("jolteon").is_some());
        assert!(is_bundled_sprite_key("1aej"));
        assert!(is_bundled_sprite_key("sprite_ws1q"));
    }
}
