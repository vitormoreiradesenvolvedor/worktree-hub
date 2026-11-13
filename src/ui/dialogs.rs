//! Modal dialogs: new worktree, remove worktree, add/remove project.

use egui::{Align, Layout, Modal, Ui};

use super::components::{self as c, tokens};
use super::icons::Icon;
use super::picker;
use crate::app::{Action, App, Dialog, EditorPick};
use crate::config;
use crate::git::AddOptions;
use crate::i18n::{tr, tr1};
use crate::theme::{self, Weight};

pub fn show(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    if !app.dialog.is_open() {
        return;
    }

    // The editor picker reads the icon cache while it draws, so it takes the
    // dialog out of the app for the duration and puts it back if it stays open.
    if matches!(app.dialog, Dialog::Editor(_)) {
        let Dialog::Editor(mut pick) = std::mem::replace(&mut app.dialog, Dialog::None) else {
            unreachable!()
        };
        if editor_picker(app, ctx, &mut pick, actions) {
            app.dialog = Dialog::Editor(pick);
        }
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

            Dialog::Editor(_) => {}

            Dialog::None => {}
        }
    });

    if close_requested || response.should_close() {
        actions.push(Action::CancelDialog);
    }
}

/// Menu of the editors the desktop knows about. Returns true while it is still
/// open.
fn editor_picker(
    app: &mut App,
    ctx: &egui::Context,
    pick: &mut EditorPick,
    actions: &mut Vec<Action>,
) -> bool {
    let tokens = theme::tokens(app.cfg.settings.look());
    let frame = egui::Frame::new()
        .fill(tokens.surface)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(theme::R_CARD))
        .shadow(theme::card_shadow(tokens))
        .inner_margin(egui::Margin::same(theme::S5 as i8));

    let mut chosen: Option<usize> = None;
    let mut close = false;

    let response = Modal::new(egui::Id::unique("worktree-hub-editor"))
        .frame(frame)
        .show(ctx, |ui| {
            let room = ctx.viewport_rect().width() - 4.0 * theme::S5;
            ui.set_width(600.0_f32.min(room.max(320.0)));
            title(ui, tr("dlg.editor_title"), tr("dlg.editor_sub"));
            ui.add_space(theme::S2);
            c::truncated(
                ui,
                &pick.path.to_string_lossy(),
                11.5,
                Weight::Regular,
                tokens.text_faint,
                ui.available_width(),
            );
            ui.add_space(theme::S4);

            let height = ctx.viewport_rect().height() * 0.45;
            egui::ScrollArea::vertical()
                .max_height(height.clamp(180.0, 420.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (index, editor) in pick.apps.iter().enumerate() {
                            let texture = editor
                                .icon
                                .clone()
                                .and_then(|path| icon_texture(ctx, &mut app.icons, &path));
                            if tile(ui, &editor.name, &editor.initials(), texture).clicked() {
                                chosen = Some(index);
                            }
                        }
                    });
                });

            ui.add_space(theme::S3);
            c::toggle_row(
                ui,
                tr("dlg.editor_remember"),
                tr("dlg.editor_remember_hint"),
                &mut pick.remember,
            );

            ui.add_space(theme::S3);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if c::Btn::ghost(tr("dlg.cancel")).show(ui).clicked() {
                        close = true;
                    }
                });
            });
        });

    if let Some(index) = chosen {
        actions.push(Action::OpenEditorWith {
            path: pick.path.clone(),
            app: pick.apps[index].clone(),
            remember: pick.remember,
        });
        return false;
    }
    // The picker stays open until something closes it.
    !(close || response.should_close())
}

/// One application in the menu: its picture over its name.
fn tile(
    ui: &mut Ui,
    name: &str,
    initials: &str,
    texture: Option<egui::TextureHandle>,
) -> egui::Response {
    const W: f32 = 168.0;
    const H: f32 = 104.0;
    let t = tokens(ui);

    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(W, H), egui::Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter();
    if hovered {
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(theme::R_INPUT),
            t.surface_hover,
        );
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(theme::R_INPUT),
            egui::Stroke::new(1.0, t.brand),
            egui::StrokeKind::Inside,
        );
    }

    let art = egui::Rect::from_center_size(
        egui::Pos2::new(rect.center().x, rect.top() + 34.0),
        egui::Vec2::splat(40.0),
    );
    match texture {
        Some(texture) => {
            painter.image(
                texture.id(),
                art,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            painter.circle_filled(art.center(), 20.0, t.brand_tint);
            painter.text(
                art.center(),
                egui::Align2::CENTER_CENTER,
                initials,
                theme::font(15.0, Weight::SemiBold),
                t.brand,
            );
        }
    }

    // Two lines at most, centred under the picture.
    let galley = painter.layout(
        name.to_owned(),
        theme::font(12.5, Weight::Medium),
        if hovered { t.text } else { t.text_dim },
        W - 2.0 * theme::S2,
    );
    let top = rect.top() + 62.0;
    painter.galley(
        egui::Pos2::new(rect.center().x - galley.size().x / 2.0, top),
        galley,
        t.text,
    );

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(name)
}

/// Decodes an application icon once and keeps the texture around.
fn icon_texture(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<std::path::PathBuf, Option<egui::TextureHandle>>,
    path: &std::path::Path,
) -> Option<egui::TextureHandle> {
    if let Some(found) = cache.get(path) {
        return found.clone();
    }
    let loaded = std::fs::read(path)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|picture| {
            let rgba = picture.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            ctx.load_texture("app-icon", image, egui::TextureOptions::LINEAR)
        });
    cache.insert(path.to_path_buf(), loaded.clone());
    loaded
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
