//! Embedded terminal: a real PTY per pane, parsed by `alacritty_terminal` and
//! drawn with egui.

pub mod images;
pub mod keys;
pub mod mouse;
pub mod session;
pub mod shim;
pub mod widget;

pub use session::{SpawnSpec, TermSize, TerminalSession};
pub use widget::{TermTheme, show_terminal};
