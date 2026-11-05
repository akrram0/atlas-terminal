//! Smart, local-first User Profile & Environment Discovery for Atlas Terminal.
//!
//! Stores `profile.json` in `%APPDATA%\AtlasTerminal\profile.json`.
//! Probes installed shells, WSL distros (via Win32 registry), and developer/AI CLI tools
//! asynchronously with zero UI latency and stale-while-revalidate caching.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::icons;
use crate::session::Profile;
use crate::wsl;

/// Version marker for `profile.json` schema.
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// User profile and detected environment state persisted in `%APPDATA%\AtlasTerminal\profile.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub last_scanned: u64,
    #[serde(default = "default_shell_id")]
    pub default_shell: String,
    #[serde(default)]
    pub window_geometry: WindowGeometry,
    #[serde(default)]
    pub recent_directories: Vec<String>,
    #[serde(default)]
    pub shells: Vec<DiscoveredShell>,
    #[serde(default)]
    pub wsl_distros: Vec<DiscoveredWsl>,
    #[serde(default)]
    pub tools: Vec<DiscoveredTool>,
}

fn default_schema_version() -> u32 {
    PROFILE_SCHEMA_VERSION
}

fn default_shell_id() -> String {
    "powershell".to_string()
}

/// Last persisted window geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
}

fn default_width() -> f32 {
    1100.0
}

fn default_height() -> f32 {
    720.0
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: default_width(),
            height: default_height(),
            maximized: false,
        }
    }
}

/// A shell installed on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredShell {
    pub id: String,
    pub title: String,
    pub path: String,
    pub icon: String,
    pub is_default: bool,
}

/// A WSL distribution registered in the per-user Windows Registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredWsl {
    pub name: String,
    pub title: String,
    pub icon: String,
    pub version: u32,
    pub is_default: bool,
}

/// An AI agent or developer CLI tool detected on PATH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredTool {
    pub name: String,
    pub title: String,
    pub path: String,
    pub icon: String,
}

/// Raw result of an environment probe.
#[derive(Debug, Clone)]
pub struct DiscoveredEnv {
    pub shells: Vec<DiscoveredShell>,
    pub wsl_distros: Vec<DiscoveredWsl>,
    pub tools: Vec<DiscoveredTool>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            last_scanned: 0,
            default_shell: "powershell".to_string(),
            window_geometry: WindowGeometry::default(),
            recent_directories: Vec::new(),
            shells: Vec::new(),
            wsl_distros: Vec::new(),
            tools: Vec::new(),
        }
    }
}

/// Returns `%APPDATA%\AtlasTerminal`.
pub fn profile_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata).join("AtlasTerminal")
    } else {
        crate::user_data_dir()
    }
}

/// Full path to `%APPDATA%\AtlasTerminal\profile.json`.
pub fn profile_path() -> PathBuf {
    profile_dir().join("profile.json")
}

impl UserProfile {
    /// Synchronously load `profile.json` (<1ms).
    ///
    /// If none exists, scans synchronously and saves a fresh profile.
    pub fn load_or_init() -> Self {
        let path = profile_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(mut profile) = serde_json::from_str::<UserProfile>(&content) {
                // Ensure default shells are populated if the file was empty
                if profile.shells.is_empty() {
                    let env = scan_environment_sync();
                    profile.reconcile(env);
                    profile.save();
                }
                return profile;
            }
        }

        // Check fallback location (~/.config/atlas/profile.json)
        let legacy_path = crate::user_data_dir().join("profile.json");
        if let Ok(content) = std::fs::read_to_string(&legacy_path) {
            if let Ok(profile) = serde_json::from_str::<UserProfile>(&content) {
                let _ = profile.save();
                return profile;
            }
        }

        // First run: probe environment synchronously (<15ms)
        let mut profile = UserProfile::default();
        let env = scan_environment_sync();
        profile.reconcile(env);
        profile.save();
        profile
    }

    /// Save the current profile state to `%APPDATA%\AtlasTerminal\profile.json`.
    pub fn save(&self) {
        let dir = profile_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = profile_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Reconcile cached profile with a fresh environment scan.
    /// Returns `true` if any shell, distro, or tool changed.
    pub fn reconcile(&mut self, env: DiscoveredEnv) -> bool {
        let mut changed = false;

        if self.shells != env.shells {
            self.shells = env.shells;
            changed = true;
        }

        if self.wsl_distros != env.wsl_distros {
            self.wsl_distros = env.wsl_distros;
            changed = true;
        }

        if self.tools != env.tools {
            self.tools = env.tools;
            changed = true;
        }

        // Verify default shell is still available; if not, fall back cleanly
        let default_exists = self.is_shell_or_distro_available(&self.default_shell);
        if !default_exists {
            let fallback = self.shells.first().map(|s| s.id.clone()).unwrap_or_else(|| "powershell".to_string());
            if self.default_shell != fallback {
                self.default_shell = fallback;
                changed = true;
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_scanned = now;

        changed
    }

    /// Check if a given shell id or distro name is installed.
    pub fn is_shell_or_distro_available(&self, id: &str) -> bool {
        let lower = id.to_ascii_lowercase();
        if self.shells.iter().any(|s| s.id.eq_ignore_ascii_case(&lower)) {
            return true;
        }
        if let Some(distro_name) = lower.strip_prefix("wsl:") {
            return self.wsl_distros.iter().any(|d| d.name.eq_ignore_ascii_case(distro_name));
        }
        if self.wsl_distros.iter().any(|d| d.name.eq_ignore_ascii_case(&lower)) {
            return true;
        }
        if lower == "wsl" && !self.wsl_distros.is_empty() {
            return true;
        }
        false
    }

    /// Record a recent working directory (up to 10 entries, most recent first).
    pub fn add_recent_directory(&mut self, path: &str) {
        if path.is_empty() || path == "~" {
            return;
        }
        self.recent_directories.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent_directories.insert(0, path.to_string());
        if self.recent_directories.len() > 10 {
            self.recent_directories.truncate(10);
        }
        self.save();
    }

    /// Non-blocking verification (<1ms) called when opening menus.
    ///
    /// Prunes any uninstalled tool or WSL distro. Returns `true` if changes occurred.
    pub fn verify_installed(&mut self) -> bool {
        let mut changed = false;

        // Verify tools
        let before_tools = self.tools.len();
        self.tools.retain(|t| Path::new(&t.path).is_file());
        if self.tools.len() != before_tools {
            changed = true;
        }

        // Verify shells
        let before_shells = self.shells.len();
        self.shells.retain(|s| {
            if s.id == "powershell" || s.id == "cmd" {
                true
            } else {
                Path::new(&s.path).is_file()
            }
        });
        if self.shells.len() != before_shells {
            changed = true;
        }

        // Verify WSL distros directly against registry
        let live_distros = wsl::all_distros();
        let live_names: Vec<String> = live_distros.iter().map(|d| d.name.to_ascii_lowercase()).collect();
        let before_distros = self.wsl_distros.len();
        self.wsl_distros.retain(|d| live_names.contains(&d.name.to_ascii_lowercase()));
        if self.wsl_distros.len() != before_distros {
            changed = true;
        }

        // Check default shell validity
        if !self.is_shell_or_distro_available(&self.default_shell) {
            self.default_shell = self.shells.first().map(|s| s.id.clone()).unwrap_or_else(|| "powershell".to_string());
            changed = true;
        }

        if changed {
            self.save();
        }
        changed
    }

    /// Verify a profile target before spawning.
    ///
    /// If uninstalled or missing, automatically prunes the invalid target, falls back cleanly
    /// to PowerShell, saves the profile, and returns a human-friendly warning string.
    pub fn verify_or_fallback(&mut self, profile: &Profile) -> (Profile, Option<String>) {
        match profile {
            Profile::WslDistro(name) => {
                let live_distros = wsl::all_distros();
                let exists = live_distros.iter().any(|d| d.name.eq_ignore_ascii_case(name));
                if !exists {
                    self.wsl_distros.retain(|d| !d.name.eq_ignore_ascii_case(name));
                    self.save();
                    let msg = format!("WSL distro '{name}' was not found. Falling back to PowerShell.");
                    let fallback = self.fallback_profile();
                    return (fallback, Some(msg));
                }
                (profile.clone(), None)
            }
            Profile::GitBash => {
                let candidates = git_bash_candidates();
                if candidates.is_empty() {
                    self.shells.retain(|s| s.id != "gitbash");
                    self.save();
                    let msg = "Git Bash is not installed. Falling back to PowerShell.".to_string();
                    let fallback = self.fallback_profile();
                    return (fallback, Some(msg));
                }
                (profile.clone(), None)
            }
            Profile::OpenCode => {
                let found = find_tool_in_system("opencode");
                if found.is_none() {
                    self.tools.retain(|t| t.name != "opencode");
                    self.save();
                    let msg = "OpenCode CLI is not installed. Falling back to PowerShell.".to_string();
                    let fallback = self.fallback_profile();
                    return (fallback, Some(msg));
                }
                (profile.clone(), None)
            }
            Profile::Claude => {
                let found = find_tool_in_system("claude");
                if found.is_none() {
                    self.tools.retain(|t| t.name != "claude");
                    self.save();
                    let msg = "Claude Code CLI is not installed. Falling back to PowerShell.".to_string();
                    let fallback = self.fallback_profile();
                    return (fallback, Some(msg));
                }
                (profile.clone(), None)
            }
            Profile::CustomTool { name, path } => {
                if !Path::new(path).is_file() {
                    self.tools.retain(|t| t.name != *name);
                    self.save();
                    let msg = format!("Tool '{name}' was not found. Falling back to PowerShell.");
                    let fallback = self.fallback_profile();
                    return (fallback, Some(msg));
                }
                (profile.clone(), None)
            }
            _ => (profile.clone(), None),
        }
    }

    fn fallback_profile(&self) -> Profile {
        if self.shells.iter().any(|s| s.id == "pwsh") {
            Profile::Pwsh
        } else {
            Profile::PowerShell
        }
    }
}

/// Run an asynchronous background scan and notify a completion closure.
pub fn scan_background<F>(callback: F)
where
    F: FnOnce(DiscoveredEnv) + Send + 'static,
{
    std::thread::Builder::new()
        .name("atlas-env-scanner".to_string())
        .spawn(move || {
            let env = scan_environment_sync();
            callback(env);
        })
        .expect("failed to spawn env scanner thread");
}

/// Perform synchronous scan of Registry and PATH (<15ms total).
pub fn scan_environment_sync() -> DiscoveredEnv {
    let shells = detect_shells();
    let wsl_distros = detect_wsl_distros();
    let tools = detect_tools();

    DiscoveredEnv {
        shells,
        wsl_distros,
        tools,
    }
}

/// Detect installed shells on Windows.
fn detect_shells() -> Vec<DiscoveredShell> {
    let mut shells = Vec::new();

    // 1. PowerShell 7 (pwsh)
    let pwsh_path = find_pwsh_path();
    let has_pwsh = pwsh_path.is_some();
    if let Some(path) = pwsh_path {
        shells.push(DiscoveredShell {
            id: "pwsh".to_string(),
            title: "PowerShell 7".to_string(),
            path,
            icon: icons::POWERSHELL.to_string(),
            is_default: true,
        });
    }

    // 2. Windows PowerShell
    let ps_path = find_windows_powershell_path();
    if let Some(path) = ps_path {
        shells.push(DiscoveredShell {
            id: "powershell".to_string(),
            title: "Windows PowerShell".to_string(),
            path,
            icon: icons::POWERSHELL.to_string(),
            is_default: !has_pwsh,
        });
    }

    // 3. Command Prompt
    let cmd_path = find_cmd_path();
    if let Some(path) = cmd_path {
        shells.push(DiscoveredShell {
            id: "cmd".to_string(),
            title: "Command Prompt".to_string(),
            path,
            icon: icons::CMD.to_string(),
            is_default: false,
        });
    }

    // 4. Git Bash
    let git_bash = git_bash_candidates().into_iter().next();
    if let Some(path) = git_bash {
        shells.push(DiscoveredShell {
            id: "gitbash".to_string(),
            title: "Git Bash".to_string(),
            path,
            icon: icons::GIT.to_string(),
            is_default: false,
        });
    }

    shells
}

/// Detect WSL distros from Windows Registry via Win32 advapi32.
fn detect_wsl_distros() -> Vec<DiscoveredWsl> {
    let live = wsl::all_distros();
    live.into_iter()
        .map(|d| DiscoveredWsl {
            name: d.name,
            title: d.title,
            icon: d.icon.to_string(),
            version: d.version,
            is_default: d.is_default,
        })
        .collect()
}

/// Detect developer tools and AI CLI agents by fast file existence checks.
fn detect_tools() -> Vec<DiscoveredTool> {
    let targets = [
        ("opencode", "OpenCode", icons::OPENCODE_MARK),
        ("claude", "Claude Code", icons::CLAUDE_MARK),
        ("fastfetch", "Fastfetch", icons::TERMINAL),
        ("starship", "Starship", icons::TERMINAL),
        ("oh-my-posh", "Oh My Posh", icons::TERMINAL),
        ("nvim", "Neovim", icons::TERMINAL),
        ("lazygit", "LazyGit", icons::GIT),
        ("btop", "btop", icons::TERMINAL),
        ("cargo", "Cargo", icons::TERMINAL),
        ("pnpm", "pnpm", icons::TERMINAL),
    ];

    let mut discovered = Vec::new();
    for (name, title, icon) in targets {
        if let Some(path) = find_tool_in_system(name) {
            discovered.push(DiscoveredTool {
                name: name.to_string(),
                title: title.to_string(),
                path,
                icon: icon.to_string(),
            });
        }
    }
    discovered
}

fn find_pwsh_path() -> Option<String> {
    let probes = [
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Program Files\PowerShell\7-preview\pwsh.exe",
    ];
    for p in probes {
        if Path::new(p).is_file() {
            return Some(p.to_string());
        }
    }
    find_executable_on_path("pwsh")
}

fn find_windows_powershell_path() -> Option<String> {
    let default_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if Path::new(default_path).is_file() {
        return Some(default_path.to_string());
    }
    find_executable_on_path("powershell")
}

fn find_cmd_path() -> Option<String> {
    let default_path = r"C:\Windows\System32\cmd.exe";
    if Path::new(default_path).is_file() {
        return Some(default_path.to_string());
    }
    find_executable_on_path("cmd")
}

/// Absolute candidates for Git for Windows' bash.
pub fn git_bash_candidates() -> Vec<String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(dir) = std::env::var_os(var) {
            roots.push(PathBuf::from(dir).join("Git"));
        }
    }
    if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(dir).join("Programs").join("Git"));
    }
    if let Some(dir) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(dir).join("scoop").join("apps").join("git").join("current"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join("git.exe").is_file() {
                if let Some(root) = dir.parent() {
                    roots.push(root.to_path_buf());
                }
            }
        }
    }
    roots.push(PathBuf::from(r"C:\Program Files\Git"));

    let mut candidates = Vec::new();
    for root in roots {
        for rel in [r"bin\bash.exe", r"usr\bin\bash.exe"] {
            let candidate = root.join(rel);
            if candidate.is_file() {
                let candidate = candidate.to_string_lossy().into_owned();
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
    }
    candidates
}

/// Find a CLI tool on the system using fast file existence checks.
pub fn find_tool_in_system(tool_name: &str) -> Option<String> {
    // 1. Well-known package manager directories
    let mut explicit_dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local_path = PathBuf::from(local);
        explicit_dirs.push(local_path.join("pnpm").join("bin"));
        explicit_dirs.push(local_path.join("pnpm"));
        explicit_dirs.push(local_path.join("Microsoft").join("WinGet").join("Links"));
        explicit_dirs.push(local_path.join("Programs").join(tool_name));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        explicit_dirs.push(PathBuf::from(appdata).join("npm"));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let prof_path = PathBuf::from(profile);
        explicit_dirs.push(prof_path.join(".cargo").join("bin"));
        explicit_dirs.push(prof_path.join(".bun").join("bin"));
        explicit_dirs.push(prof_path.join("scoop").join("shims"));
    }

    let extensions = [".exe", ".cmd", ".bat", ""];

    for dir in explicit_dirs {
        for ext in &extensions {
            let file = dir.join(format!("{tool_name}{ext}"));
            if file.is_file() {
                return Some(file.to_string_lossy().to_string());
            }
        }
    }

    // 2. Scan PATH directories
    find_executable_on_path(tool_name)
}

fn find_executable_on_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    let extensions = [".exe", ".cmd", ".bat", ""];

    for dir in std::env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_serialization_roundtrip() {
        let mut profile = UserProfile::default();
        profile.default_shell = "pwsh".to_string();
        profile.recent_directories = vec![r"C:\Users\Developer\atlas".to_string()];
        profile.shells.push(DiscoveredShell {
            id: "pwsh".to_string(),
            title: "PowerShell 7".to_string(),
            path: r"C:\Program Files\PowerShell\7\pwsh.exe".to_string(),
            icon: icons::POWERSHELL.to_string(),
            is_default: true,
        });
        profile.wsl_distros.push(DiscoveredWsl {
            name: "archlinux".to_string(),
            title: "Arch Linux".to_string(),
            icon: icons::ARCH.to_string(),
            version: 2,
            is_default: true,
        });
        profile.tools.push(DiscoveredTool {
            name: "opencode".to_string(),
            title: "OpenCode".to_string(),
            path: r"C:\Users\Developer\AppData\Local\pnpm\bin\opencode.cmd".to_string(),
            icon: icons::OPENCODE_MARK.to_string(),
        });

        let json = serde_json::to_string_pretty(&profile).expect("must serialize");
        let decoded: UserProfile = serde_json::from_str(&json).expect("must deserialize");

        assert_eq!(decoded.schema_version, PROFILE_SCHEMA_VERSION);
        assert_eq!(decoded.default_shell, "pwsh");
        assert_eq!(decoded.shells.len(), 1);
        assert_eq!(decoded.shells[0].title, "PowerShell 7");
        assert_eq!(decoded.wsl_distros.len(), 1);
        assert_eq!(decoded.wsl_distros[0].name, "archlinux");
        assert_eq!(decoded.tools.len(), 1);
        assert_eq!(decoded.tools[0].name, "opencode");
    }

    #[test]
    fn test_reconcile_environment_diff() {
        let mut profile = UserProfile::default();
        profile.shells.push(DiscoveredShell {
            id: "cmd".to_string(),
            title: "Command Prompt".to_string(),
            path: "cmd.exe".to_string(),
            icon: icons::CMD.to_string(),
            is_default: false,
        });

        let fresh_env = DiscoveredEnv {
            shells: vec![
                DiscoveredShell {
                    id: "pwsh".to_string(),
                    title: "PowerShell 7".to_string(),
                    path: "pwsh.exe".to_string(),
                    icon: icons::POWERSHELL.to_string(),
                    is_default: true,
                },
                DiscoveredShell {
                    id: "cmd".to_string(),
                    title: "Command Prompt".to_string(),
                    path: "cmd.exe".to_string(),
                    icon: icons::CMD.to_string(),
                    is_default: false,
                },
            ],
            wsl_distros: vec![DiscoveredWsl {
                name: "archlinux".to_string(),
                title: "Arch Linux".to_string(),
                icon: icons::ARCH.to_string(),
                version: 2,
                is_default: true,
            }],
            tools: vec![DiscoveredTool {
                name: "claude".to_string(),
                title: "Claude Code".to_string(),
                path: "claude.exe".to_string(),
                icon: icons::CLAUDE_MARK.to_string(),
            }],
        };

        let changed = profile.reconcile(fresh_env);
        assert!(changed);
        assert_eq!(profile.shells.len(), 2);
        assert_eq!(profile.wsl_distros.len(), 1);
        assert_eq!(profile.tools.len(), 1);
        assert!(profile.last_scanned > 0);
    }

    #[test]
    fn test_verify_or_fallback_graceful_handling() {
        let mut profile = UserProfile::default();
        profile.shells.push(DiscoveredShell {
            id: "powershell".to_string(),
            title: "Windows PowerShell".to_string(),
            path: "powershell.exe".to_string(),
            icon: icons::POWERSHELL.to_string(),
            is_default: true,
        });

        // Request a WSL distro that does not exist on machine
        let (fallback, warning) = profile.verify_or_fallback(&Profile::WslDistro("nonexistent_distro_xyz".to_string()));
        assert!(warning.is_some());
        assert_eq!(fallback, Profile::PowerShell);

        // Request a tool that does not exist
        let (tool_fallback, tool_warning) = profile.verify_or_fallback(&Profile::CustomTool {
            name: "nonexistent_tool".to_string(),
            path: r"C:\totally_fake_path\tool.exe".to_string(),
        });
        assert!(tool_warning.is_some());
        assert_eq!(tool_fallback, Profile::PowerShell);
    }

    #[test]
    fn test_recent_directories_limit_and_order() {
        let mut profile = UserProfile::default();
        for i in 1..=15 {
            profile.add_recent_directory(&format!(r"C:\project_{i}"));
        }
        assert_eq!(profile.recent_directories.len(), 10);
        assert_eq!(profile.recent_directories[0], r"C:\project_15");
        assert_eq!(profile.recent_directories[9], r"C:\project_6");

        // Re-adding existing directory moves it to top
        profile.add_recent_directory(r"C:\project_10");
        assert_eq!(profile.recent_directories.len(), 10);
        assert_eq!(profile.recent_directories[0], r"C:\project_10");
    }

    #[test]
    fn test_scan_environment_sync_does_not_panic() {
        let env = scan_environment_sync();
        println!("DETECTED ENVIRONMENT ON MACHINE:\n{:#?}", env);
        assert!(!env.shells.is_empty(), "must detect at least one shell on Windows");
    }
}
