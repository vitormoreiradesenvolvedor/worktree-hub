//! Launching things outside the app: the user's terminal emulator, editor and
//! file manager. Everything is spawned detached with the worktree as cwd.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow, bail};

/// Splits a user-provided command template, expanding `{path}`.
///
/// `code --new-window {path}` → `["code", "--new-window", "/the/path"]`
fn expand(template: &str, path: &Path) -> Vec<String> {
    let path_str = path.to_string_lossy().into_owned();
    let mut parts: Vec<String> = Vec::new();
    let mut had_placeholder = false;

    for token in template.split_whitespace() {
        if token.contains("{path}") {
            had_placeholder = true;
            parts.push(token.replace("{path}", &path_str));
        } else {
            parts.push(token.to_owned());
        }
    }
    if !had_placeholder {
        parts.push(path_str);
    }
    parts
}

fn spawn(program: &str, args: &[String], cwd: &Path) -> Result<()> {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| anyhow!("não foi possível iniciar `{program}`: {err}"))?;
    Ok(())
}

/// Looks for an executable on PATH (also tries `.exe`/`.cmd` on Windows).
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        vec![String::new(), ".exe".into(), ".cmd".into(), ".bat".into()]
    } else {
        vec![String::new()]
    };

    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Shell used by the embedded terminals.
pub fn default_shell() -> String {
    if cfg!(windows) {
        for candidate in ["pwsh", "powershell"] {
            if find_on_path(candidate).is_some() {
                return candidate.to_owned();
            }
        }
        return "cmd.exe".to_owned();
    }

    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.trim().is_empty() {
            return shell;
        }
    }
    for candidate in ["/bin/zsh", "/bin/bash", "/bin/sh"] {
        if Path::new(candidate).exists() {
            return candidate.to_owned();
        }
    }
    "/bin/sh".to_owned()
}

/// (program, extra args) pairs for terminal emulators we know how to point at a directory.
#[cfg(not(target_os = "windows"))]
const TERMINALS: &[(&str, &[&str])] = &[
    ("ptyxis", &["--new-window", "-d", "{path}"]),
    ("kgx", &["--working-directory={path}"]),
    ("gnome-terminal", &["--working-directory={path}"]),
    ("konsole", &["--workdir", "{path}"]),
    ("xfce4-terminal", &["--working-directory={path}"]),
    ("terminator", &["--working-directory={path}"]),
    ("tilix", &["-w", "{path}"]),
    ("alacritty", &["--working-directory", "{path}"]),
    ("kitty", &["--directory", "{path}"]),
    ("wezterm", &["start", "--cwd", "{path}"]),
    ("foot", &["--working-directory={path}"]),
    ("xterm", &[]),
];

#[cfg(target_os = "windows")]
const TERMINALS: &[(&str, &[&str])] = &[("wt.exe", &["-d", "{path}"])];

pub fn open_terminal(path: &Path, custom: &str) -> Result<()> {
    if !custom.trim().is_empty() {
        let parts = expand(custom, path);
        let (program, args) = parts
            .split_first()
            .ok_or_else(|| anyhow!("comando de terminal vazio"))?;
        return spawn(program, args, path);
    }

    if let Ok(env_term) = std::env::var("TERMINAL") {
        if !env_term.trim().is_empty() && find_on_path(env_term.trim()).is_some() {
            return spawn(env_term.trim(), &[], path);
        }
    }

    for (program, template) in TERMINALS {
        if find_on_path(program).is_none() {
            continue;
        }
        let args: Vec<String> = template
            .iter()
            .map(|a| a.replace("{path}", &path.to_string_lossy()))
            .collect();
        if spawn(program, &args, path).is_ok() {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        return spawn(
            "open",
            &[
                "-a".into(),
                "Terminal".into(),
                path.to_string_lossy().into_owned(),
            ],
            path,
        );
    }

    #[cfg(target_os = "windows")]
    {
        return spawn(
            "cmd",
            &["/C".into(), "start".into(), String::new(), "cmd".into()],
            path,
        );
    }

    #[allow(unreachable_code)]
    {
        bail!("nenhum terminal encontrado — defina um em Ajustes")
    }
}

const EDITORS: &[&str] = &[
    "code", "codium", "cursor", "zed", "zeditor", "subl", "gedit",
];

pub fn open_editor(path: &Path, custom: &str) -> Result<()> {
    if !custom.trim().is_empty() {
        let parts = expand(custom, path);
        let (program, args) = parts
            .split_first()
            .ok_or_else(|| anyhow!("comando de editor vazio"))?;
        return spawn(program, args, path);
    }

    for program in EDITORS {
        if find_on_path(program).is_some() {
            return spawn(program, &[path.to_string_lossy().into_owned()], path);
        }
    }
    bail!("nenhum editor encontrado — defina um em Ajustes")
}

pub fn open_folder(path: &Path) -> Result<()> {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    spawn(program, &[path.to_string_lossy().into_owned()], path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_path_when_template_has_no_placeholder() {
        let args = expand("code --new-window", Path::new("/tmp/wt"));
        assert_eq!(args, vec!["code", "--new-window", "/tmp/wt"]);
    }

    #[test]
    fn substitutes_placeholder_in_place() {
        let args = expand("kitty --directory {path} -e nvim", Path::new("/tmp/wt"));
        assert_eq!(args, vec!["kitty", "--directory", "/tmp/wt", "-e", "nvim"]);
    }

    #[test]
    fn substitutes_placeholder_glued_to_flag() {
        let args = expand("foot --working-directory={path}", Path::new("/a b"));
        assert_eq!(args, vec!["foot", "--working-directory=/a b"]);
    }

    #[test]
    fn default_shell_is_not_empty() {
        assert!(!default_shell().is_empty());
    }
}
