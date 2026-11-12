use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs `git` in `dir` and returns stdout, mapping a non-zero exit into an error
/// that carries stderr (that is what the UI shows the user).
pub fn git<I, S>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = raw(dir, args)?;
    if !out.ok {
        let msg = if out.stderr.trim().is_empty() {
            out.stdout.trim().to_owned()
        } else {
            out.stderr.trim().to_owned()
        };
        bail!("{msg}");
    }
    Ok(out.stdout)
}

pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn raw<I, S>(dir: &Path, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        // Stable, parseable output regardless of the user's locale/config.
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args);

    let out = command
        .output()
        .with_context(|| format!("falha ao executar git em {}", dir.display()))?;

    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// `true` when a usable `git` binary is on PATH.
pub fn available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
