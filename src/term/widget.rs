//! Renders a `TerminalSession` and feeds it keyboard/mouse input.

use super::mouse;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use egui::{
    Align2, Color32, CornerRadius, Event, EventFilter, FontId, Key, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

use std::time::{Duration, Instant};

use super::keys;
use super::session::{TermSize, TerminalSession};
use crate::theme;

const PAD: f32 = 8.0;
/// Hit area of the scrollbar on the right edge of a pane.
const SCROLLBAR_W: f32 = 12.0;
/// How long a notch sent with Alt held keeps the wheel quiet: long enough for
/// a program that redraws its whole screen to show the move before the next
/// one is asked for.
const CREEP: Duration = Duration::from_millis(160);
/// Pixels of gesture a notch of the free wheel costs, in cells: two cells per
/// notch is half the pace of the hand.
const FREE: f32 = 2.0;

pub struct TermTheme {
    pub bg: Color32,
    pub fg: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    pub ansi: [Color32; 16],
    pub font_size: f32,
    /// Rounding of the pane this terminal sits in, so its background does not
    /// square off the corners the pane drew.
    pub corner: CornerRadius,
}

/// What a pane did this frame: its response plus the bytes it sent to the PTY
/// (the app mirrors those to the other panes when broadcast is on).
pub struct TermOutput {
    pub response: Response,
    /// What the user typed or pasted — the only thing worth mirroring.
    pub sent: Vec<u8>,
    /// Ctrl+wheel over this pane: multiply the font size by this.
    pub zoom: f32,
}

struct Run {
    row: usize,
    start_col: usize,
    cells: usize,
    text: String,
    fg: Color32,
    bg: Color32,
    bold: bool,
    underline: bool,
    strike: bool,
}

pub fn show_terminal(ui: &mut Ui, session: &mut TerminalSession, t: &TermTheme) -> TermOutput {
    let font = theme::mono(t.font_size, false);
    let font_bold = theme::mono(t.font_size, true);

    let (cell_w, cell_h) = {
        let galley = ui
            .painter()
            .layout_no_wrap("M".to_owned(), font.clone(), t.fg);
        (
            galley.rect.width().max(1.0),
            galley.rect.height().max(1.0).ceil(),
        )
    };

    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let inner = Rect::from_min_size(
        rect.min + Vec2::splat(PAD),
        Vec2::new(
            (rect.width() - 2.0 * PAD).max(cell_w),
            (rect.height() - 2.0 * PAD).max(cell_h),
        ),
    );

    let cols = (inner.width() / cell_w).floor().max(2.0) as usize;
    let rows = (inner.height() / cell_h).floor().max(1.0) as usize;
    session.resize(TermSize::new(cols, rows), cell_w, cell_h);

    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    let focused = response.has_focus();
    if focused {
        // The terminal owns Tab, the arrows and Escape while it has focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    // The scrollbar is registered after the body, so grabbing it never starts a
    // text selection.
    // Clear of the rounded corners the pane is drawn with, or the ends of the
    // rail would hang over the curve of the border.
    let round = (t.corner.ne.max(t.corner.se) as f32).max(4.0);
    let bar_hit = Rect::from_min_max(
        Pos2::new(rect.max.x - SCROLLBAR_W, rect.min.y + round),
        Pos2::new(rect.max.x, rect.max.y - round),
    );
    let bar = ui.interact(
        bar_hit,
        response.id.with("scrollbar"),
        Sense::click_and_drag(),
    );

    let mut pending: Vec<u8> = Vec::new();
    // Mouse reports and scrolling belong to this pane alone: mirroring them
    // would bombard the other panes with coordinates nobody typed.
    let mut private: Vec<u8> = Vec::new();
    let mut copy_selection = false;
    let mut paste_clipboard = false;

    // Which program is in charge decides what a notch of the wheel is worth,
    // so it is asked before anything is drawn — and before the terminal is
    // locked, since it borrows the pane as a whole.
    session.watch_program();

    let mut guard = session.term.lock();
    let mode = *guard.mode();
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let bracketed = mode.contains(TermMode::BRACKETED_PASTE);
    let alt_scroll =
        mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL);
    // While a program asks for the mouse it decides what a click or a wheel
    // notch means — which pane of its own to scroll, which button was hit.
    // Holding Shift takes the mouse back for selecting text, as everywhere.
    let shift_held = ui.input(|i| i.modifiers.shift);
    let reporting = mouse::wanted(mode) && !shift_held;

    // ---- mouse -------------------------------------------------------------
    let to_point = |pos: Pos2, display_offset: usize| -> (Point, Side) {
        let rel_x = ((pos.x - inner.min.x) / cell_w).max(0.0);
        let rel_y = ((pos.y - inner.min.y) / cell_h).max(0.0);
        let col = (rel_x.floor() as usize).min(cols.saturating_sub(1));
        let row = (rel_y.floor() as usize).min(rows.saturating_sub(1));
        let side = if rel_x.fract() > 0.5 {
            Side::Right
        } else {
            Side::Left
        };
        (
            Point::new(Line(row as i32 - display_offset as i32), Column(col)),
            side,
        )
    };

    let display_offset = guard.grid().display_offset();
    if let Some(pos) = response.interact_pointer_pos()
        && !reporting
    {
        let (point, side) = to_point(pos, display_offset);
        if response.triple_clicked() {
            guard.selection = Some(Selection::new(SelectionType::Lines, point, side));
        } else if response.double_clicked() {
            guard.selection = Some(Selection::new(SelectionType::Semantic, point, side));
        } else if response.drag_started() {
            let block = ui.input(|i| i.modifiers.alt);
            let kind = if block {
                SelectionType::Block
            } else {
                SelectionType::Simple
            };
            guard.selection = Some(Selection::new(kind, point, side));
            session.selecting = true;
        } else if response.dragged() && session.selecting {
            if let Some(selection) = guard.selection.as_mut() {
                selection.update(point, side);
            }
        } else if response.clicked() && !session.selecting {
            guard.selection = None;
        }
    }
    if response.drag_stopped() {
        session.selecting = false;
    }

    // A selection dragged past the top or the bottom pulls the scrollback along.
    if response.dragged()
        && session.selecting
        && !reporting
        && let Some(pos) = response.interact_pointer_pos()
    {
        let step = if pos.y < inner.min.y {
            ((inner.min.y - pos.y) / cell_h).ceil().min(4.0) as i32
        } else if pos.y > inner.max.y {
            -((pos.y - inner.max.y) / cell_h).ceil().min(4.0) as i32
        } else {
            0
        };
        if step != 0 {
            guard.scroll_display(Scroll::Delta(step));
            ui.ctx().request_repaint();
        }
    }

    // The scrollbar itself: drag it anywhere, or click the track to jump.
    let history = guard
        .grid()
        .total_lines()
        .saturating_sub(guard.grid().screen_lines());
    let bar_track = Rect::from_min_max(
        Pos2::new(rect.max.x - 7.0, rect.min.y + round),
        Pos2::new(rect.max.x - 3.0, rect.max.y - round),
    );
    let thumb_h = if history == 0 {
        bar_track.height()
    } else {
        (bar_track.height() * guard.grid().screen_lines() as f32
            / guard.grid().total_lines().max(1) as f32)
            .max(28.0)
            .min(bar_track.height())
    };
    // A full-screen program keeps its own history, so there is nothing of ours
    // to jump to: the strip becomes a scrollbar of the program's own history,
    // built from what it does when it is told to scroll. One drag of the
    // handle covers the lot; Ctrl goes back to a line at a time. A program
    // that moves by blocks gets no rail at all: there is no honest place to
    // put the handle, so the wheel does the work (Alt for a notch at a time).
    let railed = !session.scroll.by_blocks();
    if reporting && history == 0 && railed {
        session.scroll.measure();
        // A hand holding the handle still sends no events at all, and egui
        // lets go of the drag when they stop coming. The button is the truth:
        // while it is down and the handle was taken here, the drag is on.
        let button_down = ui.input(|i| i.pointer.any_down());
        let dragging = bar.dragged() || (button_down && session.scroll.held_at.is_some());
        // A held drag wakes nobody by itself, and neither does a program that
        // is not printing: without this the strip would stall halfway.
        if dragging || session.scroll.probe.is_some() {
            ui.ctx().request_repaint();
        }
        if bar.drag_stopped() || !button_down {
            session.scroll.rest = 0.0;
            session.scroll.grab = None;
            // The handle goes back to standing where the text says it stands.
            session.scroll.held_at = None;
        }
        let pointer_y = bar
            .interact_pointer_pos()
            .or_else(|| {
                dragging
                    .then(|| ui.input(|i| i.pointer.latest_pos()))
                    .flatten()
            })
            .map(|pos| pos.y);
        if let (true, Some(y)) = (bar.drag_started(), pointer_y) {
            session.scroll.grip(bar_track, rows, y);
        }
        let fine = ui.input(|i| i.modifiers.command);

        let step = cell_h * session.scroll.lines_per_notch();
        let up_notches = if fine {
            // A line of text per line of pointer travel, for the times a
            // single line matters more than reaching the end.
            let moved = match (dragging, pointer_y, session.scroll.grab) {
                (true, Some(now), Some(before)) => {
                    session.scroll.grab = Some(now);
                    now - before
                }
                _ => 0.0,
            };
            session.scroll.rest += moved;
            let whole = (session.scroll.rest / step).trunc();
            session.scroll.rest -= whole * step;
            // Dragging down heads for the newest line.
            -whole as i32
        } else if dragging || bar.clicked() {
            match pointer_y {
                Some(y) => {
                    let lines = session.scroll.drag_to(bar_track, rows, y);
                    (lines / session.scroll.lines_per_notch()).round() as i32
                }
                None => 0,
            }
        } else {
            0
        };
        // A steady pace of batches: measuring every one would be better, but a
        // jump of hundreds of lines leaves nothing a fingerprint can match, so
        // the strip would sit waiting for an answer that never comes.
        if up_notches != 0 && (fine || session.scroll.ready()) {
            let count = (up_notches.unsigned_abs() as i32).min(session.scroll.cap());
            let button = if up_notches > 0 {
                mouse::Button::WheelUp
            } else {
                mouse::Button::WheelDown
            };
            let mods = mouse::Mods {
                shift: false,
                alt: false,
                ctrl: false,
            };
            // Reported from the middle of the pane: the last column is the
            // edge of the window and programs tend to drop events there,
            // while the middle is always inside what they draw.
            // Middle of the pane, both ways: the edges are where programs put
            // their status lines and where they drop events, and the pointer
            // is out on the strip anyway, so its own row means nothing here.
            let col = (cols / 2).max(1);
            let row = (rows / 2).max(1);
            for _ in 0..count {
                if let Some(bytes) = mouse::report(mode, button, mouse::Kind::Press, col, row, mods)
                {
                    private.extend_from_slice(&bytes);
                }
            }
            session
                .scroll
                .note(if up_notches > 0 { count } else { -count }, rows);
        }
    } else if history > 0
        && (bar.dragged() || bar.clicked())
        && let Some(pos) = bar.interact_pointer_pos()
    {
        let travel = (bar_track.height() - thumb_h).max(1.0);
        let from_top = ((pos.y - thumb_h / 2.0 - bar_track.top()) / travel).clamp(0.0, 1.0);
        let wanted = ((1.0 - from_top) * history as f32).round() as i32;
        let delta = wanted - display_offset as i32;
        if delta != 0 {
            guard.scroll_display(Scroll::Delta(delta));
        }
    }

    // ---- the mouse, when a program asked for it ----------------------------
    let cell_at = |pos: Pos2| -> (usize, usize) {
        let col = ((pos.x - inner.min.x) / cell_w).floor().max(0.0) as usize;
        let row = ((pos.y - inner.min.y) / cell_h).floor().max(0.0) as usize;
        (col.min(cols - 1) + 1, row.min(rows - 1) + 1)
    };
    if reporting {
        let mods = mouse::Mods {
            shift: false,
            alt: ui.input(|i| i.modifiers.alt),
            ctrl: ui.input(|i| i.modifiers.command),
        };
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let held = ui.input(|i| i.pointer.any_down());
        // The scroll strip is ours, not the program's: a press there drives the
        // strip, and only a drag that started over the text keeps reporting
        // once it wanders onto it.
        let on_bar = pointer.is_some_and(|pos| bar_hit.contains(pos));

        for (egui_button, button) in [
            (egui::PointerButton::Primary, mouse::Button::Left),
            (egui::PointerButton::Middle, mouse::Button::Middle),
            (egui::PointerButton::Secondary, mouse::Button::Right),
        ] {
            let pressed = ui.input(|i| i.pointer.button_pressed(egui_button));
            let released = ui.input(|i| i.pointer.button_released(egui_button));
            if !(pressed || released) {
                continue;
            }
            // A press counts only over this pane; a release is sent wherever
            // the pointer ended up, so the program never waits for one.
            let Some(pos) = pointer else { continue };
            if pressed && (!rect.contains(pos) || on_bar) {
                continue;
            }
            if released && !session.mouse_claimed {
                continue;
            }
            session.mouse_claimed = pressed;
            let (col, row) = cell_at(pos);
            let kind = if pressed {
                mouse::Kind::Press
            } else {
                mouse::Kind::Release
            };
            if let Some(bytes) = mouse::report(mode, button, kind, col, row, mods) {
                private.extend_from_slice(&bytes);
            }
        }

        // Moving inside the pane: only worth a word when the cell changes.
        let cell = pointer
            .filter(|pos| (rect.contains(*pos) || held) && (!on_bar || session.mouse_claimed))
            .map(cell_at);
        if cell != session.mouse_cell {
            if let Some((col, row)) = cell {
                let button = if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                    mouse::Button::Left
                } else if ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary)) {
                    mouse::Button::Right
                } else if ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
                    mouse::Button::Middle
                } else {
                    mouse::Button::None
                };
                if let Some(bytes) =
                    mouse::report(mode, button, mouse::Kind::Motion, col, row, mods)
                {
                    private.extend_from_slice(&bytes);
                }
            }
            session.mouse_cell = cell;
        }
    } else {
        session.mouse_cell = None;
    }

    // Ctrl+wheel is a zoom gesture in egui, and never reaches `scroll_delta`.
    let zoom = if response.hovered() {
        ui.input(|i| i.zoom_delta())
    } else {
        1.0
    };

    if response.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        // Alt turns the wheel down to its finest step. One turn of the wheel
        // does not arrive as one event: the pixels are spread over several
        // frames, and a notch sent on each of them is the flood Alt is meant
        // to stop. So with Alt a single notch goes out and the rest of the
        // gesture is swallowed until the program has had time to draw it — a
        // notch at a time is as slowly as a conversation can be walked.
        let alt_held = ui.input(|i| i.modifiers.alt);
        let swallow = alt_held && session.creep.elapsed() < CREEP;
        let crept = alt_held && !swallow;
        if crept && wheel.abs() > 0.5 {
            session.creep = Instant::now();
        }
        // The free wheel walks at half the pace of the hand: a gesture worth
        // ten lines moves five. The pixels left over wait their turn in
        // `drift` rather than being rounded away, so slow turns of the wheel
        // still add up to a notch instead of going nowhere.
        if !crept && wheel.abs() > 0.5 {
            if session.drift != 0.0 && session.drift.is_sign_positive() != wheel.is_sign_positive()
            {
                session.drift = 0.0;
            }
            session.drift += wheel;
        }
        let step = cell_h * FREE;
        let steps = if crept {
            if wheel > 0.0 { 1 } else { -1 }
        } else if swallow {
            0
        } else {
            let whole = (session.drift / step).trunc();
            session.drift -= whole * step;
            whole as i32
        };
        if steps != 0 {
            let lines = steps;
            // A program that asked for the mouse hears every notch, even the
            // small ones that are worth less than a line here.
            let notches = steps.unsigned_abs().clamp(1, 10) as i32;
            if lines != 0 || reporting {
                if reporting {
                    let button = if steps > 0 {
                        mouse::Button::WheelUp
                    } else {
                        mouse::Button::WheelDown
                    };
                    let mods = mouse::Mods {
                        shift: false,
                        alt: false,
                        ctrl: ui.input(|i| i.modifiers.command),
                    };
                    let (col, row) = ui
                        .input(|i| i.pointer.latest_pos())
                        .map(cell_at)
                        .unwrap_or((1, 1));
                    for _ in 0..notches {
                        if let Some(bytes) =
                            mouse::report(mode, button, mouse::Kind::Press, col, row, mods)
                        {
                            private.extend_from_slice(&bytes);
                        }
                    }
                    // The strip stands for the same history the wheel moves.
                    session
                        .scroll
                        .note(if steps > 0 { notches } else { -notches }, rows);
                } else if alt_scroll {
                    let seq: &[u8] = if lines > 0 {
                        if app_cursor { b"\x1bOA" } else { b"\x1b[A" }
                    } else if app_cursor {
                        b"\x1bOB"
                    } else {
                        b"\x1b[B"
                    };
                    for _ in 0..notches {
                        private.extend_from_slice(seq);
                    }
                } else {
                    guard.scroll_display(Scroll::Delta(lines));
                }
            }
        }
    }

    // ---- keyboard ----------------------------------------------------------
    if focused {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                Event::Text(text) if !text.is_empty() => pending.extend_from_slice(text.as_bytes()),
                Event::Paste(text) => pending.extend_from_slice(&keys::paste(&text, bracketed)),
                // egui's winit layer turns Ctrl+C/Ctrl+X into clipboard events
                // and never forwards the key press, so a terminal would lose
                // SIGINT and nano's "exit". Here they are control codes —
                // copying lives on Ctrl+Shift+C.
                Event::Copy => {
                    guard.scroll_display(Scroll::Bottom);
                    pending.push(0x03);
                }
                Event::Cut => {
                    guard.scroll_display(Scroll::Bottom);
                    pending.push(0x18);
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.ctrl && modifiers.shift {
                        match key {
                            Key::C => copy_selection = true,
                            Key::V => paste_clipboard = true,
                            Key::ArrowUp => guard.scroll_display(Scroll::Delta(1)),
                            Key::ArrowDown => guard.scroll_display(Scroll::Delta(-1)),
                            Key::PageUp => guard.scroll_display(Scroll::PageUp),
                            Key::PageDown => guard.scroll_display(Scroll::PageDown),
                            Key::Home => guard.scroll_display(Scroll::Top),
                            Key::End => guard.scroll_display(Scroll::Bottom),
                            // Everything else with Ctrl+Shift belongs to the app.
                            _ => {}
                        }
                        continue;
                    }
                    if let Some(bytes) = keys::encode(key, modifiers, app_cursor) {
                        // Any key press leaves scrollback and jumps to the prompt.
                        guard.scroll_display(Scroll::Bottom);
                        pending.extend_from_slice(&bytes);
                    }
                }
                _ => {}
            }
        }
    }

    if copy_selection {
        if let Some(text) = guard.selection_to_string() {
            if !text.is_empty() {
                ui.ctx().copy_text(text);
            }
        }
    }
    if paste_clipboard {
        if let Some(text) = read_clipboard() {
            pending.extend_from_slice(&keys::paste(&text, bracketed));
        }
    }

    // ---- paint -------------------------------------------------------------
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, t.corner, t.bg);

    let display_offset = guard.grid().display_offset();
    let selection = guard.selection.as_ref().and_then(|s| s.to_range(&guard));
    let cursor_point = guard.grid().cursor.point;
    let mut cursor_char = ' ';
    let colors = guard.colors();

    let mut run: Option<Run> = None;
    let flush = |run: &mut Option<Run>, painter: &egui::Painter| {
        let Some(r) = run.take() else { return };
        let x = inner.min.x + r.start_col as f32 * cell_w;
        let y = inner.min.y + r.row as f32 * cell_h;
        let w = r.cells as f32 * cell_w;

        if r.bg != t.bg {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell_h)),
                CornerRadius::ZERO,
                r.bg,
            );
        }
        if r.text.trim().is_empty() {
            // Nothing to draw, but underlines still apply to blank cells.
            if r.underline {
                painter.hline(x..=x + w, y + cell_h - 1.5, Stroke::new(1.0, r.fg));
            }
            return;
        }
        let font: FontId = if r.bold {
            font_bold.clone()
        } else {
            font.clone()
        };
        painter.text(Pos2::new(x, y), Align2::LEFT_TOP, r.text, font, r.fg);
        if r.underline {
            painter.hline(x..=x + w, y + cell_h - 1.5, Stroke::new(1.0, r.fg));
        }
        if r.strike {
            painter.hline(x..=x + w, y + cell_h * 0.5, Stroke::new(1.0, r.fg));
        }
    };

    let mut hashes = vec![0u64; rows];
    for indexed in guard.grid().display_iter() {
        let point = indexed.point;
        let cell = indexed.cell;
        let row_i = point.line.0 + display_offset as i32;
        if row_i < 0 {
            continue;
        }
        let row = row_i as usize;
        let col = point.column.0;
        if row >= rows || col >= cols {
            continue;
        }
        // Cheap fingerprint of the line, enough to recognise it again a few
        // rows higher or lower.
        hashes[row] = hashes[row]
            .wrapping_mul(31)
            .wrapping_add(cell.c as u64)
            .wrapping_add(col as u64);

        let flags = cell.flags;
        if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            continue;
        }
        if point == cursor_point {
            cursor_char = cell.c;
        }

        let selected = selection.is_some_and(|range| range.contains(point));
        let (fg, bg) = cell_colors(cell.fg, cell.bg, flags, selected, t, colors);
        let bold = flags.contains(Flags::BOLD) || flags.contains(Flags::BOLD_ITALIC);
        let underline = flags.intersects(Flags::ALL_UNDERLINES);
        let strike = flags.contains(Flags::STRIKEOUT);
        let width = if flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };

        let same_run = run.as_ref().is_some_and(|r| {
            r.row == row
                && r.start_col + r.cells == col
                && r.fg == fg
                && r.bg == bg
                && r.bold == bold
                && r.underline == underline
                && r.strike == strike
        });

        if !same_run {
            flush(&mut run, &painter);
            run = Some(Run {
                row,
                start_col: col,
                cells: 0,
                text: String::new(),
                fg,
                bg,
                bold,
                underline,
                strike,
            });
        }
        if let Some(r) = run.as_mut() {
            r.text.push(cell.c);
            r.cells += width;
            if let Some(extra) = cell.zerowidth() {
                for ch in extra {
                    r.text.push(*ch);
                }
            }
        }
    }
    flush(&mut run, &painter);

    // Pictures programs dropped into the stream sit on top of the cells they
    // cover and travel with the scrollback they landed in.
    {
        let album = session.images.clone();
        if let Ok(mut album) = album.lock() {
            let total = guard.grid().total_lines() as i64;
            let history = total - guard.grid().screen_lines() as i64;
            album.cull(history - total);
            for placed in &mut album.items {
                let row = placed.line - history + display_offset as i64;
                if row + (placed.height / cell_h).ceil() as i64 <= 0 || row >= rows as i64 {
                    continue;
                }
                let origin = Pos2::new(
                    inner.min.x + placed.col as f32 * cell_w,
                    inner.min.y + row as f32 * cell_h,
                );
                let texture = placed.texture(ui.ctx());
                painter.image(
                    texture.id(),
                    Rect::from_min_size(origin, Vec2::new(placed.width, placed.height)),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
    }

    // Cursor
    if mode.contains(TermMode::SHOW_CURSOR) && display_offset == 0 {
        let row_i = cursor_point.line.0;
        if row_i >= 0 && (row_i as usize) < rows && cursor_point.column.0 < cols {
            let x = inner.min.x + cursor_point.column.0 as f32 * cell_w;
            let y = inner.min.y + row_i as f32 * cell_h;
            let cell_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_w, cell_h));
            let shape = guard.cursor_style().shape;
            paint_cursor(
                &painter,
                cell_rect,
                shape,
                focused,
                cursor_char,
                t,
                font.clone(),
            );
        }
    }

    // Scroll strip for a program that owns the mouse: the handle is as long as
    // the share of the history on screen, and sits where that history stands.
    if reporting && history == 0 && railed {
        let lit = bar.hovered() || bar.dragged() || session.scroll.held_at.is_some();
        painter.rect_filled(
            bar_track,
            CornerRadius::same(2),
            t.fg.gamma_multiply(if lit { 0.22 } else { 0.08 }),
        );
        let (handle_h, handle_y) = session.scroll.handle(bar_track, rows);
        let handle = Rect::from_center_size(
            Pos2::new(bar_track.center().x, handle_y),
            Vec2::new(bar_track.width(), handle_h),
        );
        painter.rect_filled(
            handle,
            CornerRadius::same(2),
            if lit {
                t.cursor
            } else {
                t.fg.gamma_multiply(0.30)
            },
        );
    }

    // Scrollbar: there whenever there is history, grabbable with the mouse.
    if history > 0 {
        let lit = bar.hovered() || bar.dragged();
        painter.rect_filled(
            bar_track,
            CornerRadius::same(2),
            t.fg.gamma_multiply(if lit { 0.22 } else { 0.10 }),
        );
        let travel = (bar_track.height() - thumb_h).max(0.0);
        let from_bottom = (display_offset as f32 / history as f32).clamp(0.0, 1.0);
        let thumb_y = bar_track.bottom() - thumb_h - from_bottom * travel;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(bar_track.left(), thumb_y),
                Vec2::new(bar_track.width(), thumb_h),
            ),
            CornerRadius::same(2),
            if display_offset > 0 || lit {
                t.cursor
            } else {
                t.fg.gamma_multiply(0.35)
            },
        );
    }

    drop(guard);
    session.scroll.mark(&hashes);
    session.scroll.rows_hash = hashes;

    if !pending.is_empty() {
        session.write(&pending);
    }
    if !private.is_empty() {
        session.write(&private);
    }

    TermOutput {
        response,
        sent: pending,
        zoom,
    }
}

fn paint_cursor(
    painter: &egui::Painter,
    rect: Rect,
    shape: CursorShape,
    focused: bool,
    under: char,
    t: &TermTheme,
    font: FontId,
) {
    if matches!(shape, CursorShape::Hidden) {
        return;
    }
    if !focused {
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, t.cursor.gamma_multiply(0.8)),
            StrokeKind::Inside,
        );
        return;
    }

    match shape {
        CursorShape::Beam => {
            painter.rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                CornerRadius::ZERO,
                t.cursor,
            );
        }
        CursorShape::Underline => {
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.max.y - 2.0),
                    Vec2::new(rect.width(), 2.0),
                ),
                CornerRadius::ZERO,
                t.cursor,
            );
        }
        CursorShape::HollowBlock => {
            painter.rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(1.0, t.cursor),
                StrokeKind::Inside,
            );
        }
        _ => {
            painter.rect_filled(rect, CornerRadius::ZERO, t.cursor);
            if under != ' ' {
                painter.text(rect.min, Align2::LEFT_TOP, under, font, t.bg);
            }
        }
    }
}

fn cell_colors(
    fg: AnsiColor,
    bg: AnsiColor,
    flags: Flags,
    selected: bool,
    t: &TermTheme,
    colors: &Colors,
) -> (Color32, Color32) {
    let mut fg_color = resolve(fg, t, colors);
    let mut bg_color = resolve(bg, t, colors);

    if flags.contains(Flags::DIM) || flags.contains(Flags::DIM_BOLD) {
        fg_color = fg_color.gamma_multiply(0.65);
    }
    if flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg_color, &mut bg_color);
    }
    if flags.contains(Flags::HIDDEN) {
        fg_color = bg_color;
    }
    if selected {
        bg_color = t.selection;
        fg_color = t.fg;
    }
    (fg_color, bg_color)
}

fn resolve(color: AnsiColor, t: &TermTheme, colors: &Colors) -> Color32 {
    match color {
        AnsiColor::Spec(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(index) => {
            if let Some(rgb) = colors[index as usize] {
                Color32::from_rgb(rgb.r, rgb.g, rgb.b)
            } else {
                indexed_color(index, t)
            }
        }
        AnsiColor::Named(named) => {
            if let Some(rgb) = colors[named as usize] {
                return Color32::from_rgb(rgb.r, rgb.g, rgb.b);
            }
            match named {
                NamedColor::Foreground | NamedColor::BrightForeground => t.fg,
                NamedColor::DimForeground => t.fg.gamma_multiply(0.65),
                NamedColor::Background => t.bg,
                NamedColor::Cursor => t.cursor,
                NamedColor::Black => t.ansi[0],
                NamedColor::Red => t.ansi[1],
                NamedColor::Green => t.ansi[2],
                NamedColor::Yellow => t.ansi[3],
                NamedColor::Blue => t.ansi[4],
                NamedColor::Magenta => t.ansi[5],
                NamedColor::Cyan => t.ansi[6],
                NamedColor::White => t.ansi[7],
                NamedColor::BrightBlack => t.ansi[8],
                NamedColor::BrightRed => t.ansi[9],
                NamedColor::BrightGreen => t.ansi[10],
                NamedColor::BrightYellow => t.ansi[11],
                NamedColor::BrightBlue => t.ansi[12],
                NamedColor::BrightMagenta => t.ansi[13],
                NamedColor::BrightCyan => t.ansi[14],
                NamedColor::BrightWhite => t.ansi[15],
                NamedColor::DimBlack => t.ansi[0].gamma_multiply(0.65),
                NamedColor::DimRed => t.ansi[1].gamma_multiply(0.65),
                NamedColor::DimGreen => t.ansi[2].gamma_multiply(0.65),
                NamedColor::DimYellow => t.ansi[3].gamma_multiply(0.65),
                NamedColor::DimBlue => t.ansi[4].gamma_multiply(0.65),
                NamedColor::DimMagenta => t.ansi[5].gamma_multiply(0.65),
                NamedColor::DimCyan => t.ansi[6].gamma_multiply(0.65),
                NamedColor::DimWhite => t.ansi[7].gamma_multiply(0.65),
            }
        }
    }
}

/// 256-color palette: 0..16 themed, 16..232 the RGB cube, 232..256 grayscale.
fn indexed_color(index: u8, t: &TermTheme) -> Color32 {
    match index {
        0..=15 => t.ansi[index as usize],
        16..=231 => {
            let i = index as usize - 16;
            const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            Color32::from_rgb(STEPS[i / 36], STEPS[(i / 6) % 6], STEPS[i % 6])
        }
        _ => {
            let v = 8u8.saturating_add(10u8.saturating_mul(index - 232));
            Color32::from_rgb(v, v, v)
        }
    }
}

fn read_clipboard() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut clipboard| clipboard.get_text().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> TermTheme {
        TermTheme {
            bg: Color32::BLACK,
            fg: Color32::WHITE,
            cursor: Color32::RED,
            selection: Color32::BLUE,
            ansi: [Color32::from_rgb(1, 1, 1); 16],
            font_size: 13.0,
            corner: CornerRadius::ZERO,
        }
    }

    #[test]
    fn rgb_cube_and_grayscale() {
        let t = theme();
        assert_eq!(indexed_color(196, &t), Color32::from_rgb(255, 0, 0));
        assert_eq!(indexed_color(21, &t), Color32::from_rgb(0, 0, 255));
        assert_eq!(indexed_color(244, &t), Color32::from_rgb(128, 128, 128));
    }

    #[test]
    fn inverse_swaps_foreground_and_background() {
        let t = theme();
        let colors = Colors::default();
        let (fg, bg) = cell_colors(
            AnsiColor::Named(NamedColor::Foreground),
            AnsiColor::Named(NamedColor::Background),
            Flags::INVERSE,
            false,
            &t,
            &colors,
        );
        assert_eq!(fg, Color32::BLACK);
        assert_eq!(bg, Color32::WHITE);
    }

    #[test]
    fn selection_overrides_colors() {
        let t = theme();
        let colors = Colors::default();
        let (fg, bg) = cell_colors(
            AnsiColor::Named(NamedColor::Red),
            AnsiColor::Named(NamedColor::Background),
            Flags::empty(),
            true,
            &t,
            &colors,
        );
        assert_eq!(bg, Color32::BLUE);
        assert_eq!(fg, Color32::WHITE);
    }

    #[test]
    fn hidden_text_matches_its_background() {
        let t = theme();
        let colors = Colors::default();
        let (fg, bg) = cell_colors(
            AnsiColor::Named(NamedColor::Red),
            AnsiColor::Named(NamedColor::Background),
            Flags::HIDDEN,
            false,
            &t,
            &colors,
        );
        assert_eq!(fg, bg);
    }
}
