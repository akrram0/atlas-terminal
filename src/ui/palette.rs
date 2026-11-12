//! Command palette modal overlay and command catalog.

use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, Context, ElementId, IntoElement,
    Keystroke, MouseButton, MouseDownEvent, Window, div, px,
};

use crate::icons;
use crate::session::Profile;
use crate::theme::Theme;
use crate::view::AtlasView;
use crate::workspace::SplitAxis;
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS, OVERLAY_OUT_MS};

/// Category of items in the command palette (Antigravity-style switcher).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCategory {
    All,
    Commands,
    Shells,
    Tools,
}

impl PaletteCategory {
    pub const ALL: [PaletteCategory; 4] = [
        PaletteCategory::All,
        PaletteCategory::Commands,
        PaletteCategory::Shells,
        PaletteCategory::Tools,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Commands => "Commands",
            Self::Shells => "Shells",
            Self::Tools => "Tools",
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::All => "Search commands, shells, and tools...",
            Self::Commands => "Search terminal commands & actions...",
            Self::Shells => "Search installed shells & WSL distros...",
            Self::Tools => "Search AI agents & developer tools...",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::All => icons::SEARCH,
            Self::Commands => icons::KEYBOARD,
            Self::Shells => icons::TERMINAL,
            Self::Tools => icons::ANTIGRAVITY_MARK,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Commands,
            Self::Commands => Self::Shells,
            Self::Shells => Self::Tools,
            Self::Tools => Self::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::All => Self::Tools,
            Self::Commands => Self::All,
            Self::Shells => Self::Commands,
            Self::Tools => Self::Shells,
        }
    }
}

/// Commands offered by the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Command {
    NewWindow,
    NewTab,
    NewTabWsl,
    NewTabCmd,
    NewTabGitBash,
    NewTabOpenCode,
    NewTabAntigravity,
    SplitPane,
    SplitPaneDown,
    SplitPaneWsl,
    SplitPaneDownWsl,
    FocusNextPane,
    FocusPrevPane,
    MaximizePane,
    ClosePane,
    CloseTab,
    NextTab,
    PrevTab,
    RestartTab,
    RenameTab,
    CopySelection,
    PasteClipboard,
    SelectAll,
    ClearScrollback,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    ToggleTheme,
    NextTheme,
    PrevTheme,
    OpenSettings,
    FontInc,
    FontDec,
    FontReset,
    CheckUpdates,
    OpenLatestUrl,
    FindInTerminal,
    ToggleSidebar,
    Fullscreen,
}

/// Action to execute from the palette (a built-in command, shell, recent dir, or opening a detected URL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Command(Command),
    #[allow(dead_code)]
    OpenUrl(String),
    LaunchProfile(Profile),
    SplitProfile(SplitAxis, Profile),
    #[allow(dead_code)]
    OpenRecentDirectory(PathBuf),
    RescanEnvironment,
    SelectTheme(usize),
}

/// A structured palette item displayed in the Antigravity-style categorized list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub action: PaletteAction,
    pub category: PaletteCategory,
    pub icon: &'static str,
    pub title: String,
    pub subtitle: String,
    pub shortcut_or_badge: String,
}

pub fn command_subtitle(cmd: Command) -> &'static str {
    match cmd {
        Command::NewWindow => "Window · Open a fresh Atlas window",
        Command::ToggleSidebar => "View · Toggle tab bar / sidebar mode",
        Command::FindInTerminal => "Search · Find text in terminal output",
        Command::NewTab => "Shell · Launch default shell tab",
        Command::NewTabWsl => "WSL · Launch default WSL distribution",
        Command::NewTabCmd => "Shell · Launch Command Prompt tab",
        Command::NewTabGitBash => "Shell · Launch Git Bash session",
        Command::NewTabOpenCode => "AI Agent · Launch OpenCode CLI",
        Command::NewTabAntigravity => "AI Agent · Launch Antigravity CLI",
        Command::SplitPane => "Layout · Split pane vertically (columns)",
        Command::SplitPaneDown => "Layout · Split pane horizontally (rows)",
        Command::SplitPaneWsl => "Layout · Split pane vertically with WSL",
        Command::SplitPaneDownWsl => "Layout · Split pane horizontally with WSL",
        Command::FocusNextPane => "Navigation · Focus next split pane",
        Command::FocusPrevPane => "Navigation · Focus previous split pane",
        Command::MaximizePane => "Layout · Toggle full pane expansion",
        Command::Fullscreen => "Window · Toggle borderless fullscreen",
        Command::ClosePane => "Session · Close active pane",
        Command::CloseTab => "Session · Close active tab",
        Command::NextTab => "Navigation · Switch to next tab",
        Command::PrevTab => "Navigation · Switch to previous tab",
        Command::CopySelection => "Clipboard · Copy selected text",
        Command::PasteClipboard => "Clipboard · Paste from clipboard",
        Command::SelectAll => "Selection · Select all in terminal viewport",
        Command::ClearScrollback => "Buffer · Clear terminal screen & history",
        Command::ScrollPageUp => "Navigation · Page up through buffer",
        Command::ScrollPageDown => "Navigation · Page down through buffer",
        Command::ScrollToTop => "Navigation · Jump to top of scrollback",
        Command::ScrollToBottom => "Navigation · Jump to bottom of scrollback",
        Command::OpenLatestUrl => "Links · Open latest URL detected in buffer",
        Command::RestartTab => "Session · Restart active shell pane",
        Command::RenameTab => "Tab · Set a custom title for this tab",
        Command::NextTheme => "Appearance · Switch to next color theme (Catppuccin, Tokyo Night, etc.)",
        Command::PrevTheme => "Appearance · Switch to previous color theme",
        Command::ToggleTheme => "Appearance · Switch between Dark and Light mode",
        Command::OpenSettings => "Settings · Configure terminal, font, and themes",
        Command::FontInc => "Display · Increase terminal font size",
        Command::FontDec => "Display · Decrease terminal font size",
        Command::FontReset => "Display · Reset terminal font size to default",
        Command::CheckUpdates => "System · Check GitHub for Atlas updates",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_category_cycling() {
        let cat = PaletteCategory::All;
        assert_eq!(cat.next(), PaletteCategory::Commands);
        assert_eq!(cat.next().next(), PaletteCategory::Shells);
        assert_eq!(cat.next().next().next(), PaletteCategory::Tools);
        assert_eq!(cat.next().next().next().next(), PaletteCategory::All);

        assert_eq!(cat.prev(), PaletteCategory::Tools);
        assert_eq!(cat.prev().prev(), PaletteCategory::Shells);
        assert_eq!(cat.prev().prev().prev(), PaletteCategory::Commands);
    }

    #[test]
    fn test_palette_category_labels_and_placeholders() {
        for cat in PaletteCategory::ALL {
            assert!(!cat.label().is_empty());
            assert!(!cat.placeholder().is_empty());
            assert!(!cat.icon().is_empty());
        }
    }

    #[test]
    fn test_all_commands_have_subtitles() {
        for (cmd, _, _, _) in &COMMANDS {
            let sub = command_subtitle(*cmd);
            assert!(!sub.is_empty(), "command subtitle must not be empty");
        }
    }

    #[test]
    fn test_palette_action_theme_equality() {
        let action = PaletteAction::SelectTheme(3);
        assert_eq!(action, PaletteAction::SelectTheme(3));
        assert_ne!(action, PaletteAction::SelectTheme(4));
    }
}

/// Palette entries: command, vector icon path, label, shortcut hint.
pub const COMMANDS: [(Command, &str, &str, &str); 39] = [
    (Command::NewWindow, icons::ATLAS_MARK, "New window", "Ctrl Shift N"),
    (Command::ToggleSidebar, icons::SIDEBAR_LEFT, "Toggle Sidebar / Tab Bar", "Ctrl B"),
    (Command::FindInTerminal, icons::SEARCH, "Find in terminal...", "Ctrl F"),
    (Command::NewTab, icons::PLUS, "New tab (PowerShell)", "Ctrl T / Ctrl Shift 1"),
    (Command::NewTabWsl, icons::LINUX, "New WSL tab", "Ctrl Shift 2"),
    (Command::NewTabCmd, icons::CMD, "New Command Prompt tab", "Ctrl Shift 3"),
    (Command::NewTabGitBash, icons::GIT, "New Git Bash tab", "Ctrl Shift 4"),
    (Command::NewTabOpenCode, icons::OPENCODE_MARK, "New OpenCode tab", "Ctrl Shift 5"),
    (Command::NewTabAntigravity, icons::ANTIGRAVITY_MARK, "New Antigravity tab", "Ctrl Shift 6"),
    (Command::SplitPane, icons::SPLIT_COLUMNS, "Split pane right (columns)", "Alt Enter"),
    (Command::SplitPaneDown, icons::SPLIT_ROWS, "Split pane down (rows)", "Alt D"),
    (Command::SplitPaneWsl, icons::SPLIT_COLUMNS, "Split pane right with WSL", ""),
    (Command::SplitPaneDownWsl, icons::SPLIT_ROWS, "Split pane down with WSL", ""),
    (Command::FocusNextPane, icons::CHEVRON_RIGHT, "Focus next split pane", "Alt Right"),
    (Command::FocusPrevPane, icons::CHEVRON_LEFT, "Focus previous split pane", "Alt Left"),
    (
        Command::MaximizePane,
        icons::WINDOW_MAXIMIZE,
        "Toggle maximize pane",
        "Alt Z",
    ),
    (Command::Fullscreen, icons::WINDOW_MAXIMIZE, "Toggle fullscreen", "F11"),
    (Command::CloseTab, icons::CLOSE, "Close pane or tab", "Alt W"),
    (Command::NextTab, icons::CHEVRON_RIGHT, "Next tab", "Ctrl Tab"),
    (
        Command::PrevTab,
        icons::CHEVRON_LEFT,
        "Previous tab",
        "Ctrl Shift Tab",
    ),
    (Command::CopySelection, icons::KEYBOARD, "Copy selection", "Ctrl Shift C"),
    (Command::PasteClipboard, icons::KEYBOARD, "Paste clipboard", "Ctrl Shift V"),
    (Command::SelectAll, icons::KEYBOARD, "Select all in viewport", "Ctrl Shift A"),
    (Command::ClearScrollback, icons::REFRESH, "Clear scrollback buffer", "Ctrl Shift K"),
    (Command::ScrollPageUp, icons::CHEVRON_RIGHT, "Scroll page up", "Shift PageUp"),
    (Command::ScrollPageDown, icons::CHEVRON_RIGHT, "Scroll page down", "Shift PageDown"),
    (Command::ScrollToTop, icons::CHEVRON_RIGHT, "Scroll to top of buffer", "Shift Home"),
    (Command::ScrollToBottom, icons::CHEVRON_RIGHT, "Scroll to bottom of buffer", "Shift End"),
    (Command::OpenLatestUrl, icons::LINK, "Open latest URL from buffer", "Ctrl Shift U"),
    (Command::RestartTab, icons::REFRESH, "Restart pane", "R"),
    (Command::RenameTab, icons::PEN, "Rename tab", ""),
    (
        Command::NextTheme,
        icons::SLIDERS,
        "Next theme preset",
        "Alt T",
    ),
    (
        Command::PrevTheme,
        icons::SLIDERS,
        "Previous theme preset",
        "Alt Shift T",
    ),
    (
        Command::ToggleTheme,
        icons::SUN,
        "Toggle dark / light mode",
        "Ctrl Shift L",
    ),
    (Command::OpenSettings, icons::SETTINGS, "Open settings", "Ctrl ,"),
    (Command::FontInc, icons::SETTINGS, "Increase font size", "Ctrl ="),
    (Command::FontDec, icons::SETTINGS, "Decrease font size", "Ctrl -"),
    (Command::FontReset, icons::SETTINGS, "Reset font size (default)", "Ctrl 0"),
    (Command::CheckUpdates, icons::REFRESH, "Check for updates", ""),
];

impl AtlasView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.user_profile.verify_installed();
        self.palette_open = true;
        self.palette_closing = false;
        self.palette_orig_theme = Some((self.theme_dark_id, self.theme_light_id));
        self.palette_category = PaletteCategory::All;
        self.palette_category_menu_open = false;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_anim_index = 0.0;
        self.palette_scroll_handle.set_offset(gpui::Point::default());
        self.profile_menu = false;
        window.focus(&self.palette_focus);
        cx.notify();
    }

    /// Fade the palette out, then unmount it once the timer task fires; a
    /// fresh open cancels the pending dismissal.
    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_closing {
            return;
        }
        self.palette_closing = true;
        self.palette_category_menu_open = false;

        // If a theme was previewed but never confirmed with Enter, revert to original theme
        if let Some((orig_dark, orig_light)) = self.palette_orig_theme.take() {
            let is_light = match self.theme_mode.as_str() {
                "light" => true,
                "dark" => false,
                _ => crate::theme::is_system_light_mode(),
            };
            if is_light {
                if self.theme_light_id != orig_light {
                    self.set_light_theme(orig_light, cx);
                }
            } else if self.theme_dark_id != orig_dark {
                self.set_dark_theme(orig_dark, cx);
            }
        }

        self.focus_main(window);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.palette_closing {
                    view.palette_closing = false;
                    view.palette_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn shortcut_for_palette_command(&self, cmd: Command) -> String {
        use crate::config::*;
        let action = match cmd {
            Command::NewWindow => Some(ACTION_NEW_WINDOW),
            Command::ToggleSidebar => Some(ACTION_TOGGLE_SIDEBAR),
            Command::FindInTerminal => Some(ACTION_FIND),
            Command::NewTab => Some(ACTION_NEW_TAB),
            Command::SplitPane => Some(ACTION_SPLIT_VERTICAL),
            Command::SplitPaneDown => Some(ACTION_SPLIT_HORIZONTAL),
            Command::FocusNextPane => Some(ACTION_FOCUS_NEXT_PANE),
            Command::FocusPrevPane => Some(ACTION_FOCUS_PREV_PANE),
            Command::MaximizePane => Some(ACTION_TOGGLE_MAXIMIZE),
            Command::Fullscreen => Some(ACTION_FULLSCREEN),
            Command::ClosePane | Command::CloseTab => Some(ACTION_CLOSE_TAB),
            Command::NextTab => Some(ACTION_NEXT_TAB),
            Command::PrevTab => Some(ACTION_PREV_TAB),
            Command::CopySelection => Some(ACTION_COPY),
            Command::PasteClipboard => Some(ACTION_PASTE),
            Command::SelectAll => Some(ACTION_SELECT_ALL),
            Command::ClearScrollback => Some(ACTION_CLEAR_SCROLLBACK),
            Command::ScrollPageUp => Some(ACTION_SCROLL_PAGE_UP),
            Command::ScrollPageDown => Some(ACTION_SCROLL_PAGE_DOWN),
            Command::ScrollToTop => Some(ACTION_SCROLL_TO_TOP),
            Command::ScrollToBottom => Some(ACTION_SCROLL_TO_BOTTOM),
            Command::OpenLatestUrl => Some(ACTION_OPEN_URL_PICKER),
            Command::NextTheme => Some(ACTION_NEXT_THEME),
            Command::PrevTheme => Some(ACTION_PREV_THEME),
            Command::ToggleTheme => Some(ACTION_TOGGLE_DARK_LIGHT),
            Command::OpenSettings => Some(ACTION_OPEN_SETTINGS),
            Command::FontInc => Some(ACTION_ZOOM_IN),
            Command::FontDec => Some(ACTION_ZOOM_OUT),
            Command::FontReset => Some(ACTION_ZOOM_RESET),
            Command::RestartTab => Some("r"),
            _ => None,
        };

        if let Some(act) = action {
            if act == "r" {
                return "R".to_string();
            }
            let chord = self.get_shortcut(act);
            if !chord.is_empty() {
                return crate::config::pretty_shortcut(&chord);
            }
        }
        String::new()
    }

    /// Items matching the current query and active category in the Antigravity-style catalog.
    pub fn palette_matches(&self) -> Vec<PaletteItem> {
        let mut all_items = Vec::new();

        // 1. Built-in Commands
        for (cmd, icon, label, default_sc) in &COMMANDS {
            let mut shortcut = self.shortcut_for_palette_command(*cmd);
            if shortcut.is_empty() && !default_sc.is_empty() {
                shortcut = default_sc.to_string();
            }
            all_items.push(PaletteItem {
                action: PaletteAction::Command(*cmd),
                category: PaletteCategory::Commands,
                icon: *icon,
                title: label.to_string(),
                subtitle: command_subtitle(*cmd).to_string(),
                shortcut_or_badge: shortcut,
            });
        }

        // Rescan Environment Action
        all_items.push(PaletteItem {
            action: PaletteAction::RescanEnvironment,
            category: PaletteCategory::Commands,
            icon: icons::REFRESH,
            title: "Rescan Environment & Shells".to_string(),
            subtitle: "System · Re-probe installed shells, WSL distros, and developer tools".to_string(),
            shortcut_or_badge: "Action".to_string(),
        });

        // 2. Installed Shells
        for s in &self.user_profile.shells {
            let prof = Profile::from_name(&s.id);
            let icon = match s.id.as_str() {
                "pwsh" | "powershell" => icons::POWERSHELL,
                "cmd" => icons::CMD,
                "gitbash" => icons::GIT,
                _ => icons::TERMINAL,
            };
            let badge = if s.is_default { "Default Shell" } else { "Shell" };
            all_items.push(PaletteItem {
                action: PaletteAction::LaunchProfile(prof),
                category: PaletteCategory::Shells,
                icon,
                title: format!("New tab: {}", s.title),
                subtitle: format!("Windows Shell · {}", s.path),
                shortcut_or_badge: badge.to_string(),
            });
        }

        // WSL distros
        for d in &self.user_profile.wsl_distros {
            let icon = crate::wsl::icon_for_name(&d.name);
            let badge = if d.is_default { "Default WSL" } else { "WSL" };
            all_items.push(PaletteItem {
                action: PaletteAction::LaunchProfile(Profile::WslDistro(d.name.clone())),
                category: PaletteCategory::Shells,
                icon,
                title: format!("New tab: {} (WSL)", d.title),
                subtitle: format!("WSL Distro · {}", d.name),
                shortcut_or_badge: badge.to_string(),
            });
            all_items.push(PaletteItem {
                action: PaletteAction::SplitProfile(SplitAxis::Columns, Profile::WslDistro(d.name.clone())),
                category: PaletteCategory::Shells,
                icon: icons::SPLIT_COLUMNS,
                title: format!("Split right with {} (WSL)", d.title),
                subtitle: format!("WSL Distro · {}", d.name),
                shortcut_or_badge: "WSL Split".to_string(),
            });
            all_items.push(PaletteItem {
                action: PaletteAction::SplitProfile(SplitAxis::Rows, Profile::WslDistro(d.name.clone())),
                category: PaletteCategory::Shells,
                icon: icons::SPLIT_ROWS,
                title: format!("Split down with {} (WSL)", d.title),
                subtitle: format!("WSL Distro · {}", d.name),
                shortcut_or_badge: "WSL Split".to_string(),
            });
        }

        // 3. AI Agents & CLI Tools
        for t in &self.user_profile.tools {
            let (prof, icon, badge) = match t.name.as_str() {
                "agy" | "antigravity" => (Profile::Antigravity, icons::ANTIGRAVITY_MARK, "AI Agent"),
                "opencode" => (Profile::OpenCode, icons::OPENCODE_MARK, "AI Agent"),
                "claude" => (Profile::Claude, icons::CLAUDE_MARK, "AI Agent"),
                "lazygit" => (
                    Profile::CustomTool {
                        name: t.title.clone(),
                        path: t.path.clone(),
                    },
                    icons::GIT,
                    "CLI Tool",
                ),
                _ => (
                    Profile::CustomTool {
                        name: t.title.clone(),
                        path: t.path.clone(),
                    },
                    icons::TERMINAL,
                    "CLI Tool",
                ),
            };
            all_items.push(PaletteItem {
                action: PaletteAction::LaunchProfile(prof),
                category: PaletteCategory::Tools,
                icon,
                title: format!("Launch {}", t.title),
                subtitle: format!("Tool · {}", t.path),
                shortcut_or_badge: badge.to_string(),
            });
        }

        // 4. Themes (Direct theme selection & live preview)
        let is_light = match self.theme_mode.as_str() {
            "light" => true,
            "dark" => false,
            _ => crate::theme::is_system_light_mode(),
        };
        for (idx, preset) in self.registry.presets().iter().enumerate() {
            if is_light && !preset.supports_light() {
                continue;
            }
            if !is_light && !preset.supports_dark() {
                continue;
            }
            let name = if is_light { preset.light_name() } else { preset.dark_name() };
            all_items.push(PaletteItem {
                action: PaletteAction::SelectTheme(idx),
                category: PaletteCategory::Commands,
                icon: icons::SLIDERS,
                title: format!("Theme: {}", name),
                subtitle: format!("Appearance · {}", preset.description),
                shortcut_or_badge: if is_light { "Light Theme".to_string() } else { "Dark Theme".to_string() },
            });
        }

        // Filter by category
        let filtered_by_category: Vec<PaletteItem> = if self.palette_category == PaletteCategory::All {
            all_items
        } else {
            all_items
                .into_iter()
                .filter(|item| item.category == self.palette_category)
                .collect()
        };

        let query = self.palette_query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return filtered_by_category.into_iter().take(100).collect();
        }

        let words: Vec<&str> = query.split_whitespace().collect();
        filtered_by_category
            .into_iter()
            .filter(|item| {
                let t = item.title.to_ascii_lowercase();
                let sub = item.subtitle.to_ascii_lowercase();
                let b = item.shortcut_or_badge.to_ascii_lowercase();
                words.iter().all(|w| t.contains(w) || sub.contains(w) || b.contains(w))
            })
            .take(100)
            .collect()
    }

    /// Keep active item in visible view while navigating up/down in the command palette.
    fn scroll_palette_to_active(&mut self) {
        let row_h = 46.0;
        let gap = 3.0;
        let slot_h = row_h + gap;
        let viewport_h = 380.0;
        let cur_offset_y = f32::from(self.palette_scroll_handle.offset().y);
        let scroll_top = if cur_offset_y <= 0.0 { -cur_offset_y } else { cur_offset_y };
        let scroll_bottom = scroll_top + viewport_h;
        let item_top = self.palette_index as f32 * slot_h;
        let item_bottom = item_top + row_h;

        if item_top < scroll_top {
            let new_y = -item_top;
            self.palette_scroll_handle.set_offset(gpui::Point { x: px(0.0), y: px(new_y) });
        } else if item_bottom > scroll_bottom {
            let new_y = -(item_bottom - viewport_h + 12.0);
            self.palette_scroll_handle.set_offset(gpui::Point { x: px(0.0), y: px(new_y) });
        }
    }

    /// Palette key routing: type to filter, Tab to cycle categories, ↑/↓ to move, Enter to run, Esc to dismiss.
    pub fn handle_palette_key(
        &mut self,
        ks: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ks.key.as_str() {
            "escape" => {
                if self.palette_category_menu_open {
                    self.palette_category_menu_open = false;
                    cx.notify();
                } else {
                    self.close_palette(window, cx);
                }
            }
            "tab" => {
                if ks.modifiers.shift {
                    self.palette_category = self.palette_category.prev();
                } else {
                    self.palette_category = self.palette_category.next();
                }
                self.palette_category_menu_open = false;
                self.palette_index = 0;
                self.palette_anim_index = 0.0;
                self.palette_scroll_handle.set_offset(gpui::Point::default());
                cx.notify();
            }
            "enter" => {
                let matches = self.palette_matches();
                let index = self.palette_index.min(matches.len().saturating_sub(1));
                match matches.get(index).cloned() {
                    Some(item) => self.run_palette_action(item.action, window, cx),
                    None => self.close_palette(window, cx),
                }
            }
            "up" => {
                self.palette_index = self.palette_index.saturating_sub(1);
                self.scroll_palette_to_active();
                self.preview_palette_theme_if_selected(cx);
                cx.notify();
            }
            "down" => {
                let last = self.palette_matches().len().saturating_sub(1);
                self.palette_index = (self.palette_index + 1).min(last);
                self.scroll_palette_to_active();
                self.preview_palette_theme_if_selected(cx);
                cx.notify();
            }
            "backspace" => {
                if self.palette_query.pop().is_some() {
                    self.palette_index = 0;
                    self.palette_anim_index = 0.0;
                    self.palette_scroll_handle.set_offset(gpui::Point::default());
                    self.preview_palette_theme_if_selected(cx);
                }
                cx.notify();
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    // The space bar arrives without `key_char` (gpui treats
                    // it as an immutable key), so type it by hand.
                    let raw_typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else if let Some(text) = ks.key_char.as_deref() {
                        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
                        if !filtered.is_empty() {
                            filtered
                        } else if ks.key.chars().count() == 1 && !ks.key.chars().next().unwrap().is_control() {
                            ks.key.clone()
                        } else {
                            String::new()
                        }
                    } else if ks.key.chars().count() == 1 && !ks.key.chars().next().unwrap().is_control() {
                        ks.key.clone()
                    } else {
                        String::new()
                    };
                    let typed = crate::keymap::adjust_for_caps_lock(&raw_typed, ks.modifiers.shift);
                    if !typed.is_empty() {
                        self.palette_query.push_str(&typed);
                        self.palette_index = 0;
                        self.palette_anim_index = 0.0;
                        self.palette_scroll_handle.set_offset(gpui::Point::default());
                        self.preview_palette_theme_if_selected(cx);
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Previews the currently selected palette theme live in the background without committing.
    fn preview_palette_theme_if_selected(&mut self, cx: &mut Context<Self>) {
        let matches = self.palette_matches();
        if let Some(item) = matches.get(self.palette_index) {
            if let PaletteAction::SelectTheme(theme_idx) = item.action {
                let is_light = match self.theme_mode.as_str() {
                    "light" => true,
                    "dark" => false,
                    _ => crate::theme::is_system_light_mode(),
                };
                if is_light {
                    if self.theme_light_id != theme_idx {
                        self.set_light_theme(theme_idx, cx);
                    }
                } else if self.theme_dark_id != theme_idx {
                    self.set_dark_theme(theme_idx, cx);
                }
            }
        }
    }

    /// Run a palette action (command, URL, profile, or folder); the palette fades out on the way.
    pub fn run_palette_action(&mut self, action: PaletteAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        match action {
            PaletteAction::Command(command) => self.run_command(command, window, cx),
            PaletteAction::SelectTheme(theme_idx) => {
                let is_light = match self.theme_mode.as_str() {
                    "light" => true,
                    "dark" => false,
                    _ => crate::theme::is_system_light_mode(),
                };
                if is_light {
                    self.set_light_theme(theme_idx, cx);
                } else {
                    self.set_dark_theme(theme_idx, cx);
                }
                // Confirmed! Clear backup so it is kept permanently
                self.palette_orig_theme = None;
                self.sync_and_save_settings();
                let preset = self.registry.preset(theme_idx);
                let name = if is_light { preset.light_name() } else { preset.dark_name() };
                self.show_update_toast(format!("Applied theme '{}'", name), true, cx);
                self.focus_main(window);
            }
            PaletteAction::OpenUrl(url) => {
                crate::url::open_url(&url);
                self.focus_main(window);
            }
            PaletteAction::LaunchProfile(profile) => {
                self.new_tab(profile, cx);
                self.focus_main(window);
            }
            PaletteAction::SplitProfile(axis, profile) => {
                self.split_active_profile(axis, profile, cx);
                self.focus_main(window);
            }
            PaletteAction::OpenRecentDirectory(dir) => {
                let prof = self.get_default_profile();
                self.new_tab_with_cwd(prof, Some(dir), cx);
                self.focus_main(window);
            }
            PaletteAction::RescanEnvironment => {
                self.rescan_environment(cx);
                self.focus_main(window);
            }
        }
    }

    /// Run a palette command; the palette fades out on the way.
    pub fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        match command {
            Command::NewWindow => {
                self.new_window(cx);
                window.focus(&self.focus);
            }
            Command::NewTab => {
                let prof = self.get_default_profile();
                self.new_tab(prof, cx);
                window.focus(&self.focus);
            }
            Command::NewTabWsl => {
                self.new_tab(Profile::Wsl, cx);
                window.focus(&self.focus);
            }
            Command::NewTabCmd => {
                self.new_tab(Profile::Cmd, cx);
                window.focus(&self.focus);
            }
            Command::NewTabGitBash => {
                self.new_tab(Profile::GitBash, cx);
                window.focus(&self.focus);
            }
            Command::NewTabOpenCode => {
                self.new_tab(Profile::OpenCode, cx);
                window.focus(&self.focus);
            }
            Command::NewTabAntigravity => {
                self.new_tab(Profile::Antigravity, cx);
                window.focus(&self.focus);
            }
            Command::SplitPane => {
                self.settings_open = false;
                self.split_active(SplitAxis::Columns, cx);
                window.focus(&self.focus);
            }
            Command::SplitPaneDown => {
                self.settings_open = false;
                self.split_active(SplitAxis::Rows, cx);
                window.focus(&self.focus);
            }
            Command::SplitPaneWsl => {
                self.settings_open = false;
                self.split_active_profile(SplitAxis::Columns, Profile::Wsl, cx);
                window.focus(&self.focus);
            }
            Command::SplitPaneDownWsl => {
                self.settings_open = false;
                self.split_active_profile(SplitAxis::Rows, Profile::Wsl, cx);
                window.focus(&self.focus);
            }
            Command::FocusNextPane => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    if tab.panes.len() > 1 {
                        tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                        cx.notify();
                    }
                }
                window.focus(&self.focus);
            }
            Command::FocusPrevPane => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    if tab.panes.len() > 1 {
                        tab.active_pane = if tab.active_pane == 0 {
                            tab.panes.len() - 1
                        } else {
                            tab.active_pane - 1
                        };
                        cx.notify();
                    }
                }
                window.focus(&self.focus);
            }
            Command::MaximizePane => {
                self.toggle_maximize(cx);
                window.focus(&self.focus);
            }
            Command::ClosePane => {
                self.close_pane(cx);
                self.focus_main(window);
            }
            Command::CloseTab => {
                let index = self.active;
                self.close_tab(index, cx);
                self.focus_main(window);
            }
            Command::NextTab => {
                self.cycle(1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::PrevTab => {
                self.cycle(-1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::RestartTab => {
                let index = self.active;
                self.restart_tab(index, cx);
                window.focus(&self.focus);
            }
            Command::RenameTab => self.open_rename(window, cx),
            Command::CopySelection => {
                self.copy_selection(cx);
                self.focus_main(window);
            }
            Command::PasteClipboard => {
                self.paste(cx);
                self.focus_main(window);
            }
            Command::SelectAll => {
                self.select_all_viewport(cx);
                self.focus_main(window);
            }
            Command::ClearScrollback => {
                self.write_to_shell(b"\x1b[3J\x1b[H\x1b[2J");
                self.focus_main(window);
                cx.notify();
            }
            Command::ScrollPageUp => {
                self.scroll(15);
                self.focus_main(window);
                cx.notify();
            }
            Command::ScrollPageDown => {
                self.scroll(-15);
                self.focus_main(window);
                cx.notify();
            }
            Command::ScrollToTop => {
                self.scroll(100_000);
                self.focus_main(window);
                cx.notify();
            }
            Command::ScrollToBottom => {
                self.scroll(-100_000);
                self.focus_main(window);
                cx.notify();
            }
            Command::NextTheme => {
                self.next_theme(cx);
                self.focus_main(window);
            }
            Command::PrevTheme => {
                self.prev_theme(cx);
                self.focus_main(window);
            }
            Command::ToggleTheme => {
                self.toggle_theme(cx);
                self.focus_main(window);
            }
            Command::OpenSettings => {
                self.settings_tab_open = true;
                self.settings_open = true;
                self.profile_menu = false;
                window.focus(&self.settings_focus);
                cx.notify();
            }
            Command::FontInc => {
                let cur = self.active_font_size();
                self.adjust_font_size(cur + 1.0, cx);
                self.focus_main(window);
            }
            Command::FontDec => {
                let cur = self.active_font_size();
                self.adjust_font_size(cur - 1.0, cx);
                self.focus_main(window);
            }
            Command::FontReset => {
                self.reset_font_size(cx);
                self.focus_main(window);
            }
            Command::CheckUpdates => {
                self.check_updates_manual(cx);
            }
            Command::OpenLatestUrl => {
                self.open_buffer_urls(window, cx);
            }
            Command::FindInTerminal => {
                self.open_search(window, cx);
            }
            Command::ToggleSidebar => {
                self.toggle_sidebar(cx);
            }
            Command::Fullscreen => {
                crate::rounded_corners::window_button(1);
                cx.notify();
            }
        }
    }

    pub fn palette(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let matches = self.palette_matches();
        let closing = self.palette_closing;
        let active_cat = self.palette_category;
        let cat_menu_open = self.palette_category_menu_open;

        // Category pill button on left
        let cat_btn = div()
            .id(ElementId::named_usize("palette-category-btn", 0))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .bg(if cat_menu_open { theme.card_active } else { theme.soft_bg })
            .cursor_pointer()
            .hover(move |s| s.bg(theme.card_hover))
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.chrome_fg_active)
                    .child(active_cat.label()),
            )
            .child(
                icons::icon(icons::CHEVRON_DOWN)
                    .size(px(10.0))
                    .text_color(theme.faint_fg),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                view.palette_category_menu_open = !view.palette_category_menu_open;
                window.focus(&view.palette_focus);
                cx.notify();
            }));

        let mut text_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.0))
            .flex_1()
            .text_size(px(14.0));
        if self.palette_query.is_empty() {
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(2.0)).h(px(18.0)).rounded_full().bg(theme.accent),
                );
            }
            text_row = text_row.child(div().text_color(theme.faint_fg).child(active_cat.placeholder()));
        } else {
            text_row = text_row.child(
                div()
                    .text_color(theme.chrome_fg_active)
                    .child(self.palette_query.clone()),
            );
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(2.0)).h(px(18.0)).rounded_full().bg(theme.accent),
                );
            }
        }

        // Right side hints & clear button
        let mut right_section = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0));

        if !self.palette_query.is_empty() {
            right_section = right_section.child(
                div()
                    .id(ElementId::named_usize("palette-clear-btn", 0))
                    .p(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme.card_hover))
                    .child(icons::icon(icons::CLOSE).size(px(13.0)).text_color(theme.faint_fg))
                    .on_mouse_down(MouseButton::Left, cx.listener(|view, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        view.palette_query.clear();
                        view.palette_index = 0;
                        view.palette_anim_index = 0.0;
                        view.palette_scroll_handle.set_offset(gpui::Point::default());
                        window.focus(&view.palette_focus);
                        cx.notify();
                    })),
            );
        }

        right_section = right_section.child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .bg(theme.soft_bg)
                .text_size(px(10.5))
                .text_color(theme.faint_fg)
                .child("Tab ⇄"),
        );

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .h(px(48.0))
            .px(px(14.0))
            .text_color(theme.chrome_fg)
            .child(cat_btn)
            .child(text_row)
            .child(right_section);

        let mut list = div()
            .id(ElementId::named_usize("palette-list", 0))
            .flex()
            .flex_col()
            .px(px(10.0))
            .py(px(8.0))
            .gap(px(4.0))
            .relative()
            .max_h(px(380.0))
            .overflow_y_scroll()
            .track_scroll(&self.palette_scroll_handle);

        if matches.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .py(px(28.0))
                    .gap(px(6.0))
                    .child(
                        icons::icon(icons::SEARCH)
                            .size(px(22.0))
                            .text_color(theme.faint_fg),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme.faint_fg)
                            .child("No matching items"),
                    ),
            );
        } else {
            for (index, item) in matches.into_iter().enumerate() {
                let action = item.action.clone();
                let is_selected = index == self.palette_index;

                let is_shortcut = !item.shortcut_or_badge.is_empty()
                    && (item.shortcut_or_badge.contains("Ctrl")
                        || item.shortcut_or_badge.contains("Alt")
                        || item.shortcut_or_badge.contains("Shift")
                        || item.shortcut_or_badge == "F11"
                        || item.shortcut_or_badge == "R");

                let right_badge = if item.shortcut_or_badge.is_empty() {
                    div()
                } else {
                    div()
                        .flex_none()
                        .px(px(7.0))
                        .py(px(2.5))
                        .rounded(px(5.0))
                        .bg(theme.soft_bg)
                        .text_size(px(11.0))
                        .font_weight(if is_shortcut { gpui::FontWeight::NORMAL } else { gpui::FontWeight::MEDIUM })
                        .text_color(if is_selected { theme.chrome_fg_active } else { theme.faint_fg })
                        .child(item.shortcut_or_badge.clone())
                };

                let hover_bg = theme.card_hover;
                let active_bg = theme.card_active;
                let row = div()
                    .id(ElementId::named_usize("palette-item", index))
                    .h(px(44.0))
                    .px(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .when(is_selected, move |this| this.bg(active_bg))
                    .when(!is_selected, move |this| this.hover(move |s| s.bg(hover_bg)))
                    .child(
                        div()
                            .w(px(26.0))
                            .h(px(26.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                icons::icon(item.icon)
                                    .size(px(16.0))
                                    .text_color(if is_selected { theme.accent } else { theme.chrome_fg }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.chrome_fg_active)
                                    .line_height(px(16.0))
                                    .child(item.title),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(if is_selected { theme.chrome_fg } else { theme.faint_fg })
                                    .line_height(px(14.0))
                                    .child(item.subtitle),
                            ),
                    )
                    .child(right_badge)
                    .on_mouse_down(MouseButton::Left, cx.listener({
                        let action = action.clone();
                        move |view, _, window, cx| {
                            cx.stop_propagation();
                            view.palette_index = index;
                            view.run_palette_action(action.clone(), window, cx);
                        }
                    }));

                list = list.child(row);
            }
        }

        // Floating category dropdown menu
        let (category_dropdown, category_backdrop) = if cat_menu_open {
            let backdrop = div()
                .id(ElementId::named_usize("palette-cat-backdrop", 0))
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, cx.listener(|view, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    view.palette_category_menu_open = false;
                    window.focus(&view.palette_focus);
                    cx.notify();
                }));

            let mut menu = div()
                .id(ElementId::named_usize("palette-cat-menu", 0))
                .absolute()
                .top(px(44.0))
                .left(px(12.0))
                .w(px(170.0))
                .flex()
                .flex_col()
                .gap(px(3.0))
                .p(px(6.0))
                .rounded(px(10.0))
                .bg(theme.elevated_bg)
                .border_1()
                .border_color(theme.elevated_border)
                .shadow_xl()
                .on_scroll_wheel(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                });

            for cat in PaletteCategory::ALL {
                let is_sel = cat == active_cat;
                let hover = theme.card_hover;
                let mut item = div()
                    .id(ElementId::named_usize("palette-cat-choice", cat as usize))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(32.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .cursor_pointer();

                if is_sel {
                    item = item.bg(theme.card_active);
                } else {
                    item = item.hover(move |s| s.bg(hover));
                }

                let item = item
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                icons::icon(cat.icon())
                                    .size(px(13.0))
                                    .text_color(if is_sel { theme.accent } else { theme.chrome_fg }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(if is_sel { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
                                    .text_color(if is_sel { theme.chrome_fg_active } else { theme.chrome_fg })
                                    .child(cat.label()),
                            ),
                    )
                    .when(is_sel, |this| {
                        this.child(
                            icons::icon(icons::CHECK)
                                .size(px(12.0))
                                .text_color(theme.accent),
                        )
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        view.palette_category = cat;
                        view.palette_category_menu_open = false;
                        view.palette_index = 0;
                        view.palette_anim_index = 0.0;
                        view.palette_scroll_handle.set_offset(gpui::Point::default());
                        window.focus(&view.palette_focus);
                        cx.notify();
                    }));
                menu = menu.child(item);
            }
            (Some(menu), Some(backdrop))
        } else {
            (None, None)
        };

        div()
            .absolute()
            .top(px(64.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("palette-panel", 0))
                    .w(px(600.0))
                    .flex()
                    .flex_col()
                    .relative()
                    .rounded(px(14.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_xl()
                    .track_focus(&self.palette_focus)
                    .on_scroll_wheel(|_event, _window, cx| {
                        cx.stop_propagation();
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(|view, _, window, cx| {
                        cx.stop_propagation();
                        window.focus(&view.palette_focus);
                    }))
                    .child(input_row)
                    .child(div().h(px(1.0)).bg(theme.hairline))
                    .child(list)
                    .when_some(category_backdrop, |this, b| this.child(b))
                    .when_some(category_dropdown, |this, menu| this.child(menu)),
            )
            .with_animation(
                ElementId::named_usize("palette-anim", closing as usize),
                Animation::new(Duration::from_millis(if closing {
                    OVERLAY_OUT_MS
                } else {
                    OVERLAY_IN_MS
                }))
                .with_easing(ease_apple_snappy),
                move |el, delta| {
                    let t = if closing { 1.0 - delta } else { delta };
                    el.opacity(t).top(px(58.0 - 8.0 * (1.0 - t)))
                },
            )
            .into_any_element()
    }
}
