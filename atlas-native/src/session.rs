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
use smol::channel::{unbounded, Receiver, Sender};

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

pub struct Session {
    pub term: Term<EventProxy>,
    pub parser: Processor,
    pub cols: usize,
    pub rows: usize,
    /// Shell executable that was actually spawned ("pwsh" or
    /// "powershell.exe"), for the sidebar/status-bar labels.
    pub shell: String,
    writer: Option<Box<dyn Write + Send>>,
    master: Box<dyn MasterPty>,
    child: Box<dyn Child + Send + Sync>,
}

impl Session {
    /// Spawn a shell (pwsh, falling back to Windows PowerShell) in a new PTY.
    ///
    /// Returns the session plus the receiver for its output/event stream. The
    /// receiver must be drained (see `AtlasView::watch`), otherwise the UI
    /// never repaints.
    pub fn spawn(
        cols: usize,
        rows: usize,
        cwd: Option<PathBuf>,
    ) -> anyhow::Result<(Session, Receiver<SessionMsg>)> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let PtyPair { master, slave } = pair;

        let mut child: Option<Box<dyn Child + Send + Sync>> = None;
        let mut shell_used = "powershell.exe";
        for shell in ["pwsh", "powershell.exe"] {
            let mut cmd = CommandBuilder::new(shell);
            cmd.arg("-NoLogo");
            if let Some(dir) = cwd.as_ref() {
                if dir.is_dir() {
                    cmd.cwd(dir.clone());
                }
            }
            match slave.spawn_command(cmd) {
                Ok(spawned) => {
                    child = Some(spawned);
                    shell_used = shell;
                    break;
                }
                Err(_) => continue,
            }
        }
        let child =
            child.ok_or_else(|| anyhow::anyhow!("no shell found (tried pwsh, powershell.exe)"))?;
        drop(slave);

        let mut reader = master.try_clone_reader()?;
        let writer = master.take_writer()?;

        let (tx, rx) = unbounded::<SessionMsg>();
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
                shell: shell_used.to_string(),
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
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == 0 || rows == 0 || (self.cols == cols && self.rows == rows) {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        let _ = self.master.resize(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.term.resize(TermSize { cols, rows });
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
