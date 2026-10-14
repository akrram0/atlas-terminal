use portable_pty::{native_pty_system, CommandBuilder, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::prompt::get_prompt_script;

pub struct PtySession {
    writer: Box<dyn Write + Send>,
    // We keep the pair and child alive so the PTY isn't dropped
    _pair: portable_pty::PtyPair,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn spawn(
        &self,
        app_handle: &AppHandle,
        cols: u16,
        rows: u16,
    ) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.args(["-NoLogo", "-NoExit"]);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn shell: {}", e))?;

        let mut writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

        // Initialize custom prompt cleanly into the PTY stream without suspicious CLI flags
        let init_script = format!("{}\r\n", get_prompt_script());
        let _ = writer.write_all(init_script.as_bytes());
        let _ = writer.flush();

        // Spawn reader thread
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to get PTY reader: {}", e))?;

        let session = PtySession {
            writer,
            _pair: pair,
            _child: child,
        };

        self.sessions
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .insert(session_id.clone(), session);

        let handle = app_handle.clone();
        let sid = session_id.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = handle.emit(&format!("pty-data-{}", sid), data);
                    }
                    Err(_) => break,
                }
            }
            let _ = handle.emit(&format!("pty-exit-{}", sid), 0);
            // Auto-cleanup: drop writer/pair/child so no powershell.exe leaks
            // when the shell exits on its own (e.g. user typed `exit`).
            if let Some(manager) = handle.try_state::<Arc<PtyManager>>() {
                if let Ok(mut sessions) = manager.sessions.lock() {
                    if let Some(mut session) = sessions.remove(&sid) {
                        let _ = session._child.kill();
                    }
                }
            }
        });

        Ok(session_id)
    }

    pub fn close(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if let Some(mut session) = sessions.remove(session_id) {
            // Kill the child so no powershell.exe leaks; ignore errors
            // when the process already exited (auto-cleanup path).
            let _ = session._child.kill();
        }
        // Idempotent: closing an already-cleaned session is Ok so React
        // unmount-after-exit doesn't spam errors.
        Ok(())
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if let Some(session) = sessions.get_mut(session_id) {
            session
                .writer
                .write_all(data.as_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
            session
                .writer
                .flush()
                .map_err(|e| format!("Flush error: {}", e))?;
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if let Some(session) = sessions.get(session_id) {
            session
                ._pair
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Resize error: {}", e))?;
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }
}
