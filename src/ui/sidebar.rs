//! Left rail: brand header, screen navigation and the project list.

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::components::{self as c, tokens};
use super::icons::{self, Icon};
use crate::app::{Action, App, Screen};
use crate::i18n::tr;
use crate::theme::{self, Weight};

pub fn show(app: &App, root: &mut egui::Ui, actions: &mut Vec<Action>) {
    let t = theme::tokens(app.cfg.settings.look());
    let collapsed = app.cfg.settings.sidebar_collapsed;

    // The column slides between the two widths instead of snapping.
    let target = if collapsed {
        theme::RAIL_W
    } else {
        theme::SIDEBAR_W
    };
    let width = root.ctx().animate_value_with_time(
        egui::Id::unique("worktree-hub-sidebar-width"),
        target,
        0.16,
    );
    let rail_side = width < (theme::RAIL_W + theme::SIDEBAR_W) / 2.0;

    egui::Panel::left("worktree-hub-sidebar")
        .exact_size(width)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(t.surface)
                .stroke(Stroke::new(1.0, t.border)),
        )
        .show(root, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
            if rail_side {
                rail(app, ui, actions);
            } else {
                full(app, ui, actions);
            }
        });
}

/// Icons only: the whole window belongs to the terminals.
fn rail(app: &App, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::hover());
    c::gradient_header(ui, rect);
    let mark = Rect::from_center_size(rect.center(), Vec2::splat(34.0));
    ui.painter()
        .rect_filled(mark, CornerRadius::same(11), Color32::from_white_alpha(38));
    icons::paint(ui.painter(), mark.shrink(8.0), Icon::Branch, Color32::WHITE);

    // The fold button keeps its place at the top in both states.
    ui.add_space(theme::S2);
    if rail_item(ui, Icon::ChevronRight, false, None, tr("nav.expand")).clicked() {
        actions.push(Action::ToggleSidebar);
    }
    ui.add_space(theme::S2);
    c::divider(ui);
    ui.add_space(theme::S2);
    let screens = [
        (
            Icon::Branch,
            Screen::Worktrees,
            format!("{} (Ctrl+1)", tr("nav.worktrees")),
            None,
        ),
        (
            Icon::Terminal,
            Screen::Terminals,
            format!("{} (Ctrl+2)", tr("nav.terminals")),
            (!app.sessions.is_empty()).then_some(app.sessions.len()),
        ),
        (
            Icon::Settings,
            Screen::Settings,
            format!("{} (Ctrl+3)", tr("nav.settings")),
            None,
        ),
        (
            Icon::Info,
            Screen::About,
            format!("{} (Ctrl+4)", tr("nav.about")),
            None,
        ),
    ];
    for (icon, screen, hint, badge) in screens {
        if rail_item(ui, icon, app.screen == screen, badge, &hint).clicked() {
            actions.push(Action::SetScreen(screen));
        }
        ui.add_space(theme::S1);
    }

    let bottom = 52.0;
    ui.allocate_space(Vec2::new(
        ui.available_width(),
        (ui.available_height() - bottom).max(0.0),
    ));
    if rail_item(ui, Icon::Plus, false, None, tr("nav.add_project")).clicked() {
        actions.push(Action::AddProjectDialog);
    }
    let _ = t;
}

/// One 44px icon button in the rail.
fn rail_item(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    badge: Option<usize>,
    hint: &str,
) -> egui::Response {
    let t = tokens(ui);
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
    let rect = Rect::from_center_size(row.center(), Vec2::splat(40.0));
    let response = ui.interact(rect, ui.scope_id().with(("rail", hint)), Sense::click());

    // The highlight fades in and out instead of switching on the click.
    let lit =
        ui.ctx()
            .animate_bool_with_time(ui.scope_id().with(("rail-lit", hint)), selected, 0.15);
    let painter = ui.painter();
    if lit > 0.01 {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::R_INPUT),
            t.brand_tint.gamma_multiply(lit),
        );
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), t.surface_hover);
    }
    icons::paint(
        painter,
        rect.shrink(11.0),
        icon,
        c::lerp_color(t.text_dim, t.brand, lit),
    );
    if let Some(count) = badge {
        let pill = Rect::from_center_size(
            Pos2::new(rect.right() - 4.0, rect.top() + 6.0),
            Vec2::new(18.0, 16.0),
        );
        painter.rect_filled(pill, CornerRadius::same(theme::R_PILL), t.brand);
        painter.text(
            pill.center(),
            Align2::CENTER_CENTER,
            count.to_string(),
            theme::font(10.0, Weight::Bold),
            t.on_brand,
        );
    }

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(hint)
}

fn full(app: &App, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    if header(ui) {
        actions.push(Action::ToggleSidebar);
    }

    ui.add_space(theme::S4);
    ui.horizontal(|ui| {
        ui.add_space(theme::S3);
        ui.vertical(|ui| {
            ui.set_width(theme::SIDEBAR_W - 2.0 * theme::S3);

            if nav_item(
                ui,
                Icon::Branch,
                tr("nav.worktrees"),
                app.screen == Screen::Worktrees,
                None,
            )
            .clicked()
            {
                actions.push(Action::SetScreen(Screen::Worktrees));
            }

            let count = app.sessions.len();
            if nav_item(
                ui,
                Icon::Terminal,
                tr("nav.terminals"),
                app.screen == Screen::Terminals,
                (count > 0).then(|| count.to_string()),
            )
            .clicked()
            {
                actions.push(Action::SetScreen(Screen::Terminals));
            }

            if nav_item(
                ui,
                Icon::Settings,
                tr("nav.settings"),
                app.screen == Screen::Settings,
                None,
            )
            .clicked()
            {
                actions.push(Action::SetScreen(Screen::Settings));
            }

            if nav_item(
                ui,
                Icon::Info,
                tr("nav.about"),
                app.screen == Screen::About,
                None,
            )
            .clicked()
            {
                actions.push(Action::SetScreen(Screen::About));
            }

            ui.add_space(theme::S5);
            c::section_label(ui, tr("nav.projects"));
            ui.add_space(theme::S2);
        });
    });

    let bottom = 64.0;
    let list_height = (ui.available_height() - bottom).max(80.0);
    egui::ScrollArea::vertical()
        .max_height(list_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(theme::S3);
                ui.vertical(|ui| {
                    ui.set_width(theme::SIDEBAR_W - 2.0 * theme::S3);
                    if app.cfg.projects.is_empty() {
                        ui.add_space(theme::S2);
                        c::muted(ui, tr("nav.no_projects"));
                    }
                    for project in &app.cfg.projects {
                        let selected = app.cfg.last_project == Some(project.id);
                        let response = project_row(
                            ui,
                            &project.name,
                            &project.root.to_string_lossy(),
                            selected,
                        );
                        if response.clicked() {
                            actions.push(Action::SelectProject(project.id));
                        }
                        response.context_menu(|ui| {
                            if ui.button(tr("nav.remove_project")).clicked() {
                                actions.push(Action::RemoveProjectDialog(project.id));
                                ui.close();
                            }
                            if ui.button(tr("nav.open_folder")).clicked() {
                                actions.push(Action::OpenFolder(project.root.clone()));
                                ui.close();
                            }
                        });
                        ui.add_space(theme::S1);
                    }
                });
            });
        });

    ui.allocate_space(Vec2::new(
        ui.available_width(),
        ui.available_height() - bottom,
    ));
    ui.horizontal(|ui| {
        ui.add_space(theme::S3);
        if c::Btn::primary(tr("nav.add_project"))
            .icon(Icon::Plus)
            .min_width(theme::SIDEBAR_W - 2.0 * theme::S3)
            .show(ui)
            .clicked()
        {
            actions.push(Action::AddProjectDialog);
        }
    });
    ui.add_space(theme::S3);
}

/// Returns true when the user asked to fold the sidebar.
fn header(ui: &mut egui::Ui) -> bool {
    let t = tokens(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 112.0), Sense::hover());
    c::gradient_header(ui, rect);

    let fold = Rect::from_center_size(
        Pos2::new(rect.right() - theme::S4 - 2.0, rect.top() + theme::S4 + 6.0),
        Vec2::splat(26.0),
    );
    let fold_response = ui
        .interact(fold, ui.scope_id().with("fold"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tr("nav.collapse"));
    ui.painter().rect_filled(
        fold,
        CornerRadius::same(theme::R_PILL),
        Color32::from_white_alpha(if fold_response.hovered() { 60 } else { 28 }),
    );
    icons::paint(
        ui.painter(),
        fold.shrink(7.0),
        Icon::ChevronLeft,
        Color32::WHITE,
    );

    let painter = ui.painter();
    let mark = Rect::from_center_size(
        Pos2::new(rect.left() + theme::S4 + 20.0, rect.top() + 44.0),
        Vec2::splat(40.0),
    );
    painter.rect_filled(mark, CornerRadius::same(13), Color32::from_white_alpha(38));
    icons::paint(painter, mark.shrink(10.0), Icon::Branch, Color32::WHITE);

    painter.text(
        Pos2::new(mark.right() + theme::S3, rect.top() + 34.0),
        Align2::LEFT_TOP,
        "Worktree Hub",
        theme::font(16.0, Weight::Bold),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(mark.right() + theme::S3, rect.top() + 55.0),
        Align2::LEFT_TOP,
        format!("v{}", env!("CARGO_PKG_VERSION")),
        theme::font(11.5, Weight::Medium),
        Color32::from_white_alpha(170),
    );
    let _ = t;
    fold_response.clicked()
}

fn nav_item(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    badge: Option<String>,
) -> egui::Response {
    let t = tokens(ui);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());

    let lit =
        ui.ctx()
            .animate_bool_with_time(ui.scope_id().with(("nav-lit", label)), selected, 0.15);
    let painter = ui.painter();
    if lit > 0.01 {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::R_INPUT),
            t.brand_tint.gamma_multiply(lit),
        );
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), t.surface_hover);
    }

    let color = c::lerp_color(t.text_dim, t.brand, lit);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + theme::S3 + 9.0, rect.center().y),
        Vec2::splat(18.0),
    );
    icons::paint(painter, icon_rect, icon, color);
    painter.text(
        Pos2::new(icon_rect.right() + theme::S3, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font(
            13.5,
            if selected {
                Weight::SemiBold
            } else {
                Weight::Medium
            },
        ),
        c::lerp_color(t.text_dim, t.text, lit),
    );

    if let Some(badge) = badge {
        let pill = Rect::from_center_size(
            Pos2::new(rect.right() - theme::S3 - 13.0, rect.center().y),
            Vec2::new(26.0, 20.0),
        );
        painter.rect_filled(pill, CornerRadius::same(theme::R_PILL), t.brand);
        painter.text(
            pill.center(),
            Align2::CENTER_CENTER,
            badge,
            theme::font(11.0, Weight::Bold),
            t.on_brand,
        );
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn project_row(ui: &mut egui::Ui, name: &str, path: &str, selected: bool) -> egui::Response {
    let t = tokens(ui);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());

    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), t.brand_tint);
        painter.rect_stroke(
            rect,
            CornerRadius::same(theme::R_INPUT),
            Stroke::new(1.0, t.brand.gamma_multiply(0.45)),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), t.surface_hover);
    }

    let dot = Pos2::new(rect.left() + theme::S3 + 4.0, rect.center().y);
    painter.circle_filled(dot, 4.0, if selected { t.brand } else { t.border_strong });

    let text_left = dot.x + theme::S3;
    let max_w = rect.right() - theme::S3 - text_left;
    let name_galley = painter.layout_no_wrap(
        name.to_owned(),
        theme::font(
            13.0,
            if selected {
                Weight::SemiBold
            } else {
                Weight::Medium
            },
        ),
        t.text,
    );
    painter.galley(Pos2::new(text_left, rect.top() + 9.0), name_galley, t.text);

    let shortened = shorten_path(path, 30);
    let path_galley = painter.layout_no_wrap(shortened, theme::mono(10.5, false), t.text_faint);
    let _ = max_w;
    painter.galley(
        Pos2::new(text_left, rect.top() + 28.0),
        path_galley,
        t.text_faint,
    );

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Keeps the tail of a path, which is the part that identifies the repository.
pub fn shorten_path(path: &str, max_chars: usize) -> String {
    let count = path.chars().count();
    if count <= max_chars {
        return path.to_owned();
    }
    let tail: String = path
        .chars()
        .skip(count.saturating_sub(max_chars.saturating_sub(1)))
        .collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::shorten_path;

    #[test]
    fn short_paths_are_untouched() {
        assert_eq!(shorten_path("/home/u/p", 30), "/home/u/p");
    }

    #[test]
    fn long_paths_keep_the_tail() {
        let out = shorten_path("/home/user/Projetos-2026/epicpen-linux", 20);
        assert!(out.starts_with('…'));
        assert!(out.ends_with("epicpen-linux"));
        assert_eq!(out.chars().count(), 20);
    }
}
