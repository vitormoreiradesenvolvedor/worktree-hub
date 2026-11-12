//! Modal dialogs: new worktree, remove worktree, add/remove project.

use egui::{Align, Layout, Modal, Ui};

use super::components::{self as c, tokens};
use super::icons::Icon;
use super::picker;
use crate::app::{Action, App, Dialog};
use crate::config;
use crate::git::AddOptions;
use crate::i18n::{tr, tr1};
use crate::theme::{self, Weight};

pub fn show(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    if !app.dialog.is_open() {
        return;
    }

    let project = app.current_project().cloned();
    let branches: Vec<String> = app.branches.iter().map(|b| b.name.clone()).collect();
    let tokens = theme::tokens(app.cfg.settings.look());

    let frame = egui::Frame::new()
        .fill(tokens.surface)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(theme::R_CARD))
        .shadow(theme::card_shadow(tokens))
        .inner_margin(egui::Margin::same(theme::S5 as i8));

    let modal = Modal::new(egui::Id::unique("worktree-hub-modal")).frame(frame);
    let mut close_requested = false;

    let response = modal.show(ctx, |ui| {
        // Fits the window even at a large interface scale.
        let room = ctx.viewport_rect().width() - 4.0 * theme::S5;
        ui.set_width(560.0_f32.min(room.max(320.0)));
        match &mut app.dialog {
            Dialog::Create(form) => {
                let Some(project) = project.as_ref() else {
                    close_requested = true;
                    return;
                };
                title(ui, tr("dlg.create_title"), tr("dlg.create_sub"));
                ui.add_space(theme::S4);

                field_label(ui, tr("dlg.branch"));
                let branch_response = c::text_field(
                    ui,
                    &mut form.branch,
                    "feature/my-feature",
                    ui.available_width(),
                );
                if branch_response.changed() && !form.path_edited {
                    form.path = config::suggest_path(project, form.branch.trim())
                        .to_string_lossy()
                        .into_owned();
                }
                if form.path.is_empty() {
                    form.path = config::suggest_path(project, form.branch.trim())
                        .to_string_lossy()
                        .into_owned();
                }

                ui.add_space(theme::S3);
                c::panel(ui, |ui| {
                    if c::toggle_row(
                        ui,
                        tr("dlg.create_branch"),
                        tr("dlg.create_branch_hint"),
                        &mut form.create_branch,
                    ) && !form.create_branch
                    {
                        form.force_branch = false;
                    }
                    if form.create_branch {
                        ui.add_space(theme::S2);
                        field_label(ui, tr("dlg.base"));
                        ui.horizontal(|ui| {
                            c::text_field(ui, &mut form.base, "origin/main", 260.0);
                            egui::ComboBox::from_id_salt("worktree-hub-base")
                                .selected_text(tr("dlg.choose"))
                                .width(170.0)
                                .show_ui(ui, |ui| {
                                    for name in &branches {
                                        if ui.selectable_label(&form.base == name, name).clicked() {
                                            form.base = name.clone();
                                        }
                                    }
                                });
                        });
                    } else if !branches.is_empty() {
                        ui.add_space(theme::S2);
                        field_label(ui, tr("dlg.branch"));
                        egui::ComboBox::from_id_salt("worktree-hub-branch")
                            .selected_text(if form.branch.is_empty() {
                                tr("dlg.choose").to_owned()
                            } else {
                                form.branch.clone()
                            })
                            .width(300.0)
                            .show_ui(ui, |ui| {
                                for name in &branches {
                                    if ui.selectable_label(&form.branch == name, name).clicked() {
                                        form.branch = name.clone();
                                        if !form.path_edited {
                                            form.path =
                                                config::suggest_path(project, form.branch.trim())
                                                    .to_string_lossy()
                                                    .into_owned();
                                        }
                                    }
                                }
                            });
                    }
                });

                ui.add_space(theme::S3);
                field_label(ui, tr("dlg.path"));
                if c::text_field(ui, &mut form.path, "", ui.available_width()).changed() {
                    form.path_edited = true;
                }

                ui.add_space(theme::S3);
                c::expander(ui, "advanced", tr("dlg.advanced"), |ui| {
                    c::toggle_row(
                        ui,
                        tr("dlg.detach"),
                        tr("dlg.detach_hint"),
                        &mut form.detach,
                    );
                    c::toggle_row(ui, tr("dlg.force"), tr("dlg.force_hint"), &mut form.force);
                    if form.create_branch {
                        c::toggle_row(
                            ui,
                            tr("dlg.force_branch"),
                            tr("dlg.force_branch_hint"),
                            &mut form.force_branch,
                        );
                        c::toggle_row(ui, tr("dlg.track"), tr("dlg.track_hint"), &mut form.track);
                    }
                    c::toggle_row(
                        ui,
                        tr("dlg.no_checkout"),
                        tr("dlg.no_checkout_hint"),
                        &mut form.no_checkout,
                    );
                    c::toggle_row(
                        ui,
                        tr("dlg.open_terminal"),
                        tr("set.open_terminal_hint"),
                        &mut form.open_terminal,
                    );
                });

                ui.add_space(theme::S3);
                let preview = AddOptions {
                    path: std::path::PathBuf::from(form.path.trim()),
                    branch: form.branch.trim().to_owned(),
                    create_branch: form.create_branch,
                    base: form.base.trim().to_owned(),
                    detach: form.detach,
                    force: form.force,
                    force_branch: form.force_branch,
                    no_checkout: form.no_checkout,
                    track: form.track,
                }
                .preview();
                c::panel(ui, |ui| {
                    c::section_label(ui, "git");
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(preview)
                            .font(theme::mono(11.5, false))
                            .color(tokens.text_dim),
                    );
                });

                if let Some(error) = &form.error {
                    ui.add_space(theme::S2);
                    c::text_with(ui, error, 12.5, Weight::Medium, tokens.danger);
                }

                ui.add_space(theme::S4);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c::Btn::primary(tr("dlg.create_title"))
                            .icon(Icon::Check)
                            .enabled(!app.busy)
                            .show(ui)
                            .clicked()
                        {
                            actions.push(Action::SubmitCreate);
                        }
                        if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                            close_requested = true;
                        }
                    });
                });
            }

            Dialog::Delete(form) => {
                title(
                    ui,
                    &tr1("dlg.delete_title", &form.name),
                    tr("dlg.delete_sub"),
                );
                ui.add_space(theme::S4);
                c::panel(ui, |ui| {
                    c::label_value(ui, &format!("{}: ", tr("wt.chip_worktree")), &form.name);
                    c::label_value(
                        ui,
                        &format!("{}: ", tr("dlg.branch")),
                        form.branch.as_deref().unwrap_or("(detached)"),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(form.path.to_string_lossy())
                            .font(theme::mono(11.5, false))
                            .color(tokens.text_faint),
                    );
                });

                ui.add_space(theme::S3);
                c::toggle_row(
                    ui,
                    tr("dlg.delete_force"),
                    tr("dlg.delete_force_hint"),
                    &mut form.force,
                );
                if form.branch.is_some() {
                    c::toggle_row(
                        ui,
                        tr("dlg.delete_branch"),
                        tr("dlg.delete_branch_hint"),
                        &mut form.also_branch,
                    );
                }

                ui.add_space(theme::S4);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c::Btn::danger(tr("dlg.remove"))
                            .icon(Icon::Trash)
                            .enabled(!app.busy)
                            .show(ui)
                            .clicked()
                        {
                            actions.push(Action::ConfirmDelete);
                        }
                        if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                            close_requested = true;
                        }
                    });
                });
            }

            Dialog::AddProject(state) => {
                title(ui, tr("dlg.add_project_title"), tr("dlg.add_project_sub"));
                ui.add_space(theme::S4);
                if let Some(path) = picker::show(state, ui) {
                    actions.push(Action::AddProjectPath(path));
                }
                ui.add_space(theme::S3);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                            close_requested = true;
                        }
                    });
                });
            }

            Dialog::RemoveProject(id) => {
                let id = *id;
                let name = app
                    .cfg
                    .project(id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                title(
                    ui,
                    tr("dlg.remove_project_title"),
                    tr("dlg.remove_project_sub"),
                );
                ui.add_space(theme::S4);
                c::body(ui, tr1("dlg.delete_title", &name));
                ui.add_space(theme::S4);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c::Btn::danger(tr("dlg.remove")).show(ui).clicked() {
                            app.cfg.remove_project(id);
                            actions.push(Action::SaveConfig);
                            actions.push(Action::Refresh);
                            close_requested = true;
                        }
                        if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                            close_requested = true;
                        }
                    });
                });
            }

            Dialog::Quick(form) => {
                title(ui, tr("dlg.quick_title"), tr("dlg.quick_sub"));
                ui.add_space(theme::S4);

                field_label(ui, tr("dlg.quick_label"));
                c::text_field(ui, &mut form.label, "build", ui.available_width());

                ui.add_space(theme::S3);
                field_label(ui, tr("dlg.quick_command"));
                c::text_field(
                    ui,
                    &mut form.command,
                    "git status -sb",
                    ui.available_width(),
                );

                ui.add_space(theme::S2);
                c::muted(ui, tr("dlg.quick_hint"));

                ui.add_space(theme::S4);
                let filled = !form.command.trim().is_empty();
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c::Btn::primary(tr("dlg.save"))
                            .icon(Icon::Check)
                            .enabled(filled)
                            .show(ui)
                            .clicked()
                        {
                            actions.push(Action::SaveQuick {
                                slot: form.slot,
                                label: form.label.clone(),
                                command: form.command.clone(),
                            });
                        }
                        if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                            close_requested = true;
                        }
                        if filled
                            && c::Btn::danger(tr("dlg.remove"))
                                .icon(Icon::Trash)
                                .show(ui)
                                .clicked()
                        {
                            // An empty command clears the slot.
                            actions.push(Action::SaveQuick {
                                slot: form.slot,
                                label: String::new(),
                                command: String::new(),
                            });
                        }
                    });
                });
            }

            Dialog::None => {}
        }
    });

    if close_requested || response.should_close() {
        actions.push(Action::CancelDialog);
    }
}

fn title(ui: &mut Ui, heading: &str, subtitle: &str) {
    let t = tokens(ui);
    c::text_with(ui, heading, 19.0, Weight::SemiBold, t.text);
    ui.add_space(2.0);
    c::text_with(ui, subtitle, 12.5, Weight::Regular, t.text_dim);
}

fn field_label(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    c::text_with(ui, text, 12.0, Weight::SemiBold, t.text_dim);
    ui.add_space(4.0);
}
