//! Persisted state: the list of repositories plus user preferences.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::{Lang, tr};
use crate::theme::{Accent, Look, Mode, MonoFont, UiFont};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub root: PathBuf,
    /// Where new worktrees go by default; `None` means "next to the repository".
    #[serde(default)]
    pub worktree_dir: Option<PathBuf>,
}

impl Project {
    /// Directory new worktrees are created in.
    pub fn worktree_base(&self) -> PathBuf {
        if let Some(dir) = &self.worktree_dir {
            return dir.clone();
        }
        let parent = self.root.parent().unwrap_or(Path::new("."));
        parent.join(format!("{}-worktrees", self.name))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub theme: Mode,
    pub accent: Accent,
    pub ui_font: UiFont,
    pub mono_font: MonoFont,
    /// Whole-interface scale, 0.8..1.4.
    pub ui_scale: f32,
    /// Window transparency, 0.5..1.0 (1 = opaque).
    pub app_opacity: f32,
    /// Terminal background transparency, 0.5..1.0.
    pub term_opacity: f32,
    /// Shell for the embedded terminals; empty means "detect".
    pub shell: String,
    /// External terminal command, `{path}` is replaced by the worktree path.
    pub terminal_cmd: String,
    pub editor_cmd: String,
    pub term_font_size: f32,
    pub scrollback: usize,
    /// Open an embedded terminal right after creating a worktree.
    pub open_terminal_after_create: bool,
    pub show_clean_status: bool,
    pub pane_layout: PaneLayout,
    /// Left rail folded down to icons only.
    pub sidebar_collapsed: bool,
    /// Up to three commands kept at hand, sent to every pane with one click.
    pub quick_commands: Vec<QuickCommand>,
}

/// A button the user saves: a short name and the command it sends.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuickCommand {
    pub label: String,
    pub command: String,
}

/// How many of them fit in the broadcast bar.
pub const QUICK_SLOTS: usize = 3;

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: Lang::default(),
            theme: Mode::Dark,
            accent: Accent::default(),
            ui_font: UiFont::default(),
            mono_font: MonoFont::default(),
            ui_scale: 1.0,
            app_opacity: 1.0,
            term_opacity: 1.0,
            shell: String::new(),
            terminal_cmd: String::new(),
            editor_cmd: String::new(),
            term_font_size: 13.0,
            scrollback: 10_000,
            open_terminal_after_create: false,
            show_clean_status: true,
            pane_layout: PaneLayout::Grid,
            sidebar_collapsed: false,
            quick_commands: Vec::new(),
        }
    }
}

impl Settings {
    /// The saved command in that slot, if the slot was filled in.
    pub fn quick(&self, slot: usize) -> Option<&QuickCommand> {
        self.quick_commands
            .get(slot)
            .filter(|q| !q.command.trim().is_empty())
    }

    /// Writes a slot, growing the list up to [`QUICK_SLOTS`]; an empty command
    /// clears the slot instead.
    pub fn set_quick(&mut self, slot: usize, label: &str, command: &str) {
        if slot >= QUICK_SLOTS {
            return;
        }
        if self.quick_commands.len() < QUICK_SLOTS {
            self.quick_commands
                .resize(QUICK_SLOTS, QuickCommand::default());
        }
        let label = label.trim();
        let command = command.trim();
        self.quick_commands[slot] = if command.is_empty() {
            QuickCommand::default()
        } else {
            QuickCommand {
                label: if label.is_empty() {
                    command.to_owned()
                } else {
                    label.to_owned()
                },
                command: command.to_owned(),
            }
        };
    }

    pub fn look(&self) -> Look {
        Look {
            mode: self.theme,
            accent: self.accent,
            app_opacity: self.app_opacity.clamp(0.4, 1.0),
            term_opacity: self.term_opacity.clamp(0.4, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PaneLayout {
    #[default]
    Grid,
    Columns,
    Rows,
    Tabs,
    /// Panes the user places and sizes with the mouse.
    Free,
}

impl PaneLayout {
    pub fn label(self) -> &'static str {
        match self {
            PaneLayout::Grid => tr("layout.grid"),
            PaneLayout::Columns => tr("layout.columns"),
            PaneLayout::Rows => tr("layout.rows"),
            PaneLayout::Tabs => tr("layout.tabs"),
            PaneLayout::Free => tr("layout.free"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub projects: Vec<Project>,
    pub last_project: Option<u64>,
    pub settings: Settings,
    /// Folders the picker offers as shortcuts.
    pub recent_folders: Vec<PathBuf>,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "WorktreeHub", "worktree-hub")
            .map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("não foi possível descobrir a pasta de configuração")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("criando {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text).with_context(|| format!("escrevendo {}", path.display()))?;
        Ok(())
    }

    pub fn project(&self, id: u64) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn add_project(&mut self, name: String, root: PathBuf) -> u64 {
        if let Some(existing) = self.projects.iter().find(|p| p.root == root) {
            return existing.id;
        }
        let id = self.projects.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        self.projects.push(Project {
            id,
            name,
            root,
            worktree_dir: None,
        });
        id
    }

    pub fn remove_project(&mut self, id: u64) {
        self.projects.retain(|p| p.id != id);
        if self.last_project == Some(id) {
            self.last_project = self.projects.first().map(|p| p.id);
        }
    }

    pub fn remember_folder(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.recent_folders.retain(|p| p != &path);
        self.recent_folders.insert(0, path);
        self.recent_folders.truncate(8);
    }

    pub fn shell(&self) -> String {
        if self.settings.shell.trim().is_empty() {
            crate::external::default_shell()
        } else {
            self.settings.shell.trim().to_owned()
        }
    }
}

/// Filesystem-safe directory name for a branch (`feature/login` → `feature-login`).
pub fn slug(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    let mut last_dash = false;
    for ch in branch.chars() {
        let keep = ch.is_ascii_alphanumeric() || ch == '.' || ch == '_';
        if keep {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') || out.ends_with('.') {
        out.pop();
    }
    if out.is_empty() {
        "worktree".to_owned()
    } else {
        out
    }
}

/// Default path for a new worktree of `branch` in `project`.
pub fn suggest_path(project: &Project, branch: &str) -> PathBuf {
    project.worktree_base().join(slug(branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_command_takes_the_command_as_its_name_when_none_is_given() {
        let mut settings = Settings::default();
        settings.set_quick(0, "  ", " git status -sb ");
        let saved = settings.quick(0).expect("slot 0 was not filled");
        assert_eq!(saved.label, "git status -sb");
        assert_eq!(saved.command, "git status -sb");
    }

    #[test]
    fn an_empty_command_clears_the_slot_and_leaves_the_others_alone() {
        let mut settings = Settings::default();
        settings.set_quick(0, "build", "cargo build");
        settings.set_quick(1, "test", "cargo test");
        settings.set_quick(0, "build", "   ");
        assert!(settings.quick(0).is_none());
        assert_eq!(settings.quick(1).unwrap().command, "cargo test");
    }

    #[test]
    fn there_is_no_fourth_slot() {
        let mut settings = Settings::default();
        settings.set_quick(QUICK_SLOTS, "nope", "rm -rf /");
        assert_eq!(settings.quick_commands.len(), 0);
        settings.set_quick(2, "last", "echo last");
        assert_eq!(settings.quick_commands.len(), QUICK_SLOTS);
    }

    fn project() -> Project {
        Project {
            id: 1,
            name: "epicpen-linux".into(),
            root: PathBuf::from("/home/u/Projetos/epicpen-linux"),
            worktree_dir: None,
        }
    }

    #[test]
    fn slug_flattens_branch_names() {
        assert_eq!(slug("feature/login-v2"), "feature-login-v2");
        assert_eq!(slug("fix/#123 crash"), "fix-123-crash");
        assert_eq!(slug("release/1.2.0"), "release-1.2.0");
        assert_eq!(slug("///"), "worktree");
    }

    #[test]
    fn suggest_path_sits_next_to_the_repo() {
        let path = suggest_path(&project(), "feature/x");
        assert_eq!(
            path,
            PathBuf::from("/home/u/Projetos/epicpen-linux-worktrees/feature-x")
        );
    }

    #[test]
    fn suggest_path_honours_custom_base() {
        let mut p = project();
        p.worktree_dir = Some(PathBuf::from("/wt"));
        assert_eq!(suggest_path(&p, "main"), PathBuf::from("/wt/main"));
    }

    #[test]
    fn add_project_is_idempotent_per_root() {
        let mut cfg = Config::default();
        let a = cfg.add_project("x".into(), PathBuf::from("/r"));
        let b = cfg.add_project("x again".into(), PathBuf::from("/r"));
        assert_eq!(a, b);
        assert_eq!(cfg.projects.len(), 1);
    }

    #[test]
    fn recent_folders_are_deduped_and_capped() {
        let mut cfg = Config::default();
        for i in 0..12 {
            cfg.remember_folder(Path::new(&format!("/p{i}")));
        }
        cfg.remember_folder(Path::new("/p11"));
        assert_eq!(cfg.recent_folders.len(), 8);
        assert_eq!(cfg.recent_folders[0], PathBuf::from("/p11"));
    }

    #[test]
    fn defaults_round_trip_through_json() {
        let cfg = Config::default();
        let text = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(back.settings.term_font_size, cfg.settings.term_font_size);
    }
}
