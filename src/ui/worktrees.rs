//! Main screen: the worktree list, its filters and the selection bar that
//! opens the multi-terminal view.

use std::path::PathBuf;

use egui::{Align, Layout, Ui, Vec2};

use super::components::{self as c, tokens};
use super::icons::Icon;
use crate::app::{Action, App, Row};
use crate::i18n::{tr, tr1, trn};
use crate::theme::{self, Weight};

pub fn show(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);

    let Some(project) = app.current_project().cloned() else {
        c::empty_state(ui, Icon::Folder, tr("wt.empty_title"), tr("wt.empty_body"));
        ui.vertical_centered(|ui| {
            if c::Btn::primary(tr("nav.add_project"))
                .icon(Icon::Plus)
                .show(ui)
                .clicked()
            {
                actions.push(Action::AddProjectDialog);
            }
        });
        return;
    };

    // ---- header -----------------------------------------------------------
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            c::heading(ui, &project.name);
            ui.add_space(2.0);
            c::truncated(
                ui,
                &project.root.to_string_lossy(),
                12.0,
                Weight::Regular,
                t.text_dim,
                520.0,
            );
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if c::Btn::primary(tr("wt.new"))
                .icon(Icon::Plus)
                .hint("Ctrl+N")
                .show(ui)
                .clicked()
            {
                actions.push(Action::CreateDialog);
            }
            if c::Btn::new(tr("wt.refresh"))
                .icon(Icon::Refresh)
                .hint("Ctrl+R")
                .enabled(!app.loading)
                .show(ui)
                .clicked()
            {
                actions.push(Action::Refresh);
            }
            if c::Btn::ghost(tr("wt.prune"))
                .icon(Icon::Trash)
                .hint(tr("wt.prune_hint"))
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                actions.push(Action::Prune);
            }
        });
    });

    ui.add_space(theme::S4);

    // ---- toolbar ----------------------------------------------------------
    let search_width = (ui.available_width() * 0.42).clamp(220.0, 420.0);
    ui.horizontal(|ui| {
        c::search_field(ui, &mut app.search, tr("wt.search"), search_width);
        if app.loading {
            ui.add_space(theme::S2);
            c::spinner(ui, 16.0);
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let total = app.rows.iter().filter(|r| !r.wt.is_main).count();
            let dirty = app
                .rows
                .iter()
                .filter(|r| r.status.as_ref().is_some_and(|s| s.dirty()))
                .count();
            let counted = if total == 1 {
                tr1("wt.count_one", total)
            } else {
                tr1("wt.count_many", total)
            };
            c::chip(ui, &counted, t.brand, t.brand_tint, Some(Icon::Branch));
            if dirty > 0 {
                c::chip(
                    ui,
                    &tr1("wt.dirty_count", dirty),
                    t.warning,
                    t.warning_tint,
                    Some(Icon::Warning),
                );
            }
            if !app.selected.is_empty() {
                if c::Btn::ghost(tr("wt.clear_selection"))
                    .small()
                    .show(ui)
                    .clicked()
                {
                    actions.push(Action::ClearSelection);
                }
            } else if c::Btn::ghost(tr("wt.select_all"))
                .small()
                .show(ui)
                .clicked()
            {
                actions.push(Action::SelectAll);
            }
        });
    });

    ui.add_space(theme::S3);

    // ---- list -------------------------------------------------------------
    let search = app.search.clone();
    let visible: Vec<usize> = app
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.matches(&search))
        .map(|(i, _)| i)
        .collect();

    if app.rows.is_empty() && !app.loading {
        c::empty_state(
            ui,
            Icon::Branch,
            tr("wt.none_listed"),
            tr("wt.none_listed_body"),
        );
        return;
    }
    if visible.is_empty() {
        c::empty_state(
            ui,
            Icon::Search,
            tr("wt.nothing_found"),
            tr("wt.nothing_found_body"),
        );
        return;
    }

    let show_clean = app.cfg.settings.show_clean_status;
    let bottom_reserve = if app.selected.is_empty() { 0.0 } else { 76.0 };
    // The repository itself comes first, under its own heading: everything
    // below it was created from it.
    let (official, derived): (Vec<usize>, Vec<usize>) =
        visible.into_iter().partition(|&i| app.rows[i].wt.is_main);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height((ui.available_height() - bottom_reserve).max(120.0))
        .show(ui, |ui| {
            let mut draw = |ui: &mut Ui, index: usize| {
                let row = &app.rows[index];
                let selected = app.selected.contains(&row.wt.path);
                let has_session = app.sessions.iter().any(|s| s.cwd == row.wt.path);
                card(ui, row, selected, has_session, show_clean, actions);
                ui.add_space(theme::S3);
            };

            if !official.is_empty() {
                c::section_label(ui, tr("wt.section_official"));
                ui.add_space(theme::S2);
                for index in official {
                    draw(ui, index);
                }
                ui.add_space(theme::S2);
            }

            c::section_label(ui, &tr1("wt.section_worktrees", derived.len()));
            ui.add_space(theme::S2);
            if derived.is_empty() {
                c::muted(ui, tr("wt.empty_worktrees"));
                ui.add_space(theme::S3);
            }
            for index in derived {
                draw(ui, index);
            }
        });

    if !app.selected.is_empty() {
        selection_bar(ui, app, actions);
    }
}

fn card(
    ui: &mut Ui,
    row: &Row,
    selected: bool,
    has_session: bool,
    show_clean: bool,
    actions: &mut Vec<Action>,
) {
    let t = tokens(ui);
    let accent = if selected {
        Some(t.brand)
    } else if row.wt.is_main {
        Some(t.brand.gamma_multiply(0.45))
    } else {
        None
    };

    c::card_with(ui, theme::S4, accent, |ui| {
        ui.horizontal(|ui| {
            let mut checked = selected;
            if row.wt.exists() {
                if c::round_checkbox(ui, &mut checked).changed() {
                    actions.push(Action::ToggleSelect(row.wt.path.clone()));
                }
            } else {
                ui.add_space(26.0);
            }
            ui.add_space(theme::S2);

            // The buttons on the right claim their room first, so a long
            // branch, subject or path is cut instead of running under them.
            let info_w = (ui.available_width() - actions_width(ui, row) - theme::S3).max(200.0);

            ui.vertical(|ui| {
                ui.set_min_width(info_w);
                ui.set_max_width(info_w);
                // Title line — the chips wrap under the name on a narrow window.
                ui.horizontal_wrapped(|ui| {
                    c::truncated_end(
                        ui,
                        &row.wt.name(),
                        15.0,
                        Weight::SemiBold,
                        t.text,
                        (info_w * 0.6).max(140.0),
                    );
                    ui.add_space(theme::S1);
                    chips(ui, row, has_session, show_clean);
                });
                ui.add_space(3.0);
                // Branch + commit line
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                    super::icons::paint(ui.painter(), rect, Icon::Branch, t.brand);
                    // Says out loud which branch this worktree sits on.
                    if row.wt.branch.is_some() {
                        c::text_with(ui, tr("dlg.branch"), 11.5, Weight::Regular, t.text_faint);
                    }
                    c::truncated_end(
                        ui,
                        &row.wt.ref_label(),
                        12.5,
                        Weight::Medium,
                        t.brand,
                        (ui.available_width() - 76.0).max(110.0),
                    );
                    c::mono_label(ui, row.wt.short_head(), t.text_faint);
                    if let Some(commit) = &row.commit {
                        let room = ui.available_width() - 4.0;
                        if room > 60.0 {
                            c::truncated_end(
                                ui,
                                &commit.subject,
                                12.0,
                                Weight::Regular,
                                t.text_dim,
                                room,
                            );
                        }
                    }
                });
                ui.add_space(2.0);
                c::truncated(
                    ui,
                    &row.wt.path.to_string_lossy(),
                    11.5,
                    Weight::Regular,
                    t.text_faint,
                    info_w,
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let exists = row.wt.exists();
                if row.wt.is_main {
                    c::icon_button(ui, Icon::Trash, tr("wt.cant_remove_main"), true);
                } else if c::icon_button(ui, Icon::Trash, tr("wt.remove"), true).clicked() {
                    actions.push(Action::DeleteDialog(row.wt.path.clone()));
                }
                if c::icon_button(ui, Icon::Copy, tr("wt.copy_path"), false).clicked() {
                    actions.push(Action::CopyPath(row.wt.path.clone()));
                }
                if !row.wt.is_main {
                    let locked = row.wt.locked.is_some();
                    let hint = if locked {
                        tr("wt.unlock_hint")
                    } else {
                        tr("wt.lock_hint")
                    };
                    if c::icon_button(ui, Icon::Lock, hint, false).clicked() {
                        actions.push(Action::ToggleLock(row.wt.path.clone(), !locked));
                    }
                }
                if exists {
                    if c::icon_button(ui, Icon::Folder, tr("nav.open_folder"), false).clicked() {
                        actions.push(Action::OpenFolder(row.wt.path.clone()));
                    }
                    if c::icon_button(ui, Icon::Code, tr("wt.open_editor"), false).clicked() {
                        actions.push(Action::OpenEditor(row.wt.path.clone()));
                    }
                    if c::icon_button(ui, Icon::External, tr("wt.open_external"), false).clicked() {
                        actions.push(Action::ExternalTerminal(row.wt.path.clone()));
                    }
                    if c::Btn::soft(tr("wt.terminal"))
                        .icon(Icon::Terminal)
                        .small()
                        .hint(tr("wt.terminal_hint"))
                        .show(ui)
                        .clicked()
                    {
                        actions.push(Action::OpenTerminal(row.wt.path.clone()));
                    }
                }
            });
        });
    });
}

/// Room the action buttons on the right of a card need, measured instead of
/// guessed: the text column is capped to whatever is left of it.
fn actions_width(ui: &Ui, row: &Row) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    // Copy and remove are always there; lock only on a worktree; folder,
    // editor and external terminal only while the folder is on disk.
    let mut buttons = 2.0_f32;
    if !row.wt.is_main {
        buttons += 1.0;
    }
    if row.wt.exists() {
        buttons += 3.0;
    }
    let mut width = buttons * (32.0 + gap);
    if row.wt.exists() {
        // The "Terminal" pill: icon + label + its own padding.
        let galley = ui.painter().layout_no_wrap(
            tr("wt.terminal").to_owned(),
            theme::font(12.5, Weight::SemiBold),
            egui::Color32::PLACEHOLDER,
        );
        width += galley.size().x + 13.0 + theme::S2 + 32.0 + gap;
    }
    width
}

fn chips(ui: &mut Ui, row: &Row, has_session: bool, show_clean: bool) {
    let t = tokens(ui);

    if row.wt.is_main {
        c::chip(
            ui,
            tr("wt.chip_official"),
            t.brand,
            t.brand_tint,
            Some(Icon::Home),
        )
        .on_hover_text(tr("wt.chip_official_hint"));
    } else {
        c::chip(
            ui,
            tr("wt.chip_worktree"),
            t.text_dim,
            t.surface_alt,
            Some(Icon::Branch),
        );
    }
    if row.wt.bare {
        c::chip(ui, tr("wt.chip_bare"), t.info, t.info_tint, None);
    }
    if row.wt.detached {
        c::chip(ui, tr("wt.chip_detached"), t.info, t.info_tint, None);
    }
    if let Some(reason) = &row.wt.locked {
        c::chip(
            ui,
            tr("wt.chip_locked"),
            t.warning,
            t.warning_tint,
            Some(Icon::Lock),
        )
        .on_hover_text(reason);
    }
    if !row.wt.exists() {
        c::chip(
            ui,
            tr("wt.chip_missing"),
            t.danger,
            t.danger_tint,
            Some(Icon::Warning),
        );
    } else if row.wt.prunable.is_some() {
        c::chip(
            ui,
            tr("wt.chip_orphan"),
            t.danger,
            t.danger_tint,
            Some(Icon::Warning),
        );
    }

    if let Some(status) = &row.status {
        if status.dirty() {
            let count = status.changes();
            let text = if count == 1 {
                tr1("wt.chip_changes_one", count)
            } else {
                tr1("wt.chip_changes_many", count)
            };
            c::chip(ui, &text, t.warning, t.warning_tint, None).on_hover_text(trn(
                "wt.status_tooltip",
                &[
                    &status.staged.to_string(),
                    &status.unstaged.to_string(),
                    &status.untracked.to_string(),
                    &status.conflicts.to_string(),
                ],
            ));
        } else if show_clean {
            c::chip(
                ui,
                tr("wt.chip_clean"),
                t.success,
                t.success_tint,
                Some(Icon::Check),
            );
        }
        if status.ahead > 0 || status.behind > 0 {
            c::chip(
                ui,
                &format!("↑{} ↓{}", status.ahead, status.behind),
                t.info,
                t.info_tint,
                None,
            )
            .on_hover_text(
                status
                    .upstream
                    .clone()
                    .unwrap_or_else(|| tr("wt.no_upstream").to_owned()),
            );
        }
    }

    if has_session {
        c::chip(
            ui,
            tr("wt.chip_terminal"),
            t.brand,
            t.brand_tint,
            Some(Icon::Terminal),
        );
    }
}

/// Floating pill at the bottom with the multi-terminal call to action.
fn selection_bar(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let t = tokens(ui);
    let count = app.selected.len();

    egui::Area::new(egui::Id::unique("worktree-hub-selection"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -theme::S5))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(t.surface)
                .stroke(egui::Stroke::new(1.0, t.brand.gamma_multiply(0.4)))
                .corner_radius(egui::CornerRadius::same(theme::R_PILL))
                .shadow(theme::card_shadow(t))
                .inner_margin(egui::Margin::symmetric(theme::S4 as i8, theme::S3 as i8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        c::text_with(
                            ui,
                            &if count == 1 {
                                tr1("wt.selected_one", count)
                            } else {
                                tr1("wt.selected_many", count)
                            },
                            13.0,
                            Weight::SemiBold,
                            t.text,
                        );
                        ui.add_space(theme::S2);
                        if c::Btn::primary(&tr1("wt.open_multi", count))
                            .icon(Icon::Grid)
                            .hint("Ctrl+Shift+T")
                            .show(ui)
                            .clicked()
                        {
                            let paths: Vec<PathBuf> = app.selected.iter().cloned().collect();
                            actions.push(Action::OpenTerminals(paths));
                        }
                        if c::Btn::ghost(tr("wt.clear")).small().show(ui).clicked() {
                            actions.push(Action::ClearSelection);
                        }
                    });
                });
        });
}
