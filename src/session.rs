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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// PowerShell 7 (`pwsh`), falling back to Windows PowerShell.
    PowerShell,
    /// Command Prompt (`cmd.exe`).
    Cmd,
    /// `wsl.exe` — the default distro's login shell via Windows Subsystem for
    /// Linux. The ConPTY path is identical; wsl.exe maps the Windows cwd.
    Wsl,
    /// Git for Windows bash.
    GitBash,
    /// OpenCode CLI session.
    OpenCode,
}

impl Profile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::PowerShell => "powershell",
            Profile::Cmd => "cmd",
            Profile::Wsl => "wsl",
            Profile::GitBash => "gitbash",
            Profile::OpenCode => "opencode",
        }
    }

    pub fn from_name(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "cmd" | "command prompt" => Profile::Cmd,
            "wsl" | "ubuntu" | "arch" | "debian" | "kali" | "fedora" | "linux" => Profile::Wsl,
            "gitbash" | "git bash" | "bash" => Profile::GitBash,
            "opencode" => Profile::OpenCode,
            _ => Profile::PowerShell,
        }
    }
}

/// Absolute candidates for Git for Windows' bash, most preferred first.
///
/// `bash.exe` on PATH is ambiguous: the Microsoft Store WSL alias lives in
/// `%LOCALAPPDATA%\Microsoft\WindowsApps`, so spawning a bare `bash.exe` can
/// silently open a WSL distro instead of Git Bash. Only explicit Git
/// installations are considered here.
fn git_bash_candidates() -> Vec<String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(dir) = std::env::var_os(var) {
            roots.push(PathBuf::from(dir).join("Git"));
        }
    }
    if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(dir).join("Programs").join("Git"));
    }
    // Non-standard installs that expose `git.exe` on PATH: derive the Git
    // root from `<root>\cmd\git.exe`.
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join("git.exe").is_file() {
                if let Some(root) = dir.parent() {
                    roots.push(root.to_path_buf());
                }
            }
        }
    }
    // Standard install location last so environment-derived roots win.
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
    pub shell: String,
    /// PID of the shell process; lets the UI poll its working directory
    /// (see `crate::cwd`).
    pub pid: Option<u32>,
    /// Scrollback limit the terminal was created with; used to prune inline
    /// images whose anchor has scrolled out of the buffer.
    scrollback_limit: usize,
    /// Last painted grid layout: (left, top, cell_width, cell_height).
    pub grid_bounds: Option<(f32, f32, f32, f32)>,
    /// Active inline images anchored to the terminal scrollback grid.
    pub placed_images: Vec<PlacedImage>,
    next_image_id: usize,
    osc_scanner: Osc1337Scanner,
    writer: Option<Box<dyn Write + Send>>,
    master: Box<dyn MasterPty>,
    child: Box<dyn Child + Send + Sync>,
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
            let probes = [
                std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("pnpm").join("bin").join("opencode.cmd")),
                std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("pnpm").join("opencode.cmd")),
                std::env::var_os("APPDATA")
                    .map(|a| PathBuf::from(a).join("npm").join("opencode.cmd")),
                std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("Programs").join("OpenCode").join("opencode.exe")),
                std::env::var_os("USERPROFILE")
                    .map(|u| PathBuf::from(u).join(".cargo").join("bin").join("opencode.exe")),
                std::env::var_os("USERPROFILE")
                    .map(|u| PathBuf::from(u).join(".bun").join("bin").join("opencode.exe")),
                std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("Microsoft").join("WinGet").join("Links").join("opencode.exe")),
                std::env::var_os("USERPROFILE")
                    .map(|u| PathBuf::from(u).join("scoop").join("shims").join("opencode.exe")),
            ];
            for cand in probes.into_iter().flatten() {
                if cand.is_file() {
                    opencode_path = Some(cand.to_string_lossy().to_string());
                    break;
                }
            }
        }

        let candidates_vec: Vec<String> = match profile {
            Profile::PowerShell => vec!["pwsh".into(), "powershell.exe".into()],
            Profile::Cmd => vec!["cmd.exe".into()],
            Profile::Wsl => vec!["wsl.exe".into()],
            // Never fall back to a bare `bash.exe`: on Windows that name is
            // usually the Microsoft Store WSL launcher (WindowsApps), not Git
            // for Windows. Resolve Git's own bash explicitly so the Git Bash
            // profile cannot silently open a WSL distro.
            Profile::GitBash => git_bash_candidates(),
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
        };
        let mut child: Option<Box<dyn Child + Send + Sync>> = None;
        let mut shell_used = candidates_vec.last().cloned().unwrap_or_else(|| "powershell.exe".into());
        for shell in &candidates_vec {
            let mut cmd = CommandBuilder::new(shell);
            match profile {
                Profile::PowerShell => {
                    cmd.arg("-NoLogo");
                    cmd.arg("-NoExit");
                    cmd.arg("-Command");
                    cmd.arg(
                        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; \
                         $OutputEncoding=[System.Text.Encoding]::UTF8; \
                         [Console]::InputEncoding=[System.Text.Encoding]::UTF8; \
                         $atlas_old = (Get-Command prompt -ErrorAction SilentlyContinue).ScriptBlock; \
                         function prompt { \
                             try { [System.Environment]::CurrentDirectory = $ExecutionContext.SessionState.Path.CurrentLocation.Path } catch {}; \
                             if ($atlas_old) { & $atlas_old } else { 'PS ' + (Get-Location) + '> ' } \
                         }; \
                         $opencode_cmd = Join-Path $env:LOCALAPPDATA 'pnpm\\bin\\opencode.cmd'; \
                         $npm_cmd = Join-Path $env:APPDATA 'npm\\opencode.cmd'; \
                         function global:opencode { \
                             [Console]::Write(\"`e]0;OpenCode`a\"); \
                             try { \
                                 if (Test-Path `$opencode_cmd) { & `$opencode_cmd @args } \
                                 elseif (Test-Path `$npm_cmd) { & `$npm_cmd @args } \
                                 else { & (Get-Command opencode.cmd, opencode.ps1, opencode.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1) @args } \
                             } finally { \
                                 [Console]::Write(\"`e]0;PowerShell`a\"); \
                             } \
                         }; \
                         function global:update-opencode { \
                             Write-Host 'Checking for OpenCode updates...' -ForegroundColor Cyan; \
                             if (Get-Command pnpm -ErrorAction SilentlyContinue) { \
                                 pnpm update -g @opencode/cli \
                             } elseif (Get-Command npm -ErrorAction SilentlyContinue) { \
                                 npm update -g @opencode/cli \
                             } else { \
                                 opencode upgrade \
                             } \
                         };"
                    );
                }
                Profile::Cmd => {}
                Profile::Wsl => {}
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
            }
            // Ensure package manager bins (pnpm, npm) are included in PATH so
            // devtools like OpenCode are always discoverable by child shells.
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                let mut added = false;
                if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                    let pnpm_bin = PathBuf::from(local_app_data).join("pnpm").join("bin");
                    if pnpm_bin.is_dir() && !paths.contains(&pnpm_bin) {
                        paths.insert(0, pnpm_bin);
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
                if added {
                    if let Ok(new_path) = std::env::join_paths(paths) {
                        cmd.env("PATH", new_path);
                    }
                }
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
            if matches!(profile, Profile::Wsl) {
                // wsl.exe starts a Linux process from a Windows one; only
                // variables listed in WSLENV are copied into the distro. Without
                // this, fastfetch and other CLIs inside WSL see no terminal
                // identity at all (TERM_PROGRAM is what they report).
                let mut wsl_env = std::env::var("WSLENV").unwrap_or_default();
                for var in ["TERM_PROGRAM", "TERM_PROGRAM_VERSION", "COLORTERM", "TERM"] {
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
                placed_images: Vec::new(),
                next_image_id: 1,
                osc_scanner: Osc1337Scanner::new(),
                writer: Some(writer),
                master,
                child,
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

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Killing the child closes the ConPTY; the reader thread then exits
        // and the channel closes, which stops the UI watcher task.
        let _ = self.child.kill();
    }
}
