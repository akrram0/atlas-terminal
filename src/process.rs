//! Foreground child process detection for terminal sessions.
//!
//! Inspects the child processes of spawned shells using Win32 Toolhelp32 snapshots
//! to identify active foreground CLI commands (e.g., `agy`, `claude`, `opencode`, `nvim`, `cargo`).
//! Enables dynamic tab title and brand icon switching with zero UI latency.

/// Active foreground process information for a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundProcess {
    /// Raw base name (e.g. "agy", "claude", "cargo").
    pub name: String,
    /// Human-friendly display label (e.g. "Antigravity", "Claude Code", "Cargo").
    pub display_name: String,
    /// Brand icon path or symbol.
    pub icon: &'static str,
}

#[cfg(windows)]
mod imp {
    use std::collections::{HashMap, HashSet};
    use std::ffi::c_void;
    use crate::icons;
    use super::ForegroundProcess;

    const TH32CS_SNAPPROCESS: u32 = 0x00000002;
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    #[repr(C)]
    struct PROCESSENTRY32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> *mut c_void;
        fn Process32FirstW(h_snapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
        fn Process32NextW(h_snapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
        fn CloseHandle(h_object: *mut c_void) -> i32;
    }

    struct RawProc {
        pid: u32,
        ppid: u32,
        exe: String,
    }

    /// Read all running processes via a single snapshot (<0.3ms).
    fn snapshot_processes() -> Vec<RawProc> {
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Vec::new();
        }

        let mut procs = Vec::with_capacity(256);
        let mut entry = PROCESSENTRY32W {
            dw_size: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0; 260],
        };

        unsafe {
            if Process32FirstW(handle, &mut entry) != 0 {
                loop {
                    let len = entry
                        .sz_exe_file
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.sz_exe_file.len());
                    let exe = String::from_utf16_lossy(&entry.sz_exe_file[..len]);

                    procs.push(RawProc {
                        pid: entry.th32_process_id,
                        ppid: entry.th32_parent_process_id,
                        exe,
                    });

                    entry.dw_size = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                    if Process32NextW(handle, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(handle);
        }

        procs
    }

    /// Check if an executable is an auxiliary console or system host process.
    fn is_auxiliary(name_lower: &str) -> bool {
        matches!(
            name_lower,
            "conhost.exe"
                | "openconsole.exe"
                | "wslhost.exe"
                | "wslservice.exe"
                | "svchost.exe"
                | "runtimebroker.exe"
                | "backgroundtaskhost.exe"
                | "smartscreen.exe"
        )
    }

    /// Check if a process is an idle shell executable.
    pub(crate) fn is_shell(name_lower: &str) -> bool {
        matches!(
            name_lower,
            "powershell.exe"
                | "pwsh.exe"
                | "cmd.exe"
                | "bash.exe"
                | "zsh.exe"
                | "fish.exe"
                | "nu.exe"
                | "wsl.exe"
                | "wslhost.exe"
        )
    }

    /// Map an executable name to brand details (display name and icon).
    pub(crate) fn brand_for_exe(exe: &str) -> ForegroundProcess {
        let base = exe.strip_suffix(".exe").unwrap_or(exe);
        let lower = base.to_ascii_lowercase();

        let (display_name, icon) = match lower.as_str() {
            "agy" | "antigravity" => ("Antigravity".to_string(), icons::ANTIGRAVITY_MARK),
            "opencode" => ("OpenCode".to_string(), icons::OPENCODE_MARK),
            "claude" => ("Claude Code".to_string(), icons::CLAUDE_MARK),
            "cursor" => ("Cursor".to_string(), icons::CURSOR_MARK),
            "lazygit" => ("LazyGit".to_string(), icons::GIT),
            "git" => ("Git".to_string(), icons::GIT),
            "fastfetch" => ("Fastfetch".to_string(), icons::TERMINAL),
            "starship" => ("Starship".to_string(), icons::TERMINAL),
            "oh-my-posh" => ("Oh My Posh".to_string(), icons::TERMINAL),
            "nvim" => ("Neovim".to_string(), icons::TERMINAL),
            "vim" => ("Vim".to_string(), icons::TERMINAL),
            "cargo" => ("Cargo".to_string(), icons::TERMINAL),
            "rustc" => ("Rustc".to_string(), icons::TERMINAL),
            "python" | "python3" => ("Python".to_string(), icons::TERMINAL),
            "node" => ("Node".to_string(), icons::TERMINAL),
            "pnpm" => ("pnpm".to_string(), icons::TERMINAL),
            "npm" => ("npm".to_string(), icons::TERMINAL),
            "bun" => ("Bun".to_string(), icons::TERMINAL),
            "deno" => ("Deno".to_string(), icons::TERMINAL),
            "btop" => ("btop".to_string(), icons::TERMINAL),
            "htop" => ("htop".to_string(), icons::TERMINAL),
            "ssh" => ("ssh".to_string(), icons::TERMINAL),
            "docker" => ("Docker".to_string(), icons::TERMINAL),
            "wsl" => {
                if let Some(distro) = crate::wsl::default_distro() {
                    (distro.title.clone(), distro.icon)
                } else {
                    ("WSL".to_string(), icons::LINUX)
                }
            }
            _ => (base.to_string(), icons::TERMINAL),
        };

        ForegroundProcess {
            name: base.to_string(),
            display_name,
            icon,
        }
    }

    /// Find the active foreground process for each queried shell PID.
    pub fn get_foreground_processes(shell_pids: &[u32]) -> HashMap<u32, ForegroundProcess> {
        let mut results = HashMap::new();
        if shell_pids.is_empty() {
            return results;
        }

        let procs = snapshot_processes();
        if procs.is_empty() {
            return results;
        }

        // Build parent -> children map
        let mut children_map: HashMap<u32, Vec<&RawProc>> = HashMap::new();
        for proc in &procs {
            children_map.entry(proc.ppid).or_default().push(proc);
        }

        for &shell_pid in shell_pids {
            let mut visited = HashSet::new();
            let mut queue = vec![shell_pid];
            let mut non_aux_descendants: Vec<&RawProc> = Vec::new();

            while let Some(parent) = queue.pop() {
                if !visited.insert(parent) {
                    continue;
                }
                if let Some(children) = children_map.get(&parent) {
                    for child in children {
                        let name_lower = child.exe.to_ascii_lowercase();
                        if !is_auxiliary(&name_lower) {
                            non_aux_descendants.push(child);
                            queue.push(child.pid);
                        }
                    }
                }
            }

            if non_aux_descendants.is_empty() {
                continue;
            }

            // Priority 1: Check if any descendant is a recognized high-value tool (e.g. agy, opencode, claude)
            let high_value = non_aux_descendants.iter().find(|d| {
                let lower = d.exe.to_ascii_lowercase();
                let base = lower.strip_suffix(".exe").unwrap_or(&lower);
                matches!(
                    base,
                    "agy"
                        | "antigravity"
                        | "opencode"
                        | "claude"
                        | "cursor"
                        | "lazygit"
                        | "nvim"
                        | "cargo"
                        | "fastfetch"
                        | "btop"
                )
            });

            if let Some(target) = high_value {
                results.insert(shell_pid, brand_for_exe(&target.exe));
                continue;
            }

            // Priority 2: Filter out shell intermediaries (e.g. cmd.exe wrapper spawning node)
            let non_shells: Vec<&&RawProc> = non_aux_descendants
                .iter()
                .filter(|d| !is_shell(&d.exe.to_ascii_lowercase()))
                .collect();

            if let Some(target) = non_shells.last().copied() {
                results.insert(shell_pid, brand_for_exe(&target.exe));
            }
        }

        results
    }
}

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;
    use super::ForegroundProcess;

    pub fn get_foreground_processes(_shell_pids: &[u32]) -> HashMap<u32, ForegroundProcess> {
        HashMap::new()
    }
}

pub use imp::get_foreground_processes;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brand_resolution() {
        #[cfg(windows)]
        {
            let agy = imp::brand_for_exe("agy.exe");
            assert_eq!(agy.display_name, "Antigravity");
            assert_eq!(agy.icon, crate::icons::ANTIGRAVITY_MARK);

            let claude = imp::brand_for_exe("claude.exe");
            assert_eq!(claude.display_name, "Claude Code");
            assert_eq!(claude.icon, crate::icons::CLAUDE_MARK);

            let opencode = imp::brand_for_exe("opencode.exe");
            assert_eq!(opencode.display_name, "OpenCode");
            assert_eq!(opencode.icon, crate::icons::OPENCODE_MARK);

            let cargo = imp::brand_for_exe("cargo.exe");
            assert_eq!(cargo.display_name, "Cargo");
            assert_eq!(cargo.icon, crate::icons::TERMINAL);

            let lazygit = imp::brand_for_exe("lazygit.exe");
            assert_eq!(lazygit.display_name, "LazyGit");
            assert_eq!(lazygit.icon, crate::icons::GIT);

            let wsl = imp::brand_for_exe("wsl.exe");
            if let Some(distro) = crate::wsl::default_distro() {
                assert_eq!(wsl.display_name, distro.title);
                assert_eq!(wsl.icon, distro.icon);
            } else {
                assert_eq!(wsl.display_name, "WSL");
                assert_eq!(wsl.icon, crate::icons::LINUX);
            }

            assert!(imp::is_shell("wsl.exe"));
            assert!(imp::is_shell("wslhost.exe"));
            assert!(imp::is_shell("powershell.exe"));
            assert!(imp::is_shell("bash.exe"));
        }
    }

    #[test]
    fn test_get_foreground_processes_empty() {
        let results = get_foreground_processes(&[]);
        assert!(results.is_empty());
    }
}
