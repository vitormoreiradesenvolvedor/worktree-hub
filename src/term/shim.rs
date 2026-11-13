//! Tiny wrapper programs put at the front of a pane's `PATH`.
//!
//! A few programs decide how to draw their pictures from the name of the
//! terminal that started them, and they have never heard of this one, so they
//! fall back to coloured text. Instead of changing the user's own
//! configuration — which would also affect the terminals that cannot show
//! pictures — each pane gets a directory of one-line scripts that pass the
//! right flag along. The flag goes before the user's own arguments, so anything
//! typed by hand still wins.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Program name and the arguments we slip in ahead of the user's.
const SHIMS: &[(&str, &[&str])] = &[("fastfetch", &["--logo-type", "kitty"])];

/// Extra arguments for the pane's shell, when it can be told to run a file of
/// ours after the user's own startup files.
///
/// Putting our directory at the front of `PATH` is not enough on its own: a
/// startup file is free to rebuild `PATH` from scratch, and this machine's
/// does, dropping us. Bash reads `--rcfile` instead of `~/.bashrc`, so we hand
/// it a file that sources the real one and then puts us back.
pub fn shell_args(shell: &str) -> Vec<String> {
    let name = std::path::Path::new(shell)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name != "bash" {
        return Vec::new();
    }
    match bash_rc() {
        Some(rc) => vec!["--rcfile".to_owned(), rc.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

/// Writes the startup file bash gets, once per run.
fn bash_rc() -> Option<PathBuf> {
    static RC: OnceLock<Option<PathBuf>> = OnceLock::new();
    RC.get_or_init(|| {
        let dir = dir()?;
        let path = dir.join("bashrc");
        let text = format!(
            "# Written by Worktree Hub. Your own startup file runs first and is\n\
             # left untouched; only the wrappers are put back in front of PATH.\n\
             if [ -f \"$HOME/.bashrc\" ]; then . \"$HOME/.bashrc\"; fi\n\
             case \":$PATH:\" in\n\
             \u{20}\u{20}\":{dir}:\"*) ;;\n\
             \u{20}\u{20}*) PATH=\"{dir}:$PATH\"; export PATH ;;\n\
             esac\n",
            dir = dir.to_string_lossy()
        );
        std::fs::write(&path, text).ok()?;
        Some(path)
    })
    .clone()
}

/// `PATH` for a pane: our wrappers first, then everything the app inherited.
pub fn path() -> Option<std::ffi::OsString> {
    let dir = dir()?;
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![dir];
    dirs.extend(std::env::split_paths(&inherited));
    std::env::join_paths(dirs).ok()
}

/// Writes the wrappers once per run and hands back the directory holding them.
fn dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(build).clone()
}

fn build() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let dir = directories::ProjectDirs::from("dev", "WorktreeHub", "worktree-hub")?
        .cache_dir()
        .join("shims");
    std::fs::create_dir_all(&dir).ok()?;

    let mut wrote = false;
    for (program, args) in SHIMS {
        // Resolved now, while our own directory is still out of `PATH`, so the
        // script can never end up calling itself.
        let Some(real) = crate::external::find_on_path(program) else {
            continue;
        };
        if std::fs::write(dir.join(program), script(&real, args)).is_err() {
            continue;
        }
        if executable(&dir.join(program)).is_err() {
            continue;
        }
        wrote = true;
    }

    wrote.then_some(dir)
}

fn script(real: &std::path::Path, args: &[&str]) -> String {
    let args = args
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "#!/bin/sh\n# Written by Worktree Hub: this terminal draws kitty graphics.\nexec '{}' {args} \"$@\"\n",
        real.to_string_lossy().replace('\'', "'\\''")
    )
}

#[cfg(unix)]
fn executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn executable(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_comes_before_the_user_arguments() {
        let text = script(
            std::path::Path::new("/usr/bin/fastfetch"),
            &["--logo-type", "kitty"],
        );
        assert!(text.starts_with("#!/bin/sh\n"));
        assert!(text.ends_with("exec '/usr/bin/fastfetch' '--logo-type' 'kitty' \"$@\"\n"));
    }

    #[test]
    fn quotes_in_a_path_cannot_break_out_of_the_script() {
        let text = script(std::path::Path::new("/opt/it's here/ff"), &[]);
        assert!(text.contains("exec '/opt/it'\\''s here/ff'"));
    }
}
