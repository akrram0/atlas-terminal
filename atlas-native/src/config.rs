//! Application configuration, settings persistence, shortcuts and key chords.

use std::collections::HashMap;
use gpui::Keystroke;
use serde::{Deserialize, Serialize};

pub const ACTION_NEW_TAB: &str = "new_tab";
pub const ACTION_CLOSE_TAB: &str = "close_tab";
pub const ACTION_SPLIT_HORIZONTAL: &str = "split_horizontal";
pub const ACTION_SPLIT_VERTICAL: &str = "split_vertical";
pub const ACTION_NEXT_TAB: &str = "next_tab";
pub const ACTION_PREV_TAB: &str = "prev_tab";
pub const ACTION_COPY: &str = "copy";
pub const ACTION_PASTE: &str = "paste";
pub const ACTION_OPEN_SETTINGS: &str = "open_settings";
pub const ACTION_ZOOM_IN: &str = "zoom_in";
pub const ACTION_ZOOM_OUT: &str = "zoom_out";
pub const ACTION_PALETTE: &str = "palette";
pub const ACTION_FULLSCREEN: &str = "fullscreen";
pub const ACTION_CLEAR_SCROLLBACK: &str = "clear_scrollback";
pub const ACTION_FOCUS_NEXT_PANE: &str = "focus_next_pane";
pub const ACTION_FOCUS_PREV_PANE: &str = "focus_prev_pane";

pub fn action_display_name(action: &str) -> &'static str {
    match action {
        ACTION_NEW_TAB => "New Tab",
        ACTION_CLOSE_TAB => "Close Tab / Pane",
        ACTION_SPLIT_HORIZONTAL => "Split Horizontal (Rows)",
        ACTION_SPLIT_VERTICAL => "Split Vertical (Columns)",
        ACTION_NEXT_TAB => "Next Tab",
        ACTION_PREV_TAB => "Previous Tab",
        ACTION_COPY => "Copy Selection",
        ACTION_PASTE => "Paste Clipboard",
        ACTION_OPEN_SETTINGS => "Open Settings",
        ACTION_ZOOM_IN => "Zoom In (Font Size)",
        ACTION_ZOOM_OUT => "Zoom Out (Font Size)",
        ACTION_PALETTE => "Command Palette",
        ACTION_FULLSCREEN => "Toggle Fullscreen / Maximize",
        ACTION_CLEAR_SCROLLBACK => "Clear Scrollback Buffer",
        ACTION_FOCUS_NEXT_PANE => "Focus Next Split Pane",
        ACTION_FOCUS_PREV_PANE => "Focus Previous Split Pane",
        _ => "Custom Action",
    }
}

pub fn default_shortcuts() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(ACTION_NEW_TAB.to_string(), "ctrl+shift+n".to_string());
    m.insert(ACTION_CLOSE_TAB.to_string(), "ctrl+shift+w".to_string());
    m.insert(ACTION_SPLIT_HORIZONTAL.to_string(), "ctrl+shift+d".to_string());
    m.insert(ACTION_SPLIT_VERTICAL.to_string(), "ctrl+shift+e".to_string());
    m.insert(ACTION_NEXT_TAB.to_string(), "ctrl+tab".to_string());
    m.insert(ACTION_PREV_TAB.to_string(), "ctrl+shift+tab".to_string());
    m.insert(ACTION_COPY.to_string(), "ctrl+shift+c".to_string());
    m.insert(ACTION_PASTE.to_string(), "ctrl+shift+v".to_string());
    m.insert(ACTION_OPEN_SETTINGS.to_string(), "ctrl+,".to_string());
    m.insert(ACTION_ZOOM_IN.to_string(), "ctrl+=".to_string());
    m.insert(ACTION_ZOOM_OUT.to_string(), "ctrl+-".to_string());
    m.insert(ACTION_PALETTE.to_string(), "ctrl+k".to_string());
    m.insert(ACTION_FULLSCREEN.to_string(), "f11".to_string());
    m.insert(ACTION_CLEAR_SCROLLBACK.to_string(), "ctrl+shift+k".to_string());
    m.insert(ACTION_FOCUS_NEXT_PANE.to_string(), "alt+right".to_string());
    m.insert(ACTION_FOCUS_PREV_PANE.to_string(), "alt+left".to_string());
    m
}

pub fn format_keystroke_chord(ks: &Keystroke) -> Option<String> {
    let key = ks.key.to_lowercase();
    if matches!(
        key.as_str(),
        "control" | "shift" | "alt" | "super" | "meta" | "windows" | "menu"
    ) {
        return None;
    }
    let mut parts = Vec::new();
    if ks.modifiers.control {
        parts.push("ctrl");
    }
    if ks.modifiers.alt {
        parts.push("alt");
    }
    if ks.modifiers.shift {
        parts.push("shift");
    }
    let key_clean = match key.as_str() {
        "+" => "=",
        other => other,
    };
    parts.push(key_clean);
    Some(parts.join("+"))
}

pub fn pretty_shortcut(chord: &str) -> String {
    chord
        .split('+')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            "tab" => "Tab".to_string(),
            "enter" => "Enter".to_string(),
            "backspace" => "Backspace".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            "escape" => "Esc".to_string(),
            "," => ",".to_string(),
            "=" => "=".to_string(),
            "-" => "-".to_string(),
            other => {
                let mut c = other.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Appearance,
    Shortcuts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsFile {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_blur")]
    pub blur_enabled: bool,
    #[serde(default = "default_cursor_shape")]
    pub cursor_shape: String,
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,

    #[serde(default = "default_shortcuts")]
    pub shortcuts: HashMap<String, String>,

    #[serde(default = "default_shell")]
    pub default_shell: String,
    #[serde(default = "default_start_dir")]
    pub start_directory: String,
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: usize,
    #[serde(default = "default_confirm_close")]
    pub confirm_close_process: bool,
    #[serde(default = "default_copy_on_select")]
    pub copy_on_select: bool,
    #[serde(default = "default_startup_mode")]
    pub startup_mode: String,

    #[serde(default = "default_auto_check")]
    pub auto_check_updates: bool,
    #[serde(default = "default_interval_mins")]
    pub update_check_interval_mins: u32,
    #[serde(default)]
    pub last_check_time: Option<u64>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_seen_version: Option<String>,
    #[serde(default)]
    pub skipped_version: Option<String>,
}

fn default_theme_mode() -> String {
    "auto".to_string()
}
fn default_theme_name() -> String {
    "atlas".to_string()
}
fn default_font_family() -> String {
    "Cascadia Code".to_string()
}
fn default_font_size() -> f32 {
    13.0
}
fn default_line_height() -> f32 {
    1.20
}
fn default_corner_radius() -> f32 {
    14.0
}
fn default_opacity() -> f32 {
    1.0
}
fn default_blur() -> bool {
    false
}
fn default_cursor_shape() -> String {
    "block".to_string()
}
fn default_cursor_blink() -> bool {
    true
}
fn default_shell() -> String {
    "PowerShell".to_string()
}
fn default_start_dir() -> String {
    "home".to_string()
}
fn default_scrollback() -> usize {
    10000
}
fn default_confirm_close() -> bool {
    true
}
fn default_copy_on_select() -> bool {
    true
}
fn default_startup_mode() -> String {
    "restore_session".to_string()
}
fn default_auto_check() -> bool {
    true
}
fn default_interval_mins() -> u32 {
    60
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            theme_mode: default_theme_mode(),
            theme: default_theme_name(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            line_height: default_line_height(),
            corner_radius: default_corner_radius(),
            opacity: default_opacity(),
            blur_enabled: default_blur(),
            cursor_shape: default_cursor_shape(),
            cursor_blink: default_cursor_blink(),
            shortcuts: default_shortcuts(),
            default_shell: default_shell(),
            start_directory: default_start_dir(),
            scrollback_lines: default_scrollback(),
            confirm_close_process: default_confirm_close(),
            copy_on_select: default_copy_on_select(),
            startup_mode: default_startup_mode(),
            auto_check_updates: default_auto_check(),
            update_check_interval_mins: default_interval_mins(),
            last_check_time: None,
            etag: None,
            last_seen_version: None,
            skipped_version: None,
        }
    }
}

pub fn load_app_settings() -> AppSettingsFile {
    let settings_path = crate::user_data_dir().join("settings.json");
    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Ok(s) = serde_json::from_str::<AppSettingsFile>(&content) {
            return s;
        }
    }
    AppSettingsFile::default()
}

pub fn save_app_settings(s: &AppSettingsFile) {
    let base = crate::user_data_dir();
    let _ = std::fs::create_dir_all(&base);
    let settings_path = base.join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(&settings_path, json);
    }
}
