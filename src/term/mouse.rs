//! Mouse reporting: what a full-screen program hears when the pointer moves
//! over its window.
//!
//! Programs like `claude`, `htop` or `vim` ask the terminal to forward clicks,
//! drags and the wheel (`CSI ? 1000 h` and friends). Until the terminal does
//! that, the wheel only moves our own scrollback and a click on something the
//! program drew — a close button, the handle of a column — never reaches it.

use alacritty_terminal::term::TermMode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Middle,
    Right,
    /// The pointer moving with nothing held down.
    None,
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Press,
    Release,
    /// The pointer moved to another cell, with or without a button held.
    Motion,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// True while a program wants the mouse for itself. Holding Shift is the usual
/// way to take it back for selecting text, so the caller checks that too.
pub fn wanted(mode: TermMode) -> bool {
    mode.intersects(TermMode::MOUSE_MODE)
}

/// The bytes for one event, or `None` when this mode does not report it.
///
/// `col` and `row` are 1-based, counted from the top left of the screen.
pub fn report(
    mode: TermMode,
    button: Button,
    kind: Kind,
    col: usize,
    row: usize,
    mods: Mods,
) -> Option<Vec<u8>> {
    if !wanted(mode) {
        return None;
    }
    let wheel = matches!(button, Button::WheelUp | Button::WheelDown);
    match kind {
        // A wheel notch is only ever a press.
        Kind::Release if wheel => return None,
        Kind::Motion if wheel => return None,
        Kind::Motion => {
            // A drag is reported in both motion modes; a bare move only in the
            // one that asked for every move.
            let wanted = if button == Button::None {
                TermMode::MOUSE_MOTION
            } else {
                TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG
            };
            if !mode.intersects(wanted) {
                return None;
            }
        }
        Kind::Press | Kind::Release if button == Button::None => return None,
        _ => {}
    }

    let mut code = match button {
        Button::Left => 0,
        Button::Middle => 1,
        Button::Right => 2,
        Button::None => 3,
        Button::WheelUp => 64,
        Button::WheelDown => 65,
    };
    if kind == Kind::Motion {
        code += 32;
    }
    if mods.shift {
        code += 4;
    }
    if mods.alt {
        code += 8;
    }
    if mods.ctrl {
        code += 16;
    }

    if mode.contains(TermMode::SGR_MOUSE) {
        let end = if kind == Kind::Release { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{col};{row}{end}").into_bytes());
    }

    // The original encoding has one byte per field, so anything past column or
    // row 223 cannot be said at all — and a release says only "some button".
    if col > 223 || row > 223 {
        return None;
    }
    if kind == Kind::Release {
        code = 3;
        if mods.shift {
            code += 4;
        }
        if mods.alt {
            code += 8;
        }
        if mods.ctrl {
            code += 16;
        }
    }
    let mut out = b"\x1b[M".to_vec();
    out.push(32 + code as u8);
    out.push(32 + col as u8);
    out.push(32 + row as u8);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGR: TermMode = TermMode::MOUSE_REPORT_CLICK.union(TermMode::SGR_MOUSE);

    #[test]
    fn a_click_is_reported_with_its_cell() {
        let press = report(SGR, Button::Left, Kind::Press, 12, 3, Mods::default()).unwrap();
        assert_eq!(press, b"\x1b[<0;12;3M");
        let release = report(SGR, Button::Left, Kind::Release, 12, 3, Mods::default()).unwrap();
        assert_eq!(release, b"\x1b[<0;12;3m");
    }

    #[test]
    fn the_wheel_has_no_release_and_the_old_encoding_has_no_button() {
        assert!(report(SGR, Button::WheelUp, Kind::Release, 1, 1, Mods::default()).is_none());
        let up = report(SGR, Button::WheelUp, Kind::Press, 1, 1, Mods::default()).unwrap();
        assert_eq!(up, b"\x1b[<64;1;1M");

        let old = TermMode::MOUSE_REPORT_CLICK;
        let release = report(old, Button::Right, Kind::Release, 2, 4, Mods::default()).unwrap();
        assert_eq!(release, b"\x1b[M\x23\x22\x24");
    }

    #[test]
    fn a_bare_move_needs_the_mode_that_asks_for_every_move() {
        let drag = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE | TermMode::MOUSE_DRAG;
        assert!(report(drag, Button::None, Kind::Motion, 7, 2, Mods::default()).is_none());

        let motion = drag | TermMode::MOUSE_MOTION;
        let moved = report(motion, Button::None, Kind::Motion, 7, 2, Mods::default()).unwrap();
        assert_eq!(moved, b"\x1b[<35;7;2M");
    }

    #[test]
    fn a_drag_is_only_reported_when_the_program_asked_for_it() {
        let click_only = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert!(
            report(
                click_only,
                Button::Left,
                Kind::Motion,
                5,
                5,
                Mods::default()
            )
            .is_none()
        );

        let drag = click_only | TermMode::MOUSE_DRAG;
        let moved = report(drag, Button::Left, Kind::Motion, 5, 5, Mods::default()).unwrap();
        assert_eq!(moved, b"\x1b[<32;5;5M");
    }

    #[test]
    fn modifiers_ride_along_with_the_button() {
        let mods = Mods {
            shift: false,
            alt: true,
            ctrl: true,
        };
        let press = report(SGR, Button::Left, Kind::Press, 1, 1, mods).unwrap();
        assert_eq!(press, b"\x1b[<24;1;1M");
    }

    #[test]
    fn nothing_is_reported_while_no_program_asked() {
        let quiet = TermMode::SGR_MOUSE;
        assert!(report(quiet, Button::Left, Kind::Press, 1, 1, Mods::default()).is_none());
    }
}
