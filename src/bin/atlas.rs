//! Atlas Satellites CLI Runner & Windows Terminal Launcher
//!
//! Native Windows console subsystem binary for the Atlas suite.
//! - Running `atlas` with no arguments (or a directory path) spawns the GUI terminal in background.
//! - Running `atlas <command>` (e.g. `atlas tools`, `atlas -v`, `atlas port 3000`, `atlas jwt ...`)
//!   runs the satellite companion tool synchronously in the current console, blocking the shell prompt
//!   until completion and returning the exit code cleanly.

#![windows_subsystem = "console"]

use std::env;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

fn resolve_gui_exe() -> Option<PathBuf> {
    if let Ok(current) = env::current_exe() {
        if let Some(dir) = current.parent() {
            let candidate = dir.join("atlas-terminal.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // Fallback: check LOCALAPPDATA standard install location
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        let candidate = PathBuf::from(local)
            .join("Programs")
            .join("Atlas Terminal")
            .join("atlas-terminal.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn main() {
    let target = match resolve_gui_exe() {
        Some(p) => p,
        None => {
            eprintln!("atlas: unable to locate atlas-terminal.exe in installation directory");
            exit(1);
        }
    };

    let args: Vec<String> = env::args().skip(1).collect();

    // Determine if invocation is requesting GUI terminal launch
    let is_gui = if args.is_empty() {
        true
    } else {
        let first = &args[0];
        Path::new(first).is_dir() || first == "." || first == ".."
    };

    if is_gui {
        let mut cmd = Command::new(&target);
        cmd.args(&args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            cmd.creation_flags(DETACHED_PROCESS);
        }
        match cmd.spawn() {
            Ok(_) => exit(0),
            Err(e) => {
                eprintln!("atlas: failed to launch GUI terminal {}: {}", target.display(), e);
                exit(1);
            }
        }
    }

    // CLI satellite tools: run synchronously, inherit standard handles, wait for exit
    let status = match Command::new(&target).args(&args).status() {
        Ok(s) => s.code().unwrap_or(0),
        Err(e) => {
            eprintln!("atlas: failed to execute companion tool {}: {}", target.display(), e);
            1
        }
    };
    exit(status);
}
