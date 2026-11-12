//! The "about" screen: what the tool is, who wrote it and what it is made of.

use egui::{Align2, CornerRadius, Sense, Ui, Vec2};

use super::components::{self as c, tokens};
use super::icons::{self, Icon};
use crate::app::{Action, App};
use crate::config::Config;
use crate::i18n::tr;
use crate::theme::{self, Weight};

const AUTHOR: &str = "Vitor Moreira";
const CONTACT: &str = "vitormoreiradesenvolvedor@gmail.com";

pub fn show(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);

    c::heading(ui, tr("nav.about"));
    c::muted(ui, tr("about.subtitle"));
    ui.add_space(theme::S4);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(720.0_f32.min(ui.available_width()));

            // ---- the tool ------------------------------------------------
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(20), t.brand_tint);
                    icons::paint(ui.painter(), rect.shrink(16.0), Icon::Branch, t.brand);
                    ui.add_space(theme::S3);
                    ui.vertical(|ui| {
                        c::text_with(ui, crate::APP_NAME, 22.0, Weight::Bold, t.text);
                        ui.add_space(2.0);
                        c::mono_label(ui, format!("v{}", env!("CARGO_PKG_VERSION")), t.brand);
                    });
                });
                ui.add_space(theme::S3);
                c::body(ui, tr("about.tagline"));
            });

            ui.add_space(theme::S4);

            // ---- who ------------------------------------------------------
            card(ui, |ui| {
                c::section_label(ui, tr("about.developer"));
                ui.add_space(theme::S3);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 22.0, t.surface_alt);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        initials(AUTHOR),
                        theme::font(15.0, Weight::Bold),
                        t.brand,
                    );
                    ui.add_space(theme::S3);
                    ui.vertical(|ui| {
                        c::text_with(ui, AUTHOR, 15.0, Weight::SemiBold, t.text);
                        ui.add_space(2.0);
                        c::label_value(ui, &format!("{}: ", tr("about.contact")), CONTACT);
                    });
                });
            });

            ui.add_space(theme::S4);

            // ---- what it does ---------------------------------------------
            card(ui, |ui| {
                c::section_label(ui, tr("about.what"));
                ui.add_space(theme::S3);
                c::body(ui, tr("about.what_body"));
            });

            ui.add_space(theme::S4);

            // ---- what it is made of ---------------------------------------
            card(ui, |ui| {
                c::section_label(ui, tr("about.built_with"));
                ui.add_space(theme::S3);
                for (name, what) in [
                    ("Rust", env!("CARGO_PKG_RUST_VERSION")),
                    ("eframe / egui", "0.37 · OpenGL (glow)"),
                    ("portable-pty", "PTY · ConPTY"),
                    ("alacritty_terminal", "VT"),
                ] {
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 3.0, t.brand);
                        ui.add_space(theme::S2);
                        c::text_with(ui, name, 13.0, Weight::SemiBold, t.text);
                        ui.add_space(theme::S2);
                        c::muted(ui, what);
                    });
                    ui.add_space(2.0);
                }
            });

            ui.add_space(theme::S4);

            // ---- licence and where things live ----------------------------
            card(ui, |ui| {
                c::section_label(ui, tr("about.license"));
                ui.add_space(theme::S3);
                c::label_value(ui, &format!("{}: ", tr("about.license")), "MIT");
                ui.add_space(2.0);
                c::muted(ui, tr("about.fonts"));

                ui.add_space(theme::S3);
                c::divider(ui);
                ui.add_space(theme::S3);
                let path = Config::path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "—".to_owned());
                c::label_value(ui, tr("set.config"), &path);
                ui.add_space(theme::S3);
                ui.horizontal(|ui| {
                    if c::Btn::new(tr("set.open_config"))
                        .icon(Icon::Folder)
                        .show(ui)
                        .clicked()
                        && let Some(dir) =
                            Config::path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    {
                        let _ = std::fs::create_dir_all(&dir);
                        actions.push(Action::OpenFolder(dir));
                    }
                });
            });

            ui.add_space(theme::S6);
            let _ = &app.cfg;
        });
}

/// A card that always fills the column, so they all line up.
fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    c::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        add(ui)
    })
}

/// First letters of the name, for the little avatar.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_take_the_first_two_words() {
        assert_eq!(initials("Vitor Moreira"), "VM");
        assert_eq!(initials("ana"), "A");
        assert_eq!(initials(""), "");
    }
}
