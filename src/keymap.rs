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
        "enter" | "return" | "\r" | "\n" | "numpadenter" => vec![b'\r'],
        "backspace" if modifiers.control => vec![0x08], // Ctrl+Backspace: ASCII BS for word deletion
        "backspace" => vec![0x7f],
        "escape" | "esc" => vec![0x1b],
        // gpui never fills `key_char` for the space bar (VK_SPACE lives in its
        // "immutable keys" table), so forward it here — Ctrl+Space stays in
        // the control-chord arm below (0x00).
        "space" if !modifiers.control => vec![b' '],
        "tab" if modifiers.shift => vec![0x1b, b'[', b'Z'], // CBT
        "tab" => vec![b'\t'],
        "up" | "arrowup" => return Some(cursor_key(b'A', modifiers, app_cursor)),
        "down" | "arrowdown" => return Some(cursor_key(b'B', modifiers, app_cursor)),
        "right" | "arrowright" => return Some(cursor_key(b'C', modifiers, app_cursor)),
        "left" | "arrowleft" => return Some(cursor_key(b'D', modifiers, app_cursor)),
        "home" => return Some(cursor_key(b'H', modifiers, app_cursor)),
        "end" => return Some(cursor_key(b'F', modifiers, app_cursor)),
        "pageup" => return Some(tilde_key(b'5', modifiers)),
        "pagedown" => return Some(tilde_key(b'6', modifiers)),
        "insert" => return Some(tilde_key(b'2', modifiers)),
        "delete" => return Some(tilde_key(b'3', modifiers)),
        "f1" => return Some(fn_key(1, modifiers)),
        "f2" => return Some(fn_key(2, modifiers)),
        "f3" => return Some(fn_key(3, modifiers)),
        "f4" => return Some(fn_key(4, modifiers)),
        "f5" => return Some(fn_key(5, modifiers)),
        "f6" => return Some(fn_key(6, modifiers)),
        "f7" => return Some(fn_key(7, modifiers)),
        "f8" => return Some(fn_key(8, modifiers)),
        "f9" => return Some(fn_key(9, modifiers)),
        "f10" => return Some(fn_key(10, modifiers)),
        "f11" => return Some(fn_key(11, modifiers)),
        "f12" => return Some(fn_key(12, modifiers)),
        _ => {
            // Control chords.
            if modifiers.control && !modifiers.shift {
                if key == "space" {
                    return Some(vec![0x00]);
                }
                if key.chars().count() == 1 {
                    let ch = key.chars().next().unwrap();
                    let control = match ch {
                        'a'..='z' | 'A'..='Z' => Some(ch.to_ascii_lowercase() as u8 - b'a' + 1),
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

            // Printable characters (already shift-resolved by the platform,
            // with OS Caps Lock inversion applied where needed).
            match ks.key_char.as_ref() {
                Some(text) if !text.is_empty() => {
                    let adjusted = adjust_for_caps_lock(text, modifiers.shift);
                    adjusted.into_bytes()
                }
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

/// Chords that line-editing shells (PSReadLine in Windows PowerShell 5.1,
/// GNU readline, etc.) bind to text-selection commands. The UI layer must let
/// these reach the PTY instead of consuming them as application shortcuts.
pub(crate) fn is_shell_editing_chord(chord: &str) -> bool {
    matches!(
        chord,
        "ctrl+a" | "ctrl+shift+left" | "ctrl+shift+right" | "shift+home" | "shift+end"
    )
}

/// xterm / ECMA-48 modifier code:
/// 1 = none, 2 = Shift, 3 = Alt, 4 = Shift+Alt, 5 = Ctrl,
/// 6 = Shift+Ctrl, 7 = Alt+Ctrl, 8 = Shift+Alt+Ctrl.
pub fn modifier_param(modifiers: &gpui::Modifiers) -> u8 {
    let mut code = 1u8;
    if modifiers.shift {
        code += 1;
    }
    if modifiers.alt {
        code += 2;
    }
    if modifiers.control {
        code += 4;
    }
    code
}

fn cursor_key(final_byte: u8, modifiers: &gpui::Modifiers, app_cursor: bool) -> Vec<u8> {
    let m = modifier_param(modifiers);
    if m > 1 {
        // xterm / ANSI standard modified cursor key sequence: ESC [ 1 ; {m} {final_byte}
        vec![0x1b, b'[', b'1', b';', b'0' + m, final_byte]
    } else if app_cursor {
        vec![0x1b, b'O', final_byte] // SS3 (DECCKM)
    } else {
        vec![0x1b, b'[', final_byte] // CSI
    }
}

fn tilde_key(number: u8, modifiers: &gpui::Modifiers) -> Vec<u8> {
    let m = modifier_param(modifiers);
    if m > 1 {
        // xterm / ANSI standard modified tilde key: ESC [ {num} ; {m} ~
        vec![0x1b, b'[', number, b';', b'0' + m, b'~']
    } else {
        vec![0x1b, b'[', number, b'~']
    }
}

fn fn_key(num: u8, modifiers: &gpui::Modifiers) -> Vec<u8> {
    let m = modifier_param(modifiers);
    if (1..=4).contains(&num) {
        let final_byte = b'P' + (num - 1);
        if m > 1 {
            vec![0x1b, b'[', b'1', b';', b'0' + m, final_byte]
        } else {
            vec![0x1b, b'O', final_byte]
        }
    } else {
        let code: &[u8] = match num {
            5 => b"15",
            6 => b"17",
            7 => b"18",
            8 => b"19",
            9 => b"20",
            10 => b"21",
            11 => b"23",
            12 => b"24",
            _ => return vec![],
        };
        if m > 1 {
            let mut v = Vec::with_capacity(code.len() + 5);
            v.extend_from_slice(b"\x1b[");
            v.extend_from_slice(code);
            v.push(b';');
            v.push(b'0' + m);
            v.push(b'~');
            v
        } else {
            let mut v = Vec::with_capacity(code.len() + 3);
            v.extend_from_slice(b"\x1b[");
            v.extend_from_slice(code);
            v.push(b'~');
            v
        }
    }
}

#[cfg(windows)]
pub fn is_caps_lock_active() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetKeyState(n_virt_key: i32) -> i16;
    }
    // VK_CAPITAL = 0x14; low-order bit is 1 if toggled on.
    unsafe { (GetKeyState(0x14) & 1) != 0 }
}

#[cfg(not(windows))]
pub fn is_caps_lock_active() -> bool {
    false
}

/// Invert letter case if Caps Lock is active on the operating system.
/// If Shift is held while Caps Lock is active, letters become lowercase.
pub fn adjust_for_caps_lock(text: &str, shift: bool) -> String {
    if !is_caps_lock_active() {
        return text.to_string();
    }
    text.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else if c.is_ascii_uppercase() && shift {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    #[test]
    fn test_escape_to_bytes() {
        let ks = Keystroke {
            modifiers: Modifiers::default(),
            key: "escape".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks, TermMode::empty()), Some(vec![0x1b]));

        let ks_esc = Keystroke {
            modifiers: Modifiers::default(),
            key: "esc".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_esc, TermMode::empty()), Some(vec![0x1b]));
    }

    #[test]
    fn test_enter_variants() {
        for key in ["enter", "return", "\r", "\n", "numpadenter"] {
            let ks = Keystroke {
                modifiers: Modifiers::default(),
                key: key.to_string(),
                key_char: None,
            };
            assert_eq!(to_bytes(&ks, TermMode::empty()), Some(vec![b'\r']));
        }
    }

    #[test]
    fn test_cursor_keys() {
        let ks_up = Keystroke {
            modifiers: Modifiers::default(),
            key: "up".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_up, TermMode::empty()), Some(vec![0x1b, b'[', b'A']));
        assert_eq!(to_bytes(&ks_up, TermMode::APP_CURSOR), Some(vec![0x1b, b'O', b'A']));

        let ks_arrow = Keystroke {
            modifiers: Modifiers::default(),
            key: "arrowup".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_arrow, TermMode::empty()), Some(vec![0x1b, b'[', b'A']));
    }

    #[test]
    fn test_modified_cursor_keys_and_word_nav() {
        // Ctrl+Left (backward-word): \x1b[1;5D
        let mut ctrl = Modifiers::default();
        ctrl.control = true;
        let ks_ctrl_left = Keystroke {
            modifiers: ctrl,
            key: "left".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_ctrl_left, TermMode::empty()), Some(b"\x1b[1;5D".to_vec()));

        // Ctrl+Right (forward-word): \x1b[1;5C
        let ks_ctrl_right = Keystroke {
            modifiers: ctrl,
            key: "arrowright".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_ctrl_right, TermMode::empty()), Some(b"\x1b[1;5C".to_vec()));

        // Ctrl+Up / Down
        let ks_ctrl_up = Keystroke {
            modifiers: ctrl,
            key: "up".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_ctrl_up, TermMode::empty()), Some(b"\x1b[1;5A".to_vec()));

        // Shift+Left: \x1b[1;2D
        let mut shift = Modifiers::default();
        shift.shift = true;
        let ks_shift_left = Keystroke {
            modifiers: shift,
            key: "left".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_shift_left, TermMode::empty()), Some(b"\x1b[1;2D".to_vec()));

        // Ctrl+Shift+Left: \x1b[1;6D
        let mut ctrl_shift = Modifiers::default();
        ctrl_shift.control = true;
        ctrl_shift.shift = true;
        let ks_cs_left = Keystroke {
            modifiers: ctrl_shift,
            key: "left".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_cs_left, TermMode::empty()), Some(b"\x1b[1;6D".to_vec()));

        // Ctrl+Backspace: 0x08 (ASCII BS)
        let ks_bs = Keystroke {
            modifiers: ctrl,
            key: "backspace".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_bs, TermMode::empty()), Some(vec![0x08]));

        // Ctrl+Delete: \x1b[3;5~
        let ks_del = Keystroke {
            modifiers: ctrl,
            key: "delete".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_del, TermMode::empty()), Some(b"\x1b[3;5~".to_vec()));
    }

    #[test]
    fn test_control_chords() {
        let mut ctrl = Modifiers::default();
        ctrl.control = true;
        let ks_c = Keystroke {
            modifiers: ctrl,
            key: "c".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_c, TermMode::empty()), Some(vec![3])); // SIGINT

        let ks_upper_c = Keystroke {
            modifiers: ctrl,
            key: "C".to_string(),
            key_char: None,
        };
        assert_eq!(to_bytes(&ks_upper_c, TermMode::empty()), Some(vec![3]));
    }

    #[test]
    fn test_ctrl_a_forwarded_as_select_all() {
        let mut ctrl = Modifiers::default();
        ctrl.control = true;
        let ks = Keystroke {
            modifiers: ctrl,
            key: "a".to_string(),
            key_char: Some("a".to_string()),
        };
        assert_eq!(to_bytes(&ks, TermMode::empty()), Some(vec![0x01]));
    }

    #[test]
    fn test_ctrl_shift_right_forwarded_as_select_next_word() {
        let mut ctrl_shift = Modifiers::default();
        ctrl_shift.control = true;
        ctrl_shift.shift = true;
        let ks = Keystroke {
            modifiers: ctrl_shift,
            key: "right".to_string(),
            key_char: None,
        };
        assert_eq!(
            to_bytes(&ks, TermMode::empty()),
            Some(b"\x1b[1;6C".to_vec())
        );
    }

    #[test]
    fn test_shift_home_end_forwarded() {
        let mut shift = Modifiers::default();
        shift.shift = true;
        let home = Keystroke {
            modifiers: shift,
            key: "home".to_string(),
            key_char: None,
        };
        let end = Keystroke {
            modifiers: shift,
            key: "end".to_string(),
            key_char: None,
        };
        assert_eq!(
            to_bytes(&home, TermMode::empty()),
            Some(b"\x1b[1;2H".to_vec())
        );
        assert_eq!(
            to_bytes(&end, TermMode::empty()),
            Some(b"\x1b[1;2F".to_vec())
        );
    }

    #[test]
    fn test_shell_editing_chords_classification() {
        // These chords bind to PSReadLine line-selection functions and must reach the shell.
        for chord in [
            "ctrl+a",
            "ctrl+shift+left",
            "ctrl+shift+right",
            "shift+home",
            "shift+end",
        ] {
            assert!(
                is_shell_editing_chord(chord),
                "{chord} must not be consumed by the UI"
            );
        }
        // Application shortcuts and shell chords that already pass through.
        for chord in [
            "ctrl+shift+up",
            "ctrl+shift+down",
            "ctrl+home",
            "ctrl+end",
            "shift+pageup",
        ] {
            assert!(
                !is_shell_editing_chord(chord),
                "{chord} is not a shell editing chord"
            );
        }
    }
}
