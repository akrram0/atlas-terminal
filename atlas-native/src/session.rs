//! PTY session: portable-pty + alacritty_terminal.
//!
//! One `Session` per tab. The reader thread owns the PTY read side and pushes
//! bytes into an async channel; the UI task drains it and feeds the terminal
//! state machine. Terminal events (title, exit) travel the same channel.

use std::io::{Read, Write};
use std::path::PathBuf;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtyPair, PtySize};
use smol::channel::{bounded, Receiver, Sender};

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
}

pub struct Session {
    pub term: Term<EventProxy>,
    pub parser: Processor,
    pub cols: usize,
    pub rows: usize,
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
        let pair = pty_system.openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let PtyPair { master, slave } = pair;

        let candidates: &[&str] = match profile {
            Profile::PowerShell => &["pwsh", "powershell.exe"],
            Profile::Cmd => &["cmd.exe"],
            Profile::Wsl => &["wsl.exe"],
            Profile::GitBash => &["bash.exe", "C:\\Program Files\\Git\\bin\\bash.exe"],
        };
        let mut child: Option<Box<dyn Child + Send + Sync>> = None;
        let mut shell_used = *candidates.last().unwrap_or(&"powershell.exe");
        for shell in candidates {
            let mut cmd = CommandBuilder::new(shell);
            match profile {
                Profile::PowerShell => {
                    cmd.arg("-NoLogo");
                    if shell.eq_ignore_ascii_case("powershell.exe") {
                        // Windows PowerShell 5.1 defaults its console to the
                        // OEM code page (CP850…): tools like fastfetch then
                        // report `en_US.CP850` and legacy encodings. Point
                        // .NET — and through it the console — at UTF-8; the
                        // shell stays interactive after the one-shot command.
                        cmd.arg("-NoExit");
                        cmd.arg("-Command");
                        cmd.arg("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8");
                    }
                }
                Profile::Cmd => {}
                Profile::Wsl => {}
                Profile::GitBash => {
                    cmd.arg("--login");
                    cmd.arg("-i");
                }
            }
            // Terminal identity every CLI tool reads for colour, locale and
            // capability detection. Without these, native tools fall back to
            // the Windows console defaults.
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
            cmd.env("LANG", "en_US.UTF-8");
            cmd.env("LC_ALL", "en_US.UTF-8");
            if let Some(dir) = cwd.as_ref() {
                if dir.is_dir() {
                    cmd.cwd(dir.clone());
                }
            }
            match slave.spawn_command(cmd) {
                Ok(spawned) => {
                    child = Some(spawned);
                    shell_used = *shell;
                    break;
                }
                Err(_) => continue,
            }
        }
        let child =
            child.ok_or_else(|| anyhow::anyhow!("no shell found (tried {candidates:?})"))?;
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
                pty_cols: cols,
                pty_rows: rows,
                shell: shell_used.to_string(),
                pid,
                grid_bounds: None,
                writer: Some(writer),
                master,
                child,
            },
            rx,
        ))
    }

    /// Feed PTY bytes into the terminal state machine.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
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
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == 0 || rows == 0 {
            return;
        }
        if self.cols != cols || self.rows != rows {
            self.cols = cols;
            self.rows = rows;
            self.term.resize(TermSize { cols, rows });
        }
        if self.pty_cols != cols || self.pty_rows != rows {
            let size = PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: 0,
                pixel_height: 0,
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
