//! Preferences screen.

use egui::{Sense, Ui, Vec2};

use super::components::{self as c, tokens};
use crate::app::{Action, App};
use crate::i18n::{self, Lang, tr, tr1};
use crate::theme::{self, Accent, Look, Mode, MonoFont, UiFont, Weight};

pub fn show(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);

    c::heading(ui, tr("set.title"));
    c::muted(ui, tr("set.subtitle"));
    ui.add_space(theme::S4);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Never wider than the room there is: at a big interface scale the
            // cards would otherwise run off the right edge.
            ui.set_max_width(720.0_f32.min(ui.available_width()));

            c::card(ui, |ui| {
                c::section_label(ui, tr("set.appearance"));
                ui.add_space(theme::S3);

                row(ui, tr("set.language"), |ui| {
                    let mut lang = app.cfg.settings.lang;
                    let options: Vec<(Lang, &str)> =
                        Lang::ALL.iter().map(|l| (*l, l.label())).collect();
                    if c::segmented(ui, &mut lang, &options) {
                        app.cfg.settings.lang = lang;
                        actions.push(Action::ApplyLook { fonts: true });
                    }
                });
                if app.cfg.settings.lang.needs_cjk() && theme::system_cjk().is_none() {
                    ui.add_space(theme::S2);
                    c::dot_label(ui, t.warning, tr("set.cjk_missing"));
                }

                ui.add_space(theme::S3);
                row(ui, tr("set.theme"), |ui| {
                    let mut mode = app.cfg.settings.theme;
                    if c::segmented(
                        ui,
                        &mut mode,
                        &[
                            (Mode::Dark, Mode::Dark.label()),
                            (Mode::Light, Mode::Light.label()),
                        ],
                    ) {
                        actions.push(Action::SetTheme(mode));
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.accent"), |ui| {
                    if accent_picker(ui, &mut app.cfg.settings.accent, app.cfg.settings.theme) {
                        actions.push(Action::ApplyLook { fonts: false });
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.ui_font"), |ui| {
                    if combo(
                        ui,
                        "ui-font",
                        &mut app.cfg.settings.ui_font,
                        &UiFont::ALL,
                        |f| f.label(),
                    ) {
                        actions.push(Action::ApplyLook { fonts: true });
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.mono_font"), |ui| {
                    if combo(
                        ui,
                        "mono-font",
                        &mut app.cfg.settings.mono_font,
                        &MonoFont::ALL,
                        |f| f.label(),
                    ) {
                        actions.push(Action::ApplyLook { fonts: true });
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.ui_scale"), |ui| {
                    // The zoom factor is only pushed into egui when the drag
                    // ends: changing it every frame rebuilds the font atlas and
                    // the whole interface jumps under the cursor.
                    let response = ui.add(
                        egui::Slider::new(&mut app.cfg.settings.ui_scale, 0.8..=1.4)
                            .step_by(0.05)
                            .custom_formatter(|v, _| percent(v)),
                    );
                    let settled = response.drag_stopped()
                        || (response.changed() && !response.dragged())
                        || response.lost_focus();
                    if settled {
                        actions.push(Action::ApplyLook { fonts: false });
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.app_opacity"), |ui| {
                    if opacity_slider(ui, &mut app.cfg.settings.app_opacity) {
                        actions.push(Action::ApplyLook { fonts: false });
                    }
                });
                ui.add_space(theme::S3);
                row(ui, tr("set.term_opacity"), |ui| {
                    if opacity_slider(ui, &mut app.cfg.settings.term_opacity) {
                        actions.push(Action::ApplyLook { fonts: false });
                    }
                });
                c::muted(ui, tr("set.opacity_hint"));

                ui.add_space(theme::S3);
                c::divider(ui);
                ui.add_space(theme::S3);
                if c::toggle_row(
                    ui,
                    tr("set.show_clean"),
                    tr("set.show_clean_hint"),
                    &mut app.cfg.settings.show_clean_status,
                ) {
                    actions.push(Action::SaveConfig);
                }
            });

            ui.add_space(theme::S4);

            c::card(ui, |ui| {
                c::section_label(ui, tr("set.terminal_section"));
                ui.add_space(theme::S3);

                c::body(ui, tr("set.shell"));
                ui.add_space(4.0);
                let detected = tr1("set.shell_hint", app.cfg.shell());
                let width = ui.available_width();
                if c::text_field(ui, &mut app.cfg.settings.shell, &detected, width).changed() {
                    actions.push(Action::SaveConfig);
                }

                ui.add_space(theme::S3);
                row(ui, tr("set.font_size"), |ui| {
                    if ui
                        .add(
                            egui::Slider::new(&mut app.cfg.settings.term_font_size, 9.0..=22.0)
                                .step_by(0.5)
                                .suffix(" px"),
                        )
                        .changed()
                    {
                        actions.push(Action::SaveConfig);
                    }
                });

                ui.add_space(theme::S3);
                row(ui, tr("set.scrollback"), |ui| {
                    let mut lines = app.cfg.settings.scrollback as u32;
                    if ui
                        .add(
                            egui::Slider::new(&mut lines, 1_000..=200_000)
                                .logarithmic(true)
                                .suffix(tr("set.lines")),
                        )
                        .changed()
                    {
                        app.cfg.settings.scrollback = lines as usize;
                        actions.push(Action::SaveConfig);
                    }
                });
                c::muted(ui, tr("set.scrollback_hint"));

                ui.add_space(theme::S3);
                c::muted(ui, tr("set.ctrl_c_hint"));

                ui.add_space(theme::S3);
                if c::toggle_row(
                    ui,
                    tr("set.open_terminal"),
                    tr("set.open_terminal_hint"),
                    &mut app.cfg.settings.open_terminal_after_create,
                ) {
                    actions.push(Action::SaveConfig);
                }
            });

            ui.add_space(theme::S4);

            c::card(ui, |ui| {
                c::section_label(ui, tr("set.external"));
                ui.add_space(theme::S3);
                c::muted(ui, tr("set.external_hint"));
                ui.add_space(theme::S2);

                c::body(ui, tr("set.system_terminal"));
                ui.add_space(4.0);
                if c::text_field(
                    ui,
                    &mut app.cfg.settings.terminal_cmd,
                    "ptyxis --new-window -d {path}",
                    ui.available_width(),
                )
                .changed()
                {
                    actions.push(Action::SaveConfig);
                }

                ui.add_space(theme::S3);
                c::body(ui, tr("set.editor"));
                ui.add_space(4.0);
                if c::text_field(
                    ui,
                    &mut app.cfg.settings.editor_cmd,
                    "code {path}",
                    ui.available_width(),
                )
                .changed()
                {
                    actions.push(Action::SaveConfig);
                }
            });

            ui.add_space(theme::S4);

            c::card(ui, |ui| {
                c::section_label(ui, tr("set.shortcuts"));
                ui.add_space(theme::S3);
                for (keys, what) in [
                    ("Ctrl+N", tr("sc.new")),
                    ("Ctrl+R / F5", tr("sc.refresh")),
                    ("Ctrl+Shift+T", tr("sc.multi")),
                    ("Ctrl+Shift+W", tr("sc.close_pane")),
                    ("Ctrl+Shift+B", tr("sc.mirror")),
                    ("F9", tr("sc.sidebar")),
                    ("F11", tr("sc.fullscreen")),
                    ("Ctrl+C", tr("sc.sigint")),
                    ("Ctrl+Shift+C / V", tr("sc.copy_paste")),
                    ("Ctrl+Shift+↑ ↓", tr("sc.scroll")),
                    ("Ctrl + scroll", tr("sc.font")),
                    ("Ctrl+1 / 2 / 3", tr("sc.screens")),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(keys)
                                .font(theme::mono(11.5, true))
                                .color(t.brand),
                        );
                        ui.add_space(theme::S2);
                        c::muted(ui, what);
                    });
                }
            });

            ui.add_space(theme::S6);
            let _ = i18n::lang();
        });
}

/// Drop-down over a fixed list of options. Returns true when the pick changed.
fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    current: &mut T,
    options: &[T],
    label: impl Fn(T) -> &'static str,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(label(*current))
        .width(190.0)
        .show_ui(ui, |ui| {
            for option in options {
                if ui
                    .selectable_label(*current == *option, label(*option))
                    .clicked()
                    && *current != *option
                {
                    *current = *option;
                    changed = true;
                }
            }
        });
    changed
}

/// A label on the left and its control on the right, on one line.
fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let t = tokens(ui);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(170.0, 20.0), Sense::hover());
        ui.painter().text(
            egui::Pos2::new(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            theme::font(13.0, Weight::Medium),
            t.text,
        );
        add(ui)
    })
    .inner
}

fn percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

fn opacity_slider(ui: &mut Ui, value: &mut f32) -> bool {
    ui.add(
        egui::Slider::new(value, 0.4..=1.0)
            .step_by(0.01)
            .custom_formatter(|v, _| percent(v)),
    )
    .changed()
}

/// Round swatches, one per accent, painted in the color they stand for.
fn accent_picker(ui: &mut Ui, current: &mut Accent, mode: Mode) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for accent in Accent::ALL {
            let look = Look {
                mode,
                accent,
                app_opacity: 1.0,
                term_opacity: 1.0,
            };
            let color = theme::tokens(look).brand;
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
            let selected = *current == accent;
            let painter = ui.painter();
            let center = rect.center();
            if selected || response.hovered() {
                painter.circle_stroke(
                    center,
                    13.0,
                    egui::Stroke::new(if selected { 2.0 } else { 1.0 }, color),
                );
            }
            painter.circle_filled(center, 8.0, color);
            if response.clicked() && !selected {
                *current = accent;
                changed = true;
            }
            response
                .on_hover_text(accent.label())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    });
    changed
}
