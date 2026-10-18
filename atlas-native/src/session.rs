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
use alacritty_terminal::term::{Config, Term};
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
                    .map(|l| PathBuf::from(l).join("pnpm").join("bin").join("opencode.exe")),
                std::env::var_os("USERPROFILE")
                    .map(|u| PathBuf::from(u).join("AppData").join("Local").join("pnpm").join("bin").join("opencode.exe")),
                std::env::var_os("APPDATA")
                    .map(|a| PathBuf::from(a).join("npm").join("opencode.cmd")),
                std::env::var_os("USERPROFILE")
                    .map(|u| PathBuf::from(u).join(".cargo").join("bin").join("opencode.exe")),
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
            Profile::GitBash => vec!["bash.exe".into(), "C:\\Program Files\\Git\\bin\\bash.exe".into()],
            Profile::OpenCode => {
                let mut v = Vec::new();
                if let Some(ref path) = opencode_path {
                    v.push(path.clone());
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
                         $atlas_old = (Get-Command prompt -ErrorAction SilentlyContinue).ScriptBlock; \
                         function prompt { \
                             try { [System.Environment]::CurrentDirectory = $ExecutionContext.SessionState.Path.CurrentLocation.Path } catch {}; \
                             if ($atlas_old) { & $atlas_old } else { 'PS ' + (Get-Location) + '> ' } \
                         }; \
                         $opencode_bin = Join-Path $env:LOCALAPPDATA 'pnpm\\bin\\opencode.exe'; \
                         if (Test-Path $opencode_bin) { \
                             function global:opencode { & (Join-Path $env:LOCALAPPDATA 'pnpm\\bin\\opencode.exe') @args } \
                         } elseif (Get-Command opencode.cmd -ErrorAction SilentlyContinue) { \
                             function global:opencode { & (Get-Command opencode.cmd).Source @args } \
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
                        cmd.arg("opencode");
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
        let pid = child.process_id();
        drop(slave);

        let mut reader = master.try_clone_reader()?;
        let writer = master.take_writer()?;

        let (tx, rx) = bounded::<SessionMsg>(512);
        let proxy = EventProxy { tx: tx.clone() };
        let term = Term::new(
            Config {
                scrolling_history: 10_000,
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
                    // Clear placed images on full screen/scrollback reset (ED 3: \x1b[3J or RIS: \x1bc)
                    if clean_bytes.windows(4).any(|w| w == b"\x1b[3J")
                        || clean_bytes.windows(2).any(|w| w == b"\x1bc")
                    {
                        self.placed_images.clear();
                    } else if clean_bytes.windows(4).any(|w| w == b"\x1b[2J") {
                        // Clear images on the active screen for ED 2 (e.g. `cls` or `clear`)
                        let cur_history = self.term.history_size() as i32;
                        self.placed_images.retain(|img| img.anchor_line < cur_history);
                    }

                    if !clean_bytes.is_empty() {
                        self.parser.advance(&mut self.term, &clean_bytes);
                    }
                }
                OscScannerEvent::Image(decoded) => {
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
                    let history = self.term.history_size() as i32;
                    let anchor_line = history + cursor_line;

                    let id = self.next_image_id;
                    self.next_image_id += 1;

                    self.placed_images.push(PlacedImage {
                        id,
                        image: decoded.render_image,
                        anchor_line,
                        col: cursor_col,
                        cols: img_cols,
                        rows: img_rows,
                    });

                    // Advance cursor below the image so that shell prompt, command output,
                    // and scrollback wrap naturally underneath the image, matching iTerm2 behavior.
                    if !decoded.params.do_not_move_cursor {
                        let mut advance_bytes = Vec::with_capacity(img_rows + 16);
                        for _ in 0..img_rows {
                            advance_bytes.push(b'\n');
                        }
                        // Advance column to cursor_col + img_cols so Device Status Report
                        // CPR queries (\e[6n) report the true image dimensions to fastfetch.
                        let target_col = (cursor_col + img_cols).min(self.cols.saturating_sub(1));
                        let col_seq = format!("\x1b[{}G", target_col + 1);
                        advance_bytes.extend_from_slice(col_seq.as_bytes());

                        self.parser.advance(&mut self.term, &advance_bytes);
                    }
                }
            }
        }

        // Prune images whose anchor line has scrolled past the maximum scrollback limit (10,000 lines)
        let cur_history = self.term.history_size() as i32;
        self.placed_images.retain(|img| {
            img.anchor_line >= cur_history - 10_000
        });
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
