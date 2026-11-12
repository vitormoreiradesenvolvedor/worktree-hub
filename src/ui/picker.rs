//! Built-in folder browser. Shipping our own keeps the binary free of GTK/portal
//! dependencies, which matters for a portable AppImage and a single-file .exe.

use std::path::{Path, PathBuf};

use egui::{Align, Layout, Sense, Ui, Vec2};

use super::components::{self as c, tokens};
use super::icons::Icon;
use crate::i18n::tr;
use crate::theme::{self, Weight};

pub struct Picker {
    pub current: PathBuf,
    pub entries: Vec<Entry>,
    pub manual: String,
    pub error: Option<String>,
}

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_repo: bool,
}

impl Picker {
    pub fn new(start: Option<PathBuf>) -> Self {
        let current = start
            .filter(|p| p.is_dir())
            .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("/"));
        let mut picker = Self {
            manual: current.to_string_lossy().into_owned(),
            current,
            entries: Vec::new(),
            error: None,
        };
        picker.read();
        picker
    }

    pub fn navigate(&mut self, path: PathBuf) {
        self.current = path;
        self.manual = self.current.to_string_lossy().into_owned();
        self.read();
    }

    pub fn read(&mut self) {
        self.entries.clear();
        self.error = None;

        match std::fs::read_dir(&self.current) {
            Ok(iter) => {
                for entry in iter.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') && name != ".git" {
                        continue;
                    }
                    if name == ".git" {
                        continue;
                    }
                    self.entries.push(Entry {
                        is_repo: is_repo(&path),
                        name,
                        path,
                    });
                }
                self.entries.sort_by(|a, b| {
                    b.is_repo
                        .cmp(&a.is_repo)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            Err(err) => self.error = Some(format!("{err}")),
        }
    }

    pub fn current_is_repo(&self) -> bool {
        is_repo(&self.current)
    }
}

/// A directory is offered as a repository when it holds a `.git` entry
/// (a directory for normal clones, a file for worktrees and submodules).
pub fn is_repo(path: &Path) -> bool {
    path.join(".git").exists() || path.join("HEAD").is_file() && path.join("objects").is_dir()
}

/// Draws the browser; returns a path when the user double-clicks a repository.
pub fn show(picker: &mut Picker, ui: &mut Ui) -> Option<PathBuf> {
    let t = tokens(ui);
    let mut chosen = None;

    ui.horizontal(|ui| {
        if c::icon_button(ui, Icon::Home, tr("dlg.home"), false).clicked() {
            if let Some(dirs) = directories::UserDirs::new() {
                picker.navigate(dirs.home_dir().to_path_buf());
            }
        }
        if c::icon_button(ui, Icon::ArrowUp, tr("dlg.up"), false).clicked() {
            if let Some(parent) = picker.current.parent().map(Path::to_path_buf) {
                picker.navigate(parent);
            }
        }
        let width = ui.available_width();
        let response = c::text_field(
            ui,
            &mut picker.manual,
            "/caminho/do/repositorio",
            width - 10.0,
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let typed = PathBuf::from(picker.manual.trim());
            if typed.is_dir() {
                picker.navigate(typed);
            } else {
                picker.error = Some(tr("dlg.folder_missing").to_owned());
            }
        }
    });

    ui.add_space(theme::S2);
    if let Some(error) = &picker.error {
        c::text_with(ui, error, 12.0, Weight::Medium, t.danger);
        ui.add_space(theme::S2);
    }

    egui::ScrollArea::vertical()
        .max_height(280.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if picker.entries.is_empty() {
                c::muted(ui, tr("dlg.no_subfolders"));
            }
            let mut navigate_to = None;
            for entry in &picker.entries {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
                let painter = ui.painter();
                if response.hovered() {
                    painter.rect_filled(
                        rect,
                        egui::CornerRadius::same(theme::R_INPUT),
                        t.surface_hover,
                    );
                }
                let icon_rect = egui::Rect::from_center_size(
                    egui::Pos2::new(rect.left() + 20.0, rect.center().y),
                    Vec2::splat(16.0),
                );
                super::icons::paint(
                    painter,
                    icon_rect,
                    if entry.is_repo {
                        Icon::Branch
                    } else {
                        Icon::Folder
                    },
                    if entry.is_repo { t.brand } else { t.text_dim },
                );
                painter.text(
                    egui::Pos2::new(icon_rect.right() + theme::S3, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &entry.name,
                    theme::font(13.0, Weight::Medium),
                    t.text,
                );
                if entry.is_repo {
                    painter.text(
                        egui::Pos2::new(rect.right() - theme::S2, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        tr("dlg.git_repo"),
                        theme::font(11.0, Weight::SemiBold),
                        t.brand,
                    );
                }

                if response.double_clicked() && entry.is_repo {
                    chosen = Some(entry.path.clone());
                } else if response.clicked() || response.double_clicked() {
                    navigate_to = Some(entry.path.clone());
                }
                response.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            if let Some(path) = navigate_to {
                picker.navigate(path);
            }
        });

    ui.add_space(theme::S2);
    ui.horizontal(|ui| {
        let is_repo = picker.current_is_repo();
        c::dot_label(
            ui,
            if is_repo { t.success } else { t.text_faint },
            if is_repo {
                tr("dlg.is_repo")
            } else {
                tr("dlg.not_repo")
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if c::Btn::primary(tr("dlg.use_folder"))
                .icon(Icon::Check)
                .enabled(is_repo)
                .show(ui)
                .clicked()
            {
                chosen = Some(picker.current.clone());
            }
        });
    });

    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_repo_by_its_git_entry() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_repo(dir.path()));
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(is_repo(dir.path()));
    }

    #[test]
    fn detects_a_bare_repo_layout() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir(dir.path().join("objects")).unwrap();
        assert!(is_repo(dir.path()));
    }

    #[test]
    fn hides_dotfolders_but_lists_normal_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".cache")).unwrap();
        std::fs::create_dir(dir.path().join("projeto")).unwrap();
        std::fs::write(dir.path().join("arquivo.txt"), "x").unwrap();

        let picker = Picker {
            current: dir.path().to_path_buf(),
            entries: Vec::new(),
            manual: String::new(),
            error: None,
        };
        let mut picker = picker;
        picker.read();
        let names: Vec<&str> = picker.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["projeto"]);
    }
}
