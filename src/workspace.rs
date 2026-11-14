//! Workspace structures: tabs, split panes, path condensation, and process branding.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::icons;
use crate::session::{Profile, Session};
use crate::wsl;

#[allow(unused_imports)]
pub use crate::tiling::{Direction, SplitAxis, TileNode, TileTree};

/// One shell inside a tab (a tab is one or more panes side by side).
/// 4-corner warp smear cursor animation state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorMotionState {
    /// Previous settled cell position (col, row) before recent movement.
    pub prev_pos: (f32, f32),
    /// Current target cell position (col, row) reported by terminal grid.
    pub target_pos: (f32, f32),
    /// Elapsed seconds since movement started.
    pub elapsed_secs: f32,
    /// Total duration for the trailing smear to contract into target cell (~0.16s).
    pub duration_secs: f32,
    /// Whether the motion has settled completely at target_pos.
    pub is_settled: bool,
}

impl Default for CursorMotionState {
    fn default() -> Self {
        Self {
            prev_pos: (0.0, 0.0),
            target_pos: (0.0, 0.0),
            elapsed_secs: 1.0,
            duration_secs: 0.16,
            is_settled: true,
        }
    }
}

impl CursorMotionState {
    pub fn current_pos(&self) -> (f32, f32) {
        if self.is_settled {
            self.target_pos
        } else {
            let t = (self.elapsed_secs / self.duration_secs).clamp(0.0, 1.0);
            let prog = (1.0 - (t - 1.0) * (t - 1.0)).sqrt(); // EaseOutCirc
            (
                self.prev_pos.0 + (self.target_pos.0 - self.prev_pos.0) * prog,
                self.prev_pos.1 + (self.target_pos.1 - self.prev_pos.1) * prog,
            )
        }
    }

    pub fn snap_to(&mut self, pos: (f32, f32)) {
        self.prev_pos = pos;
        self.target_pos = pos;
        self.elapsed_secs = self.duration_secs;
        self.is_settled = true;
    }
}

pub struct Pane {
    /// Bumped on every (re)spawn; stale session messages are dropped.
    pub epoch: u64,
    /// Which profile this pane runs (Restart reuses it).
    pub profile: Profile,
    /// Latest shell-reported title (OSC 0/2), e.g. `vite` or `user@host: /mnt/c/Users/me`.
    pub title: String,
    pub session: Rc<RefCell<Session>>,
    /// Working directory of the shell, polled from the process (`crate::cwd`).
    pub cwd: Option<PathBuf>,
    /// Git branch of the working directory, polled alongside it.
    pub branch: Option<String>,
    pub exited: bool,
    /// 4-corner warp smear motion state for the cursor.
    pub cursor_motion: CursorMotionState,
    /// Smooth split expansion progress (0.0 .. 1.0).
    pub pane_expansion: f32,
    /// Smooth focus transition progress (0.0 .. 1.0).
    pub focus_progress: f32,
    /// Smooth pane collapse on close.
    pub closing: bool,
    /// Timestamp when a foreground command started running (for background job notifications).
    pub cmd_start: Option<std::time::Instant>,
    /// Name/title of the currently running foreground command.
    pub running_cmd: Option<String>,
    /// Brand icon of the currently running foreground command.
    pub running_icon: Option<&'static str>,
}

pub struct Tab {
    pub id: u64,
    /// User-set name (Rename dialog / palette); wins over the shell title.
    pub custom_title: Option<String>,
    /// Split panes; always at least one.
    pub panes: Vec<Pane>,
    /// Pane that owns the keyboard and the caret.
    pub active_pane: usize,
    /// Direction the panes are tiled in (the last split decides).
    pub axis: SplitAxis,
    /// Hyprland-style binary split tree for arbitrary vertical and horizontal compositions.
    pub tile_tree: TileTree,
    /// While true only the active pane is shown (header maximize action).
    pub maximized: bool,
    /// Smooth width collapse on close.
    pub closing: bool,
    pub close_progress: f32,
    /// Smooth hover expansion for the tab close button (0.0 .. 1.0).
    pub close_hover_progress: f32,
    /// Per-tab isolated font size (zoom level). None uses global setting.
    pub font_size: Option<f32>,
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn is_internal_app_dir(p: &Path) -> bool {
    let p_str = p.to_string_lossy().to_lowercase();
    if p_str.contains("appdata\\local\\programs\\atlas")
        || p_str.contains("program files\\atlas")
        || p_str.ends_with("system32")
    {
        return true;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if p == parent {
                return true;
            }
        }
    }
    false
}

pub fn sanitize_working_dir(candidate: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = candidate {
        if dir.is_dir() && !is_internal_app_dir(&dir) {
            return Some(dir);
        }
    }
    None
}

pub fn resolve_start_directory(
    start_dir_setting: &str,
    initial_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    // 1. Explicit directory passed to CLI
    if let Some(dir) = sanitize_working_dir(initial_dir) {
        return Some(dir);
    }

    if start_dir_setting.eq_ignore_ascii_case("current") {
        // 2. Process current working directory (e.g. if invoked from shell)
        if let Some(dir) = sanitize_working_dir(std::env::current_dir().ok()) {
            return Some(dir);
        }

        // 3. Last saved session active working directory
        let session_path = crate::user_data_dir().join("sessions").join("last_session.json");
        if let Ok(data) = std::fs::read_to_string(&session_path) {
            if let Ok(saved) = serde_json::from_str::<crate::workspace::SavedSession>(&data) {
                if let Some(tab) = saved.tabs.get(saved.active_tab).or_else(|| saved.tabs.first()) {
                    if let Some(pane) = tab.panes.get(tab.active_pane).or_else(|| tab.panes.first()) {
                        if let Some(dir) = sanitize_working_dir(pane.cwd.clone()) {
                            return Some(dir);
                        }
                    }
                }
            }
        }

        // 4. Most recent directory from jump frecency store (dirs.json)
        let store = crate::tools::system::jump::load_store();
        for entry in &store.entries {
            let p = PathBuf::from(&entry.path);
            if let Some(dir) = sanitize_working_dir(Some(p)) {
                return Some(dir);
            }
        }
    }

    // Default fallback: User's home directory
    home_dir()
}

pub fn title_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Handle Linux/WSL/SSH style "user@host: /path" or "user@host:~"
    let candidate = if let Some((prefix, path)) = trimmed.split_once(':') {
        if prefix.contains('@') {
            path.trim().trim_end_matches(['$', '#', '>', '%', ' '])
        } else if trimmed.contains(": ") {
            if let Some((_, p)) = trimmed.split_once(": ") {
                p.trim().trim_end_matches(['$', '#', '>', '%', ' '])
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    let looks_like_path = candidate.starts_with('/')
        || candidate.starts_with('~')
        || (candidate.len() >= 3
            && candidate.as_bytes()[1] == b':'
            && (candidate.as_bytes()[2] == b'\\' || candidate.as_bytes()[2] == b'/'))
        || candidate.contains(":\\")
        || candidate.contains(":/")
        || candidate.starts_with("\\\\");
    let is_executable = candidate.to_ascii_lowercase().ends_with(".exe");
    if looks_like_path && !candidate.contains('@') && !is_executable {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Directory label for a quiet tab pill: the basename of the shell's working
/// directory — `~` while it sits in the home folder — so the folder mark and
/// the words agree (never the raw shell name like "PowerShell").
#[allow(dead_code)]
pub fn dir_label(pane: &Pane) -> Option<String> {
    let raw = if matches!(pane.profile, Profile::Wsl | Profile::WslDistro(_)) {
        title_path(&pane.title).or_else(|| pane.cwd.as_ref().map(|dir| dir.display().to_string()))
    } else {
        pane.cwd.as_ref().map(|dir| dir.display().to_string()).or_else(|| title_path(&pane.title))
    }?;
    let trimmed = raw.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() || trimmed == "~" {
        return Some("~".to_string());
    }
    if let Some(home) = home_dir() {
        let home = home.display().to_string();
        if trimmed.eq_ignore_ascii_case(home.trim_end_matches(['\\', '/'])) {
            return Some("~".to_string());
        }
    }
    trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|base| !base.is_empty())
        .map(str::to_string)
}

/// Working directory to run git against: the title path mapped back to
/// Windows (`/mnt/c/…` → `C:\…`), `None` for Linux-only paths, the PEB poll
/// otherwise.
pub fn git_cwd(pane: &Pane) -> Option<PathBuf> {
    if let Some(path) = title_path(&pane.title) {
        if let Some(rest) = path.strip_prefix("/mnt/") {
            let bytes = rest.as_bytes();
            if bytes.len() > 2 && bytes[1] == b'/' && bytes[0].is_ascii_alphabetic() {
                return Some(PathBuf::from(format!(
                    "{}:\\{}",
                    (bytes[0] as char).to_ascii_uppercase(),
                    &rest[2..]
                )));
            }
            return None;
        }
        if path.starts_with('/') || path.starts_with('~') {
            return None;
        }
        return Some(PathBuf::from(path));
    }
    pane.cwd.clone()
}

pub fn format_path_condensed(raw_path: &str) -> String {
    let normalized = if let Some(rest) = raw_path.strip_prefix("/mnt/") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b'/' && bytes[0].is_ascii_alphabetic() {
            format!("{}:/{}", (bytes[0] as char).to_ascii_uppercase(), &rest[2..])
        } else {
            raw_path.replace('\\', "/")
        }
    } else {
        raw_path.replace('\\', "/")
    };
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        return "~".to_string();
    }
    let with_home = if let Some(home) = home_dir() {
        let home_norm = home.display().to_string().replace('\\', "/");
        let home_trimmed = home_norm.trim_end_matches('/');
        if trimmed.eq_ignore_ascii_case(home_trimmed) {
            "~".to_string()
        } else if trimmed.len() > home_trimmed.len()
            && trimmed[..home_trimmed.len()].eq_ignore_ascii_case(home_trimmed)
            && trimmed.as_bytes().get(home_trimmed.len()) == Some(&b'/')
        {
            format!("~{}", &trimmed[home_trimmed.len()..])
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    if with_home == "~" {
        return "~".to_string();
    }

    if with_home.chars().count() > 18 {
        let parts: Vec<&str> = with_home.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            format!("../{}", parts.last().unwrap())
        } else if let Some(last) = parts.last() {
            format!("../{}", last)
        } else {
            with_home
        }
    } else if let Some(stripped) = with_home.strip_prefix("~/") {
        stripped.to_string()
    } else {
        with_home
    }
}

pub fn format_tab_title(pane: &Pane) -> String {
    let p_name = profile_title(&pane.profile);
    let cmd = if let Some(ref running) = pane.running_cmd {
        running.clone()
    } else {
        tab_name(&pane.title, "")
    };
    let path = if matches!(pane.profile, Profile::Wsl | Profile::WslDistro(_)) {
        title_path(&pane.title).or_else(|| pane.cwd.as_ref().map(|d| d.display().to_string()))
    } else {
        pane.cwd
            .as_ref()
            .map(|d| d.display().to_string())
            .or_else(|| title_path(&pane.title))
    };

    let dir = if let Some(p) = path {
        format_path_condensed(&p)
    } else {
        "~".to_string()
    };

    let label_name = if !cmd.is_empty() && cmd != p_name {
        cmd
    } else {
        p_name.to_string()
    };

    if dir == label_name || dir.is_empty() {
        label_name
    } else {
        format!("{label_name} · {dir}")
    }
}

/// Dynamic process brand mark for a pane, taking active foreground process into account.
pub fn session_icon_for_pane(pane: &Pane) -> &'static str {
    if let Some(icon) = pane.running_icon {
        return icon;
    }
    if let Some(ref cmd) = pane.running_cmd {
        let lower = cmd.to_ascii_lowercase();
        if lower.contains("antigravity") || lower.contains("agy") {
            return icons::ANTIGRAVITY_MARK;
        } else if lower.contains("claude") {
            return icons::CLAUDE_MARK;
        } else if lower.contains("opencode") || lower.contains("open-code") {
            return icons::OPENCODE_MARK;
        } else if lower.contains("cursor") {
            return icons::CURSOR_MARK;
        } else if lower.contains("lazygit") || lower.contains("git") {
            return icons::GIT;
        }
    }
    session_icon(&pane.title, &pane.profile)
}

/// Process mark for a tab, keyed off the shell-reported title (OSC 0/2) and profile:
/// Matches PowerShell, Ubuntu/WSL, CMD, Git Bash, OpenCode, Claude, Cursor, Antigravity.
/// Never falls back to a folder icon.
pub fn session_icon(title: &str, profile: &Profile) -> &'static str {
    let lower = title.to_ascii_lowercase();
    if lower.contains("claude") || matches!(profile, Profile::Claude) {
        icons::CLAUDE_MARK
    } else if lower.contains("cursor") {
        icons::CURSOR_MARK
    } else if lower.contains("antigravity") || lower.contains("agy") || matches!(profile, Profile::Antigravity) {
        icons::ANTIGRAVITY_MARK
    } else if matches!(profile, Profile::OpenCode)
        || lower.contains("opencode")
        || lower.contains("open-code")
    {
        icons::OPENCODE_MARK
    } else if let Profile::WslDistro(name) = profile {
        crate::wsl::icon_for_name(name)
    } else if matches!(profile, Profile::Wsl) {
        wsl::default_distro()
            .map(|distro| distro.icon)
            .unwrap_or(icons::LINUX)
    } else if matches!(profile, Profile::Cmd) {
        icons::CMD
    } else if matches!(profile, Profile::GitBash) {
        icons::GIT
    } else if matches!(profile, Profile::PowerShell | Profile::Pwsh) {
        icons::POWERSHELL
    } else if lower.contains("ubuntu") {
        icons::UBUNTU
    } else if lower.contains("arch") {
        icons::ARCH
    } else if lower.contains("debian") {
        icons::DEBIAN
    } else if lower.contains("kali") {
        icons::KALI
    } else if lower.contains("fedora") {
        icons::FEDORA
    } else if lower.contains("wsl") {
        wsl::default_distro()
            .map(|distro| distro.icon)
            .unwrap_or(icons::LINUX)
    } else if lower.contains("cmd.exe") || lower == "cmd" {
        icons::CMD
    } else if lower.contains("bash") || lower.contains("git") {
        icons::GIT
    } else if lower.contains("powershell") || lower.contains("pwsh") {
        icons::POWERSHELL
    } else {
        icons::TERMINAL
    }
}

/// Default title for a freshly spawned profile.
pub fn profile_title(profile: &Profile) -> String {
    match profile {
        Profile::PowerShell => "PowerShell".to_string(),
        Profile::Pwsh => "PowerShell 7".to_string(),
        Profile::Cmd => "Command Prompt".to_string(),
        Profile::Wsl => wsl::default_distro()
            .map(|distro| distro.title.clone())
            .unwrap_or_else(|| "WSL".to_string()),
        Profile::WslDistro(name) => crate::wsl::title_for_name(name),
        Profile::GitBash => "Git Bash".to_string(),
        Profile::OpenCode => "OpenCode".to_string(),
        Profile::Claude => "Claude Code".to_string(),
        Profile::Antigravity => "Antigravity".to_string(),
        Profile::CustomTool { name, .. } => name.clone(),
    }
}

/// Branch name for a working directory, read from `<cwd>/.git/HEAD`
/// (`ref: refs/heads/main` -> `main`). `None` outside a repository and on a
/// detached HEAD.
pub fn git_branch(cwd: &Path) -> Option<String> {
    let head = cwd.join(".git").join("HEAD");
    let text = std::fs::read_to_string(head).ok()?;
    Some(
        text.trim()
            .strip_prefix("ref: refs/heads/")?
            .trim()
            .to_string(),
    )
}

/// Sidebar/pane label for a tab: the shell-reported title when it reads as a
/// real name (e.g. "vite"), the profile label when it is shell-ish, a prompt
/// string (`user@host: /path`) or a bare path (the path itself is shown in
/// the subtitle and status bar instead).
pub fn tab_name(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let shellish = matches!(
        lower.as_str(),
        "powershell"
            | "pwsh"
            | "windows powershell"
            | "powershell 7"
            | "ubuntu"
            | "wsl"
            | "arch"
            | "archlinux"
            | "debian"
            | "fedora"
            | "kali"
            | "alpine"
            | "bash"
            | "zsh"
            | "fish"
    ) || lower.contains("windows powershell");
    let promptish = trimmed.contains('@') && trimmed.contains(':');
    if trimmed.is_empty() || shellish || promptish || trimmed.contains(['\\', '/']) {
        return fallback.to_string();
    }
    let mut name: String = trimmed.chars().take(24).collect();
    if trimmed.chars().count() > 24 {
        name.push('…');
    }
    name
}

/// Format dropped file/folder paths for shell pasting.
///
/// Quoting rules:
/// - Paths containing whitespace are enclosed in double quotes.
/// - When `is_wsl` is true, Windows paths are converted to WSL mount points (`/mnt/c/...`).
/// - Multiple items are space-separated, with a trailing space for CLI typing ergonomics.
pub fn format_dropped_paths(paths: &[PathBuf], is_wsl: bool) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let mut parts = Vec::with_capacity(paths.len());
    for path in paths {
        let formatted = if is_wsl {
            crate::wsl::windows_to_wsl_path(path)
        } else {
            path.to_string_lossy().to_string()
        };
        if formatted.contains(' ') || formatted.contains('\t') {
            parts.push(format!("\"{formatted}\""));
        } else {
            parts.push(formatted);
        }
    }
    format!("{} ", parts.join(" "))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SavedSession {
    #[serde(default)]
    pub active_tab: usize,
    pub tabs: Vec<SavedTab>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SavedTab {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default)]
    pub active_pane: usize,
    #[serde(default = "default_axis")]
    pub axis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_tree: Option<TileTree>,
    pub panes: Vec<SavedPane>,
}

fn default_axis() -> String {
    "columns".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SavedPane {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_dropped_paths() {
        let p1 = PathBuf::from(r"C:\Users\Neweye\Desktop\file.txt");
        let p2 = PathBuf::from(r"C:\Users\Neweye\My Documents\notes.txt");
        
        let win = format_dropped_paths(&[p1.clone(), p2.clone()], false);
        assert_eq!(
            win,
            r#"C:\Users\Neweye\Desktop\file.txt "C:\Users\Neweye\My Documents\notes.txt" "#
        );

        let wsl = format_dropped_paths(&[p1, p2], true);
        assert_eq!(
            wsl,
            r#"/mnt/c/Users/Neweye/Desktop/file.txt "/mnt/c/Users/Neweye/My Documents/notes.txt" "#
        );
    }

    #[test]
    fn test_saved_session_serde() {
        let session = SavedSession {
            active_tab: 1,
            tabs: vec![
                SavedTab {
                    custom_title: Some("Project A".into()),
                    active_pane: 0,
                    axis: "columns".into(),
                    tile_tree: None,
                    panes: vec![SavedPane {
                        profile: "powershell".into(),
                        cwd: Some(PathBuf::from(r"C:\Users\test")),
                    }],
                },
                SavedTab {
                    custom_title: None,
                    active_pane: 1,
                    axis: "rows".into(),
                    tile_tree: None,
                    panes: vec![
                        SavedPane {
                            profile: "wsl".into(),
                            cwd: Some(PathBuf::from(r"/home/test")),
                        },
                        SavedPane {
                            profile: "cmd".into(),
                            cwd: None,
                        },
                    ],
                },
            ],
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: SavedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session, deserialized);
    }

    #[test]
    fn test_line_wrap_discontinuity_logic() {
        let cur_col = 79.0;
        let cur_row = 5.0;
        let tgt_col = 0.0;
        let tgt_row = 6.0;

        let d_col = tgt_col - cur_col;
        let d_row = tgt_row - cur_row;

        let is_line_wrap = d_row == 1.0 && d_col < -20.0;
        assert!(is_line_wrap, "Must detect line-wrap from col 79 to col 0");

        let d_col_normal = 5.0;
        let d_row_normal = 0.0;
        let is_normal_wrap = d_row_normal == 1.0 && d_col_normal < -20.0;
        assert!(!is_normal_wrap);
    }

    #[test]
    fn test_cursor_warp_geometry() {
        let mut motion = CursorMotionState {
            prev_pos: (0.0, 1.0),
            target_pos: (0.0, 6.0),
            elapsed_secs: 0.0,
            duration_secs: 0.16,
            is_settled: false,
        };

        // At t = 0, current pos is at prev_pos
        let p0 = motion.current_pos();
        assert_eq!(p0, (0.0, 1.0));

        // Advance half-way
        motion.elapsed_secs = 0.08;
        let p_mid = motion.current_pos();
        assert!(p_mid.1 > 1.0 && p_mid.1 < 6.0);

        // Advance to full duration
        motion.elapsed_secs = 0.16;
        motion.is_settled = true;
        let p_end = motion.current_pos();
        assert_eq!(p_end, (0.0, 6.0));
    }
}
