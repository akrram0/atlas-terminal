//! PTY session: portable-pty + alacritty_terminal.
//!
//! One `Session` per tab. The reader thread owns the PTY read side and pushes
//! bytes into an async channel; the UI task drains it and feeds the terminal
//! state machine. Terminal events (title, exit) travel the same channel.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtyPair, PtySize};
use smol::channel::{bounded, Receiver, Sender};

use crate::image_osc::{compute_image_grid_size, Osc1337Scanner, OscScannerEvent};

/// An inline graphic placed on the terminal grid.
#[derive(Clone)]
pub struct PlacedImage {
    #[allow(dead_code)]
    pub id: usize,
    pub image: Arc<gpui::RenderImage>,
    pub anchor_line: i32,
    pub col: usize,
    pub cols: usize,
    pub rows: usize,
    pub is_alt_screen: bool,
}


/// Minimal grid dimensions for `Term`.
pub struct TermSize {
    pub cols: usize,
    pub rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Messages flowing from the PTY side into the UI.
pub enum SessionMsg {
    /// Raw bytes from the shell.
    Output(Vec<u8>),
    /// Terminal events (title, exit, bell, ...).
    Event(Event),
}

/// Bridges alacritty events into the session channel (called on the UI thread
/// while feeding, and by the terminal itself for title changes).
#[derive(Clone)]
pub struct EventProxy {
    tx: Sender<SessionMsg>,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.tx.try_send(SessionMsg::Event(event));
    }
}

/// Which program a new session runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Profile {
    /// PowerShell 7 (`pwsh`), falling back to Windows PowerShell.
    PowerShell,
    /// PowerShell 7 specifically.
    Pwsh,
    /// Command Prompt (`cmd.exe`).
    Cmd,
    /// `wsl.exe` — the default distro's login shell via Windows Subsystem for
    /// Linux. The ConPTY path is identical; wsl.exe maps the Windows cwd.
    Wsl,
    /// `wsl.exe -d <distro>` — a specific registered WSL distro.
    WslDistro(String),
    /// Git for Windows bash.
    GitBash,
    /// OpenCode CLI session.
    OpenCode,
    /// Claude Code CLI session.
    Claude,
    /// Antigravity CLI session.
    Antigravity,
    /// Custom tool session.
    CustomTool { name: String, path: String },
}

impl Profile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::PowerShell => "powershell",
            Profile::Pwsh => "pwsh",
            Profile::Cmd => "cmd",
            Profile::Wsl | Profile::WslDistro(_) => "wsl",
            Profile::GitBash => "gitbash",
            Profile::OpenCode => "opencode",
            Profile::Claude => "claude",
            Profile::Antigravity => "antigravity",
            Profile::CustomTool { .. } => "tool",
        }
    }

    pub fn to_serialized_id(&self) -> String {
        match self {
            Profile::PowerShell => "powershell".to_string(),
            Profile::Pwsh => "pwsh".to_string(),
            Profile::Cmd => "cmd".to_string(),
            Profile::Wsl => "wsl".to_string(),
            Profile::WslDistro(distro) => format!("wsl:{distro}"),
            Profile::GitBash => "gitbash".to_string(),
            Profile::OpenCode => "opencode".to_string(),
            Profile::Claude => "claude".to_string(),
            Profile::Antigravity => "antigravity".to_string(),
            Profile::CustomTool { name, .. } => format!("tool:{name}"),
        }
    }

    pub fn from_name(s: &str) -> Self {
        if let Some(distro) = s.strip_prefix("wsl:") {
            return Profile::WslDistro(distro.to_string());
        }
        match s.to_ascii_lowercase().as_str() {
            "pwsh" | "powershell 7" => Profile::Pwsh,
            "cmd" | "command prompt" => Profile::Cmd,
            "wsl" => Profile::Wsl,
            "gitbash" | "git bash" | "bash" => Profile::GitBash,
            "opencode" => Profile::OpenCode,
            "claude" | "claude code" => Profile::Claude,
            "agy" | "antigravity" => Profile::Antigravity,
            other => {
                if crate::wsl::is_known_distro(other) {
                    Profile::WslDistro(other.to_string())
                } else {
                    Profile::PowerShell
                }
            }
        }
    }
}

#[cfg(windows)]
pub struct JobObject {
    handle: *mut core::ffi::c_void,
}

#[cfg(windows)]
impl JobObject {
    pub fn new() -> Option<Self> {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateJobObjectW(lpJobAttributes: *mut core::ffi::c_void, lpName: *const u16) -> *mut core::ffi::c_void;
            fn SetInformationJobObject(
                hJob: *mut core::ffi::c_void,
                JobObjectInformationClass: u32,
                lpJobObjectInformation: *const core::ffi::c_void,
                cbJobObjectInformationLength: u32,
            ) -> i32;
        }

        #[repr(C)]
        struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
            per_process_user_time_limit: i64,
            per_job_user_time_limit: i64,
            limit_flags: u32,
            minimum_working_set_size: usize,
            maximum_working_set_size: usize,
            active_process_limit: u32,
            affinity: usize,
            priority_class: u32,
            scheduling_class: u32,
        }

        #[repr(C)]
        struct IO_COUNTERS {
            read_operation_count: u64,
            write_operation_count: u64,
            other_operation_count: u64,
            read_transfer_count: u64,
            write_transfer_count: u64,
            other_transfer_count: u64,
        }

        #[repr(C)]
        struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
            io_info: IO_COUNTERS,
            process_memory_limit: usize,
            job_memory_limit: usize,
            peak_process_memory_limit: usize,
            peak_job_memory_limit: usize,
        }

        const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
        const JOB_OBJECT_LIMIT_BREAKAWAY_OK: u32 = 0x0800;
        const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: u32 = 0x1000;
        const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

        unsafe {
            let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if handle.is_null() {
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_BREAKAWAY_OK
                | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;

            let ok = SetInformationJobObject(
                handle,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                #[link(name = "kernel32")]
                unsafe extern "system" {
                    fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
                }
                CloseHandle(handle);
                return None;
            }

            Some(Self { handle })
        }
    }

    pub fn assign_process(&self, pid: u32) -> bool {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut core::ffi::c_void;
            fn AssignProcessToJobObject(hJob: *mut core::ffi::c_void, hProcess: *mut core::ffi::c_void) -> i32;
            fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
        }

        const PROCESS_SET_QUOTA: u32 = 0x0100;
        const PROCESS_TERMINATE: u32 = 0x0001;

        unsafe {
            let h_proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if h_proc.is_null() {
                return false;
            }
            let res = AssignProcessToJobObject(self.handle, h_proc);
            CloseHandle(h_proc);
            res != 0
        }
    }
}

#[cfg(windows)]
impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
            }
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(windows)]
pub fn init_process_job_object() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn AssignProcessToJobObject(hJob: *mut core::ffi::c_void, hProcess: *mut core::ffi::c_void) -> i32;
    }
    if let Some(job) = JobObject::new() {
        unsafe {
            // Assign our process to this job. All children spawned by Atlas will inherit it.
            let _ = AssignProcessToJobObject(job.handle, GetCurrentProcess());
        }
        // Leak the job handle deliberately so it stays alive until Atlas exits, at which point
        // the Windows kernel closes the job and terminates all child processes automatically.
        std::mem::forget(job);
    }
}

#[allow(dead_code)]
#[cfg(not(windows))]
pub fn init_process_job_object() {}

#[cfg(windows)]
pub fn terminate_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(not(windows))]
pub fn terminate_process_tree(_pid: u32) {}

#[cfg(windows)]
pub fn trim_working_set() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn SetProcessWorkingSetSize(hProcess: *mut core::ffi::c_void, dwMinimumWorkingSetSize: usize, dwMaximumWorkingSetSize: usize) -> i32;
    }
    unsafe {
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

pub struct Session {
    pub term: Term<EventProxy>,
    pub parser: Processor,
    pub cols: usize,
    pub rows: usize,
    pub cell_width: f32,
    pub cell_height: f32,
    /// Grid size the pseudo-console most recently accepted. Kept separately
    /// from `cols`/`rows` so a rejected ConPTY resize is retried instead of
    /// silently leaving the child on a stale grid.
    pty_cols: usize,
    pty_rows: usize,
    /// Shell executable that was actually spawned ("pwsh" or
    /// "powershell.exe"), for the sidebar/status-bar labels.
    #[allow(dead_code)]
    pub shell: String,
    /// PID of the shell process; lets the UI poll its working directory
    /// (see `crate::cwd`).
    pub pid: Option<u32>,
    /// Scrollback limit the terminal was created with; used to prune inline
    /// images whose anchor has scrolled out of the buffer.
    scrollback_limit: usize,
    /// Last painted grid layout: (left, top, cell_width, cell_height).
    pub grid_bounds: Option<(f32, f32, f32, f32)>,
    /// Highest row on the visible screen containing active text or the prompt cursor.
    pub max_active_row: usize,
    /// Active inline images anchored to the terminal scrollback grid.
    pub placed_images: Vec<PlacedImage>,
    next_image_id: usize,
    osc_scanner: Osc1337Scanner,
    writer: Option<Box<dyn Write + Send>>,
    master: Box<dyn MasterPty>,
    child: Box<dyn Child + Send + Sync>,
    #[cfg(windows)]
    _job: Option<JobObject>,
}

impl Session {
    /// Spawn a shell in a new PTY for the given profile.
    ///
    /// Returns the session plus the receiver for its output/event stream. The
    /// receiver must be drained (see `AtlasView::watch`), otherwise the UI
    /// never repaints.
    pub fn spawn(
        cols: usize,
        rows: usize,
        cwd: Option<PathBuf>,
        profile: Profile,
        scrollback_lines: usize,
    ) -> anyhow::Result<(Session, Receiver<SessionMsg>)> {
        let pty_system = native_pty_system();
        let pixel_width = (cols as f32 * 8.0).round() as u16;
        let pixel_height = (rows as f32 * 16.0).round() as u16;
        let pair = pty_system.openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width,
            pixel_height,
        })?;

        let PtyPair { master, slave } = pair;

        let mut opencode_path = None;
        if matches!(profile, Profile::OpenCode) {
            opencode_path = crate::discovery::find_tool_in_system("opencode");
        }
        let mut claude_path = None;
        if matches!(profile, Profile::Claude) {
            claude_path = crate::discovery::find_tool_in_system("claude");
        }
        let mut agy_path = None;
        if matches!(profile, Profile::Antigravity) {
            agy_path = crate::discovery::find_tool_in_system("agy")
                .or_else(|| crate::discovery::find_tool_in_system("antigravity"));
        }

        let candidates_vec: Vec<String> = match &profile {
            Profile::PowerShell | Profile::Pwsh => vec!["pwsh".into(), "powershell.exe".into()],
            Profile::Cmd => vec!["cmd.exe".into()],
            Profile::Wsl | Profile::WslDistro(_) => vec!["wsl.exe".into()],
            // Never fall back to a bare `bash.exe`: on Windows that name is
            // usually the Microsoft Store WSL launcher (WindowsApps), not Git
            // for Windows. Resolve Git's own bash explicitly so the Git Bash
            // profile cannot silently open a WSL distro.
            Profile::GitBash => crate::discovery::git_bash_candidates(),
            Profile::OpenCode => {
                let mut v = Vec::new();
                if let Some(ref path) = opencode_path {
                    if path.ends_with(".cmd") || path.ends_with(".bat") {
                        v.push("cmd.exe".into());
                    } else {
                        v.push(path.clone());
                    }
                }
                v.push("opencode.exe".into());
                v.push("cmd.exe".into());
                v
            }
            Profile::Claude => {
                let mut v = Vec::new();
                if let Some(ref path) = claude_path {
                    if path.ends_with(".cmd") || path.ends_with(".bat") {
                        v.push("cmd.exe".into());
                    } else {
                        v.push(path.clone());
                    }
                }
                v.push("claude.exe".into());
                v.push("cmd.exe".into());
                v
            }
            Profile::Antigravity => {
                let mut v = Vec::new();
                if let Some(ref path) = agy_path {
                    if path.ends_with(".cmd") || path.ends_with(".bat") {
                        v.push("cmd.exe".into());
                    } else {
                        v.push(path.clone());
                    }
                }
                v.push("agy.exe".into());
                v.push("antigravity.exe".into());
                v.push("cmd.exe".into());
                v
            }
            Profile::CustomTool { path, .. } => {
                if path.ends_with(".cmd") || path.ends_with(".bat") {
                    vec!["cmd.exe".into()]
                } else {
                    vec![path.clone()]
                }
            }
        };
        let mut child: Option<Box<dyn Child + Send + Sync>> = None;
        let mut shell_used = candidates_vec.last().cloned().unwrap_or_else(|| "powershell.exe".into());
        for shell in &candidates_vec {
            let mut cmd = CommandBuilder::new(shell);
            match &profile {
                Profile::PowerShell | Profile::Pwsh => {
                    cmd.arg("-NoLogo");
                    cmd.arg("-NoExit");
                    cmd.arg("-Command");
                    cmd.arg(
                        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; \
                         $OutputEncoding=[System.Text.Encoding]::UTF8; \
                         [Console]::InputEncoding=[System.Text.Encoding]::UTF8; \
                         try { if ($PSStyle.Progress) { $PSStyle.Progress.View = 'Classic' } } catch {}; \
                         function global:atlas { & \"$env:ATLAS_EXE\" @args }; \
                         $atlas_old = (Get-Command prompt -ErrorAction SilentlyContinue).ScriptBlock; \
                         function prompt { \
                             try { [System.Environment]::CurrentDirectory = $ExecutionContext.SessionState.Path.CurrentLocation.Path } catch {}; \
                             if ($atlas_old) { & $atlas_old } else { 'PS ' + (Get-Location) + '> ' } \
                         }; \
                         function global:update-opencode { \
                             Write-Host 'Checking for OpenCode updates...' -ForegroundColor Cyan; \
                             if (Get-Command pnpm -ErrorAction SilentlyContinue) { \
                                 pnpm update -g @opencode/cli \
                             } elseif (Get-Command npm -ErrorAction SilentlyContinue) { \
                                 npm update -g @opencode/cli \
                             } elseif (Get-Command bun -ErrorAction SilentlyContinue) { \
                                 bun update -g @opencode/cli \
                             } elseif (Get-Command scoop -ErrorAction SilentlyContinue) { \
                                 scoop update opencode \
                             } elseif (Get-Command winget -ErrorAction SilentlyContinue) { \
                                 winget upgrade opencode \
                             } else { \
                                 opencode upgrade \
                             } \
                         };"
                    );
                }
                Profile::Cmd => {}
                Profile::Wsl => {}
                Profile::WslDistro(distro) => {
                    cmd.arg("-d");
                    cmd.arg(distro);
                }
                Profile::GitBash => {
                    cmd.arg("--login");
                    cmd.arg("-i");
                }
                Profile::OpenCode => {
                    if shell == "cmd.exe" {
                        cmd.arg("/k");
                        if let Some(ref path) = opencode_path {
                            cmd.arg(path);
                        } else {
                            cmd.arg("opencode");
                        }
                    }
                }
                Profile::Claude => {
                    if shell == "cmd.exe" {
                        cmd.arg("/k");
                        if let Some(ref path) = claude_path {
                            cmd.arg(path);
                        } else {
                            cmd.arg("claude");
                        }
                    }
                }
                Profile::Antigravity => {
                    if shell == "cmd.exe" {
                        cmd.arg("/k");
                        if let Some(ref path) = agy_path {
                            cmd.arg(path);
                        } else {
                            cmd.arg("agy");
                        }
                    }
                }
                Profile::CustomTool { path, .. } => {
                    if shell == "cmd.exe" {
                        cmd.arg("/k");
                        cmd.arg(path);
                    }
                }
            }
            // Ensure package manager bins (pnpm, npm) are included in PATH so
            // devtools like OpenCode are always discoverable by child shells.
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                let mut added = false;
                if let Ok(exe) = std::env::current_exe() {
                    cmd.env("ATLAS_EXE", &exe);
                    if let Some(exe_dir) = exe.parent() {
                        let shim = exe_dir.join("atlas.cmd");
                        if !shim.exists() {
                            let _ = std::fs::write(&shim, "@echo off\r\n\"%~dp0atlas-terminal.exe\" %*\r\n");
                        }
                        let exe_dir_buf = exe_dir.to_path_buf();
                        if !paths.contains(&exe_dir_buf) {
                            paths.insert(0, exe_dir_buf);
                            added = true;
                        }
                    }
                }
                if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                    let pnpm_bin = PathBuf::from(&local_app_data).join("pnpm").join("bin");
                    if pnpm_bin.is_dir() && !paths.contains(&pnpm_bin) {
                        paths.insert(0, pnpm_bin);
                        added = true;
                    }
                    let pnpm_root = PathBuf::from(&local_app_data).join("pnpm");
                    if pnpm_root.is_dir() && !paths.contains(&pnpm_root) {
                        paths.insert(0, pnpm_root);
                        added = true;
                    }
                    let winget_links = PathBuf::from(&local_app_data).join("Microsoft").join("WinGet").join("Links");
                    if winget_links.is_dir() && !paths.contains(&winget_links) {
                        paths.insert(0, winget_links);
                        added = true;
                    }
                }
                if let Some(app_data) = std::env::var_os("APPDATA") {
                    let npm_bin = PathBuf::from(app_data).join("npm");
                    if npm_bin.is_dir() && !paths.contains(&npm_bin) {
                        paths.insert(0, npm_bin);
                        added = true;
                    }
                }
                if let Some(user_profile) = std::env::var_os("USERPROFILE") {
                    let bun_bin = PathBuf::from(&user_profile).join(".bun").join("bin");
                    if bun_bin.is_dir() && !paths.contains(&bun_bin) {
                        paths.insert(0, bun_bin);
                        added = true;
                    }
                    let scoop_shims = PathBuf::from(&user_profile).join("scoop").join("shims");
                    if scoop_shims.is_dir() && !paths.contains(&scoop_shims) {
                        paths.insert(0, scoop_shims);
                        added = true;
                    }
                }
                if added {
                    if let Ok(new_path) = std::env::join_paths(paths) {
                        cmd.env("PATH", new_path);
                    }
                }
            }
            if let Some(comspec) = std::env::var_os("COMSPEC") {
                cmd.env("COMSPEC", comspec);
            }
            // Terminal identity every CLI tool reads for colour, locale and
            // capability detection. Without these, native tools fall back to
            // the Windows console defaults.
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
            cmd.env("LANG", "en_US.UTF-8");
            cmd.env("LC_ALL", "en_US.UTF-8");
            cmd.env("WT_SESSION", "1");
            cmd.env("TERM_PROGRAM", "Atlas");
            cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
            if matches!(profile, Profile::Wsl | Profile::WslDistro(_)) {
                // wsl.exe starts a Linux process from a Windows one; only
                // variables listed in WSLENV are copied into the distro. Without
                // this, fastfetch and other CLIs inside WSL see no terminal
                // identity at all (TERM_PROGRAM is what they report).
                let mut wsl_env = std::env::var("WSLENV").unwrap_or_default();
                for var in ["TERM_PROGRAM", "TERM_PROGRAM_VERSION", "COLORTERM", "TERM", "ATLAS_EXE"] {
                    // Already listed, with or without a WSLENV flag (`/p`, `/u`…).
                    let listed = wsl_env
                        .split(':')
                        .any(|entry| entry.split('/').next() == Some(var));
                    if listed {
                        continue;
                    }
                    if !wsl_env.is_empty() {
                        wsl_env.push(':');
                    }
                    wsl_env.push_str(var);
                    wsl_env.push_str("/u");
                }
                cmd.env("WSLENV", wsl_env);
            }
            if let Some(dir) = cwd.as_ref() {
                if dir.is_dir() {
                    cmd.cwd(dir.clone());
                }
            }
            match slave.spawn_command(cmd) {
                Ok(spawned) => {
                    child = Some(spawned);
                    shell_used = shell.clone();
                    break;
                }
                Err(_) => continue,
            }
        }
        let child =
            child.ok_or_else(|| anyhow::anyhow!("no shell found (tried {candidates_vec:?})"))?;
        if matches!(profile, Profile::GitBash) {
            // The whole point of the explicit resolver: prove which bash won.
            crate::log_line(&format!("git bash: using {shell_used}"));
        }
        let pid = child.process_id();
        drop(slave);

        let mut reader = master.try_clone_reader()?;
        let writer = master.take_writer()?;

        let (tx, rx) = bounded::<SessionMsg>(512);
        let proxy = EventProxy { tx: tx.clone() };
        let term = Term::new(
            Config {
                scrolling_history: scrollback_lines,
                ..Config::default()
            },
            &TermSize { cols, rows },
            proxy,
        );
        let parser = Processor::new();

        // Reader thread: PTY -> channel. Ends when the PTY closes (child exit
        // or session drop), emitting a final Exit event so the UI can show the
        // restart affordance.
        std::thread::Builder::new()
            .name("atlas-pty-reader".into())
            .spawn(move || {
                let mut buffer = [0u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx
                                .send_blocking(SessionMsg::Output(buffer[..n].to_vec()))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
                let _ = tx.send_blocking(SessionMsg::Event(Event::Exit));
            })?;

        #[cfg(windows)]
        let _job = JobObject::new().map(|job| {
            if let Some(p) = pid {
                job.assign_process(p);
            }
            job
        });

        Ok((
            Session {
                term,
                parser,
                cols,
                rows,
                cell_width: 8.0,
                cell_height: 16.0,
                pty_cols: cols,
                pty_rows: rows,
                shell: shell_used.to_string(),
                pid,
                scrollback_limit: scrollback_lines,
                grid_bounds: None,
                max_active_row: 0,
                placed_images: Vec::new(),
                next_image_id: 1,
                osc_scanner: Osc1337Scanner::new(),
                writer: Some(writer),
                master,
                child,
                #[cfg(windows)]
                _job,
            },
            rx,
        ))
    }

    /// Feed PTY bytes into the terminal state machine, intercepting inline images.
    pub fn feed(&mut self, bytes: &[u8]) {
        let events = self.osc_scanner.process(bytes);

        for event in events {
            match event {
                OscScannerEvent::Bytes(clean_bytes) => {
                    let was_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);

                    // Clear placed images on full screen/scrollback reset (ED 3: \x1b[3J or RIS: \x1bc)
                    if clean_bytes.windows(4).any(|w| w == b"\x1b[3J")
                        || clean_bytes.windows(2).any(|w| w == b"\x1bc")
                    {
                        self.placed_images.clear();
                    }

                    if !clean_bytes.is_empty() {
                        self.parser.advance(&mut self.term, &clean_bytes);
                    }

                    let is_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
                    if was_alt_screen && !is_alt_screen {
                        // Exited alternate screen: purge images placed in alternate screen
                        self.placed_images.retain(|img| !img.is_alt_screen);
                    }
                }
                OscScannerEvent::SideBySideCompensation => {
                    // Fastfetch side-by-side compensation sequence detected and absorbed.
                }
                OscScannerEvent::Image(decoded) => {
                    let is_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
                    let (img_cols, img_rows) = compute_image_grid_size(
                        decoded.pixel_width,
                        decoded.pixel_height,
                        &decoded.params,
                        self.cell_width,
                        self.cell_height,
                        self.cols,
                        self.rows,
                    );

                    let (cursor_col, cursor_line) = {
                        let cursor = self.term.renderable_content().cursor;
                        (cursor.point.column.0, cursor.point.line.0)
                    };

                    // Robust column positioning:
                    // If cursor_col is past half the screen or would cause the image to overflow
                    // the screen's right edge, snap to column 0. Otherwise keep cursor_col (e.g. 1 for padding).
                    let col = if cursor_col + img_cols > self.cols || cursor_col > self.cols / 2 {
                        0
                    } else {
                        cursor_col
                    };

                    let history = if is_alt_screen {
                        0
                    } else {
                        self.term.history_size() as i32
                    };
                    // For side-by-side inline graphics (e.g. Fastfetch on the left with specs on the right),
                    // apply 2 rows of vertical padding so the image sits in the vertical center ("f waset")
                    // directly beneath the title and separator, rather than being attached to the prompt line.
                    let is_side_by_side = col <= 2 && img_cols * 2 <= self.cols;
                    let v_pad = if is_side_by_side && !is_alt_screen { 2 } else { 0 };
                    let anchor_line = history + cursor_line + v_pad;

                    let id = self.next_image_id;
                    self.next_image_id += 1;

                    self.placed_images.push(PlacedImage {
                        id,
                        image: decoded.render_image,
                        anchor_line,
                        col,
                        cols: img_cols,
                        rows: img_rows,
                        is_alt_screen,
                    });
                }
            }
        }

        // Prune images whose anchor line has scrolled past the configured
        // scrollback limit.
        let cur_history = self.term.history_size() as i32;
        let limit = self.scrollback_limit as i32;
        self.placed_images
            .retain(|img| img.is_alt_screen || img.anchor_line >= cur_history - limit);
        // Memory optimization: cap retained scrollback images to the 5 newest
        // to avoid uncompressed image texture memory accumulation.
        if self.placed_images.len() > 5 {
            let overflow = self.placed_images.len() - 5;
            self.placed_images.drain(0..overflow);
        }
    }

    /// Send bytes to the shell.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    /// Resize PTY + terminal grid. No-op when unchanged or degenerate.
    ///
    /// The terminal grid always tracks the requested size, but the
    /// pseudo-console is retried until it accepts the size: ConPTY can
    /// transiently reject a resize (e.g. while the shell is still starting),
    /// and a resize dropped here desyncs the child's idea of the grid from
    /// ours — output then wraps at the stale width while the UI shows the
    /// new one. `prepaint` calls this every frame, so a failed resize heals
    /// on the next paint.
    pub fn resize(&mut self, cols: usize, rows: usize, cell_width: f32, cell_height: f32) {
        if cols == 0 || rows == 0 {
            return;
        }
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        if self.cols != cols || self.rows != rows {
            self.cols = cols;
            self.rows = rows;
            self.term.resize(TermSize { cols, rows });
        }
        let p_width = (cols as f32 * cell_width).round() as u16;
        let p_height = (rows as f32 * cell_height).round() as u16;
        if self.pty_cols != cols || self.pty_rows != rows {
            let size = PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: p_width,
                pixel_height: p_height,
            };
            if self.master.resize(size).is_ok() {
                self.pty_cols = cols;
                self.pty_rows = rows;
            }
            // On failure the stale pty size stays recorded so the next
            // call retries instead of assuming success.
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    #[allow(dead_code)]
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        if let Some(pid) = self.pid {
            terminate_process_tree(pid);
        }
        trim_working_set();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Killing the child closes the ConPTY; the reader thread then exits
        // and the channel closes, which stops the UI watcher task.
        let _ = self.child.kill();
        if let Some(pid) = self.pid {
            terminate_process_tree(pid);
        }
        trim_working_set();
    }
}
