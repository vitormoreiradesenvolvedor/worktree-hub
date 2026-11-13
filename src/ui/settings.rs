//! Preferences screen.

use egui::{Sense, Ui, Vec2};

use super::components::{self as c, tokens};
use super::icons::Icon;
use crate::app::{Action, App};
use crate::i18n::{self, Lang, tr, tr1};
use crate::theme::{self, Accent, Look, Mode, MonoFont, UiFont, Weight};

pub fn show(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    c::heading(ui, tr("set.title"));
    c::muted(ui, tr("set.subtitle"));
    ui.add_space(theme::S4);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // The controls stay in a readable column; a wide window gets a
            // second one beside it instead of a field of empty background.
            let room = ui.available_width();
            let two = room >= MAIN + ASIDE + theme::S4;
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(MAIN.min(room));
                    main_column(app, ui, actions);
                    if !two {
                        ui.add_space(theme::S4);
                        aside(app, ui, actions);
                    }
                });
                if two {
                    ui.add_space(theme::S4);
                    ui.vertical(|ui| {
                        // Both cards the same width, however long a
                        // shortcut line turns out to be.
                        ui.set_min_width(ASIDE);
                        ui.set_max_width(ASIDE);
                        aside(app, ui, actions);
                    });
                }
            });

            ui.add_space(theme::S6);
            let _ = i18n::lang();
        });
}

/// Width of the column of controls, and of the column beside it.
const MAIN: f32 = 720.0;
const ASIDE: f32 = 380.0;

/// Everything the user can change.
fn main_column(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);

    c::card(ui, |ui| {
        c::section_label(ui, tr("set.appearance"));
        ui.add_space(theme::S3);

        row(ui, tr("set.language"), |ui| {
            let mut lang = app.cfg.settings.lang;
            let options: Vec<(Lang, &str)> = Lang::ALL.iter().map(|l| (*l, l.label())).collect();
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
}

/// The column beside the controls: what the choices look like, the shortcuts
/// and where this copy keeps its files.
fn aside(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);
    // A card is as wide as what it holds, and these hold very different
    // things; without a floor they would step in and out along the column.
    let w = (ui.available_width() - theme::S4 * 2.0).max(0.0);

    c::card(ui, |ui| {
        ui.set_min_width(w);
        c::section_label(ui, tr("set.preview"));
        ui.add_space(theme::S2);
        c::muted(ui, tr("set.preview_hint"));
        ui.add_space(theme::S3);
        preview(app, ui);
    });

    ui.add_space(theme::S4);

    c::card(ui, |ui| {
        ui.set_min_width(w);
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
            ("Shift + mouse", tr("sc.mouse")),
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

    ui.add_space(theme::S4);

    c::card(ui, |ui| {
        ui.set_min_width(w);
        c::section_label(ui, tr("set.install"));
        ui.add_space(theme::S3);
        c::label_value(ui, tr("set.version"), env!("CARGO_PKG_VERSION"));
        ui.add_space(theme::S2);
        c::label_value(
            ui,
            tr("set.projects_count"),
            &app.cfg.projects.len().to_string(),
        );
        ui.add_space(theme::S2);
        c::label_value(ui, tr("set.panes_count"), &app.sessions.len().to_string());

        let Some(file) = crate::config::Config::path() else {
            return;
        };
        ui.add_space(theme::S3);
        c::muted(ui, tr("set.config_file"));
        ui.add_space(4.0);
        let width = ui.available_width();
        c::truncated(
            ui,
            &file.to_string_lossy(),
            11.5,
            Weight::Regular,
            t.text_dim,
            width,
        );
        ui.add_space(theme::S3);
        if let Some(folder) = file.parent() {
            if c::Btn::soft(tr("set.open_config"))
                .icon(Icon::Folder)
                .small()
                .show(ui)
                .clicked()
            {
                actions.push(Action::OpenFolder(folder.to_path_buf()));
            }
        }
    });
}

/// A small mock of the interface, painted with the settings as they stand, so
/// the accent, the fonts and the two opacities can be judged without leaving
/// the screen.
fn preview(app: &App, ui: &mut Ui) {
    let t = tokens(ui);
    let s = &app.cfg.settings;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 168.0), Sense::hover());
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(theme::R_INPUT);

    // The window itself, at the interface opacity, over a checker so that a
    // see-through setting reads as see-through.
    checker(painter, rect, radius, t.border);
    painter.rect_filled(rect, radius, fade(t.bg, s.app_opacity));
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );

    let pad = theme::S3;
    let inner = rect.shrink(pad);

    // Title bar: three dots in the accent and the app name in the chosen font.
    for (i, color) in [t.brand, t.brand_soft, t.border_strong].iter().enumerate() {
        painter.circle_filled(
            egui::Pos2::new(inner.left() + 5.0 + i as f32 * 14.0, inner.top() + 6.0),
            4.0,
            *color,
        );
    }
    painter.text(
        egui::Pos2::new(inner.left() + 52.0, inner.top() + 6.0),
        egui::Align2::LEFT_CENTER,
        "Worktree Hub",
        theme::font(11.5, Weight::SemiBold),
        t.text,
    );

    // A worktree card, to show the accent against a surface.
    let card = egui::Rect::from_min_size(
        egui::Pos2::new(inner.left(), inner.top() + 20.0),
        Vec2::new(inner.width(), 38.0),
    );
    painter.rect_filled(card, egui::CornerRadius::same(theme::R_INPUT), t.surface);
    painter.circle_filled(
        egui::Pos2::new(card.left() + 14.0, card.center().y),
        4.0,
        t.success,
    );
    painter.text(
        egui::Pos2::new(card.left() + 26.0, card.center().y - 6.0),
        egui::Align2::LEFT_CENTER,
        "feature/login",
        theme::font(11.5, Weight::Medium),
        t.text,
    );
    painter.text(
        egui::Pos2::new(card.left() + 26.0, card.center().y + 7.0),
        egui::Align2::LEFT_CENTER,
        "~/code/app-worktrees",
        theme::font(10.0, Weight::Regular),
        t.text_faint,
    );
    let pill = egui::Rect::from_center_size(
        egui::Pos2::new(card.right() - 34.0, card.center().y),
        Vec2::new(52.0, 20.0),
    );
    painter.rect_filled(pill, egui::CornerRadius::same(theme::R_PILL), t.brand_tint);
    painter.text(
        pill.center(),
        egui::Align2::CENTER_CENTER,
        tr("set.preview_open"),
        theme::font(10.0, Weight::SemiBold),
        t.brand,
    );

    // A terminal pane, at the terminal opacity and the terminal font size.
    let pane = egui::Rect::from_min_max(
        egui::Pos2::new(inner.left(), card.bottom() + theme::S2),
        inner.max,
    );
    painter.rect_filled(
        pane,
        egui::CornerRadius::same(theme::R_INPUT),
        fade(t.term_bg, s.term_opacity),
    );
    let size = s.term_font_size.min(13.0);
    let mut y = pane.top() + size * 0.9;
    for (text, color) in [
        ("$ git status", t.term_fg),
        ("On branch feature/login", t.ansi[10]),
        ("nothing to commit", t.ansi[8]),
    ] {
        painter.text(
            egui::Pos2::new(pane.left() + 10.0, y),
            egui::Align2::LEFT_TOP,
            text,
            theme::mono(size, false),
            color,
        );
        y += size * 1.45;
    }
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::Pos2::new(pane.left() + 10.0, y),
            Vec2::new(size * 0.6, size),
        ),
        egui::CornerRadius::ZERO,
        t.term_cursor,
    );
}

/// Faint squares behind the mock, so a see-through setting is visible as such.
fn checker(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: egui::CornerRadius,
    color: egui::Color32,
) {
    const STEP: f32 = 8.0;
    painter.rect_filled(rect, radius, egui::Color32::TRANSPARENT);
    let clip = painter.with_clip_rect(rect);
    let mut row = 0;
    let mut y = rect.top();
    while y < rect.bottom() {
        let mut col = 0;
        while rect.left() + col as f32 * STEP < rect.right() {
            if (row + col) % 2 == 0 {
                let square = egui::Rect::from_min_size(
                    egui::Pos2::new(rect.left() + col as f32 * STEP, y),
                    Vec2::splat(STEP),
                )
                .intersect(rect);
                clip.rect_filled(square, egui::CornerRadius::ZERO, color);
            }
            col += 1;
        }
        y += STEP;
        row += 1;
    }
}

/// The color as it would look over the desktop at this opacity.
fn fade(color: egui::Color32, opacity: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
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
