//! Terminal clipboard manager & pipeline sink/source.
//!
//! Enables seamless piping to and from system clipboard:
//!   `git status | atlas clip`
//!   `atlas clip paste`
//!   `atlas clip copy "hello"`

use std::io::{self, IsTerminal, Read, Write};

pub fn run(args: &[String]) {
    let stdin = io::stdin();
    let is_piped = !stdin.is_terminal();

    if is_piped && (args.is_empty() || args[0] == "copy" || args[0] == "set") {
        // Read everything from piped stdin
        let mut buffer = String::new();
        match io::stdin().read_to_string(&mut buffer) {
            Ok(bytes_read) => {
                if set_clipboard_text(&buffer) {
                    println!("\x1b[32m✓\x1b[0m Copied {} bytes to clipboard.", bytes_read);
                } else {
                    eprintln!("\x1b[31m[atlas clip]\x1b[0m Failed to set clipboard.");
                }
            }
            Err(e) => {
                eprintln!("\x1b[31m[atlas clip]\x1b[0m Failed to read from stdin: {}", e);
            }
        }
        return;
    }

    if args.is_empty() {
        // Default when run interactively: paste clipboard to stdout
        paste_clipboard();
        return;
    }

    match args[0].to_lowercase().as_str() {
        "-h" | "--help" | "help" => {
            print_help();
        }
        "paste" | "get" | "p" => {
            paste_clipboard();
        }
        "copy" | "set" | "c" => {
            let text = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                let mut buf = String::new();
                let _ = io::stdin().read_to_string(&mut buf);
                buf
            };
            if set_clipboard_text(&text) {
                println!("\x1b[32m✓\x1b[0m Copied {} bytes to clipboard.", text.len());
            } else {
                eprintln!("\x1b[31m[atlas clip]\x1b[0m Failed to set clipboard.");
            }
        }
        "clear" => {
            if clear_clipboard() {
                println!("\x1b[32m✓\x1b[0m Clipboard cleared.");
            } else {
                eprintln!("\x1b[31m[atlas clip]\x1b[0m Failed to clear clipboard.");
            }
        }
        _text => {
            // Treat entire argument list as text to copy
            let full_text = args.join(" ");
            if set_clipboard_text(&full_text) {
                println!("\x1b[32m✓\x1b[0m Copied {} bytes to clipboard.", full_text.len());
            } else {
                eprintln!("\x1b[31m[atlas clip]\x1b[0m Failed to set clipboard.");
            }
        }
    }
}

fn print_help() {
    println!();
    println!("\x1b[1;36m  Atlas Clipboard Manager & Pipeline Sink/Source\x1b[0m");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[1mUSAGE:\x1b[0m");
    println!("    <command> | atlas clip           Copy command output to clipboard");
    println!("    atlas clip paste                 Print clipboard contents to stdout");
    println!("    atlas clip copy <text...>        Copy arguments directly to clipboard");
    println!("    atlas clip clear                 Clear clipboard contents");
    println!();
    println!("  \x1b[1mEXAMPLES:\x1b[0m");
    println!("    git status | atlas clip");
    println!("    atlas hash file.zip | atlas clip");
    println!("    atlas clip paste > notes.txt");
    println!();
}

fn paste_clipboard() {
    if let Some(text) = get_clipboard_text() {
        let _ = io::stdout().write_all(text.as_bytes());
        // Add trailing newline if output was missing one and writing to terminal
        if io::stdout().is_terminal() && !text.ends_with('\n') {
            println!();
        }
    }
}

#[cfg(windows)]
fn set_clipboard_text(text: &str) -> bool {
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
        fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(hMem: *mut c_void) -> i32;
    }

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = utf16.len() * 2;

    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return false;
        }

        EmptyClipboard();

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }

        let locked = GlobalLock(h_mem) as *mut u16;
        if locked.is_null() {
            CloseClipboard();
            return false;
        }

        ptr::copy_nonoverlapping(utf16.as_ptr(), locked, utf16.len());
        GlobalUnlock(h_mem);

        let res = SetClipboardData(CF_UNICODETEXT, h_mem);
        CloseClipboard();

        !res.is_null()
    }
}

#[cfg(windows)]
fn get_clipboard_text() -> Option<String> {
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(uFormat: u32) -> *mut c_void;
        fn IsClipboardFormatAvailable(uFormat: u32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(hMem: *mut c_void) -> i32;
    }

    const CF_UNICODETEXT: u32 = 13;

    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return None;
        }

        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            CloseClipboard();
            return None;
        }

        let h_mem = GetClipboardData(CF_UNICODETEXT);
        if h_mem.is_null() {
            CloseClipboard();
            return None;
        }

        let locked = GlobalLock(h_mem) as *const u16;
        if locked.is_null() {
            CloseClipboard();
            return None;
        }

        let mut len = 0;
        while *locked.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(locked, len);
        let s = String::from_utf16_lossy(slice);

        GlobalUnlock(h_mem);
        CloseClipboard();

        Some(s)
    }
}

#[cfg(windows)]
fn clear_clipboard() -> bool {
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
    }

    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return false;
        }
        let res = EmptyClipboard() != 0;
        CloseClipboard();
        res
    }
}

#[cfg(not(windows))]
fn set_clipboard_text(_text: &str) -> bool {
    false
}

#[cfg(not(windows))]
fn get_clipboard_text() -> Option<String> {
    None
}

#[cfg(not(windows))]
fn clear_clipboard() -> bool {
    false
}
