//! Drives the complete interface headlessly, one real egui pass at a time.
//!
//! This is the test that would have caught a panic in layout code, a bad widget
//! id, or a regression in the create/remove flow — without opening a window.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use egui::{Event, PointerButton, Pos2, Vec2, pos2, vec2};
use worktree_hub::app::{Action, App, Dialog, Screen};
use worktree_hub::config::PaneLayout;
use worktree_hub::i18n::Lang;
use worktree_hub::theme::{self, Mode};

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Teste")
        .env("GIT_AUTHOR_EMAIL", "teste@example.com")
        .env("GIT_COMMITTER_NAME", "Teste")
        .env("GIT_COMMITTER_EMAIL", "teste@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git deve estar instalado");
    assert!(
        out.status.success(),
        "git {args:?} falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Frames share one clock: egui refuses to see time move backwards.
fn tick(step: f64) -> f64 {
    thread_local! {
        static CLOCK: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }
    CLOCK.with(|clock| {
        clock.set(clock.get() + step);
        clock.get()
    })
}

/// One UI pass at a realistic window size, carrying raw input events.
fn pass(ctx: &egui::Context, app: &mut App, step: f64, events: Vec<Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 820.0))),
        time: Some(tick(step)),
        events,
        ..Default::default()
    };
    let output = ctx.run_ui(input, |ui| app.render(ui));
    output.drop_without_applying_deltas();
}

fn frame(ctx: &egui::Context, app: &mut App) {
    pass(ctx, app, 0.016, Vec::new());
}

fn frames(ctx: &egui::Context, app: &mut App, count: usize) {
    for _ in 0..count {
        frame(ctx, app);
    }
}

/// Runs frames until `done` holds, so background git jobs can land.
fn frames_until(ctx: &egui::Context, app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        frame(ctx, app);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!("tempo esgotado esperando: {what}");
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Clicks where a real pointer would, so the widget under it takes focus.
fn click(ctx: &egui::Context, app: &mut App, at: Pos2) {
    for events in [
        vec![Event::PointerMoved(at)],
        vec![press(at, true)],
        vec![press(at, false)],
        Vec::new(),
    ] {
        pass(ctx, app, 0.05, events);
    }
}

/// Press, move, release — the gesture that moves and resizes free panes.
fn drag(ctx: &egui::Context, app: &mut App, from: Pos2, delta: Vec2) {
    let mid = from + delta * 0.5;
    let to = from + delta;
    for events in [
        vec![Event::PointerMoved(from)],
        vec![press(from, true)],
        vec![Event::PointerMoved(mid)],
        vec![Event::PointerMoved(to)],
        vec![press(to, false)],
        Vec::new(),
    ] {
        pass(ctx, app, 0.05, events);
    }
}

#[test]
fn every_screen_and_dialog_renders_and_the_flows_work() {
    let tmp = tempfile::tempdir().unwrap();
    // Keep the test away from the real user configuration.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    }

    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("README.md"), "# teste\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "commit inicial"]);
    git(&repo, &["branch", "develop"]);

    let ctx = egui::Context::default();
    let mut app = App::new_with_ctx(&ctx, &[]);

    // ---- empty state ---------------------------------------------------
    frames(&ctx, &mut app, 2);
    assert!(app.cfg.projects.is_empty());

    // ---- add project through the real action ---------------------------
    app.apply(Action::AddProjectPath(repo.clone()), &ctx);
    frames_until(&ctx, &mut app, "worktree principal listado", |app| {
        app.rows.len() == 1
    });
    assert_eq!(app.cfg.projects.len(), 1);
    assert!(app.rows[0].wt.is_main);
    assert!(app.rows[0].status.is_some(), "status carregado");
    assert!(app.branches.iter().any(|b| b.name == "develop"));

    // ---- create a worktree through the dialog --------------------------
    app.apply(Action::CreateDialog, &ctx);
    frames(&ctx, &mut app, 2);
    let target = tmp.path().join("wt-feature");
    match &mut app.dialog {
        Dialog::Create(form) => {
            form.branch = "feature/ui".to_owned();
            form.create_branch = true;
            form.base = "main".to_owned();
            form.path = target.to_string_lossy().into_owned();
            form.path_edited = true;
        }
        _ => panic!("diálogo de criação deveria estar aberto"),
    }
    frames(&ctx, &mut app, 2);
    app.apply(Action::SubmitCreate, &ctx);
    frames_until(&ctx, &mut app, "worktree nova na lista", |app| {
        app.rows.len() == 2
    });
    assert!(target.join("README.md").is_file());
    assert!(
        app.rows
            .iter()
            .any(|r| r.wt.branch.as_deref() == Some("feature/ui"))
    );

    // ---- selection + multi-terminal ------------------------------------
    app.apply(Action::SelectAll, &ctx);
    frames(&ctx, &mut app, 2);
    assert_eq!(app.selected.len(), 2);

    let paths: Vec<PathBuf> = app.selected.iter().cloned().collect();
    app.apply(Action::OpenTerminals(paths), &ctx);
    assert_eq!(app.sessions.len(), 2, "um PTY por worktree selecionada");
    assert_eq!(app.screen, Screen::Terminals);
    frames(&ctx, &mut app, 4);

    // Every layout must lay the panes out without panicking.
    for layout in [
        PaneLayout::Grid,
        PaneLayout::Columns,
        PaneLayout::Rows,
        PaneLayout::Tabs,
        PaneLayout::Free,
    ] {
        app.apply(Action::SetLayout(layout), &ctx);
        frames(&ctx, &mut app, 3);
    }
    app.apply(Action::SetLayout(PaneLayout::Grid), &ctx);
    frames(&ctx, &mut app, 3);

    // The panes are real terminals: the shell must answer.
    for session in &app.sessions {
        session.write(b"echo HELLO_FROM_HUB\r");
    }
    frames_until(&ctx, &mut app, "saída do shell nos dois painéis", |app| {
        app.sessions
            .iter()
            .all(|s| s.screen_text().contains("HELLO_FROM_HUB"))
    });

    // Broadcast reaches every pane.
    app.broadcast = true;
    app.apply(Action::Broadcast("echo BROADCASTED".to_owned()), &ctx);
    frames_until(&ctx, &mut app, "broadcast em todos os painéis", |app| {
        app.sessions
            .iter()
            .all(|s| s.screen_text().contains("BROADCASTED"))
    });

    // A pane knows whether it sits in the repository or in a worktree.
    assert!(
        app.sessions.iter().any(|s| s.is_main),
        "o repositório oficial tem painel"
    );
    assert!(
        app.sessions.iter().any(|s| !s.is_main),
        "a worktree tem painel"
    );

    // ---- Ctrl+C interrupts the program, it does not close the pane ------
    app.apply(Action::SetLayout(PaneLayout::Tabs), &ctx);
    app.apply(Action::FocusPane(0), &ctx);
    frames(&ctx, &mut app, 3);
    let center = app.pane_area.center();
    click(&ctx, &mut app, center);

    app.sessions[0].write(b"printf 'RUN%s\n' 1; sleep 300\r");
    frames_until(&ctx, &mut app, "shell rodando o sleep", |app| {
        app.sessions[0].screen_text().contains("RUN1")
    });

    pass(&ctx, &mut app, 0.05, vec![Event::Copy]);
    frames(&ctx, &mut app, 2);
    // Only reaches the shell if the sleep really took the SIGINT.
    app.sessions[0].write(b"printf 'MARK%s\n' 42\r");
    frames_until(&ctx, &mut app, "prompt de volta depois do Ctrl+C", |app| {
        app.sessions[0].screen_text().contains("MARK42")
    });
    assert!(!app.sessions[0].has_exited(), "o painel continua vivo");
    assert_eq!(app.sessions.len(), 2, "nenhum terminal fechou");

    // ---- free layout: panes dock against each other and fill the area ---
    app.apply(Action::SetLayout(PaneLayout::Free), &ctx);
    frames(&ctx, &mut app, 3);
    let area = app.pane_area;
    let tree = app.tiles.clone().expect("o mosaico existe");
    assert_eq!(tree.leaves().len(), 2, "cada painel entra no mosaico");
    let rects = tree.layout(area, 10.0);
    let covered: f32 = rects.iter().map(|(_, r)| r.width() * r.height()).sum();
    assert!(
        covered > area.width() * area.height() * 0.9,
        "os painéis preenchem a área: {covered}"
    );
    assert!(
        !rects[0].1.shrink(2.0).intersects(rects[1].1.shrink(2.0)),
        "painéis não se sobrepõem"
    );

    // Dragging a header onto the other pane's bottom edge re-docks it there.
    let first = app.sessions[0].id;
    let before: Vec<(u64, egui::Rect)> = rects.clone();
    let source = before.iter().find(|(id, _)| *id == first).unwrap().1;
    let other = before.iter().find(|(id, _)| *id != first).unwrap().1;
    let grab = pos2(source.left() + 60.0, source.top() + 16.0);
    let landing = pos2(other.center().x, other.bottom() - 20.0);
    drag(&ctx, &mut app, grab, landing - grab);
    frames(&ctx, &mut app, 3);
    let after = app
        .tiles
        .as_ref()
        .expect("mosaico")
        .layout(app.pane_area, 10.0);
    let moved = after.iter().find(|(id, _)| *id == first).unwrap().1;
    let neighbour = after.iter().find(|(id, _)| *id != first).unwrap().1;
    assert!(
        moved.top() >= neighbour.bottom() - 1.0,
        "o painel arrastado desce para baixo do outro: {moved:?} / {neighbour:?}"
    );
    assert!(
        (moved.width() - neighbour.width()).abs() < 2.0,
        "os dois dividem a mesma coluna"
    );
    let covered: f32 = after.iter().map(|(_, r)| r.width() * r.height()).sum();
    assert!(
        covered > app.pane_area.width() * app.pane_area.height() * 0.9,
        "depois de mover, a área continua cheia"
    );

    // Dragging the gutter hands room from one pane to the other.
    let gutter = pos2(moved.center().x, moved.top() - 5.0);
    let before_h = moved.height();
    drag(&ctx, &mut app, gutter, vec2(0.0, -60.0));
    frames(&ctx, &mut app, 3);
    let grown = app
        .tiles
        .as_ref()
        .unwrap()
        .layout(app.pane_area, 10.0)
        .into_iter()
        .find(|(id, _)| *id == first)
        .unwrap()
        .1;
    assert!(
        grown.height() > before_h + 20.0,
        "a divisão arrastada cresce o painel: {before_h} -> {}",
        grown.height()
    );

    // One pane can take the whole area, and go back.
    app.apply(Action::MaximizePane(first), &ctx);
    frames(&ctx, &mut app, 2);
    assert_eq!(app.maximized, Some(first));
    app.apply(Action::MaximizePane(first), &ctx);
    frames(&ctx, &mut app, 2);
    assert_eq!(app.maximized, None);

    // Fullscreen is just a flag plus a viewport command.
    assert!(!app.fullscreen);
    app.apply(Action::ToggleFullscreen, &ctx);
    frames(&ctx, &mut app, 2);
    assert!(app.fullscreen, "F11 liga a tela cheia");
    app.apply(Action::ToggleFullscreen, &ctx);
    frames(&ctx, &mut app, 2);

    // "Organizar" lays everything out evenly again.
    app.apply(Action::TidyPanes, &ctx);
    frames(&ctx, &mut app, 2);
    assert_eq!(app.tiles.as_ref().unwrap().leaves().len(), 2);
    app.apply(Action::SetLayout(PaneLayout::Grid), &ctx);
    frames(&ctx, &mut app, 2);

    // ---- settings + theme ----------------------------------------------
    app.apply(Action::SetScreen(Screen::Settings), &ctx);
    frames(&ctx, &mut app, 3);
    app.apply(Action::SetTheme(Mode::Light), &ctx);
    frames(&ctx, &mut app, 3);
    assert_eq!(app.cfg.settings.theme, Mode::Light);
    app.apply(Action::SetTheme(Mode::Dark), &ctx);
    frames(&ctx, &mut app, 3);

    // Every accent, every language and both fonts have to render without a
    // missing key or a panic — `tr` asserts on an unknown key in debug builds.
    for accent in theme::Accent::ALL {
        app.cfg.settings.accent = accent;
        app.apply(Action::ApplyLook { fonts: false }, &ctx);
        frames(&ctx, &mut app, 2);
    }
    app.cfg.settings.accent = theme::Accent::Green;
    for lang in Lang::ALL {
        app.cfg.settings.lang = lang;
        app.apply(Action::ApplyLook { fonts: true }, &ctx);
        frames(&ctx, &mut app, 2);
        assert_eq!(worktree_hub::i18n::lang(), lang);
    }
    app.cfg.settings.lang = Lang::En;
    app.cfg.settings.ui_font = theme::UiFont::Inter;
    app.cfg.settings.mono_font = theme::MonoFont::JetBrains;
    app.cfg.settings.app_opacity = 0.7;
    app.cfg.settings.term_opacity = 0.6;
    app.apply(Action::ApplyLook { fonts: true }, &ctx);
    frames(&ctx, &mut app, 3);
    assert!(theme::tokens(app.cfg.settings.look()).bg.a() < 255);

    // The side column folds and opens again.
    let folded = app.cfg.settings.sidebar_collapsed;
    app.apply(Action::ToggleSidebar, &ctx);
    frames(&ctx, &mut app, 2);
    assert_ne!(app.cfg.settings.sidebar_collapsed, folded);
    app.apply(Action::ToggleSidebar, &ctx);
    frames(&ctx, &mut app, 2);

    // ---- about ----------------------------------------------------------
    app.apply(Action::SetScreen(Screen::About), &ctx);
    frames(&ctx, &mut app, 3);
    assert_eq!(app.screen, Screen::About);

    // ---- the remaining dialogs -----------------------------------------
    app.apply(Action::SetScreen(Screen::Worktrees), &ctx);
    app.apply(Action::AddProjectDialog, &ctx);
    frames(&ctx, &mut app, 3);
    assert!(matches!(app.dialog, Dialog::AddProject(_)));
    app.apply(Action::CancelDialog, &ctx);

    let project_id = app.cfg.projects[0].id;
    app.apply(Action::RemoveProjectDialog(project_id), &ctx);
    frames(&ctx, &mut app, 3);
    app.apply(Action::CancelDialog, &ctx);

    // ---- close a pane, then remove the worktree ------------------------
    let first = app.sessions[0].id;
    app.apply(Action::CloseSession(first), &ctx);
    assert_eq!(app.sessions.len(), 1);
    frames(&ctx, &mut app, 2);

    app.apply(Action::DeleteDialog(target.clone()), &ctx);
    frames(&ctx, &mut app, 3);
    match &mut app.dialog {
        Dialog::Delete(form) => {
            assert_eq!(form.branch.as_deref(), Some("feature/ui"));
            form.also_branch = true;
            form.force = true;
        }
        _ => panic!("diálogo de remoção deveria estar aberto"),
    }
    app.apply(Action::ConfirmDelete, &ctx);
    frames_until(&ctx, &mut app, "worktree removida", |app| {
        app.rows.len() == 1
    });
    assert!(!target.exists(), "a pasta saiu do disco");

    // Prune and refresh must stay quiet on a healthy repository.
    app.apply(Action::Prune, &ctx);
    frames_until(&ctx, &mut app, "prune concluído", |app| !app.busy);
    app.apply(Action::Refresh, &ctx);
    frames_until(&ctx, &mut app, "refresh concluído", |app| !app.loading);

    app.apply(Action::CloseAllSessions, &ctx);
    frames(&ctx, &mut app, 2);
    assert!(app.sessions.is_empty());
}
