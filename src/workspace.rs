//! Workspace structures: tabs, split panes, path condensation, and process branding.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::icons;
use crate::session::{Profile, Session};
use crate::wsl;

/// How a tab's panes tile the sheet.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    /// Side by side, divided by a vertical hairline.
    Columns,
    /// Stacked, divided by a horizontal hairline.
    Rows,
}

/// One shell inside a tab (a tab is one or more panes side by side).
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
    /// Smooth cursor glide: (interpolated_col, interpolated_row) in cell units.
    pub cursor_pos: Option<(f32, f32)>,
    /// Target cursor position reported by the terminal grid.
    pub cursor_target: (f32, f32),
    /// Smooth split expansion progress (0.0 .. 1.0).
    pub pane_expansion: f32,
    /// Smooth pane collapse on close.
    pub closing: bool,
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
    /// While true only the active pane is shown (header maximize action).
    pub maximized: bool,
    /// Smooth width collapse on close.
    pub closing: bool,
    pub close_progress: f32,
    /// Smooth hover expansion for the tab close button (0.0 .. 1.0).
    pub close_hover_progress: f32,
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
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
pub fn dir_label(pane: &Pane) -> Option<String> {
    let raw = if matches!(pane.profile, Profile::Wsl) {
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
    let p_name = profile_title(pane.profile);
    let cmd = tab_name(&pane.title, "");
    let path = if matches!(pane.profile, Profile::Wsl) {
        title_path(&pane.title).or_else(|| pane.cwd.as_ref().map(|d| d.display().to_string()))
    } else {
        pane.cwd
            .as_ref()
            .map(|d| d.display().to_string())
            .or_else(|| title_path(&pane.title))
    };

    if !cmd.is_empty() && cmd != p_name {
        return cmd;
    }

    if let Some(p) = path {
        format_path_condensed(&p)
    } else {
        "~".to_string()
    }
}

/// Process mark for a tab, keyed off the shell-reported title (OSC 0/2) and profile:
/// Matches PowerShell, Ubuntu/WSL, CMD, Git Bash, OpenCode, Claude, Cursor, Antigravity.
/// Never falls back to a folder icon.
pub fn session_icon(title: &str, profile: Profile) -> &'static str {
    let lower = title.to_ascii_lowercase();
    if lower.contains("claude") {
        icons::CLAUDE_MARK
    } else if lower.contains("cursor") {
        icons::CURSOR_MARK
    } else if lower.contains("antigravity") || lower.contains("agy") {
        icons::ANTIGRAVITY_MARK
    } else if matches!(profile, Profile::OpenCode)
        || lower.contains("opencode")
        || lower.contains("open-code")
    {
        icons::OPENCODE_MARK
    } else if matches!(profile, Profile::Wsl) {
        wsl::default_distro()
            .map(|distro| distro.icon)
            .unwrap_or(icons::LINUX)
    } else if matches!(profile, Profile::Cmd) {
        icons::CMD
    } else if matches!(profile, Profile::GitBash) {
        icons::GIT
    } else if matches!(profile, Profile::PowerShell) {
        icons::POWERSHELL
    } else if lower.contains("ubuntu") || lower.contains("wsl") {
        icons::UBUNTU
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
pub fn profile_title(profile: Profile) -> String {
    match profile {
        Profile::PowerShell => "PowerShell".to_string(),
        Profile::Cmd => "Command Prompt".to_string(),
        Profile::Wsl => wsl::default_distro()
            .map(|distro| distro.title.clone())
            .unwrap_or_else(|| "WSL".to_string()),
        Profile::GitBash => "Git Bash".to_string(),
        Profile::OpenCode => "OpenCode".to_string(),
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
        "powershell" | "pwsh" | "windows powershell" | "powershell 7" | "ubuntu" | "wsl"
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
