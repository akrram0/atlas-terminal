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
pub const PROFILE_SCHEMA_VERSION: u32 = 2;

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
                // Ensure default shells and tools are populated if older schema or empty
                if profile.schema_version < PROFILE_SCHEMA_VERSION || profile.shells.is_empty() {
                    let env = scan_environment_sync();
                    profile.reconcile(env);
                    profile.schema_version = PROFILE_SCHEMA_VERSION;
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
            Profile::Antigravity => {
                let found = find_tool_in_system("agy").or_else(|| find_tool_in_system("antigravity"));
                if found.is_none() {
                    self.tools.retain(|t| t.name != "agy" && t.name != "antigravity");
                    self.save();
                    let msg = "Antigravity CLI is not installed. Falling back to PowerShell.".to_string();
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
        ("agy", "Antigravity", icons::ANTIGRAVITY_MARK),
        ("opencode", "OpenCode", icons::OPENCODE_MARK),
        ("claude", "Claude Code", icons::CLAUDE_MARK),
        ("fastfetch", "Fastfetch", icons::SPARKLE),
        ("starship", "Starship", icons::TERMINAL),
        ("oh-my-posh", "Oh My Posh", icons::TERMINAL),
        ("nvim", "Neovim", icons::TERMINAL),
        ("lazygit", "LazyGit", icons::GIT),
        ("btop", "btop", icons::TERMINAL),
        ("cargo", "Cargo", icons::CARGO),
        ("pnpm", "pnpm", icons::PACKAGE),
    ];

    let mut discovered = Vec::new();
    for (name, title, icon) in targets {
        let path = if name == "agy" {
            find_tool_in_system("agy").or_else(|| find_tool_in_system("antigravity"))
        } else {
            find_tool_in_system(name)
        };
        if let Some(path) = path {
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

#[cfg(windows)]
pub fn ensure_opencode_binary_link() {
    let local = match std::env::var_os("LOCALAPPDATA") {
        Some(l) => PathBuf::from(l),
        None => return,
    };
    let pnpm_bin = local.join("pnpm").join("bin");
    let target_exe = pnpm_bin.join("opencode.exe");
    if !pnpm_bin.is_dir() {
        let _ = std::fs::create_dir_all(&pnpm_bin);
    }

    let mut latest_candidate: Option<PathBuf> = None;
    let mut latest_mtime = std::time::SystemTime::UNIX_EPOCH;

    // Search in pnpm/global/v11 for @opencode/cli/bin/opencode.exe
    let global_v11 = local.join("pnpm").join("global").join("v11");
    if global_v11.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&global_v11) {
            for entry in entries.flatten() {
                let candidate = entry
                    .path()
                    .join("node_modules")
                    .join("@opencode")
                    .join("cli")
                    .join("bin")
                    .join("opencode.exe");
                if candidate.is_file() {
                    let mtime = candidate
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    if mtime >= latest_mtime {
                        latest_mtime = mtime;
                        latest_candidate = Some(candidate);
                    }
                }
            }
        }
    }

    // Also check pnpm/store
    let store_v11 = local.join("pnpm").join("store").join("v11");
    if store_v11.is_dir() {
        let links = store_v11.join("links").join("@opencode").join("cli");
        if links.is_dir() {
            if let Ok(versions) = std::fs::read_dir(&links) {
                for v in versions.flatten() {
                    if let Ok(hashes) = std::fs::read_dir(v.path()) {
                        for h in hashes.flatten() {
                            let candidate = h.path().join("bin").join("opencode.exe");
                            if candidate.is_file() {
                                let mtime = candidate
                                    .metadata()
                                    .and_then(|m| m.modified())
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                if mtime >= latest_mtime {
                                    latest_mtime = mtime;
                                    latest_candidate = Some(candidate);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(candidate) = latest_candidate {
        let needs_copy = match (target_exe.metadata(), candidate.metadata()) {
            (Ok(tm), Ok(cm)) => tm.len() != cm.len(),
            _ => true,
        };
        if needs_copy {
            let _ = std::fs::remove_file(&target_exe);
            let _ = std::fs::copy(&candidate, &target_exe);
        }
    }

    // Repair pnpm wrapper shims if broken or pointing to stale hashes
    repair_pnpm_opencode_shims(&pnpm_bin);
}

#[cfg(windows)]
fn repair_pnpm_opencode_shims(pnpm_bin: &Path) {
    let ps1_shim = pnpm_bin.join("opencode.ps1");
    let cmd_shim = pnpm_bin.join("opencode.cmd");
    let target_exe = pnpm_bin.join("opencode.exe");

    if target_exe.is_file() {
        if ps1_shim.is_file() {
            if let Ok(content) = std::fs::read_to_string(&ps1_shim) {
                if content.contains("/global/v11/") && !content.contains("Join-Path $basedir \"opencode.exe\"") {
                    let fixed_ps1 = "#!/usr/bin/env pwsh\n\
$basedir = Split-Path $MyInvocation.MyCommand.Definition -Parent\n\
$target = Join-Path $basedir \"opencode.exe\"\n\
if (-not (Test-Path $target)) {\n\
    $found = Resolve-Path \"$basedir/../global/v11/*/node_modules/@opencode/cli/bin/opencode.exe\" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Path -Last 1\n\
    if ($found) { $target = $found }\n\
}\n\
if ($MyInvocation.ExpectingInput) {\n\
    $input | & $target $args\n\
} else {\n\
    & $target $args\n\
}\n\
exit $LASTEXITCODE\n";
                    let _ = std::fs::write(&ps1_shim, fixed_ps1);
                }
            }
        }

        if cmd_shim.is_file() {
            if let Ok(content) = std::fs::read_to_string(&cmd_shim) {
                if content.contains("\\global\\v11\\") && !content.contains("%~dp0\\opencode.exe") {
                    let fixed_cmd = "@SETLOCAL\r\n\
@IF EXIST \"%~dp0\\opencode.exe\" (\r\n\
  @\"%~dp0\\opencode.exe\" %*\r\n\
) ELSE (\r\n\
  @\"%~dp0\\..\\global\\v11\\6778c4801c795a91f7162d2c5e61c64b62eb29408dbf407e4bd7d76eef2ea239\\node_modules\\@opencode\\cli\\bin\\opencode.exe\" %*\r\n\
)\r\n";
                    let _ = std::fs::write(&cmd_shim, fixed_cmd);
                }
            }
        }
    }
}

/// Find a CLI tool on the system using fast file existence checks.
pub fn find_tool_in_system(tool_name: &str) -> Option<String> {
    #[cfg(windows)]
    if tool_name == "opencode" {
        ensure_opencode_binary_link();
    }

    // 1. Well-known package manager directories
    let mut explicit_dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local_path = PathBuf::from(local);
        explicit_dirs.push(local_path.join("agy").join("bin"));
        explicit_dirs.push(local_path.join("pnpm").join("bin"));
        explicit_dirs.push(local_path.join("pnpm"));
        explicit_dirs.push(local_path.join("Microsoft").join("WinGet").join("Links"));
        explicit_dirs.push(local_path.join("Microsoft").join("WindowsApps"));
        explicit_dirs.push(local_path.join("Programs").join("oh-my-posh").join("bin"));
        explicit_dirs.push(local_path.join("Programs").join("starship"));
        explicit_dirs.push(local_path.join("Programs").join("starship").join("bin"));
        explicit_dirs.push(local_path.join("Programs").join(tool_name));
        explicit_dirs.push(local_path.join("Programs").join(tool_name).join("bin"));
        explicit_dirs.push(local_path.join("Programs").join("Antigravity"));
        explicit_dirs.push(local_path.join("Programs").join("Antigravity").join("bin"));
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

pub fn find_executable_on_path(name: &str) -> Option<String> {
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

#[cfg(windows)]
fn query_registry_path(hkey: isize, subkey: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        fn RegOpenKeyExW(
            hKey: isize,
            lpSubKey: *const u16,
            ulOptions: u32,
            samDesired: u32,
            phkResult: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            hKey: isize,
            lpValueName: *const u16,
            lpReserved: *mut u32,
            lpType: *mut u32,
            lpData: *mut u8,
            lpcbData: *mut u32,
        ) -> i32;
        fn RegCloseKey(hKey: isize) -> i32;
        fn ExpandEnvironmentStringsW(
            lpSrc: *const u16,
            lpDst: *mut u16,
            nSize: u32,
        ) -> u32;
    }

    const KEY_READ: u32 = 0x20019;
    let subkey_w: Vec<u16> = std::ffi::OsStr::new(subkey)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let value_w: Vec<u16> = std::ffi::OsStr::new("Path")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut key: isize = 0;
    let status = unsafe {
        RegOpenKeyExW(
            hkey,
            subkey_w.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if status != 0 || key == 0 {
        return None;
    }

    let mut buf_size: u32 = 0;
    let status = unsafe {
        RegQueryValueExW(
            key,
            value_w.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut buf_size,
        )
    };
    if status != 0 || buf_size == 0 {
        unsafe { RegCloseKey(key) };
        return None;
    }

    let mut buf = vec![0u8; buf_size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            value_w.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            &mut buf_size,
        )
    };
    unsafe { RegCloseKey(key) };

    if status != 0 {
        return None;
    }

    let u16_slice: &[u16] = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf_size as usize / 2)
    };
    let raw_w = if let Some((&0, rest)) = u16_slice.split_last() {
        rest
    } else {
        u16_slice
    };

    let raw_w_null: Vec<u16> = raw_w.iter().copied().chain(std::iter::once(0)).collect();
    let needed = unsafe { ExpandEnvironmentStringsW(raw_w_null.as_ptr(), std::ptr::null_mut(), 0) };
    if needed > 0 {
        let mut expanded = vec![0u16; needed as usize];
        let written = unsafe {
            ExpandEnvironmentStringsW(raw_w_null.as_ptr(), expanded.as_mut_ptr(), needed)
        };
        if written > 0 {
            if let Some((&0, rest)) = expanded.split_last() {
                return Some(String::from_utf16_lossy(rest));
            }
            return Some(String::from_utf16_lossy(&expanded));
        }
    }

    Some(String::from_utf16_lossy(raw_w))
}

/// Builds an enriched, de-duplicated `PATH` for spawned terminal sessions.
/// - Loads the real-time User (`HKCU\Environment`) and System (`HKLM\...\Environment`) Registry `PATH`
/// - Ensures well-known prompt engines (`oh-my-posh`, `starship`) and package managers (`pnpm`, `npm`, `cargo`, `scoop`, `bun`, `winget`) are discoverable
/// - Embeds `ATLAS_DIR` so `atlas` CLI commands always work inside all shells
pub fn build_enriched_path() -> std::ffi::OsString {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_path = |p: PathBuf| {
        let s = p.to_string_lossy();
        let trimmed = s.trim().trim_matches('"').trim();
        if trimmed.is_empty() {
            return;
        }
        let key = if cfg!(windows) {
            trimmed.to_lowercase()
        } else {
            trimmed.to_string()
        };
        if seen.insert(key) {
            paths.push(PathBuf::from(trimmed));
        }
    };

    // 0. Atlas directory so `atlas` CLI is always in PATH
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            add_path(exe_dir.to_path_buf());
        }
    }

    // 1. Existing process PATH entries
    if let Some(current_path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&current_path) {
            add_path(p);
        }
    }

    // 2. Windows Registry: User PATH (HKCU\Environment) & Machine PATH (HKLM)
    #[cfg(windows)]
    {
        // HKEY_CURRENT_USER = 0x80000001
        if let Some(user_reg) = query_registry_path(-2147483647, "Environment") {
            for p in std::env::split_paths(&user_reg) {
                add_path(p);
            }
        }
        // HKEY_LOCAL_MACHINE = 0x80000002
        if let Some(machine_reg) = query_registry_path(
            -2147483646,
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        ) {
            for p in std::env::split_paths(&machine_reg) {
                add_path(p);
            }
        }
    }

    // 3. Explicit well-known tool directories
    let mut explicit_dirs = Vec::new();

    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local_path = PathBuf::from(&local);
        explicit_dirs.push(local_path.join("Programs").join("oh-my-posh").join("bin"));
        explicit_dirs.push(local_path.join("Microsoft").join("WindowsApps"));
        explicit_dirs.push(local_path.join("Microsoft").join("WinGet").join("Links"));
        explicit_dirs.push(local_path.join("Programs").join("starship"));
        explicit_dirs.push(local_path.join("Programs").join("starship").join("bin"));
        explicit_dirs.push(local_path.join("pnpm").join("bin"));
        explicit_dirs.push(local_path.join("pnpm"));
        explicit_dirs.push(local_path.join("agy").join("bin"));

        // Scan WinGet package directories for JanDeDobbeleer.OhMyPosh
        let winget_pkgs = local_path.join("Microsoft").join("WinGet").join("Packages");
        if let Ok(entries) = std::fs::read_dir(&winget_pkgs) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.file_name().and_then(|n| n.to_str()).map_or(false, |s| s.contains("OhMyPosh")) {
                    let bin = p.join("bin");
                    explicit_dirs.push(if bin.is_dir() { bin } else { p });
                }
            }
        }
    }

    if let Some(appdata) = std::env::var_os("APPDATA") {
        explicit_dirs.push(PathBuf::from(appdata).join("npm"));
    }

    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let prof_path = PathBuf::from(profile);
        explicit_dirs.push(prof_path.join("scoop").join("shims"));
        explicit_dirs.push(prof_path.join("scoop").join("apps").join("oh-my-posh").join("current").join("bin"));
        explicit_dirs.push(prof_path.join(".cargo").join("bin"));
        explicit_dirs.push(prof_path.join(".bun").join("bin"));
        explicit_dirs.push(prof_path.join(".local").join("bin"));
        explicit_dirs.push(prof_path.join(".oh-my-posh").join("bin"));
    }

    if let Some(prog_files) = std::env::var_os("ProgramFiles") {
        explicit_dirs.push(PathBuf::from(prog_files).join("oh-my-posh").join("bin"));
    }

    if let Some(prog_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        explicit_dirs.push(PathBuf::from(prog_files_x86).join("oh-my-posh").join("bin"));
    }

    for dir in explicit_dirs {
        if dir.is_dir() {
            add_path(dir);
        }
    }

    std::env::join_paths(paths).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
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

        // Request Antigravity fallback when not found
        let (agy_fallback, agy_warning) = profile.verify_or_fallback(&Profile::Antigravity);
        // On this machine, agy is installed, so agy_fallback is Profile::Antigravity or PowerShell
        if agy_warning.is_some() {
            assert_eq!(agy_fallback, Profile::PowerShell);
        } else {
            assert_eq!(agy_fallback, Profile::Antigravity);
        }
    }

    #[test]
    fn test_antigravity_profile_mapping() {
        assert_eq!(Profile::from_name("agy"), Profile::Antigravity);
        assert_eq!(Profile::from_name("antigravity"), Profile::Antigravity);
        assert_eq!(Profile::Antigravity.as_str(), "antigravity");
        assert_eq!(Profile::Antigravity.to_serialized_id(), "antigravity");
    }

    #[test]
    fn test_antigravity_tool_detection() {
        if Path::new(r"C:\Users\Neweye\AppData\Local\agy\bin\agy.exe").is_file() {
            let tools = detect_tools();
            let agy = tools.iter().find(|t| t.name == "agy");
            assert!(agy.is_some(), "detect_tools must detect agy when installed at %LOCALAPPDATA%\\agy\\bin");
            let agy = agy.unwrap();
            assert_eq!(agy.title, "Antigravity");
            assert_eq!(agy.icon, icons::ANTIGRAVITY_MARK);
        }
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

    #[test]
    fn test_build_enriched_path() {
        let enriched = build_enriched_path();
        let paths: Vec<PathBuf> = std::env::split_paths(&enriched).collect();
        assert!(!paths.is_empty(), "enriched PATH must never be empty");
        // Verify no empty paths or malformed segments
        for p in &paths {
            assert!(!p.as_os_str().is_empty(), "PATH segments must not be empty");
        }
    }

    #[test]
    fn test_opencode_tool_detection() {
        let tool = find_tool_in_system("opencode");
        assert!(tool.is_some(), "opencode must be found on system");
        let path = tool.unwrap();
        assert!(path.to_lowercase().contains("opencode"));
    }
}
