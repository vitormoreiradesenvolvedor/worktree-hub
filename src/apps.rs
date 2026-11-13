//! The applications the desktop already knows about.
//!
//! Linux ships every installed program as a `.desktop` file; reading those is
//! how the app menu of any desktop environment is built, and it is how we offer
//! a choice of editors instead of guessing one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One entry of the desktop menu, trimmed down to what we need to show it and
/// to launch it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopApp {
    /// File name without `.desktop`, used to weed out duplicates.
    pub id: String,
    pub name: String,
    /// Program and arguments, already free of the `%F`-style field codes.
    pub exec: Vec<String>,
    /// A picture we can actually decode (PNG), when the entry points at one.
    pub icon: Option<PathBuf>,
}

impl DesktopApp {
    /// Two or three letters to stand in for a missing icon.
    pub fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }

    /// The command template this app would be saved as, with `{path}` where the
    /// worktree goes.
    pub fn template(&self) -> String {
        let mut parts = self.exec.clone();
        parts.push("{path}".to_owned());
        parts.join(" ")
    }
}

/// Every code editor and IDE the desktop knows about, sorted by name.
pub fn editors() -> Vec<DesktopApp> {
    let mut found: HashMap<String, DesktopApp> = HashMap::new();

    for dir in menu_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(app) = parse(&text) else { continue };
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| app.name.clone());
            // The first directory to offer an app wins: a user's own entry
            // overrides the system one.
            found.entry(id.clone()).or_insert(DesktopApp { id, ..app });
        }
    }

    let mut editors: Vec<DesktopApp> = found
        .into_values()
        .filter(|app| runnable(&app.exec))
        .collect();

    // Editors without a desktop entry still deserve a place.
    for program in FALLBACK {
        if editors.iter().any(|app| {
            app.exec
                .first()
                .is_some_and(|first| first.ends_with(program))
        }) {
            continue;
        }
        if crate::external::find_on_path(program).is_some() {
            editors.push(DesktopApp {
                id: (*program).to_owned(),
                name: (*program).to_owned(),
                exec: vec![(*program).to_owned()],
                icon: None,
            });
        }
    }

    editors.sort_by_key(|app| app.name.to_lowercase());
    editors
}

/// Editors we know by command name, for systems without desktop entries. Only
/// ones that open a window of their own: a terminal editor has nowhere to go.
const FALLBACK: &[&str] = &["code", "codium", "cursor", "zed", "subl"];

fn menu_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        dirs.push(data_home.join("applications"));
        dirs.push(data_home.join("flatpak/exports/share/applications"));
    } else if let Some(home) = &home {
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }

    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    for dir in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(dir).join("applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));

    dirs.dedup();
    dirs
}

/// Reads the `[Desktop Entry]` group and keeps it only if it is a code editor.
fn parse(text: &str) -> Option<DesktopApp> {
    let mut keys: HashMap<&str, &str> = HashMap::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Only the main group counts; the rest are extra launcher actions.
            if inside {
                break;
            }
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            keys.entry(key.trim()).or_insert(value.trim());
        }
    }

    if keys.get("Type") != Some(&"Application") {
        return None;
    }
    if matches!(keys.get("NoDisplay"), Some(&"true"))
        || matches!(keys.get("Hidden"), Some(&"true"))
        || matches!(keys.get("Terminal"), Some(&"true"))
    {
        return None;
    }
    // Writing code, not writing text: an entry has to be filed under both
    // development and editing, which leaves out word processors and viewers.
    let categories: Vec<&str> = keys
        .get("Categories")
        .copied()
        .unwrap_or_default()
        .split(';')
        .collect();
    let develops = categories
        .iter()
        .any(|c| *c == "Development" || *c == "IDE");
    let edits = categories.iter().any(|c| *c == "TextEditor" || *c == "IDE");
    if !(develops && edits) {
        return None;
    }

    let exec = command(keys.get("Exec").copied().unwrap_or_default());
    if exec.is_empty() {
        return None;
    }
    let name = keys.get("Name").copied().unwrap_or_default().to_owned();
    if name.is_empty() {
        return None;
    }

    Some(DesktopApp {
        id: name.clone(),
        name,
        icon: keys.get("Icon").and_then(|icon| picture(icon)),
        exec,
    })
}

/// Splits an `Exec=` line into program and arguments, dropping the field codes
/// the desktop spec fills in (`%F`, `%U`, `%i`, …).
fn command(exec: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for ch in exec.chars() {
        match ch {
            '"' | '\'' if quote.is_none() => {
                quote = Some(ch);
                started = true;
            }
            c if Some(c) == quote => quote = None,
            c if c.is_whitespace() && quote.is_none() => {
                if started || !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => current.push(c),
        }
    }
    if started || !current.is_empty() {
        parts.push(current);
    }

    parts.retain(|part| {
        let field_code = part.len() == 2
            && part.starts_with('%')
            && part.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic());
        // `@@`/`@@u` are flatpak's own placeholders for the files to open.
        let forwarding = part == "@@" || part == "@@u";
        !(field_code || forwarding)
    });
    parts
}

/// True when the program can actually be started on this machine.
fn runnable(exec: &[String]) -> bool {
    let Some(program) = exec.first() else {
        return false;
    };
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    crate::external::find_on_path(program).is_some()
}

/// Finds a PNG for an `Icon=` value: an absolute path, or a name to look up in
/// the icon themes. SVG-only icons come back as `None` and get initials.
fn picture(icon: &str) -> Option<PathBuf> {
    if icon.is_empty() {
        return None;
    }
    if icon.contains('/') {
        let path = PathBuf::from(icon);
        // AppImage launchers often point at an extensionless file that is a
        // PNG all the same, so the bytes decide, not the name.
        if path.is_file() {
            return Some(path);
        }
        for ext in ["png", "PNG"] {
            let candidate = path.with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        return None;
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".icons"));
        roots.push(home.join(".local/share/flatpak/exports/share/icons"));
    }
    roots.push(PathBuf::from("/var/lib/flatpak/exports/share/icons"));
    roots.push(PathBuf::from("/usr/share/icons"));
    roots.push(PathBuf::from("/usr/local/share/icons"));

    // Biggest first: these end up on a 48px tile, and scaling down looks better.
    const SIZES: &[&str] = &[
        "256x256", "192x192", "128x128", "96x96", "64x64", "48x48", "512x512", "scalable",
    ];
    for root in &roots {
        for theme in ["hicolor", "Papirus", "breeze", "Adwaita"] {
            for size in SIZES {
                let candidate = root
                    .join(theme)
                    .join(size)
                    .join("apps")
                    .join(format!("{icon}.png"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    for dir in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
        let candidate = PathBuf::from(dir).join(format!("{icon}.png"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_codes_and_quotes_leave_a_runnable_command() {
        let exec = command("\"/opt/My Editor/bin/edit\" --new-window %F");
        assert_eq!(exec, vec!["/opt/My Editor/bin/edit", "--new-window"]);
    }

    #[test]
    fn an_env_prefix_is_kept_as_the_program() {
        let exec = command("env DESKTOPINTEGRATION=1 /home/me/cursor.appimage %U");
        assert_eq!(
            exec,
            vec!["env", "DESKTOPINTEGRATION=1", "/home/me/cursor.appimage"]
        );
    }

    #[test]
    fn only_editors_and_ides_come_out_of_an_entry() {
        let editor = "[Desktop Entry]\nType=Application\nName=Edit\nExec=edit %F\nCategories=Development;TextEditor;\n";
        let browser = "[Desktop Entry]\nType=Application\nName=Surf\nExec=surf %U\nCategories=Network;WebBrowser;\n";
        assert_eq!(parse(editor).unwrap().name, "Edit");
        assert!(parse(browser).is_none());
    }

    #[test]
    fn hidden_entries_and_extra_actions_are_skipped() {
        let hidden = "[Desktop Entry]\nType=Application\nName=Ghost\nExec=ghost\nCategories=IDE;\nNoDisplay=true\n";
        assert!(parse(hidden).is_none());

        // The second group is a launcher action, and must not overwrite Exec.
        let with_action = "[Desktop Entry]\nType=Application\nName=Edit\nExec=edit %F\nCategories=IDE;\n\n[Desktop Action new]\nName=New Window\nExec=edit --new-window\n";
        assert_eq!(parse(with_action).unwrap().exec, vec!["edit"]);
    }

    #[test]
    fn a_saved_app_keeps_the_path_placeholder() {
        let app = DesktopApp {
            id: "edit".into(),
            name: "Edit".into(),
            exec: vec!["edit".into(), "--new-window".into()],
            icon: None,
        };
        assert_eq!(app.template(), "edit --new-window {path}");
        assert_eq!(app.initials(), "E");
    }
}
