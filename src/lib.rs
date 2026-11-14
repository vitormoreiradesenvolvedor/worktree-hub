//! Worktree Hub — graphical orchestration for git worktrees.
//!
//! The crate is a library plus a thin binary so the whole UI can be exercised
//! headlessly from integration tests.

pub mod app;
pub mod apps;
pub mod config;
pub mod desktop;
pub mod external;
pub mod git;
pub mod i18n;
pub mod icon;
pub mod term;
pub mod theme;
pub mod ui;

pub const APP_NAME: &str = "Worktree Hub";
