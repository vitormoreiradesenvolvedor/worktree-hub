//! Application state, background jobs and the frame loop.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Key, KeyboardShortcut, Modifiers, Rect};

use crate::config::{self, Config, PaneLayout};
use crate::external;
use crate::git::{self, Worktree, repo::BranchRef};
use crate::i18n::{self, tr, tr1};
use crate::term::{SpawnSpec, TermSize, TerminalSession};
use crate::theme::{self, Mode};
use crate::ui;
use crate::ui::tiles::Tile;

/// One worktree plus the extra information we fetch about it.
#[derive(Clone, Debug)]
pub struct Row {
    pub wt: Worktree,
    pub status: Option<git::Status>,
    pub commit: Option<git::Commit>,
}

impl Row {
    pub fn matches(&self, needle: &str) -> bool {
        if needle.trim().is_empty() {
            return true;
        }
        let needle = needle.to_lowercase();
        self.wt.name().to_lowercase().contains(&needle)
            || self
                .wt
                .branch
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle)
            || self
                .wt
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(&needle)
    }
}

/// Result of a background git job.
pub enum JobResult {
    Loaded {
        project: u64,
        rows: Vec<Row>,
        branches: Vec<BranchRef>,
        base: String,
    },
    Done {
        project: u64,
        message: String,
        created: Option<PathBuf>,
    },
    Failed {
        message: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Worktrees,
    Terminals,
    Settings,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub born: f64,
}

#[derive(Clone, Debug, Default)]
pub struct CreateForm {
    pub branch: String,
    pub create_branch: bool,
    pub base: String,
    pub path: String,
    /// Once the user edits the path we stop deriving it from the branch name.
    pub path_edited: bool,
    pub detach: bool,
    pub force: bool,
    pub force_branch: bool,
    pub no_checkout: bool,
    pub track: bool,
    pub open_terminal: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DeleteForm {
    pub path: PathBuf,
    pub name: String,
    pub branch: Option<String>,
    pub force: bool,
    pub also_branch: bool,
}

/// Choosing which editor opens a worktree.
pub struct EditorPick {
    pub path: PathBuf,
    pub apps: Vec<crate::apps::DesktopApp>,
    /// Save the pick as the editor from now on.
    pub remember: bool,
}

/// Editing one of the saved broadcast buttons.
pub struct QuickForm {
    pub slot: usize,
    pub label: String,
    pub command: String,
}

pub enum Dialog {
    None,
    AddProject(ui::picker::Picker),
    Create(CreateForm),
    Delete(DeleteForm),
    RemoveProject(u64),
    Quick(QuickForm),
    Editor(EditorPick),
}

impl Dialog {
    pub fn is_open(&self) -> bool {
        !matches!(self, Dialog::None)
    }
}

/// Everything a view can ask the app to do. Views only push actions, so they
/// never need mutable access to the whole app while iterating its data.
pub enum Action {
    SetScreen(Screen),
    SelectProject(u64),
    AddProjectDialog,
    RemoveProjectDialog(u64),
    Refresh,
    CreateDialog,
    DeleteDialog(PathBuf),
    Prune,
    ToggleSelect(PathBuf),
    ClearSelection,
    SelectAll,
    OpenTerminals(Vec<PathBuf>),
    OpenTerminal(PathBuf),
    ExternalTerminal(PathBuf),
    OpenEditor(PathBuf),
    /// Opens the worktree in the app the user picked from the menu.
    OpenEditorWith {
        path: PathBuf,
        app: crate::apps::DesktopApp,
        remember: bool,
    },
    OpenFolder(PathBuf),
    CopyPath(PathBuf),
    ToggleLock(PathBuf, bool),
    CloseSession(u64),
    RestartSession(u64),
    CloseAllSessions,
    SetLayout(PaneLayout),
    /// Free layout: throw the arrangement away and tile the panes evenly.
    TidyPanes,
    /// Free layout: blow one pane up to the whole area, or put it back.
    MaximizePane(u64),
    /// Window fullscreen, for terminals on the whole screen.
    ToggleFullscreen,
    FocusPane(usize),
    Broadcast(String),
    /// Sends the command saved in that slot to every pane.
    RunQuick(usize),
    /// Opens the editor for that slot.
    QuickDialog(usize),
    SaveQuick {
        slot: usize,
        label: String,
        command: String,
    },
    SubmitCreate,
    ConfirmDelete,
    CancelDialog,
    AddProjectPath(PathBuf),
    SetTheme(Mode),
    /// Re-read the appearance settings; `fonts` also rebuilds the font atlas.
    ApplyLook {
        fonts: bool,
    },
    ToggleSidebar,
    SaveConfig,
}

pub struct App {
    pub cfg: Config,
    pub screen: Screen,
    pub rows: Vec<Row>,
    pub branches: Vec<BranchRef>,
    pub base_suggestion: String,
    pub selected: HashSet<PathBuf>,
    pub search: String,
    pub loading: bool,
    pub busy: bool,
    pub dialog: Dialog,
    /// Decoded application icons, kept so the picker does not read them again
    /// on every frame.
    pub icons: std::collections::HashMap<PathBuf, Option<egui::TextureHandle>>,
    pub toasts: Vec<Toast>,
    pub sessions: Vec<TerminalSession>,
    pub focused_pane: usize,
    pub broadcast: bool,
    pub broadcast_input: String,
    pub active_tab: usize,
    /// Free layout: how the panes divide the area between them.
    pub tiles: Option<Tile>,
    /// A pane blown up to the whole area; the tiling is kept underneath.
    pub maximized: Option<u64>,
    /// Where the panes were laid out last frame.
    pub pane_area: Rect,
    /// Window is in fullscreen (the terminals' "big screen" mode).
    pub fullscreen: bool,
    /// How far the current screen is into its entrance animation, 0..1.
    pub screen_fade: f32,
    next_session_id: u64,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open_terminals: &[PathBuf]) -> Self {
        Self::new_with_ctx(&cc.egui_ctx, open_terminals)
    }

    /// Context-only constructor, so the whole UI can also run headless in tests.
    /// `open_terminals` are worktree paths to open panes for right away.
    pub fn new_with_ctx(ctx: &egui::Context, open_terminals: &[PathBuf]) -> Self {
        let cfg = Config::load();
        i18n::set_lang(cfg.settings.lang);
        theme::install_fonts(ctx, cfg.settings.ui_font, cfg.settings.mono_font);
        theme::apply(ctx, cfg.settings.look());
        ctx.set_zoom_factor(cfg.settings.ui_scale.clamp(0.7, 1.6));
        ui::components::set_tokens(ctx, theme::tokens(cfg.settings.look()));

        let (tx, rx) = channel();
        let mut app = Self {
            screen: Screen::Worktrees,
            rows: Vec::new(),
            branches: Vec::new(),
            base_suggestion: String::new(),
            selected: HashSet::new(),
            search: String::new(),
            loading: false,
            busy: false,
            dialog: Dialog::None,
            icons: std::collections::HashMap::new(),
            toasts: Vec::new(),
            sessions: Vec::new(),
            focused_pane: 0,
            broadcast: false,
            broadcast_input: String::new(),
            active_tab: 0,
            tiles: None,
            maximized: None,
            pane_area: Rect::ZERO,
            fullscreen: false,
            screen_fade: 1.0,
            next_session_id: 1,
            tx,
            rx,
            cfg,
        };

        if app.cfg.last_project.is_none() {
            app.cfg.last_project = app.cfg.projects.first().map(|p| p.id);
        }
        if !git::cmd::available() {
            app.toast(tr("msg.git_missing"), ToastKind::Error);
        }
        app.start_refresh(ctx);

        for path in open_terminals {
            if path.is_dir() {
                app.spawn_session(ctx, path, None);
            } else {
                app.toast(tr1("msg.not_exist", path.display()), ToastKind::Error);
            }
        }
        if !app.sessions.is_empty() {
            app.screen = Screen::Terminals;
        }
        app
    }

    pub fn current_project(&self) -> Option<&config::Project> {
        self.cfg.last_project.and_then(|id| self.cfg.project(id))
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            born: 0.0,
        });
    }

    // ------------------------------------------------------------------ jobs

    pub fn start_refresh(&mut self, ctx: &egui::Context) {
        let Some(project) = self.current_project().cloned() else {
            self.rows.clear();
            self.branches.clear();
            return;
        };
        self.loading = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<JobResult> {
                let worktrees = git::worktree::list(&project.root)?;
                let rows = worktrees
                    .into_iter()
                    .map(|wt| {
                        let (status, commit) = if wt.exists() && !wt.bare {
                            (
                                git::repo::status(&wt.path).ok(),
                                git::repo::last_commit(&wt.path).ok().flatten(),
                            )
                        } else {
                            (None, None)
                        };
                        Row { wt, status, commit }
                    })
                    .collect();
                let branches = git::repo::branches(&project.root).unwrap_or_default();
                let base = git::repo::default_base(&project.root, &branches);
                Ok(JobResult::Loaded {
                    project: project.id,
                    rows,
                    branches,
                    base,
                })
            })();

            let message = match result {
                Ok(ok) => ok,
                Err(err) => JobResult::Failed {
                    message: format!("{err}"),
                },
            };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
    }

    /// Runs a git operation off the UI thread and refreshes afterwards.
    /// `label` is a translation key used when the operation says nothing.
    fn run_op<F>(&mut self, ctx: &egui::Context, label: &'static str, op: F)
    where
        F: FnOnce(&Path) -> anyhow::Result<(String, Option<PathBuf>)> + Send + 'static,
    {
        let Some(project) = self.current_project().cloned() else {
            return;
        };
        self.busy = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = op(&project.root);
            let message = match result {
                Ok((message, created)) => JobResult::Done {
                    project: project.id,
                    message: if message.trim().is_empty() {
                        tr(label).to_owned()
                    } else {
                        message
                    },
                    created,
                },
                Err(err) => JobResult::Failed {
                    message: format!("{err}"),
                },
            };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
    }

    fn poll_jobs(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.rx.try_recv() {
            match result {
                JobResult::Loaded {
                    project,
                    rows,
                    branches,
                    base,
                } => {
                    self.loading = false;
                    if self.cfg.last_project == Some(project) {
                        let alive: HashSet<PathBuf> =
                            rows.iter().map(|r| r.wt.path.clone()).collect();
                        self.selected.retain(|p| alive.contains(p));
                        self.rows = rows;
                        self.branches = branches;
                        self.base_suggestion = base;
                    }
                }
                JobResult::Done {
                    project,
                    message,
                    created,
                } => {
                    self.busy = false;
                    self.toast(message, ToastKind::Success);
                    if let Some(path) = created {
                        if self.cfg.settings.open_terminal_after_create {
                            self.spawn_session(ctx, &path, None);
                            self.screen = Screen::Terminals;
                        }
                    }
                    if self.cfg.last_project == Some(project) {
                        self.start_refresh(ctx);
                    }
                }
                JobResult::Failed { message } => {
                    self.loading = false;
                    self.busy = false;
                    self.toast(message, ToastKind::Error);
                }
            }
        }
    }

    // -------------------------------------------------------------- sessions

    fn spawn_session(&mut self, ctx: &egui::Context, path: &Path, branch: Option<String>) {
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let branch = branch.unwrap_or_else(|| {
            self.rows
                .iter()
                .find(|r| r.wt.path == path)
                .map(|r| r.wt.ref_label())
                .or_else(|| {
                    // Panes opened before the list finished loading (or straight
                    // from the CLI) still deserve a branch label.
                    git::repo::status(path).ok().and_then(|s| s.branch)
                })
                .unwrap_or_default()
        });

        // The repository itself and its worktrees look alike in a pane header,
        // so the session remembers which one it is.
        let is_main = self
            .rows
            .iter()
            .find(|r| r.wt.path == path)
            .map(|r| r.wt.is_main)
            .unwrap_or_else(|| self.current_project().is_some_and(|p| p.root == path));

        // Which project this folder belongs to: the one whose repository is
        // this very path, or the one it was created from.
        let owner = self
            .cfg
            .projects
            .iter()
            .find(|project| path == project.root || path.starts_with(&project.root))
            .or_else(|| {
                self.cfg
                    .projects
                    .iter()
                    .find(|project| Some(project.id) == self.cfg.last_project)
            })
            .map(|project| (project.id, project.name.clone()));

        let shell = self.cfg.shell();
        let spec = SpawnSpec {
            id: self.next_session_id,
            label,
            branch,
            project: owner
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_default(),
            project_id: owner.as_ref().map(|(id, _)| *id),
            is_main,
            cwd: path,
            shell: &shell,
            scrollback: self.cfg.settings.scrollback,
            size: TermSize::default(),
        };
        match TerminalSession::spawn(ctx, spec) {
            Ok(session) => {
                self.next_session_id += 1;
                self.sessions.push(session);
                self.focused_pane = self.sessions.len() - 1;
                self.active_tab = self.focused_pane;
                // The sidebar follows the pane: a terminal always says which
                // project it came from.
                if let Some((id, _)) = owner {
                    self.select_project(ctx, id);
                }
            }
            Err(err) => self.toast(format!("{err}"), ToastKind::Error),
        }
    }

    /// Makes that project the current one, reloading its worktrees.
    fn select_project(&mut self, ctx: &egui::Context, id: u64) {
        if self.cfg.last_project == Some(id) {
            return;
        }
        self.cfg.last_project = Some(id);
        self.selected.clear();
        self.rows.clear();
        self.search.clear();
        let _ = self.cfg.save();
        self.start_refresh(ctx);
    }

    fn close_session(&mut self, id: u64) {
        if let Some(index) = self.sessions.iter().position(|s| s.id == id) {
            let mut session = self.sessions.remove(index);
            session.kill();
        }
        self.tiles = ui::tiles::remove(self.tiles.take(), id);
        if self.maximized == Some(id) {
            self.maximized = None;
        }
        self.focused_pane = self.focused_pane.min(self.sessions.len().saturating_sub(1));
        self.active_tab = self.active_tab.min(self.sessions.len().saturating_sub(1));
        if self.sessions.is_empty() {
            self.screen = Screen::Worktrees;
        }
    }

    // --------------------------------------------------------------- actions

    pub fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::SetScreen(screen) => self.screen = screen,
            Action::SelectProject(id) => {
                self.select_project(ctx, id);
                self.screen = Screen::Worktrees;
            }
            Action::AddProjectDialog => {
                self.dialog = Dialog::AddProject(ui::picker::Picker::new(
                    self.cfg.recent_folders.first().cloned(),
                ));
            }
            Action::RemoveProjectDialog(id) => self.dialog = Dialog::RemoveProject(id),
            Action::Refresh => self.start_refresh(ctx),
            Action::CreateDialog => {
                let base = if self.base_suggestion.is_empty() {
                    "HEAD".to_owned()
                } else {
                    self.base_suggestion.clone()
                };
                self.dialog = Dialog::Create(CreateForm {
                    branch: String::new(),
                    create_branch: true,
                    base,
                    path: String::new(),
                    path_edited: false,
                    track: false,
                    open_terminal: self.cfg.settings.open_terminal_after_create,
                    ..Default::default()
                });
            }
            Action::DeleteDialog(path) => {
                if let Some(row) = self.rows.iter().find(|r| r.wt.path == path) {
                    self.dialog = Dialog::Delete(DeleteForm {
                        path: row.wt.path.clone(),
                        name: row.wt.name(),
                        branch: row.wt.branch.clone(),
                        force: row.status.as_ref().is_some_and(|s| s.dirty()),
                        also_branch: false,
                    });
                }
            }
            Action::Prune => self.run_op(ctx, "msg.pruned", |repo| {
                let out = git::worktree::prune(repo)?;
                Ok((
                    if out.trim().is_empty() {
                        tr("msg.nothing_to_clean").to_owned()
                    } else {
                        out.trim().to_owned()
                    },
                    None,
                ))
            }),
            Action::ToggleSelect(path) => {
                if !self.selected.remove(&path) {
                    self.selected.insert(path);
                }
            }
            Action::ClearSelection => self.selected.clear(),
            Action::SelectAll => {
                let search = self.search.clone();
                self.selected = self
                    .rows
                    .iter()
                    .filter(|r| r.wt.exists() && r.matches(&search))
                    .map(|r| r.wt.path.clone())
                    .collect();
            }
            Action::OpenTerminals(paths) => {
                for path in paths {
                    if self.sessions.iter().all(|s| s.cwd != path) {
                        self.spawn_session(ctx, &path, None);
                    }
                }
                if !self.sessions.is_empty() {
                    self.screen = Screen::Terminals;
                }
            }
            Action::OpenTerminal(path) => {
                self.spawn_session(ctx, &path, None);
                self.screen = Screen::Terminals;
            }
            Action::ExternalTerminal(path) => {
                if let Err(err) = external::open_terminal(&path, &self.cfg.settings.terminal_cmd) {
                    self.toast(format!("{err}"), ToastKind::Error);
                }
            }
            Action::OpenEditor(path) => {
                // A command set by hand in the settings wins; otherwise the
                // desktop's own editors are offered, so the choice can change
                // from one worktree to the next.
                if !self.cfg.settings.editor_cmd.trim().is_empty() {
                    if let Err(err) = external::open_editor(&path, &self.cfg.settings.editor_cmd) {
                        self.toast(format!("{err}"), ToastKind::Error);
                    }
                    return;
                }
                let apps = crate::apps::editors();
                if apps.is_empty() {
                    self.toast(tr("dlg.editor_none"), ToastKind::Error);
                    return;
                }
                self.dialog = Dialog::Editor(EditorPick {
                    path,
                    apps,
                    remember: false,
                });
            }
            Action::OpenEditorWith {
                path,
                app,
                remember,
            } => {
                self.dialog = Dialog::None;
                if remember {
                    self.cfg.settings.editor_cmd = app.template();
                    let _ = self.cfg.save();
                }
                if let Err(err) = external::open_with(&app.exec, &path) {
                    self.toast(format!("{err}"), ToastKind::Error);
                }
            }
            Action::OpenFolder(path) => {
                if let Err(err) = external::open_folder(&path) {
                    self.toast(format!("{err}"), ToastKind::Error);
                }
            }
            Action::CopyPath(path) => {
                ctx.copy_text(path.to_string_lossy().into_owned());
                self.toast(tr("msg.path_copied"), ToastKind::Info);
            }
            Action::ToggleLock(path, lock) => {
                self.run_op(ctx, "msg.wt_updated", move |repo| {
                    if lock {
                        git::worktree::lock(repo, &path, "locked by Worktree Hub")?;
                        Ok((tr("msg.wt_locked").to_owned(), None))
                    } else {
                        git::worktree::unlock(repo, &path)?;
                        Ok((tr("msg.wt_unlocked").to_owned(), None))
                    }
                });
            }
            Action::CloseSession(id) => self.close_session(id),
            Action::RestartSession(id) => {
                if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
                    let path = session.cwd.clone();
                    let branch = session.branch.clone();
                    self.close_session(id);
                    self.spawn_session(ctx, &path, Some(branch));
                    self.screen = Screen::Terminals;
                }
            }
            Action::CloseAllSessions => {
                for mut session in self.sessions.drain(..) {
                    session.kill();
                }
                self.tiles = None;
                self.maximized = None;
                self.focused_pane = 0;
                self.active_tab = 0;
                self.screen = Screen::Worktrees;
            }
            Action::SetLayout(layout) => {
                self.cfg.settings.pane_layout = layout;
                let _ = self.cfg.save();
            }
            Action::TidyPanes => {
                let ids: Vec<u64> = self.sessions.iter().map(|s| s.id).collect();
                self.tiles = Tile::balanced(&ids);
                self.maximized = None;
            }
            Action::MaximizePane(id) => {
                self.maximized = (self.maximized != Some(id)).then_some(id);
            }
            Action::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::FocusPane(index) => {
                self.focused_pane = index;
                self.active_tab = index;
                // The sidebar follows the pane in use, so the project a
                // terminal belongs to is never in doubt.
                if let Some(id) = self.sessions.get(index).and_then(|s| s.project_id) {
                    self.select_project(ctx, id);
                }
            }
            Action::Broadcast(line) => {
                let mut bytes = line.into_bytes();
                bytes.push(b'\r');
                for session in &self.sessions {
                    session.write(&bytes);
                }
            }
            Action::RunQuick(slot) => {
                if let Some(quick) = self.cfg.settings.quick(slot) {
                    let mut bytes = quick.command.clone().into_bytes();
                    bytes.push(b'\r');
                    for session in &self.sessions {
                        session.write(&bytes);
                    }
                }
            }
            Action::QuickDialog(slot) => {
                let saved = self.cfg.settings.quick(slot);
                self.dialog = Dialog::Quick(QuickForm {
                    slot,
                    label: saved.map(|q| q.label.clone()).unwrap_or_default(),
                    command: saved.map(|q| q.command.clone()).unwrap_or_default(),
                });
            }
            Action::SaveQuick {
                slot,
                label,
                command,
            } => {
                self.cfg.settings.set_quick(slot, &label, &command);
                self.dialog = Dialog::None;
                if let Err(err) = self.cfg.save() {
                    self.toast(format!("{err}"), ToastKind::Error);
                }
            }
            Action::SubmitCreate => self.submit_create(ctx),
            Action::ConfirmDelete => self.confirm_delete(ctx),
            Action::CancelDialog => self.dialog = Dialog::None,
            Action::AddProjectPath(path) => self.add_project(ctx, &path),
            Action::SetTheme(mode) => {
                self.cfg.settings.theme = mode;
                self.refresh_look(ctx, false);
            }
            Action::ApplyLook { fonts } => self.refresh_look(ctx, fonts),
            Action::ToggleSidebar => {
                self.cfg.settings.sidebar_collapsed = !self.cfg.settings.sidebar_collapsed;
                let _ = self.cfg.save();
            }
            Action::SaveConfig => {
                if let Err(err) = self.cfg.save() {
                    self.toast(format!("{err}"), ToastKind::Error);
                }
            }
        }
    }

    /// Pushes the appearance settings into egui and saves them.
    fn refresh_look(&mut self, ctx: &egui::Context, fonts: bool) {
        let settings = &self.cfg.settings;
        i18n::set_lang(settings.lang);
        if fonts {
            theme::install_fonts(ctx, settings.ui_font, settings.mono_font);
        }
        theme::apply(ctx, settings.look());
        ctx.set_zoom_factor(settings.ui_scale.clamp(0.7, 1.6));
        ui::components::set_tokens(ctx, theme::tokens(settings.look()));
        let _ = self.cfg.save();
    }

    fn add_project(&mut self, ctx: &egui::Context, path: &Path) {
        match git::repo::discover(path) {
            Ok(info) => {
                // The project is named after the folder the user chose, even
                // when the repository root sits somewhere above it.
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().trim_end_matches(".git").to_owned())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| info.name.clone());
                let id = self.cfg.add_project(name.clone(), info.root.clone());
                self.cfg
                    .remember_folder(info.root.parent().unwrap_or(&info.root));
                self.cfg.last_project = Some(id);
                let _ = self.cfg.save();
                self.dialog = Dialog::None;
                self.selected.clear();
                self.rows.clear();
                if info.bare {
                    self.toast(tr1("msg.bare", &name), ToastKind::Info);
                } else {
                    self.toast(tr1("msg.added", &name), ToastKind::Success);
                }
                self.start_refresh(ctx);
            }
            Err(err) => self.toast(format!("{err}"), ToastKind::Error),
        }
    }

    fn submit_create(&mut self, ctx: &egui::Context) {
        let project = self.current_project().cloned();
        let Dialog::Create(form) = &mut self.dialog else {
            return;
        };
        let branch = form.branch.trim().to_owned();
        if branch.is_empty() && !form.detach {
            form.error = Some(tr("msg.branch_required").to_owned());
            return;
        }
        let path = if form.path.trim().is_empty() {
            match project.as_ref() {
                Some(project) => config::suggest_path(project, &branch),
                None => return,
            }
        } else {
            PathBuf::from(form.path.trim())
        };
        if path.exists() && !form.force {
            form.error = Some(tr1("msg.path_exists", path.display()));
            return;
        }

        let opts = git::AddOptions {
            path: path.clone(),
            branch,
            create_branch: form.create_branch,
            base: form.base.trim().to_owned(),
            detach: form.detach,
            force: form.force,
            force_branch: form.force_branch,
            no_checkout: form.no_checkout,
            track: form.track,
        };
        let open_terminal = form.open_terminal;
        self.dialog = Dialog::None;
        self.cfg.settings.open_terminal_after_create = open_terminal;

        let created = path.clone();
        self.run_op(ctx, "msg.wt_created", move |repo| {
            git::worktree::add(repo, &opts)?;
            Ok((tr1("msg.wt_created_at", opts.path.display()), Some(created)))
        });
    }

    fn confirm_delete(&mut self, ctx: &egui::Context) {
        let Dialog::Delete(form) = &self.dialog else {
            return;
        };
        let form = form.clone();
        self.dialog = Dialog::None;
        self.selected.remove(&form.path);
        if let Some(index) = self.sessions.iter().position(|s| s.cwd == form.path) {
            let id = self.sessions[index].id;
            self.close_session(id);
        }

        self.run_op(ctx, "msg.wt_removed", move |repo| {
            git::worktree::remove(repo, &form.path, form.force)?;
            let mut message = tr1("msg.removed", &form.name);
            if form.also_branch {
                if let Some(branch) = &form.branch {
                    match git::worktree::delete_branch(repo, branch, true) {
                        Ok(_) => message.push_str(tr("msg.branch_deleted")),
                        Err(err) => message.push_str(&tr1("msg.branch_kept", err)),
                    }
                }
            }
            Ok((message, None))
        });
    }

    // ---------------------------------------------------------------- frame

    fn shortcuts(&mut self, ctx: &egui::Context) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.dialog.is_open() {
            return actions;
        }

        let hit = |ctx: &egui::Context, mods: Modifiers, key: Key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key)))
        };

        if hit(ctx, Modifiers::COMMAND, Key::N) {
            actions.push(Action::CreateDialog);
        }
        // On the terminals screen Ctrl+R belongs to the shell (reverse-i-search);
        // F5 still refreshes from anywhere.
        let in_terminals = self.screen == Screen::Terminals;
        if (!in_terminals && hit(ctx, Modifiers::COMMAND, Key::R))
            || hit(ctx, Modifiers::NONE, Key::F5)
        {
            actions.push(Action::Refresh);
        }
        if hit(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::T) {
            let paths: Vec<PathBuf> = self.selected.iter().cloned().collect();
            if !paths.is_empty() {
                actions.push(Action::OpenTerminals(paths));
            }
        }
        if hit(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::W) {
            if let Some(session) = self.sessions.get(self.focused_pane) {
                actions.push(Action::CloseSession(session.id));
            }
        }
        if hit(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::B) {
            self.broadcast = !self.broadcast;
        }
        if hit(ctx, Modifiers::NONE, Key::F9) {
            actions.push(Action::ToggleSidebar);
        }
        if hit(ctx, Modifiers::NONE, Key::F11) {
            actions.push(Action::ToggleFullscreen);
        }
        if hit(ctx, Modifiers::COMMAND, Key::Num1) {
            actions.push(Action::SetScreen(Screen::Worktrees));
        }
        if hit(ctx, Modifiers::COMMAND, Key::Num2) && !self.sessions.is_empty() {
            actions.push(Action::SetScreen(Screen::Terminals));
        }
        if hit(ctx, Modifiers::COMMAND, Key::Num3) {
            actions.push(Action::SetScreen(Screen::Settings));
        }
        if hit(ctx, Modifiers::COMMAND, Key::Num4) {
            actions.push(Action::SetScreen(Screen::About));
        }
        actions
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        for toast in &mut self.toasts {
            if toast.born == 0.0 {
                toast.born = now;
            }
        }
        self.toasts.retain(|t| now - t.born < 6.0);
        if self.toasts.is_empty() {
            return;
        }

        let tokens = theme::tokens(self.cfg.settings.look());
        egui::Area::new(egui::Id::unique("worktree-hub-toasts"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::vec2(-theme::S5, -theme::S5),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for toast in self.toasts.iter().rev().take(4) {
                        // Slide up on the way in, fade out on the way out.
                        let life = (now - toast.born) as f32;
                        let enter = (life / 0.18).clamp(0.0, 1.0);
                        let leave = ((6.0 - life) / 0.4).clamp(0.0, 1.0);
                        ui.set_opacity(enter * leave);
                        ui.add_space((1.0 - enter) * 10.0);
                        let (fg, bg, icon) = match toast.kind {
                            ToastKind::Success => {
                                (tokens.success, tokens.success_tint, ui::Icon::Check)
                            }
                            ToastKind::Error => {
                                (tokens.danger, tokens.danger_tint, ui::Icon::Warning)
                            }
                            ToastKind::Info => (tokens.brand, tokens.brand_tint, ui::Icon::Dot),
                        };
                        egui::Frame::new()
                            .fill(bg)
                            .stroke(egui::Stroke::new(1.0, fg.gamma_multiply(0.35)))
                            .corner_radius(egui::CornerRadius::same(theme::R_INPUT))
                            .shadow(theme::card_shadow(tokens))
                            .inner_margin(egui::Margin::symmetric(theme::S4 as i8, theme::S3 as i8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::Vec2::splat(16.0),
                                        egui::Sense::hover(),
                                    );
                                    ui::icons::paint(ui.painter(), rect, icon, fg);
                                    ui.set_max_width(420.0);
                                    ui.label(
                                        egui::RichText::new(&toast.text)
                                            .font(theme::font(12.5, theme::Weight::Medium))
                                            .color(tokens.text),
                                    );
                                });
                            });
                        ui.add_space(theme::S2);
                    }
                });
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }

    /// The window itself is painted with the background the theme asks for, so
    /// the transparency setting reaches the parts egui never draws over.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::tokens(self.cfg.settings.look())
            .bg
            .to_normalized_gamma_f32()
    }
}

impl App {
    /// One frame of the whole interface. Split out of `eframe::App::ui` so tests
    /// can drive it through `egui::Context::run_ui`.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_jobs(&ctx);
        ui::components::set_tokens(&ctx, theme::tokens(self.cfg.settings.look()));

        let mut actions = self.shortcuts(&ctx);
        let tokens = theme::tokens(self.cfg.settings.look());

        ui::sidebar::show(self, ui, &mut actions);

        // Screens fade and glide in; every screen keeps its own animation, so
        // coming back to one plays the entrance again.
        let current = self.screen;
        let mut fade = 1.0;
        for screen in [
            Screen::Worktrees,
            Screen::Terminals,
            Screen::Settings,
            Screen::About,
        ] {
            let value = ctx.animate_bool_with_time(
                egui::Id::unique(("screen-enter", screen as u8)),
                screen == current,
                0.18,
            );
            if screen == current {
                fade = value;
            }
        }
        self.screen_fade = fade;
        let slide = (1.0 - fade) * 10.0;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(tokens.bg)
                    .inner_margin(egui::Margin::same(theme::S5 as i8)),
            )
            .show(ui, |ui| {
                ui.set_opacity(fade.max(0.05));
                let inner = ui.available_rect_before_wrap();
                let builder = egui::UiBuilder::new()
                    .max_rect(inner.translate(egui::Vec2::new(0.0, slide)))
                    .id_salt(("screen", current as u8));
                ui.scope_builder(builder, |ui| match current {
                    Screen::Worktrees => ui::worktrees::show(self, ui, &mut actions),
                    Screen::Terminals => ui::terminals::show(self, ui, &mut actions),
                    Screen::Settings => ui::settings::show(self, ui, &mut actions),
                    Screen::About => ui::about::show(self, ui, &mut actions),
                });
            });

        ui::dialogs::show(self, &ctx, &mut actions);
        self.toasts(&ctx);

        for action in actions {
            self.apply(action, &ctx);
        }

        // Keep repainting while terminals are alive so output shows up promptly
        // even when the PTY thread's repaint request races with this frame.
        if !self.sessions.is_empty() && self.screen == Screen::Terminals {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        for session in &mut self.sessions {
            session.kill();
        }
        let _ = self.cfg.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::Worktree;

    fn row(name: &str, branch: &str) -> Row {
        Row {
            wt: Worktree {
                path: PathBuf::from(format!("/wt/{name}")),
                branch: Some(branch.to_owned()),
                head: Some("abcdef1234".into()),
                ..Default::default()
            },
            status: None,
            commit: None,
        }
    }

    #[test]
    fn empty_search_matches_everything() {
        assert!(row("a", "main").matches(""));
        assert!(row("a", "main").matches("   "));
    }

    #[test]
    fn search_matches_name_branch_and_path() {
        let r = row("feature-login", "feature/login");
        assert!(r.matches("LOGIN"));
        assert!(r.matches("feature/"));
        assert!(r.matches("/wt/"));
        assert!(!r.matches("hotfix"));
    }
}
