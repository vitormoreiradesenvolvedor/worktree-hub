use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event as TermEvent, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte::ansi::{Processor, Rgb, StdSyncHandler};
use anyhow::{Context as _, Result};
use std::time::{Duration, Instant};

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::images::{Album, Piece, Scanner, SharedAlbum};
use super::strip::ScrollTrack;

/// Terminal dimensions in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TermSize {
    pub cols: usize,
    pub rows: usize,
}

impl TermSize {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols: cols.max(2),
            rows: rows.max(1),
        }
    }
}

impl Default for TermSize {
    fn default() -> Self {
        Self::new(80, 24)
    }
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

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

/// Bridges terminal-side events (title changes, PTY replies, bell) back into the app.
#[derive(Clone)]
pub struct Proxy {
    ctx: egui::Context,
    writer: SharedWriter,
    title: Arc<Mutex<String>>,
    window_size: Arc<Mutex<WindowSize>>,
    bell: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    default_title: String,
}

impl Proxy {
    fn pty_write(&self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }
}

impl EventListener for Proxy {
    fn send_event(&self, event: TermEvent) {
        match event {
            TermEvent::PtyWrite(text) => self.pty_write(text.as_bytes()),
            TermEvent::Title(title) => {
                if let Ok(mut slot) = self.title.lock() {
                    *slot = title;
                }
                self.ctx.request_repaint();
            }
            TermEvent::ResetTitle => {
                if let Ok(mut slot) = self.title.lock() {
                    *slot = self.default_title.clone();
                }
                self.ctx.request_repaint();
            }
            TermEvent::ClipboardStore(_, text) => self.ctx.copy_text(text),
            TermEvent::ClipboardLoad(_, formatter) => {
                // Reading the clipboard is not available from a background thread;
                // answering with an empty payload keeps the requesting app unblocked.
                self.pty_write(formatter("").as_bytes());
            }
            TermEvent::ColorRequest(index, formatter) => {
                self.pty_write(formatter(xterm_color(index)).as_bytes());
            }
            TermEvent::TextAreaSizeRequest(formatter) => {
                let size = self.window_size.lock().map(|s| *s).unwrap_or(WindowSize {
                    num_lines: 24,
                    num_cols: 80,
                    cell_width: 8,
                    cell_height: 16,
                });
                self.pty_write(formatter(size).as_bytes());
            }
            TermEvent::Bell => {
                self.bell.store(true, Ordering::Relaxed);
                self.ctx.request_repaint();
            }
            TermEvent::ChildExit(_) => {
                self.exited.store(true, Ordering::Relaxed);
                self.ctx.request_repaint();
            }
            TermEvent::Wakeup | TermEvent::MouseCursorDirty | TermEvent::CursorBlinkingChange => {
                self.ctx.request_repaint();
            }
            TermEvent::Exit => {}
        }
    }
}

/// Standard xterm palette, used only to answer OSC color queries.
fn xterm_color(index: usize) -> Rgb {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    let (r, g, b) = match index {
        0..=15 => BASE[index],
        16..=231 => {
            let i = index - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            (steps[i / 36], steps[(i / 6) % 6], steps[i % 6])
        }
        232..=255 => {
            let v = 8 + 10 * (index as u8 - 232);
            (v, v, v)
        }
        256 => (229, 229, 229), // foreground
        _ => (0, 0, 0),         // background and everything else
    };
    Rgb { r, g, b }
}

/// Everything a new pane needs. A struct keeps the call sites readable (and
/// clippy happy) as the terminal grows more knobs.
pub struct SpawnSpec<'a> {
    pub id: u64,
    /// Worktree folder name, shown in the pane header.
    pub label: String,
    pub branch: String,
    /// Name of the project this worktree belongs to, shown in the pane header.
    pub project: String,
    /// Id of that project, so the sidebar can follow the focused pane.
    pub project_id: Option<u64>,
    /// True when this folder is the repository itself, not a worktree of it.
    pub is_main: bool,
    pub cwd: &'a Path,
    pub shell: &'a str,
    pub scrollback: usize,
    pub size: TermSize,
}

pub struct TerminalSession {
    pub id: u64,
    /// Worktree name, shown in the pane header.
    pub label: String,
    pub branch: String,
    /// Project this pane belongs to.
    pub project: String,
    /// Id of that project, so the sidebar can follow the focused pane.
    pub project_id: Option<u64>,
    /// True when this folder is the repository itself, not a worktree of it.
    pub is_main: bool,
    pub cwd: PathBuf,
    pub term: Arc<FairMutex<Term<Proxy>>>,
    writer: SharedWriter,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    size: TermSize,
    cell: (f32, f32),
    title: Arc<Mutex<String>>,
    window_size: Arc<Mutex<WindowSize>>,
    bell: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    /// Pictures programs have drawn over this pane's grid.
    pub images: SharedAlbum,
    /// Program in charge of the pane the last time it was asked, and when.
    running: Option<String>,
    asked: Instant,
    /// Pending selection anchor while the user drags the mouse.
    pub selecting: bool,
    /// True while a press reported to the program has not been released yet,
    /// so the release reaches it even if the pointer wandered off.
    pub mouse_claimed: bool,
    /// What the scroll strip has worked out about the program's own history:
    /// how far a notch goes, where the view stands and where the ends are.
    pub scroll: ScrollTrack,
    /// Last cell reported to a program that asked for the mouse, so a move is
    /// only sent when the pointer actually changes cell.
    pub mouse_cell: Option<(usize, usize)>,
    /// Wheel pixels not yet worth a notch. The free wheel moves at half the
    /// pace of the gesture, and what is left over waits here instead of being
    /// rounded away.
    pub drift: f32,
    /// When the last Alt notch was sent: holding Alt lets one notch through at
    /// a time, so a program that jumps a block per notch can be crept through.
    pub creep: Instant,
    /// Font size for this pane alone, set with Ctrl+wheel.
    pub font_size: Option<f32>,
}

impl TerminalSession {
    pub fn spawn(ctx: &egui::Context, spec: SpawnSpec<'_>) -> Result<Self> {
        let SpawnSpec {
            id,
            label,
            branch,
            project,
            project_id,
            is_main,
            cwd,
            shell,
            scrollback,
            size,
        } = spec;

        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: size.rows as u16,
                cols: size.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("não foi possível abrir um PTY")?;

        let mut cmd = CommandBuilder::new(shell);
        // Some shells let us run a file of ours after the user's own startup
        // files, which is the only way to keep our wrappers at the front of
        // `PATH` when a startup file rebuilds it.
        for arg in super::shim::shell_args(shell) {
            cmd.arg(arg);
        }
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "worktree-hub");
        cmd.env("WORKTREE_HUB", "1");
        cmd.env("WORKTREE_HUB_BRANCH", &branch);
        // Programs that draw pictures look at who started them; the wrappers
        // tell the few that cannot tell what this terminal can do.
        if let Some(path) = super::shim::path() {
            cmd.env("PATH", path);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("não foi possível iniciar `{shell}`"))?;
        let killer = child.clone_killer();
        drop(pair.slave);

        let writer: SharedWriter = Arc::new(Mutex::new(
            pair.master.take_writer().context("PTY sem writer")?,
        ));
        let reader = pair.master.try_clone_reader().context("PTY sem reader")?;

        let title = Arc::new(Mutex::new(label.clone()));
        let window_size = Arc::new(Mutex::new(WindowSize {
            num_lines: size.rows as u16,
            num_cols: size.cols as u16,
            cell_width: 8,
            cell_height: 16,
        }));
        let bell = Arc::new(AtomicBool::new(false));
        let exited = Arc::new(AtomicBool::new(false));

        let proxy = Proxy {
            ctx: ctx.clone(),
            writer: writer.clone(),
            title: title.clone(),
            window_size: window_size.clone(),
            bell: bell.clone(),
            exited: exited.clone(),
            default_title: label.clone(),
        };

        let config = TermConfig {
            scrolling_history: scrollback,
            ..Default::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, proxy)));

        let images: SharedAlbum = Arc::new(Mutex::new(Album::default()));
        spawn_reader(Reader {
            source: reader,
            term: term.clone(),
            ctx: ctx.clone(),
            exited: exited.clone(),
            images: images.clone(),
            writer: writer.clone(),
            window_size: window_size.clone(),
        });

        Ok(Self {
            id,
            label,
            branch,
            project,
            project_id,
            is_main,
            cwd: cwd.to_path_buf(),
            term,
            writer,
            master: pair.master,
            killer,
            child: Arc::new(Mutex::new(child)),
            size,
            cell: (8.0, 16.0),
            title,
            window_size,
            bell,
            exited,
            images,
            running: None,
            asked: Instant::now() - Duration::from_secs(1),
            selecting: false,
            mouse_claimed: false,
            scroll: ScrollTrack::default(),
            mouse_cell: None,
            drift: 0.0,
            creep: Instant::now() - Duration::from_secs(1),
            font_size: None,
        })
    }

    pub fn write(&self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    pub fn size(&self) -> TermSize {
        self.size
    }

    /// Re-sizes both the emulator grid and the PTY; a no-op when nothing changed.
    pub fn resize(&mut self, size: TermSize, cell_w: f32, cell_h: f32) {
        let cell = (cell_w.max(1.0), cell_h.max(1.0));
        if size == self.size && cell == self.cell {
            return;
        }
        self.size = size;
        self.cell = cell;

        self.term.lock().resize(size);
        let _ = self.master.resize(PtySize {
            rows: size.rows as u16,
            cols: size.cols as u16,
            pixel_width: (size.cols as f32 * cell.0) as u16,
            pixel_height: (size.rows as f32 * cell.1) as u16,
        });
        if let Ok(mut slot) = self.window_size.lock() {
            *slot = WindowSize {
                num_lines: size.rows as u16,
                num_cols: size.cols as u16,
                cell_width: cell.0 as u16,
                cell_height: cell.1 as u16,
            };
        }
    }

    /// The visible screen as plain text. Used by the tests and by anything that
    /// wants to read what a pane is showing.
    pub fn screen_text(&self) -> String {
        let term = self.term.lock();
        let mut out = String::new();
        let mut current = None;
        for indexed in term.grid().display_iter() {
            if current != Some(indexed.point.line.0) {
                if current.is_some() {
                    out.push('\n');
                }
                current = Some(indexed.point.line.0);
            }
            out.push(indexed.cell.c);
        }
        out
    }

    /// Keeps the strip told about which program is in charge: a program that
    /// takes over the pane brings its own history and its own idea of what a
    /// notch is worth, so the strip starts clean on each change.
    pub fn watch_program(&mut self) {
        if self.asked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.asked = Instant::now();
        let Some(now) = self.program().map(Some) else {
            // A pane between programs says nothing; what the strip already
            // knows is better than forgetting it for a frame.
            return;
        };
        if now != self.running {
            self.scroll
                .taught(now.as_deref().and_then(super::strip::habit_of));
            self.running = now;
        }
    }

    /// Name of the program in charge of this pane right now: the leader of
    /// the terminal's foreground process group, which is what the kernel
    /// hands the keyboard to. A shell sitting at its prompt answers with its
    /// own name; a program the shell started answers with that program's.
    /// Only unix answers: the process group of a pty and `/proc` are where
    /// this is written down, and Windows has neither.
    #[cfg(not(unix))]
    pub fn program(&self) -> Option<String> {
        None
    }

    #[cfg(unix)]
    pub fn program(&self) -> Option<String> {
        let pid = self.master.process_group_leader()?;
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
        let comm = comm.trim().to_string();
        // An interpreter says nothing about what it is running, so the name
        // of the script it was handed is the honest answer.
        if matches!(
            comm.as_str(),
            "node" | "python3" | "python" | "ruby" | "bun" | "deno"
        ) {
            if let Ok(line) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) {
                if let Some(script) = line
                    .split('\0')
                    .nth(1)
                    .and_then(|arg| arg.rsplit('/').next())
                    .filter(|arg| !arg.is_empty() && !arg.starts_with('-'))
                {
                    return Some(script.to_string());
                }
            }
        }
        Some(comm)
    }

    pub fn title(&self) -> String {
        self.title
            .lock()
            .map(|t| t.clone())
            .unwrap_or_else(|_| self.label.clone())
    }

    pub fn has_exited(&self) -> bool {
        if self.exited.load(Ordering::Relaxed) {
            return true;
        }
        if let Ok(mut child) = self.child.lock() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                self.exited.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    pub fn take_bell(&self) -> bool {
        self.bell.swap(false, Ordering::Relaxed)
    }

    pub fn kill(&mut self) {
        let _ = self.killer.kill();
        self.exited.store(true, Ordering::Relaxed);
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

/// Everything the reader thread owns.
struct Reader {
    source: Box<dyn Read + Send>,
    term: Arc<FairMutex<Term<Proxy>>>,
    ctx: egui::Context,
    exited: Arc<AtomicBool>,
    images: SharedAlbum,
    writer: SharedWriter,
    window_size: Arc<Mutex<WindowSize>>,
}

fn spawn_reader(mut reader: Reader) {
    std::thread::Builder::new()
        .name("worktree-hub-pty".into())
        .spawn(move || {
            let mut processor = Processor::<StdSyncHandler>::new();
            let mut scanner = Scanner::default();
            let mut buf = [0u8; 65536];
            loop {
                match reader.source.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        {
                            let mut guard = reader.term.lock();
                            for piece in scanner.feed(&buf[..n]) {
                                match piece {
                                    Piece::Bytes(bytes) => processor.advance(&mut *guard, &bytes),
                                    Piece::Picture(picture) => {
                                        // Dropped where the cursor stands right
                                        // now, counted from the oldest line the
                                        // scrollback still holds, so it travels
                                        // with the text around it.
                                        let grid = guard.grid();
                                        let history =
                                            grid.total_lines().saturating_sub(grid.screen_lines())
                                                as i64;
                                        let point = grid.cursor.point;
                                        let line = history + point.line.0 as i64;
                                        if let Ok(mut album) = reader.images.lock() {
                                            album.place(&picture, line, point.column.0);
                                        }
                                        // The sender counts on the cursor coming
                                        // out below the picture — the cell size
                                        // it used is the one we reported to it.
                                        let cell_h = reader
                                            .window_size
                                            .lock()
                                            .map(|size| size.cell_height.max(1))
                                            .unwrap_or(16)
                                            as u32;
                                        let rows = picture.height.div_ceil(cell_h);
                                        let feed = b"\n".repeat(rows as usize);
                                        processor.advance(&mut *guard, &feed);
                                        if let Some(id) = picture.id {
                                            // kitty's senders wait for this.
                                            let reply = format!("\x1b_Gi={id};OK\x1b\\");
                                            if let Ok(mut writer) = reader.writer.lock() {
                                                let _ = writer.write_all(reply.as_bytes());
                                                let _ = writer.flush();
                                            }
                                        }
                                    }
                                    Piece::Clear => {
                                        if let Ok(mut album) = reader.images.lock() {
                                            album.clear();
                                        }
                                    }
                                }
                            }
                        }
                        reader.ctx.request_repaint();
                    }
                }
            }
            reader.exited.store(true, Ordering::Relaxed);
            reader.ctx.request_repaint();
        })
        .expect("spawn pty reader thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_size_clamps_to_usable_minimum() {
        let size = TermSize::new(0, 0);
        assert_eq!(size.cols, 2);
        assert_eq!(size.rows, 1);
        assert_eq!(size.columns(), 2);
        assert_eq!(size.screen_lines(), 1);
    }

    #[test]
    fn xterm_cube_and_grayscale_are_in_range() {
        assert_eq!(xterm_color(0), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(xterm_color(196), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(
            xterm_color(231),
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(xterm_color(232), Rgb { r: 8, g: 8, b: 8 });
    }
}
