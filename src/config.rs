//! Application configuration, settings persistence, shortcuts and key chords.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

static LAST_SAVED_SETTINGS_HASH: AtomicU64 = AtomicU64::new(0);

pub fn hash_settings_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    std::hash::Hash::hash_slice(s.as_bytes(), &mut hasher);
    hasher.finish()
}

pub fn is_saved_settings_hash(content: &str) -> bool {
    let h = hash_settings_str(content);
    let last = LAST_SAVED_SETTINGS_HASH.load(Ordering::SeqCst);
    last != 0 && h == last
}

pub fn update_saved_settings_hash(content: &str) {
    LAST_SAVED_SETTINGS_HASH.store(hash_settings_str(content), Ordering::SeqCst);
}
use gpui::Keystroke;
use serde::{Deserialize, Serialize};

pub const ACTION_NEW_WINDOW: &str = "new_window";
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
pub const ACTION_ZOOM_RESET: &str = "zoom_reset";
pub const ACTION_SELECT_ALL: &str = "select_all";
pub const ACTION_SCROLL_PAGE_UP: &str = "scroll_page_up";
pub const ACTION_SCROLL_PAGE_DOWN: &str = "scroll_page_down";
pub const ACTION_SCROLL_TO_TOP: &str = "scroll_to_top";
pub const ACTION_SCROLL_TO_BOTTOM: &str = "scroll_to_bottom";
pub const ACTION_OPEN_URL_PICKER: &str = "open_url_picker";
pub const ACTION_TOGGLE_THEME: &str = "toggle_theme";
pub const ACTION_NEXT_THEME: &str = "next_theme";
pub const ACTION_PREV_THEME: &str = "prev_theme";
pub const ACTION_TOGGLE_DARK_LIGHT: &str = "toggle_dark_light";
pub const ACTION_TOGGLE_MAXIMIZE: &str = "toggle_maximize";
pub const ACTION_FIND: &str = "find";
pub const ACTION_TOGGLE_SIDEBAR: &str = "toggle_sidebar";
pub const ACTION_PROFILE_MENU: &str = "profile_menu";
pub const ACTION_NEW_TAB_WSL: &str = "new_tab_wsl";
pub const ACTION_NEW_TAB_POWERSHELL: &str = "new_tab_powershell";
pub const ACTION_NEW_TAB_CMD: &str = "new_tab_cmd";
pub const ACTION_NEW_TAB_GITBASH: &str = "new_tab_gitbash";
pub const ACTION_NEW_TAB_OPENCODE: &str = "new_tab_opencode";
pub const ACTION_NEW_TAB_ANTIGRAVITY: &str = "new_tab_antigravity";

pub fn action_display_name(action: &str) -> &'static str {
    match action {
        ACTION_NEW_WINDOW => "New Window",
        ACTION_NEW_TAB => "New Tab (Default)",
        ACTION_CLOSE_TAB => "Close Tab / Pane",
        ACTION_SPLIT_HORIZONTAL => "Split Horizontal (Rows)",
        ACTION_SPLIT_VERTICAL => "Split Vertical (Columns)",
        ACTION_NEXT_TAB => "Next Tab",
        ACTION_PREV_TAB => "Previous Tab",
        ACTION_COPY => "Copy Selection",
        ACTION_PASTE => "Paste Clipboard",
        ACTION_FIND => "Find in Terminal",
        ACTION_OPEN_SETTINGS => "Open Settings",
        ACTION_ZOOM_IN => "Zoom In (Font Size)",
        ACTION_ZOOM_OUT => "Zoom Out (Font Size)",
        ACTION_ZOOM_RESET => "Reset Zoom (Font Size)",
        ACTION_SELECT_ALL => "Select All in Viewport",
        ACTION_SCROLL_PAGE_UP => "Scroll Page Up",
        ACTION_SCROLL_PAGE_DOWN => "Scroll Page Down",
        ACTION_SCROLL_TO_TOP => "Scroll to Top of Buffer",
        ACTION_SCROLL_TO_BOTTOM => "Scroll to Bottom of Buffer",
        ACTION_OPEN_URL_PICKER => "Open URL from Buffer",
        ACTION_PALETTE => "Command Palette",
        ACTION_FULLSCREEN => "Toggle Fullscreen",
        ACTION_CLEAR_SCROLLBACK => "Clear Scrollback Buffer",
        ACTION_FOCUS_NEXT_PANE => "Focus Next Split Pane",
        ACTION_FOCUS_PREV_PANE => "Focus Previous Split Pane",
        ACTION_NEXT_THEME => "Next Theme Preset",
        ACTION_PREV_THEME => "Previous Theme Preset",
        ACTION_TOGGLE_DARK_LIGHT => "Toggle Dark / Light Mode",
        ACTION_TOGGLE_THEME => "Toggle Dark / Light Mode",
        ACTION_TOGGLE_MAXIMIZE => "Toggle Pane Zoom / Maximize",
        ACTION_TOGGLE_SIDEBAR => "Toggle Sidebar / Tab Bar",
        ACTION_PROFILE_MENU => "Open Shell / Profile Menu",
        ACTION_NEW_TAB_WSL => "New WSL (Linux) Tab",
        ACTION_NEW_TAB_POWERSHELL => "New PowerShell Tab",
        ACTION_NEW_TAB_CMD => "New Command Prompt Tab",
        ACTION_NEW_TAB_GITBASH => "New Git Bash Tab",
        ACTION_NEW_TAB_OPENCODE => "New OpenCode Tab",
        ACTION_NEW_TAB_ANTIGRAVITY => "New Antigravity Tab",
        _ => "Custom Action",
    }
}

pub fn default_shortcuts() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(ACTION_NEW_WINDOW.to_string(), "ctrl+shift+n".to_string());
    m.insert(ACTION_NEW_TAB.to_string(), "ctrl+t".to_string());
    m.insert(ACTION_CLOSE_TAB.to_string(), "alt+w".to_string());
    m.insert(ACTION_SPLIT_VERTICAL.to_string(), "alt+enter".to_string());
    m.insert(ACTION_SPLIT_HORIZONTAL.to_string(), "alt+d".to_string());
    m.insert(ACTION_NEXT_TAB.to_string(), "ctrl+tab".to_string());
    m.insert(ACTION_PREV_TAB.to_string(), "ctrl+shift+tab".to_string());
    m.insert(ACTION_COPY.to_string(), "ctrl+shift+c".to_string());
    m.insert(ACTION_PASTE.to_string(), "ctrl+shift+v".to_string());
    m.insert(ACTION_OPEN_SETTINGS.to_string(), "ctrl+,".to_string());
    m.insert(ACTION_ZOOM_IN.to_string(), "ctrl+=".to_string());
    m.insert(ACTION_ZOOM_OUT.to_string(), "ctrl+-".to_string());
    m.insert(ACTION_ZOOM_RESET.to_string(), "ctrl+0".to_string());
    m.insert(ACTION_SELECT_ALL.to_string(), "ctrl+shift+a".to_string());
    m.insert(ACTION_SCROLL_PAGE_UP.to_string(), "shift+pageup".to_string());
    m.insert(ACTION_SCROLL_PAGE_DOWN.to_string(), "shift+pagedown".to_string());
    m.insert(ACTION_SCROLL_TO_TOP.to_string(), "shift+home".to_string());
    m.insert(ACTION_SCROLL_TO_BOTTOM.to_string(), "shift+end".to_string());
    m.insert(ACTION_OPEN_URL_PICKER.to_string(), "ctrl+shift+u".to_string());
    m.insert(ACTION_PALETTE.to_string(), "ctrl+k".to_string());
    m.insert(ACTION_FULLSCREEN.to_string(), "f11".to_string());
    m.insert(ACTION_CLEAR_SCROLLBACK.to_string(), "ctrl+shift+k".to_string());
    m.insert(ACTION_FOCUS_NEXT_PANE.to_string(), "alt+right".to_string());
    m.insert(ACTION_FOCUS_PREV_PANE.to_string(), "alt+left".to_string());
    m.insert(ACTION_NEXT_THEME.to_string(), "alt+t".to_string());
    m.insert(ACTION_PREV_THEME.to_string(), "alt+shift+t".to_string());
    m.insert(ACTION_TOGGLE_DARK_LIGHT.to_string(), "ctrl+shift+l".to_string());
    m.insert(ACTION_TOGGLE_THEME.to_string(), "ctrl+shift+l".to_string());
    m.insert(ACTION_TOGGLE_MAXIMIZE.to_string(), "alt+z".to_string());
    m.insert(ACTION_FIND.to_string(), "ctrl+f".to_string());
    m.insert(ACTION_TOGGLE_SIDEBAR.to_string(), "ctrl+b".to_string());
    m.insert(ACTION_PROFILE_MENU.to_string(), "ctrl+shift+p".to_string());
    m.insert(ACTION_NEW_TAB_POWERSHELL.to_string(), "ctrl+shift+1".to_string());
    m.insert(ACTION_NEW_TAB_WSL.to_string(), "ctrl+shift+2".to_string());
    m.insert(ACTION_NEW_TAB_CMD.to_string(), "ctrl+shift+3".to_string());
    m.insert(ACTION_NEW_TAB_GITBASH.to_string(), "ctrl+shift+4".to_string());
    m.insert(ACTION_NEW_TAB_OPENCODE.to_string(), "ctrl+shift+5".to_string());
    m.insert(ACTION_NEW_TAB_ANTIGRAVITY.to_string(), "ctrl+shift+6".to_string());
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
    let is_shifted_num = matches!(key.as_str(), "!" | "@" | "#" | "$" | "%" | "^" | "&" | "*" | "(" | ")");
    if ks.modifiers.shift || is_shifted_num {
        parts.push("shift");
    }
    let key_clean = match key.as_str() {
        "+" => "=",
        "arrowleft" => "left",
        "arrowright" => "right",
        "arrowup" => "up",
        "arrowdown" => "down",
        "return" | "\r" | "\n" | "numpadenter" => "enter",
        "esc" => "escape",
        // Normalize shifted punctuation characters so "ctrl+shift+1" matches when GPUI reports "!"
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsTab {
    General,
    Appearance,
    Typography,
    Scrolling,
    Window,
    Tools,
    Shortcuts,
    Updates,
}

impl SettingsTab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Typography => "typography",
            Self::Scrolling => "scrolling",
            Self::Window => "window",
            Self::Tools => "tools",
            Self::Shortcuts => "shortcuts",
            Self::Updates => "updates",
        }
    }

    pub fn from_str_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "appearance" | "themes" => Self::Appearance,
            "typography" | "font" | "caret" => Self::Typography,
            "scrolling" | "scroll" | "mouse" => Self::Scrolling,
            "window" | "layout" | "splits" => Self::Window,
            "tools" | "satellites" => Self::Tools,
            "shortcuts" | "keyboard" => Self::Shortcuts,
            "updates" | "about" | "config" => Self::Updates,
            _ => Self::General,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsFile {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default = "default_theme_dark")]
    pub theme_dark: String,
    #[serde(default = "default_theme_light")]
    pub theme_light: String,
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
    #[serde(default = "default_tab_bar_position")]
    pub tab_bar_position: String,
    #[serde(default = "default_tiling_gap")]
    pub tiling_gap: f32,

    #[serde(default)]
    pub shortcut_schema: u32,

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
    #[serde(default = "default_editor")]
    pub default_editor: String,
    #[serde(default = "default_notify_on_complete")]
    pub notify_on_complete: bool,
    #[serde(default = "default_notify_threshold_secs")]
    pub notify_threshold_secs: u64,

    #[serde(default = "default_auto_check")]
    pub auto_check_updates: bool,
    #[serde(default = "default_interval_mins")]
    pub update_check_interval_mins: u32,
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default = "default_scroll_speed")]
    pub scroll_speed: f32,
    #[serde(default = "default_scroll_to_bottom_on_input")]
    pub scroll_to_bottom_on_input: bool,
    #[serde(default = "default_show_startup_banner")]
    pub show_startup_banner: bool,
    #[serde(default = "default_last_settings_tab")]
    pub last_settings_tab: String,
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
    "dark".to_string()
}
fn default_theme_dark() -> String {
    "tokyo_night".to_string()
}
fn default_theme_light() -> String {
    "solarized".to_string()
}
fn default_theme_name() -> String {
    "tokyo_night".to_string()
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
    0.15
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
fn default_tab_bar_position() -> String {
    "top".to_string()
}
fn default_tiling_gap() -> f32 {
    6.0
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
fn default_update_channel() -> String {
    if option_env!("ATLAS_BUILD_COMMIT").is_some() {
        "preview".to_string()
    } else {
        "stable".to_string()
    }
}
fn default_editor() -> String {
    "auto".to_string()
}
fn default_notify_on_complete() -> bool {
    true
}
fn default_notify_threshold_secs() -> u64 {
    10
}
fn default_window_width() -> f32 {
    880.0
}
fn default_window_height() -> f32 {
    540.0
}
fn default_scroll_speed() -> f32 {
    1.0
}
fn default_scroll_to_bottom_on_input() -> bool {
    true
}
fn default_show_startup_banner() -> bool {
    true
}
fn default_last_settings_tab() -> String {
    "general".to_string()
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            theme_mode: default_theme_mode(),
            theme_dark: default_theme_dark(),
            theme_light: default_theme_light(),
            theme: default_theme_name(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            line_height: default_line_height(),
            corner_radius: default_corner_radius(),
            opacity: default_opacity(),
            blur_enabled: default_blur(),
            cursor_shape: default_cursor_shape(),
            cursor_blink: default_cursor_blink(),
            tab_bar_position: default_tab_bar_position(),
            tiling_gap: default_tiling_gap(),
            shortcut_schema: 2,
            shortcuts: default_shortcuts(),
            default_shell: default_shell(),
            start_directory: default_start_dir(),
            scrollback_lines: default_scrollback(),
            confirm_close_process: default_confirm_close(),
            copy_on_select: default_copy_on_select(),
            startup_mode: default_startup_mode(),
            default_editor: default_editor(),
            notify_on_complete: default_notify_on_complete(),
            notify_threshold_secs: default_notify_threshold_secs(),
            auto_check_updates: default_auto_check(),
            update_check_interval_mins: default_interval_mins(),
            update_channel: default_update_channel(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            scroll_speed: default_scroll_speed(),
            scroll_to_bottom_on_input: default_scroll_to_bottom_on_input(),
            show_startup_banner: default_show_startup_banner(),
            last_settings_tab: default_last_settings_tab(),
            last_check_time: None,
            etag: None,
            last_seen_version: None,
            skipped_version: None,
        }
    }
}

pub fn load_app_settings() -> AppSettingsFile {
    let base = crate::user_data_dir();
    let settings_path = base.join("settings.json");
    let bak_path = base.join("settings.json.bak");

    let content = std::fs::read_to_string(&settings_path)
        .or_else(|_| std::fs::read_to_string(&bak_path));

    if let Ok(content) = content {
        if let Ok(mut s) = serde_json::from_str::<AppSettingsFile>(&content) {
            if s.theme_dark.is_empty() {
                s.theme_dark = default_theme_dark();
            }
            if s.theme_light.is_empty() {
                s.theme_light = default_theme_light();
            }
            if s.shortcut_schema < 2 {
                s.shortcuts = default_shortcuts();
                s.shortcut_schema = 2;
                save_app_settings(&s);
            } else {
                let defaults = default_shortcuts();
                for (action, chord) in defaults {
                    s.shortcuts.entry(action).or_insert(chord);
                }
            }
            return s;
        }
    }
    AppSettingsFile::default()
}

pub fn save_app_settings(s: &AppSettingsFile) {
    let base = crate::user_data_dir();
    let _ = std::fs::create_dir_all(&base);
    let settings_path = base.join("settings.json");
    let bak_path = base.join("settings.json.bak");

    if let Ok(json) = serde_json::to_string_pretty(s) {
        update_saved_settings_hash(&json);
        if settings_path.exists() {
            let _ = std::fs::copy(&settings_path, &bak_path);
        }
        let _ = std::fs::write(&settings_path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_theme_fields_roundtrip() {
        let mut settings = AppSettingsFile::default();
        settings.theme_mode = "auto".to_string();
        settings.theme_dark = "tokyo_night".to_string();
        settings.theme_light = "solarized".to_string();

        let json = serde_json::to_string(&settings).unwrap();
        let loaded: AppSettingsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.theme_mode, "auto");
        assert_eq!(loaded.theme_dark, "tokyo_night");
        assert_eq!(loaded.theme_light, "solarized");
    }

    #[test]
    fn test_theme_and_mode_shortcuts_configured() {
        let defaults = default_shortcuts();
        assert_eq!(defaults.get(ACTION_NEXT_THEME).map(|s| s.as_str()), Some("alt+t"));
        assert_eq!(defaults.get(ACTION_PREV_THEME).map(|s| s.as_str()), Some("alt+shift+t"));
        assert_eq!(defaults.get(ACTION_TOGGLE_DARK_LIGHT).map(|s| s.as_str()), Some("ctrl+shift+l"));
        assert!(!action_display_name(ACTION_NEXT_THEME).is_empty());
        assert!(!action_display_name(ACTION_PREV_THEME).is_empty());
        assert!(!action_display_name(ACTION_TOGGLE_DARK_LIGHT).is_empty());
    }

    #[test]
    fn test_app_settings_backward_compatible_defaults() {
        let legacy_json = r#"{"theme": "atlas", "font_size": 14.0}"#;
        let loaded: AppSettingsFile = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(loaded.theme_mode, "dark");
        assert_eq!(loaded.theme_dark, "tokyo_night");
        assert_eq!(loaded.theme_light, "solarized");
        assert_eq!(loaded.theme, "atlas");
    }

    #[test]
    fn test_settings_tab_string_conversion() {
        let tabs = [
            SettingsTab::General,
            SettingsTab::Appearance,
            SettingsTab::Typography,
            SettingsTab::Scrolling,
            SettingsTab::Window,
            SettingsTab::Tools,
            SettingsTab::Shortcuts,
            SettingsTab::Updates,
        ];
        for tab in tabs {
            assert_eq!(SettingsTab::from_str_name(tab.as_str()), tab);
        }
        assert_eq!(SettingsTab::from_str_name("unknown"), SettingsTab::General);
    }

    #[test]
    fn test_app_settings_new_fields_roundtrip() {
        let mut settings = AppSettingsFile::default();
        settings.last_settings_tab = "scrolling".to_string();
        settings.scroll_speed = 0.6;
        settings.scroll_to_bottom_on_input = false;
        settings.show_startup_banner = false;
        settings.blur_enabled = false;
        settings.opacity = 0.45;

        let json = serde_json::to_string(&settings).unwrap();
        let loaded: AppSettingsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.last_settings_tab, "scrolling");
        assert_eq!(loaded.scroll_speed, 0.6);
        assert_eq!(loaded.scroll_to_bottom_on_input, false);
        assert_eq!(loaded.show_startup_banner, false);
        assert_eq!(loaded.blur_enabled, false);
        assert_eq!(loaded.opacity, 0.45);
    }
}
