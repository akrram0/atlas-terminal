//! Top Bar Pixel Art Companion System
//!
//! Provides handcrafted multi-frame 24x24 pixel art animations,
//! a standalone configuration file (`~/.config/atlas/pixel_art.json`),
//! a custom pixel art directory (`~/.config/atlas/pixel_art/*.json`),
//! and native GPU-accelerated GPUI canvas rendering for both top-bar
//! and Settings UI preview cards.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use gpui::prelude::*;
use gpui::{canvas, fill, point, px, rgba, size, Bounds, IntoElement, Rgba};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

pub const GRID_WIDTH: usize = 24;
pub const GRID_HEIGHT: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetCategory {
    Cozy,
    Cyber,
    System,
    #[allow(dead_code)]
    Custom,
}

impl PresetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cozy => "cozy",
            Self::Cyber => "cyber",
            Self::System => "system",
            Self::Custom => "custom",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Cozy => "Cozy & Chill",
            Self::Cyber => "Cyber & Hacker",
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
        id: "cyber_spider",
        name: "Cyber Spider",
        category: PresetCategory::Cyber,
        desc: "Jointed 8-legged arachnid with cyan cyber eyes and creeping crawl gait.",
    },
    Preset {
        id: "campfire",
        name: "Dancing Campfire",
        category: PresetCategory::Cozy,
        desc: "Organic flickering tongues of flame with rising sparks and ember coals.",
    },
    Preset {
        id: "hot_coffee",
        name: "Hot Coffee",
        category: PresetCategory::Cozy,
        desc: "Porcelain coffee mug with handle, crema swirl, and rising soft steam.",
    },
    Preset {
        id: "linux_penguin",
        name: "Tux the Penguin",
        category: PresetCategory::System,
        desc: "Chubby Linux Tux with white belly, orange beak, flapping flippers, and blink.",
    },
    Preset {
        id: "anime_girl",
        name: "Chibi Catgirl",
        category: PresetCategory::Cozy,
        desc: "Cute anime companion with purple hair, blushing cheeks, sparkling eyes, and happy blink.",
    },
    Preset {
        id: "cassette_tape",
        name: "Cassette Tape",
        category: PresetCategory::Cozy,
        desc: "Retro cassette chassis with continuously rotating spools and tape teeth.",
    },
    Preset {
        id: "matrix_rain",
        name: "Matrix Rain",
        category: PresetCategory::Cyber,
        desc: "Cascading digital green data drops falling down columns.",
    },
    Preset {
        id: "packet_radar",
        name: "Packet Radar",
        category: PresetCategory::Cyber,
        desc: "Cyber radar sweep scanner with network LEDs and packet blips.",
    },
    Preset {
        id: "server_rack",
        name: "Server Rack",
        category: PresetCategory::System,
        desc: "Mini server blade chassis with dynamically blinking read/write LEDs.",
    },
    Preset {
        id: "dot_wave",
        name: "Dot Wave",
        category: PresetCategory::System,
        desc: "Smooth sinusoidal dot-matrix oscilloscope waveform.",
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

/// Standalone config file path: `~/.config/atlas/pixel_art.json`
pub fn pixel_art_config_path() -> PathBuf {
    crate::user_data_dir().join("pixel_art.json")
}

/// Directory for user-created custom pixel art sprites: `~/.config/atlas/pixel_art/`
pub fn custom_sprites_dir() -> PathBuf {
    crate::user_data_dir().join("pixel_art")
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
// Custom Sprite Format & File Scanner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSprite {
    pub id: String,
    pub name: String,
    #[serde(default = "default_custom_category")]
    pub category: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default = "default_grid_size")]
    pub width: usize,
    #[serde(default = "default_grid_size")]
    pub height: usize,
    #[serde(default)]
    pub palette: HashMap<char, String>,
    #[serde(default)]
    pub frames: Vec<Vec<String>>,
}

fn default_custom_category() -> String {
    "custom".to_string()
}

fn default_grid_size() -> usize {
    24
}

static CUSTOM_SPRITES_CACHE: RwLock<Option<(std::time::Instant, Vec<CustomSprite>)>> = RwLock::new(None);

/// Ensures `~/.config/atlas/pixel_art/` exists and contains a standard 24x24 template file.
pub fn ensure_custom_pixel_art_dir_and_template() {
    let dir = custom_sprites_dir();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }

    let template_file = dir.join("template_24x24.json");
    if !template_file.exists() {
        let sample = CustomSprite {
            id: "custom_heart".to_string(),
            name: "Pixel Heart".to_string(),
            category: "custom".to_string(),
            desc: "Custom 24x24 companion template. Edit frames and palette to make your own!".to_string(),
            width: 24,
            height: 24,
            palette: {
                let mut map = HashMap::new();
                map.insert('R', "#EF4444".to_string());
                map.insert('P', "#F43F5E".to_string());
                map.insert('W', "#FFFFFF".to_string());
                map.insert('D', "#991B1B".to_string());
                map
            },
            frames: vec![
                vec![
                    "........................".to_string(),
                    "........................".to_string(),
                    ".....RRRR......RRRR.....".to_string(),
                    "...RRRRRRRR..RRRRRRRR...".to_string(),
                    "..RRRWWPRRRRRRRRRRRRRR..".to_string(),
                    "..RRWWPPRRRRRRRRRRRRRR..".to_string(),
                    "..RRRRRRRRRRRRRRRRRRRR..".to_string(),
                    "..RRRRRRRRRRRRRRRRRRRR..".to_string(),
                    "...RRRRRRRRRRRRRRRRRR...".to_string(),
                    "....RRRRRRRRRRRRRRRR....".to_string(),
                    ".....RRRRRRRRRRRRRR.....".to_string(),
                    "......RRRRRRRRRRRR......".to_string(),
                    ".......RRRRRRRRRR.......".to_string(),
                    "........RRRRRRRR........".to_string(),
                    ".........RRRRRR.........".to_string(),
                    "..........RRRR..........".to_string(),
                    "...........RR...........".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                ],
                vec![
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "......RRRR....RRRR......".to_string(),
                    "....RRRRRRRR.RRRRRRRR...".to_string(),
                    "...RRWWPPRRRRRRRRRRRR...".to_string(),
                    "...RRRRRRRRRRRRRRRRRR...".to_string(),
                    "...RRRRRRRRRRRRRRRRRR...".to_string(),
                    "....RRRRRRRRRRRRRRRR....".to_string(),
                    ".....RRRRRRRRRRRRRR.....".to_string(),
                    "......RRRRRRRRRRRR......".to_string(),
                    ".......RRRRRRRRRR.......".to_string(),
                    "........RRRRRRRR........".to_string(),
                    ".........RRRRRR.........".to_string(),
                    "..........RRRR..........".to_string(),
                    "...........RR...........".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                    "........................".to_string(),
                ],
            ],
        };
        if let Ok(json) = serde_json::to_string_pretty(&sample) {
            let _ = fs::write(&template_file, json);
        }
    }
}

/// Creates a new custom sprite template file inside `~/.config/atlas/pixel_art/<name>.json`.
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
    // Invalidate cache
    if let Ok(mut lock) = CUSTOM_SPRITES_CACHE.write() {
        *lock = None;
    }
    Ok(file_path)
}

/// Loads all valid custom sprite files from `~/.config/atlas/pixel_art/*.json`
pub fn load_custom_sprites() -> Vec<CustomSprite> {
    if let Ok(read_guard) = CUSTOM_SPRITES_CACHE.read() {
        if let Some((instant, ref list)) = *read_guard {
            if instant.elapsed().as_millis() < 1200 {
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
// Handcrafted 24x24 Frame-by-Frame Bitmap Sprite Definitions
// ---------------------------------------------------------------------------

struct BuiltinSprite {
    palette: &'static [(char, u32)],
    frames: &'static [&'static [&'static str]],
}

// 1. Cyber Spider (Arachnid with jointed arched legs & crawling gait)
const SPIDER_PALETTE: &[(char, u32)] = &[
    ('B', 0x0F172AFF), // Dark body
    ('S', 0x00F0FFFF), // Cyan cyber eyes
    ('R', 0xFF0055FF), // Red core mark
    ('L', 0x334155FF), // Leg joints
    ('T', 0x00F0FF88), // Web thread
];

const SPIDER_FRAMES: &[&[&str]] = &[
    &[
        "...........TT...........",
        "...........TT...........",
        "..LL...............LL...",
        ".L..L.............L..L..",
        ".L...L....BBBB....L...L.",
        "L....L...BBBBBB...L....L",
        "......L.BBSSSSBB.L......",
        ".LL...L.BBSSSSBB.L...LL.",
        "L..L...L.BBBBBB.L...L..L",
        "....L..L..RRRR..L..L....",
        "....L....BBBBBB....L....",
        ".....L..BBBBBBBB..L.....",
        "LL....L.BBBBBBBB.L....LL",
        ".L.....L.BBBBBB.L.....L.",
        ".L......L.BBBB.L......L.",
        "L........L....L........L",
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
        "...........TT...........",
        "...........TT...........",
        "...........TT...........",
        "...........TT...........",
        "...LL.............LL....",
        "..L..L...........L..L...",
        "..L...L...BBBB...L...L..",
        ".L....L..BBBBBB..L....L.",
        "L......LBBSSSSBBL......L",
        ".......LBBSSSSBBL.......",
        ".LL...L..BBBBBB..L...LL.",
        "..L..L....RRRR....L..L..",
        "...L..L..BBBBBB..L..L...",
        "....L...BBBBBBBB...L....",
        ".....L..BBBBBBBB..L.....",
        ".LL...L..BBBBBB..L...LL.",
        "..L....L..BBBB..L....L..",
        "...L....L......L....L...",
        "....L................L..",
        "........................",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 2. Campfire (Organic licking tongues of flame with rising sparks)
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
        "....RRRRRROYYYYYYORRRR..",
        "....RRRRRROYYYYYYORRRR..",
        ".....BBBBBBBBBBBBBB.....",
        "...WBBBBBBBBBBBBBBBW....",
        "..WWBBBBBBBBBBBBBBBBB...",
        "..WWWWWWWWWWWWWWWWWWW...",
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
        "....RRRRRROYYYYYYORRRR..",
        "....RRRRRROYYYYYYORRRR..",
        ".....BBBBBBBBBBBBBB.....",
        "...WBBBBBBBBBBBBBBBW....",
        "..WWBBBBBBBBBBBBBBBBB...",
        "..WWWWWWWWWWWWWWWWWWW...",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "................S.......",
        "...............OO.......",
        "....S..........YY.......",
        "..............OYYO......",
        "....OO........OYYO......",
        "....YY.......ROYYOR.....",
        "...OYYO......ROYYOR.....",
        "..ROYYOR....RROYYORR....",
        "..ROYYOR....RROYYORR....",
        "..ROYYORRR..RRROYYORR...",
        "..RROYYORRRRRRROYYORR...",
        "...RROYYOYYYYYYYORRR....",
        "....RRRRROYYYYYYORRRR...",
        "....RRRRRROYYYYYYORRRR..",
        "....RRRRRROYYYYYYORRRR..",
        ".....BBBBBBBBBBBBBB.....",
        "...WBBBBBBBBBBBBBBBW....",
        "..WWBBBBBBBBBBBBBBBBB...",
        "..WWWWWWWWWWWWWWWWWWW...",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 3. Hot Coffee (Porcelain mug with handle, crema, and rising steam)
const COFFEE_PALETTE: &[(char, u32)] = &[
    ('M', 0xD97706FF), // Amber ceramic mug body
    ('H', 0xF59E0BFF), // Highlight rim
    ('S', 0xB45309FF), // Shadow
    ('C', 0x3E1F0DFF), // Dark roast coffee
    ('L', 0xFDE68AFF), // Latte crema swirl
    ('W', 0xE2E8F0DD), // Soft steam
];

const COFFEE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        ".......WW...............",
        "........WW..............",
        "........WW...WW.........",
        ".........WW..WW.........",
        ".........WW...WW........",
        "..........WW..WW........",
        ".....HHHHHHHHHHHH.......",
        "....MSSSSSSSSSSSSM......",
        "....MCCLLCCCCCCCCM......",
        "....MCCCCCCCCCCCCM......",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM......",
        "....MMMMMMMMMMMMMM......",
        ".....SSSSSSSSSSSS.......",
        "......SSSSSSSSSS........",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........WW..............",
        ".........WW..WW.........",
        ".........WW...WW........",
        "..........WW..WW........",
        "..........WW...WW.......",
        "...........WW..WW.......",
        "............WW.WW.......",
        ".....HHHHHHHHHHHH.......",
        "....MSSSSSSSSSSSSM......",
        "....MCCLLCCCCCCCCM......",
        "....MCCCCCCCCCCCCM......",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM......",
        "....MMMMMMMMMMMMMM......",
        ".....SSSSSSSSSSSS.......",
        "......SSSSSSSSSS........",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        ".........WW...WW........",
        "..........WW..WW........",
        "...........WW..WW.......",
        "............WW.WW.......",
        ".......WW....WW.........",
        "........WW...WW.........",
        ".........WW..WW.........",
        ".....HHHHHHHHHHHH.......",
        "....MSSSSSSSSSSSSM......",
        "....MCCLLCCCCCCCCM......",
        "....MCCCCCCCCCCCCM......",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.M.M..",
        "....MMMMMMMMMMMMMM.MMM..",
        "....MMMMMMMMMMMMMM......",
        "....MMMMMMMMMMMMMM......",
        ".....SSSSSSSSSSSS.......",
        "......SSSSSSSSSS........",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 4. Tux the Linux Penguin (Chubby body, orange beak/feet, waddling flippers, blink)
const TUX_PALETTE: &[(char, u32)] = &[
    ('K', 0x111317FF), // Body black
    ('W', 0xFFFFFFFF), // Belly white
    ('O', 0xF59E0BFF), // Beak/feet orange
    ('D', 0xB45309FF), // Beak shadow
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
        ".......KKKOODKK.........",
        ".......KKKOODKK.........",
        "......KKKKKKKKKK........",
        ".....KKKWWWWWWKKK.......",
        ".....KKWWWWWWWWKK.......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        ".....KKWWWWWWWWKK.......",
        "......KKWWWWWWKK........",
        ".......KKKKKKKK.........",
        "......OOOO..OOOO........",
        "......OOOO..OOOO........",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        ".........KKKKKK.........",
        "........KKKKKKKK........",
        ".......KKWWKKWWKK.......",
        ".......KKWBKKWBKK.......",
        ".......KKWWKKWWKK.......",
        "........KKKOODKK........",
        "........KKKOODKK........",
        ".......KKKKKKKKKK.......",
        "....KKKKKWWWWWWKKK......",
        "...KKK.KKWWWWWWWWKK.....",
        "..KKK..KKWWWWWWWWKKK....",
        ".......KKWWWWWWWWKKK....",
        ".......KKWWWWWWWWKKK....",
        ".......KKWWWWWWWWKKK....",
        "........KKWWWWWWKK......",
        ".........KKWWWWKK.......",
        "..........KKKKKK........",
        ".......OOOO..OOOO.......",
        "........OOO...OO........",
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
        ".......KKKOODKK.........",
        ".......KKKOODKK.........",
        "......KKKKKKKKKK........",
        ".....KKKWWWWWWKKK.......",
        ".....KKWWWWWWWWKK.......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        "....KKKWWWWWWWWKKK......",
        ".....KKWWWWWWWWKK.......",
        "......KKWWWWWWKK........",
        ".......KKKKKKKK.........",
        "......OOOO..OOOO........",
        "......OOOO..OOOO........",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 5. Anime Chibi Catgirl (Purple twintails, cat ears/ribbons, sparkling eyes, blush, happy blink)
const ANIME_PALETTE: &[(char, u32)] = &[
    ('H', 0xA855F7FF), // Hair purple
    ('L', 0xC084FCFF), // Hair highlight
    ('S', 0xFED7AAFF), // Skin tone
    ('R', 0xF43F5EFF), // Ribbon red
    ('E', 0x6366F1FF), // Iris indigo
    ('W', 0xFFFFFFFF), // Catchlight sparkle
    ('B', 0xFB7185FF), // Blush pink
    ('C', 0x1E1B4BFF), // Uniform navy
];

const ANIME_FRAMES: &[&[&str]] = &[
    &[
        "....RR............RR....",
        "...RRRR..........RRRR...",
        "...HHHH..HHHHHH..HHHH...",
        "..HHHHHHHHLLLLHHHHHHHH..",
        "..HHHHHHHLLLLLLHHHHHHH..",
        "..HHHHHSSSSSSSSSSHHHHH..",
        "..HH.HHSSSSSSSSSSHH.HH..",
        "..H..HHSSWESSWESSHH..H..",
        ".....HHSSWESSWESSHH.....",
        ".....HHSSBSSSSBSSHH.....",
        "......HSSSSRSSSSSH......",
        ".......SSSSSSSSSS.......",
        "........CCCCCCCC........",
        ".......CCCWWWWCCC.......",
        "......CCCCWWWWCCCC......",
        "......CCCCWWWWCCCC......",
        "......CCCC....CCCC......",
        ".......CC......CC.......",
        ".......HH......HH.......",
        ".......HH......HH.......",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "....RR............RR....",
        "...RRRR..........RRRR...",
        "...HHHH..HHHHHH..HHHH...",
        "..HHHHHHHHLLLLHHHHHHHH..",
        "..HHHHHHHLLLLLLHHHHHHH..",
        "..HHHHHSSSSSSSSSSHHHHH..",
        "..HH.HHSSSSSSSSSSHH.HH..",
        "..H..HHSS..SS..SSHH..H..",
        ".....HHSSRRSSRRSSHH.....",
        ".....HHSSBSSSSBSSHH.....",
        "......HSSSSRSSSSSH......",
        ".......SSSSSSSSSS.......",
        "........CCCCCCCC........",
        ".......CCCWWWWCCC.......",
        "......CCCCWWWWCCCC......",
        "......CCCCWWWWCCCC......",
        "......CCCC....CCCC......",
        ".......CC......CC.......",
        ".......HH......HH.......",
        ".......HH......HH.......",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 6. Cassette Tape (Retro body, center window, rotating spools)
const CASSETTE_PALETTE: &[(char, u32)] = &[
    ('B', 0x23252AFF),
    ('L', 0xE2E8F0FF),
    ('W', 0x0F172AFF),
    ('R', 0x94A3B8FF),
    ('T', 0xEF4444FF),
];

const CASSETTE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
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
    ],
    &[
        "........................",
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBLLLLLLLLLLLLLLLLBB..",
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
    ],
];

// 7. Matrix Rain (Cascading digital green drops)
const MATRIX_PALETTE: &[(char, u32)] = &[
    ('H', 0xA7F3D0FF), // Bright white-green head drop
    ('G', 0x10B981FF), // Bright emerald
    ('M', 0x059669EE), // Medium green
    ('D', 0x064E3B99), // Dark green trail
    ('F', 0x022C2255), // Faint
];

const MATRIX_FRAMES: &[&[&str]] = &[
    &[
        "..H.......H.......H.....",
        "..G.......G.......G.....",
        "..M...H...M...H...M.....",
        "..D...G...D...G...D..H..",
        "..F...M...F...M...F..G..",
        "......D.......D......M..",
        "......F.......F......D..",
        "..H...............H..F..",
        "..G...H.......H...G.....",
        "..M...G...H...G...M.....",
        "..D...M...G...M...D.....",
        "..F...D...M...D...F..H..",
        "......F...D...F......G..",
        "..........F..........M..",
        "......H.......H......D..",
        "..H...G...H...G......F..",
        "..G...M...G...M...H.....",
        "..M...D...M...D...G.....",
        "..D...F...D...F...M..H..",
        "..F.......F.......D..G..",
        "..................F..M..",
        ".....................D..",
        ".....................F..",
        "........................",
    ],
    &[
        "......H.......H......F..",
        "..H...G...H...G.........",
        "..G...M...G...M...H.....",
        "..M...D...M...D...G.....",
        "..D...F...D...F...M..H..",
        "..F.......F.......D..G..",
        "..................F..M..",
        "..H.......H.......H..D..",
        "..G.......G.......G..F..",
        "..M...H...M...H...M.....",
        "..D...G...D...G...D..H..",
        "..F...M...F...M...F..G..",
        "......D.......D......M..",
        "......F.......F......D..",
        "..H...............H..F..",
        "..G...H.......H...G.....",
        "..M...G...H...G...M.....",
        "..D...M...G...M...D.....",
        "..F...D...M...D...F..H..",
        "......F...D...F......G..",
        "..........F..........M..",
        ".....................D..",
        ".....................F..",
        "........................",
    ],
];

// 8. Packet Radar (Circular sweep with blips)
const RADAR_PALETTE: &[(char, u32)] = &[
    ('R', 0x064E3BFF), // Ring
    ('S', 0x34D399FF), // Sweep line
    ('B', 0x10B981FF), // Blip
    ('C', 0x6EE7B7FF), // Bright core
];

const RADAR_FRAMES: &[&[&str]] = &[
    &[
        "........RRRRRRRR........",
        ".....RRR........RRR.....",
        "...RR..............RR...",
        "..R......S...........R..",
        ".R........S...........R.",
        ".R.........S..........R.",
        "R...........S..........R",
        "R............S.........R",
        "R.............S........R",
        "R...BB.........S.......R",
        "R...BC..........S......R",
        "R................S.....R",
        "R.................S....R",
        "R..................S...R",
        "R...................S..R",
        "R....................S.R",
        ".R....................R.",
        ".R....................R.",
        "..R..................R..",
        "...RR..............RR...",
        ".....RRR........RRR.....",
        "........RRRRRRRR........",
        "........................",
        "........................",
    ],
    &[
        "........RRRRRRRR........",
        ".....RRR........RRR.....",
        "...RR..............RR...",
        "..R..................R..",
        ".R....................R.",
        ".R..........SS........R.",
        "R............SS........R",
        "R.............SS.......R",
        "R..............SS......R",
        "R...............SS.....R",
        "R...BB...........SS....R",
        "R...BC............SS...R",
        "R..................SS..R",
        "R...................S..R",
        "R......................R",
        "R......................R",
        ".R....................R.",
        ".R....................R.",
        "..R..................R..",
        "...RR..............RR...",
        ".....RRR........RRR.....",
        "........RRRRRRRR........",
        "........................",
        "........................",
    ],
];

// 9. Server Rack (3 blades with blinking activity LEDs)
const RACK_PALETTE: &[(char, u32)] = &[
    ('B', 0x1E293BFF), // Frame
    ('V', 0x0F172AFF), // Ventilation
    ('G', 0x22C55EFF), // Green LED on
    ('C', 0x06B6D4FF), // Cyan LED on
    ('O', 0x334155FF), // LED off
];

const RACK_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.G.C.B..",
        "..B.VVVVVVVVVVVV.O.O.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.O.C.B..",
        "..B.VVVVVVVVVVVV.G.O.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.G.O.B..",
        "..B.VVVVVVVVVVVV.O.C.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.G.C.B..",
        "..B.VVVVVVVVVVVV.G.O.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
    &[
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.O.O.B..",
        "..B.VVVVVVVVVVVV.G.C.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.G.O.B..",
        "..B.VVVVVVVVVVVV.O.C.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.O.C.B..",
        "..B.VVVVVVVVVVVV.G.O.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B.VVVVVVVVVVVV.O.C.B..",
        "..B.VVVVVVVVVVVV.O.O.B..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "........................",
        "........................",
        "........................",
        "........................",
    ],
];

// 10. Dot Wave (Oscilloscope sinusoidal dot-matrix waveform)
const WAVE_PALETTE: &[(char, u32)] = &[
    ('W', 0x38BDF8FF), // Wave crest
    ('T', 0x0284C7FF), // Trail
    ('D', 0x0369A188), // Ambient
];

const WAVE_FRAMES: &[&[&str]] = &[
    &[
        "........................",
        "........................",
        "......WW................",
        ".....WTTW...............",
        "....W....W..............",
        "...W......W.............",
        "..W........W............",
        "............W..........W",
        ".............W........W.",
        "..............W......W..",
        "...............W....W...",
        "................WTTW....",
        ".................WW.....",
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
        "........................",
        ".........WW.............",
        "........WTTW............",
        ".......W....W...........",
        "......W......W..........",
        ".....W........W.........",
        "...W...........W........",
        "..W.............W.......",
        ".W...............W......",
        "..................W....W",
        "...................WTTW.",
        "....................WW..",
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
        "........................",
        "............WW..........",
        "...........WTTW.........",
        "..........W....W........",
        ".........W......W.......",
        "........W........W......",
        "......W...........W.....",
        ".....W.............W....",
        "....W...............W...",
        "..W..................W..",
        ".W....................W.",
        "W......................W",
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

fn get_builtin_sprite(preset_id: &str) -> Option<BuiltinSprite> {
    match preset_id {
        "cyber_spider" => Some(BuiltinSprite {
            palette: SPIDER_PALETTE,
            frames: SPIDER_FRAMES,
        }),
        "campfire" => Some(BuiltinSprite {
            palette: CAMPFIRE_PALETTE,
            frames: CAMPFIRE_FRAMES,
        }),
        "hot_coffee" => Some(BuiltinSprite {
            palette: COFFEE_PALETTE,
            frames: COFFEE_FRAMES,
        }),
        "linux_penguin" => Some(BuiltinSprite {
            palette: TUX_PALETTE,
            frames: TUX_FRAMES,
        }),
        "anime_girl" => Some(BuiltinSprite {
            palette: ANIME_PALETTE,
            frames: ANIME_FRAMES,
        }),
        "cassette_tape" => Some(BuiltinSprite {
            palette: CASSETTE_PALETTE,
            frames: CASSETTE_FRAMES,
        }),
        "matrix_rain" => Some(BuiltinSprite {
            palette: MATRIX_PALETTE,
            frames: MATRIX_FRAMES,
        }),
        "packet_radar" => Some(BuiltinSprite {
            palette: RADAR_PALETTE,
            frames: RADAR_FRAMES,
        }),
        "server_rack" => Some(BuiltinSprite {
            palette: RACK_PALETTE,
            frames: RACK_FRAMES,
        }),
        "dot_wave" | "cyber_glitch" => Some(BuiltinSprite {
            palette: WAVE_PALETTE,
            frames: WAVE_FRAMES,
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

/// Renders a frame of active pixels in the standard 24x24 coordinate grid.
pub fn render_frame(
    preset_id: &str,
    tick: u64,
    color_mode: &str,
    theme_accent: Rgba,
    is_dark: bool,
) -> Vec<(u8, u8, Rgba)> {
    let mut pixels = Vec::with_capacity(160);

    // 1. Check built-in sprites
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

    // 2. Check custom loaded sprites from ~/.config/atlas/pixel_art/*.json
    let custom_sprites = load_custom_sprites();
    if let Some(custom) = custom_sprites.iter().find(|s| s.id == preset_id) {
        if !custom.frames.is_empty() {
            let frame_idx = (tick as usize) % custom.frames.len();
            let frame = &custom.frames[frame_idx];

            let mut parsed_palette: HashMap<char, Rgba> = HashMap::new();
            for (&ch, hex) in &custom.palette {
                if let Some(col) = parse_hex_color(hex) {
                    parsed_palette.insert(ch, col);
                }
            }

            for (y, row) in frame.iter().enumerate() {
                if y >= GRID_HEIGHT {
                    break;
                }
                for (x, ch) in row.chars().enumerate() {
                    if x >= GRID_WIDTH || ch == '.' || ch == ' ' {
                        continue;
                    }
                    if let Some(&original) = parsed_palette.get(&ch) {
                        let final_col = map_color(original, color_mode, theme_accent, is_dark);
                        pixels.push((x as u8, y as u8, final_col));
                    }
                }
            }
            return pixels;
        }
    }

    // 3. Fallback: gentle pulsing dot
    let col = map_color(rgba(0xE08A62FF), color_mode, theme_accent, is_dark);
    pixels.push((12, 12, col));
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
    fn test_all_builtin_presets_render_within_24x24() {
        let test_ticks = [0, 1, 2, 5, 10];
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
    fn test_spider_distinct_legs_and_core() {
        let pixels = render_frame("cyber_spider", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() > 20, "Spider should have rich body and leg pixels");
        // Spider has eyes and red core
        let has_eyes = pixels.iter().any(|&(_, _, c)| c.r < 0.1 && c.g > 0.8 && c.b > 0.8);
        assert!(has_eyes, "Spider must contain cyan eyes");
    }

    #[test]
    fn test_tux_penguin_belly_and_beak() {
        let pixels = render_frame("linux_penguin", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() > 30, "Tux should have rich belly and feet");
        let has_white_belly = pixels.iter().any(|&(_, _, c)| c.r > 0.9 && c.g > 0.9 && c.b > 0.9);
        assert!(has_white_belly, "Tux must contain white belly pixels");
    }

    #[test]
    fn test_coffee_cup_handle_and_steam() {
        let pixels = render_frame("hot_coffee", 0, "original", rgba(0xE08A62FF), true);
        assert!(pixels.len() > 30, "Coffee cup must have rich body and steam");
        // Mug handle exists on columns 19..22
        let has_handle = pixels.iter().any(|&(x, _, _)| x >= 19 && x <= 22);
        assert!(has_handle, "Coffee cup must have handle on the right");
    }
}
