//! Atlas Satellites: Subterminal companion tools suite.
//!
//! Inspired by Kitty's "kittens", Atlas provides embedded zero-overhead companion
//! tools for system diagnostics, developer workflows, and visual terminal extensions.

use std::io::Write;

pub mod dev;
pub mod system;
pub mod visual;

/// Attempt to intercept and execute a companion CLI tool if arguments match.
/// Returns `true` if a tool was executed and the main GUI app should exit immediately.
pub fn maybe_handle_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return false;
    }

    let first = &args[0];

    // Check if the argument is a path or existing directory (which launches the GUI terminal in that dir)
    if std::path::Path::new(first).is_dir() {
        return false;
    }

    // Check known commands
    let is_tool_cmd = match first.as_str() {
        // Global flags
        "-v" | "--version" | "-h" | "--help" | "help" | "tools" | "satellites" => true,
        // System domain
        "version" | "info" | "env" | "jump" | "z" | "top" | "proc" | "theme" | "config" | "pixel" | "pixelart" | "hud" | "island" | "bench" | "test-render" => true,
        // Dev domain
        "port" | "ports" | "serve" | "http" | "hash" | "crypto" | "clip" | "copy" | "jwt" | "qr" | "json" | "jq" | "diff" => true,
        // Visual domain
        "img" | "image" | "view" | "pick" | "hints" | "ai" | "ask" | "fix" => true,
        _ => false,
    };

    if !is_tool_cmd {
        if std::path::Path::new(first).exists() || first == "." || first == ".." {
            return false;
        }

        attach_parent_console();
        eprintln!();
        eprintln!("  \x1b[1;31matlas:\x1b[0m '{}' is not a registered satellite tool.", first);
        eprintln!("  Even our deep-space radar couldn't find that one.");
        eprintln!();

        let known_tools = [
            "jump", "z", "serve", "http", "hash", "crypto", "clip", "copy",
            "top", "proc", "port", "jwt", "qr", "json", "jq", "diff",
            "img", "view", "pick", "ai", "version", "info", "theme", "config", "pixel", "hud", "bench"
        ];
        let mut close_matches: Vec<(&str, usize)> = known_tools
            .iter()
            .map(|t| (*t, levenshtein_distance(first, t)))
            .filter(|(_, dist)| *dist <= 3)
            .collect();
        close_matches.sort_by_key(|(_, d)| *d);

        if !close_matches.is_empty() {
            eprintln!("  \x1b[1;33mDid you mean:\x1b[0m");
            for (m, _) in close_matches.iter().take(3) {
                eprintln!("    • \x1b[1;36matlas {}\x1b[0m", m);
            }
            eprintln!();
        }

        eprintln!("  Run \x1b[1;36matlas -h\x1b[0m to view the full constellation of tools.");
        eprintln!();
        let _ = std::io::stderr().flush();
        std::process::exit(1);
    }

    // Attach to parent console and configure standard handles
    attach_parent_console();

    let subargs = if args.len() > 1 { &args[1..] } else { &[] };

    let handled = match first.as_str() {
        "-v" | "--version" | "version" => {
            system::version::run();
            true
        }
        "-h" | "--help" | "help" | "tools" | "satellites" => {
            print_tools_help();
            true
        }
        cmd => {
            if system::dispatch(cmd, subargs) {
                true
            } else if dev::dispatch(cmd, subargs) {
                true
            } else if visual::dispatch(cmd, subargs) {
                true
            } else {
                print_tools_help();
                true
            }
        }
    };

    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    handled
}

pub fn print_tools_help() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("\x1b[1;36m  Atlas Satellites - Subterminal Tools Suite (v{})\x1b[0m", version);
    println!("\x1b[2m  ----------------------------------------------------------\x1b[0m");
    println!("  \x1b[1mUSAGE:\x1b[0m");
    println!("    atlas <command> [arguments...]");
    println!("    atlas-terminal <command> [arguments...]");
    println!();
    println!("  \x1b[1;33mSYSTEM TOOLS:\x1b[0m");
    println!("    \x1b[1;36mversion, -v\x1b[0m       Rich system, GPU & runtime specification card");
    println!("    \x1b[1;36minfo, env\x1b[0m         Toolchain, detected shells & PTY environment audit");
    println!("    \x1b[1;36mjump, z\x1b[0m           Smart directory memory & ghost-text autosuggestion engine");
    println!("    \x1b[1;36mtop, proc\x1b[0m         Ultra-lightweight inline system memory & process snapshot");
    println!("    \x1b[1;36mtheme\x1b[0m             Inspect, list, and switch UI themes");
    println!("    \x1b[1;36mconfig\x1b[0m            Configuration getter, setter & direct editor");
    println!("    \x1b[1;36mpixel\x1b[0m             Top bar animated companion presets and kinematics");
    println!("    \x1b[1;36mhud, island\x1b[0m       Dynamic Island notification & status pulse trigger");
    println!("    \x1b[1;36mbench\x1b[0m             PTY throughput & 24-bit truecolor render bench");
    println!();
    println!("  \x1b[1;32mDEVELOPER PRODUCTIVITY TOOLS:\x1b[0m");
    println!("    \x1b[1;36mserve, http\x1b[0m       Instant zero-dependency static HTTP file server");
    println!("    \x1b[1;36mhash, crypto\x1b[0m      Fast SHA-256/MD5 checksums, Base64 & URL encoder/decoder");
    println!("    \x1b[1;36mclip, copy\x1b[0m        Terminal clipboard manager & pipe sink/source");
    println!("    \x1b[1;36mport\x1b[0m              Active listening port inspector & 1-shot process killer");
    println!("    \x1b[1;36mjwt\x1b[0m               Offline, privacy-safe JSON Web Token inspector");
    println!("    \x1b[1;36mqr\x1b[0m                Terminal QR code generator for URLs and auth secrets");
    println!("    \x1b[1;36mjson, jq\x1b[0m          Colorized JSON formatter and key query selector");
    println!("    \x1b[1;36mdiff\x1b[0m              Terminal diff viewer with line numbers and syntax");
    println!();
    println!("  \x1b[1;35mVISUAL SATELLITES:\x1b[0m");
    println!("    \x1b[1;36mimg, view\x1b[0m         Inline image viewer (OSC 1337 GPU texture & 24-bit halfblocks)");
    println!("    \x1b[1;36mpick, hints\x1b[0m       Screen token extractor (URLs, Git SHAs, paths, IPs)");
    println!("    \x1b[1;36mai\x1b[0m                Terminal AI companion (atlas ai ask, fix, explain)");
    println!();
}

#[cfg(windows)]
fn attach_parent_console() {
    unsafe {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn GetStdHandle(nStdHandle: u32) -> *mut core::ffi::c_void;
            fn SetStdHandle(nStdHandle: u32, hHandle: *mut core::ffi::c_void) -> i32;
            fn CreateFileW(
                lpFileName: *const u16,
                dwDesiredAccess: u32,
                dwShareMode: u32,
                lpSecurityAttributes: *mut core::ffi::c_void,
                dwCreationDisposition: u32,
                dwFlagsAndAttributes: u32,
                hTemplateFile: *mut core::ffi::c_void,
            ) -> *mut core::ffi::c_void;
            fn GetConsoleMode(hConsoleHandle: *mut core::ffi::c_void, lpMode: *mut u32) -> i32;
            fn SetConsoleMode(hConsoleHandle: *mut core::ffi::c_void, dwMode: u32) -> i32;
            fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
            fn SetConsoleCP(wCodePageID: u32) -> i32;
        }

        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
        const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;
        const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;

        const GENERIC_READ: u32 = 0x80000000;
        const GENERIC_WRITE: u32 = 0x40000000;
        const FILE_SHARE_READ: u32 = 0x00000001;
        const FILE_SHARE_WRITE: u32 = 0x00000002;
        const OPEN_EXISTING: u32 = 3;
        const INVALID_HANDLE_VALUE: *mut core::ffi::c_void = -1isize as *mut core::ffi::c_void;
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

        let existing_stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        let stdout_is_pipe = !existing_stdout.is_null() && existing_stdout != INVALID_HANDLE_VALUE;

        // Try to attach to the parent console if invoked from terminal
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        let _ = SetConsoleOutputCP(65001);
        let _ = SetConsoleCP(65001);

        if !stdout_is_pipe {
            let conout_name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
            let conout = CreateFileW(
                conout_name.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );

            if !conout.is_null() && conout != INVALID_HANDLE_VALUE {
                SetStdHandle(STD_OUTPUT_HANDLE, conout);
                SetStdHandle(STD_ERROR_HANDLE, conout);

                let mut mode: u32 = 0;
                if GetConsoleMode(conout, &mut mode) != 0 {
                    SetConsoleMode(conout, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }

            let conin_name: Vec<u16> = "CONIN$\0".encode_utf16().collect();
            let conin = CreateFileW(
                conin_name.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if !conin.is_null() && conin != INVALID_HANDLE_VALUE {
                SetStdHandle(STD_INPUT_HANDLE, conin);
            }
        } else {
            let mut mode: u32 = 0;
            if GetConsoleMode(existing_stdout, &mut mode) != 0 {
                SetConsoleMode(existing_stdout, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_tools_help_does_not_panic() {
        print_tools_help();
    }

    #[test]
    fn test_system_dispatch_version() {
        assert!(system::dispatch("version", &[]));
        assert!(system::dispatch("info", &[]));
        assert!(system::dispatch("jump", &["--list".to_string()]));
        assert!(system::dispatch("top", &["5".to_string()]));
        assert!(system::dispatch("theme", &["list".to_string()]));
        assert!(system::dispatch("config", &["path".to_string()]));
        assert!(system::dispatch("bench", &[]));
    }

    #[test]
    fn test_dev_dispatch_help() {
        assert!(dev::dispatch("port", &["--help".to_string()]));
        assert!(dev::dispatch("hash", &["--help".to_string()]));
        assert!(dev::dispatch("clip", &["--help".to_string()]));
        assert!(dev::dispatch("jwt", &["--help".to_string()]));
        assert!(dev::dispatch("qr", &["--help".to_string()]));
        assert!(dev::dispatch("json", &["--help".to_string()]));
        assert!(dev::dispatch("diff", &["--help".to_string()]));
    }

    #[test]
    fn test_visual_dispatch_help() {
        assert!(visual::dispatch("img", &["--help".to_string()]));
        assert!(visual::dispatch("pick", &["--help".to_string()]));
        assert!(visual::dispatch("ai", &["--help".to_string()]));
    }
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1].to_ascii_lowercase() == b_chars[j - 1].to_ascii_lowercase() { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}
