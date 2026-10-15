//! Keystroke -> PTY bytes.
//!
//! Pure function; no gpui state beyond the `Keystroke` value. The view calls
//! this after handling app-level shortcuts, so anything returned here goes
//! straight to the shell.

use alacritty_terminal::term::TermMode;
use gpui::Keystroke;

/// Translate a gpui keystroke into the byte sequence a terminal expects.
///
/// Returns `None` for keys we deliberately don't forward (Win key, reserved
/// Ctrl+Shift chords handled by the UI, unknown keys).
pub fn to_bytes(ks: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let modifiers = &ks.modifiers;
    if modifiers.platform {
        return None; // Windows key — leave it to the OS.
    }

    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let key = ks.key.as_str();

    let bytes = match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "escape" => vec![0x1b],
        "tab" if modifiers.shift => vec![0x1b, b'[', b'Z'], // CBT
        "tab" => vec![b'\t'],
        "up" => cursor_key(b'A', app_cursor),
        "down" => cursor_key(b'B', app_cursor),
        "right" => cursor_key(b'C', app_cursor),
        "left" => cursor_key(b'D', app_cursor),
        "home" => cursor_key(b'H', app_cursor),
        "end" => cursor_key(b'F', app_cursor),
        "pageup" => vec![0x1b, b'[', b'5', b'~'],
        "pagedown" => vec![0x1b, b'[', b'6', b'~'],
        "insert" => vec![0x1b, b'[', b'2', b'~'],
        "delete" => vec![0x1b, b'[', b'3', b'~'],
        "f1" => vec![0x1b, b'O', b'P'],
        "f2" => vec![0x1b, b'O', b'Q'],
        "f3" => vec![0x1b, b'O', b'R'],
        "f4" => vec![0x1b, b'O', b'S'],
        "f5" => vec![0x1b, b'[', b'1', b'5', b'~'],
        "f6" => vec![0x1b, b'[', b'1', b'7', b'~'],
        "f7" => vec![0x1b, b'[', b'1', b'8', b'~'],
        "f8" => vec![0x1b, b'[', b'1', b'9', b'~'],
        "f9" => vec![0x1b, b'[', b'2', b'0', b'~'],
        "f10" => vec![0x1b, b'[', b'2', b'1', b'~'],
        "f11" => vec![0x1b, b'[', b'2', b'3', b'~'],
        "f12" => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => {
            // Control chords.
            if modifiers.control && !modifiers.shift {
                if key == "space" {
                    return Some(vec![0x00]);
                }
                if key.chars().count() == 1 {
                    let ch = key.chars().next().unwrap();
                    let control = match ch {
                        'a'..='z' => Some(ch as u8 - b'a' + 1),
                        '@' | '`' => Some(0x00),
                        '[' => Some(0x1b),
                        '\\' => Some(0x1c),
                        ']' => Some(0x1d),
                        '^' => Some(0x1e),
                        '_' => Some(0x1f),
                        _ => None,
                    };
                    if let Some(control) = control {
                        return Some(vec![control]);
                    }
                }
            }

            // Reserved for the UI layer (tabs, paste, ...).
            if modifiers.control && modifiers.shift {
                return None;
            }

            if modifiers.control {
                return None;
            }

            // Printable characters (already shift-resolved by the platform).
            match ks.key_char.as_ref() {
                Some(text) if !text.is_empty() => text.as_bytes().to_vec(),
                _ => return None,
            }
        }
    };

    // Alt = Meta: prefix with ESC.
    if modifiers.alt {
        let mut out = Vec::with_capacity(bytes.len() + 1);
        out.push(0x1b);
        out.extend_from_slice(&bytes);
        return Some(out);
    }

    Some(bytes)
}

fn cursor_key(final_byte: u8, app_cursor: bool) -> Vec<u8> {
    if app_cursor {
        vec![0x1b, b'O', final_byte] // SS3 (DECCKM)
    } else {
        vec![0x1b, b'[', final_byte] // CSI
    }
}
