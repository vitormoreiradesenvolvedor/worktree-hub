//! Thin, well-tested wrapper over the `git` CLI. Everything here is blocking and
//! is meant to be called from a background thread (see `app::jobs`).

pub mod cmd;
pub mod repo;
pub mod worktree;

pub use repo::{Commit, Status};
pub use worktree::{AddOptions, Worktree};
