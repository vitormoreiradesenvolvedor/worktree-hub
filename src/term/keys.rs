//! egui key events → xterm byte sequences.

use egui::{Key, Modifiers};

/// xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl.
fn modifier_param(mods: Modifiers) -> u8 {
    1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.ctrl)
}

/// `ESC [ 1 ; m <final>` when modified, `ESC [ <final>` (or `ESC O <final>` in
/// application-cursor mode) otherwise.
fn csi_letter(final_byte: u8, mods: Modifiers, app_cursor: bool) -> Vec<u8> {
    let m = modifier_param(mods);
    if m > 1 {
        format!("\x1b[1;{m}{}", final_byte as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

/// `ESC [ <n> ; m ~` when modified, `ESC [ <n> ~` otherwise.
fn csi_tilde(number: u8, mods: Modifiers) -> Vec<u8> {
    let m = modifier_param(mods);
    if m > 1 {
        format!("\x1b[{number};{m}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

/// Bytes to send for a key press, or `None` when the key produces nothing
/// (egui delivers ordinary characters as `Event::Text` instead).
pub fn encode(key: Key, mods: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    // Ctrl+<key> control codes.
    if mods.ctrl && !mods.alt {
        if let Some(byte) = control_code(key) {
            return Some(vec![byte]);
        }
    }

    let seq = match key {
        Key::Enter => {
            if mods.alt {
                vec![0x1b, b'\r']
            } else {
                vec![b'\r']
            }
        }
        Key::Tab => {
            if mods.shift {
                b"\x1b[Z".to_vec()
            } else {
                vec![b'\t']
            }
        }
        Key::Backspace => {
            if mods.alt {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f]
            }
        }
        Key::Escape => vec![0x1b],

        Key::ArrowUp => csi_letter(b'A', mods, app_cursor),
        Key::ArrowDown => csi_letter(b'B', mods, app_cursor),
        Key::ArrowRight => csi_letter(b'C', mods, app_cursor),
        Key::ArrowLeft => csi_letter(b'D', mods, app_cursor),
        Key::Home => csi_letter(b'H', mods, app_cursor),
        Key::End => csi_letter(b'F', mods, app_cursor),

        Key::Insert => csi_tilde(2, mods),
        Key::Delete => csi_tilde(3, mods),
        Key::PageUp => csi_tilde(5, mods),
        Key::PageDown => csi_tilde(6, mods),

        Key::F1 => ss3_or_csi(b'P', 11, mods),
        Key::F2 => ss3_or_csi(b'Q', 12, mods),
        Key::F3 => ss3_or_csi(b'R', 13, mods),
        Key::F4 => ss3_or_csi(b'S', 14, mods),
        Key::F5 => csi_tilde(15, mods),
        Key::F6 => csi_tilde(17, mods),
        Key::F7 => csi_tilde(18, mods),
        Key::F8 => csi_tilde(19, mods),
        Key::F9 => csi_tilde(20, mods),
        Key::F10 => csi_tilde(21, mods),
        Key::F11 => csi_tilde(23, mods),
        Key::F12 => csi_tilde(24, mods),

        // Alt+<letter/digit> is the only other combination egui will not deliver
        // as text; everything else falls through to `Event::Text`.
        other => {
            if mods.alt && !mods.ctrl {
                let ch = plain_char(other)?;
                let ch = if mods.shift {
                    ch.to_ascii_uppercase()
                } else {
                    ch
                };
                vec![0x1b, ch as u8]
            } else {
                return None;
            }
        }
    };

    Some(seq)
}

fn ss3_or_csi(final_byte: u8, tilde_number: u8, mods: Modifiers) -> Vec<u8> {
    let m = modifier_param(mods);
    if m > 1 {
        format!("\x1b[{tilde_number};{m}~").into_bytes()
    } else {
        vec![0x1b, b'O', final_byte]
    }
}

/// Control codes for Ctrl+<key>.
fn control_code(key: Key) -> Option<u8> {
    let byte = match key {
        Key::Space => 0x00,
        Key::A => 0x01,
        Key::B => 0x02,
        Key::C => 0x03,
        Key::D => 0x04,
        Key::E => 0x05,
        Key::F => 0x06,
        Key::G => 0x07,
        Key::H => 0x08,
        Key::I => 0x09,
        Key::J => 0x0A,
        Key::K => 0x0B,
        Key::L => 0x0C,
        Key::M => 0x0D,
        Key::N => 0x0E,
        Key::O => 0x0F,
        Key::P => 0x10,
        Key::Q => 0x11,
        Key::R => 0x12,
        Key::S => 0x13,
        Key::T => 0x14,
        Key::U => 0x15,
        Key::V => 0x16,
        Key::W => 0x17,
        Key::X => 0x18,
        Key::Y => 0x19,
        Key::Z => 0x1A,
        Key::OpenBracket => 0x1B,
        Key::Backslash => 0x1C,
        Key::CloseBracket => 0x1D,
        Key::Backtick => 0x1E,
        Key::Minus => 0x1F,
        Key::Questionmark => 0x7F,
        _ => return None,
    };
    Some(byte)
}

fn plain_char(key: Key) -> Option<char> {
    let ch = match key {
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        Key::Z => 'z',
        Key::Num0 => '0',
        Key::Num1 => '1',
        Key::Num2 => '2',
        Key::Num3 => '3',
        Key::Num4 => '4',
        Key::Num5 => '5',
        Key::Num6 => '6',
        Key::Num7 => '7',
        Key::Num8 => '8',
        Key::Num9 => '9',
        Key::Period => '.',
        Key::Comma => ',',
        Key::Slash => '/',
        Key::Minus => '-',
        Key::Equals => '=',
        _ => return None,
    };
    Some(ch)
}

/// Wraps pasted text in bracketed-paste markers when the app asked for them.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    // Strip the sequences a hostile clipboard could use to end the paste early.
    let cleaned = text.replace("\x1b[201~", "").replace('\r', "\n");
    let cleaned = cleaned.replace('\n', "\r");
    if bracketed {
        let mut out = b"\x1b[200~".to_vec();
        out.extend_from_slice(cleaned.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        cleaned.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers::NONE;

    #[test]
    fn plain_arrows_use_csi() {
        assert_eq!(encode(Key::ArrowUp, NONE, false).unwrap(), b"\x1b[A");
        assert_eq!(encode(Key::ArrowLeft, NONE, false).unwrap(), b"\x1b[D");
    }

    #[test]
    fn application_cursor_mode_uses_ss3() {
        assert_eq!(encode(Key::ArrowUp, NONE, true).unwrap(), b"\x1bOA");
    }

    #[test]
    fn modified_arrows_carry_the_modifier_param() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode(Key::ArrowRight, ctrl, true).unwrap(), b"\x1b[1;5C");
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode(Key::ArrowUp, shift, false).unwrap(), b"\x1b[1;2A");
    }

    #[test]
    fn ctrl_letters_become_control_codes() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode(Key::C, ctrl, false).unwrap(), vec![0x03]);
        assert_eq!(encode(Key::D, ctrl, false).unwrap(), vec![0x04]);
        assert_eq!(encode(Key::Space, ctrl, false).unwrap(), vec![0x00]);
    }

    #[test]
    fn enter_backspace_and_tab() {
        assert_eq!(encode(Key::Enter, NONE, false).unwrap(), b"\r");
        assert_eq!(encode(Key::Backspace, NONE, false).unwrap(), vec![0x7f]);
        assert_eq!(encode(Key::Tab, NONE, false).unwrap(), b"\t");
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode(Key::Tab, shift, false).unwrap(), b"\x1b[Z");
    }

    #[test]
    fn function_keys() {
        assert_eq!(encode(Key::F1, NONE, false).unwrap(), b"\x1bOP");
        assert_eq!(encode(Key::F5, NONE, false).unwrap(), b"\x1b[15~");
        assert_eq!(encode(Key::F12, NONE, false).unwrap(), b"\x1b[24~");
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(encode(Key::Delete, NONE, false).unwrap(), b"\x1b[3~");
        assert_eq!(encode(Key::PageUp, NONE, false).unwrap(), b"\x1b[5~");
        assert_eq!(encode(Key::Home, NONE, false).unwrap(), b"\x1b[H");
    }

    #[test]
    fn alt_letter_sends_escape_prefix() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode(Key::B, alt, false).unwrap(), vec![0x1b, b'b']);
    }

    #[test]
    fn plain_letters_are_left_to_text_events() {
        assert!(encode(Key::B, NONE, false).is_none());
    }

    #[test]
    fn paste_normalizes_newlines_and_can_bracket() {
        assert_eq!(paste("a\nb", false), b"a\rb".to_vec());
        let bracketed = paste("x", true);
        assert!(bracketed.starts_with(b"\x1b[200~"));
        assert!(bracketed.ends_with(b"\x1b[201~"));
    }

    #[test]
    fn paste_strips_end_marker_injection() {
        let out = paste("safe\x1b[201~rm -rf /", false);
        assert_eq!(out, b"saferm -rf /".to_vec());
    }
}
