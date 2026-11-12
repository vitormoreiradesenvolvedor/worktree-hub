//! Terminator-style multi-terminal: one PTY per selected worktree, tiled — or
//! arranged by hand in the "free" layout, where panes snap against each other
//! and the neighbours take back whatever room a pane leaves behind.

use std::sync::Arc;

use egui::{
    Align, Color32, CornerRadius, CursorIcon, Id, Layout, Pos2, Rect, Sense, Stroke, StrokeKind,
    Ui, UiBuilder, Vec2,
};

use super::components::{self as c, tokens};
use super::icons::Icon;
use super::tiles::{self, Dir, Side, Tile};
use crate::app::{Action, App};
use crate::config::{self, PaneLayout};
use crate::i18n::{tr, tr1};
use crate::term::{TermTheme, show_terminal};
use crate::theme::{self, Weight};

const HEADER_H: f32 = 34.0;
/// Height of the broadcast field, and of the icon beside it.
const BROADCAST_H: f32 = 30.0;
const GAP: f32 = 10.0;
/// How long a pane takes to glide to its new place.
const REFLOW: f32 = 0.13;
/// Drop this close to the outer border and the pane takes half the area.
const EDGE_DOCK: f32 = 46.0;

pub fn show(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let t = tokens(ui);

    if app.sessions.is_empty() {
        c::empty_state(
            ui,
            Icon::Terminal,
            tr("term.empty_title"),
            tr("term.empty_body"),
        );
        ui.vertical_centered(|ui| {
            if c::Btn::primary(tr("term.back"))
                .icon(Icon::ChevronLeft)
                .show(ui)
                .clicked()
            {
                actions.push(Action::SetScreen(crate::app::Screen::Worktrees));
            }
        });
        return;
    }

    // ---- toolbar ----------------------------------------------------------
    let mut layout = app.cfg.settings.pane_layout;
    let free = layout == PaneLayout::Free;
    c::header_row(
        ui,
        46.0,
        |ui| {
            let tight = ui.available_width() < 600.0;
            if c::Btn::ghost(tr("term.close_all"))
                .icon(Icon::Close)
                .show(ui)
                .clicked()
            {
                actions.push(Action::CloseAllSessions);
            }

            let mut broadcast = app.broadcast;
            ui.horizontal(|ui| {
                if c::toggle(ui, &mut broadcast).changed() {
                    app.broadcast = broadcast;
                }
                if !tight {
                    c::text_with(
                        ui,
                        tr("term.mirror"),
                        12.5,
                        Weight::Medium,
                        if broadcast { t.brand } else { t.text_dim },
                    );
                }
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                super::icons::paint(
                    ui.painter(),
                    rect,
                    Icon::Broadcast,
                    if broadcast { t.brand } else { t.text_faint },
                );
                response.on_hover_text(format!(
                    "{} — {}",
                    tr("term.mirror"),
                    tr("term.mirror_hint")
                ));
            });

            ui.add_space(theme::S2);
            let options = [
                (PaneLayout::Grid, Icon::Grid, PaneLayout::Grid.label()),
                (
                    PaneLayout::Columns,
                    Icon::Columns,
                    PaneLayout::Columns.label(),
                ),
                (PaneLayout::Rows, Icon::Rows, PaneLayout::Rows.label()),
                (PaneLayout::Tabs, Icon::Tabs, PaneLayout::Tabs.label()),
                (PaneLayout::Free, Icon::Free, PaneLayout::Free.label()),
            ];
            if c::segmented_icons(ui, &mut layout, &options) {
                actions.push(Action::SetLayout(layout));
            }

            ui.add_space(theme::S1);
            let (icon, hint) = if app.fullscreen {
                (Icon::Shrink, tr("term.fullscreen_off"))
            } else {
                (Icon::Expand, tr("term.fullscreen"))
            };
            // Tidy and fullscreen share a pill, like the layout picker next to
            // them. Inside the pill the slots run left to right, so tidy — the
            // leftmost of the two — comes first.
            let mut tools: Vec<(Icon, &str)> = Vec::new();
            if free {
                tools.push((Icon::Grid, tr("term.tidy")));
            }
            tools.push((icon, hint));
            match c::icon_bar(ui, &tools) {
                Some(0) if free => actions.push(Action::TidyPanes),
                Some(_) => actions.push(Action::ToggleFullscreen),
                None => {}
            }
        },
        |ui, room| {
            c::truncated_end(
                ui,
                tr("nav.terminals"),
                20.0,
                Weight::SemiBold,
                t.text,
                room,
            );
            let hint = if free {
                tr("term.hint_free").to_owned()
            } else {
                tr1("term.hint", app.sessions.len())
            };
            c::truncated_end(ui, &hint, 12.5, Weight::Regular, t.text_dim, room);
        },
    );

    ui.add_space(theme::S3);

    // ---- broadcast bar ----------------------------------------------------
    if app.broadcast {
        let width = ui.available_width();
        c::panel(ui, |ui| {
            ui.horizontal(|ui| {
                // Allocated as tall as the field: a 16px item placed first would
                // stick to the top of the row once the field makes it taller.
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(16.0, BROADCAST_H), Sense::hover());
                super::icons::paint(
                    ui.painter(),
                    Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
                    Icon::Broadcast,
                    t.brand,
                );
                let response = c::text_field_with(
                    ui,
                    &mut app.broadcast_input,
                    tr("term.broadcast_placeholder"),
                    width - 190.0,
                    5,
                );
                let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (submit || c::Btn::primary(tr("term.send")).small().show(ui).clicked())
                    && !app.broadcast_input.trim().is_empty()
                {
                    actions.push(Action::Broadcast(app.broadcast_input.clone()));
                    app.broadcast_input.clear();
                }
            });

            quick_row(app, ui, actions);
        });
        ui.add_space(theme::S3);
    }

    // ---- tabs -------------------------------------------------------------
    if layout == PaneLayout::Tabs {
        let count = app.sessions.len();
        ui.horizontal_wrapped(|ui| {
            for index in 0..count {
                let session = &app.sessions[index];
                let selected = index == app.active_tab.min(count - 1);
                let label = format!("{} · {}", session.label, session.branch);
                let font = theme::font(12.5, Weight::SemiBold);
                let galley = ui
                    .painter()
                    .layout_no_wrap(label, font, Color32::PLACEHOLDER);
                let size = Vec2::new(galley.size().x + 46.0, 32.0);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter();
                painter.rect_filled(
                    rect,
                    CornerRadius::same(theme::R_INPUT),
                    if selected { t.brand_tint } else { t.surface },
                );
                if selected {
                    painter.rect_stroke(
                        rect,
                        CornerRadius::same(theme::R_INPUT),
                        Stroke::new(1.0, t.brand),
                        StrokeKind::Inside,
                    );
                }
                painter.galley(
                    Pos2::new(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    if selected { t.brand } else { t.text_dim },
                );
                let close = Rect::from_center_size(
                    Pos2::new(rect.right() - 16.0, rect.center().y),
                    Vec2::splat(14.0),
                );
                super::icons::paint(painter, close, Icon::Close, t.text_faint);

                if response.clicked() {
                    let pointer = response.interact_pointer_pos();
                    if pointer.is_some_and(|p| close.expand(4.0).contains(p)) {
                        actions.push(Action::CloseSession(app.sessions[index].id));
                    } else {
                        actions.push(Action::FocusPane(index));
                    }
                }
            }
        });
        ui.add_space(theme::S2);
    }

    // ---- where every pane goes --------------------------------------------
    let area = ui.available_rect_before_wrap();
    app.pane_area = area;
    let default_font_size = app.cfg.settings.term_font_size;
    let count = app.sessions.len();

    let placements: Vec<(usize, Rect)> = if free {
        tiling_placements(app, area)
    } else {
        let indices: Vec<usize> = match layout {
            PaneLayout::Tabs => vec![app.active_tab.min(count - 1)],
            _ => (0..count).collect(),
        };
        let (cols, rows) = grid_shape(indices.len(), layout);
        indices
            .iter()
            .enumerate()
            .map(|(slot, &index)| (index, grid_cell(slot, cols, rows, indices.len(), area)))
            .collect()
    };

    // Panes glide to their new place instead of jumping, except while a gesture
    // is in flight — a dragged edge has to follow the pointer exactly.
    let steady = !ui.ctx().input(|i| i.pointer.any_down());
    // The see-through punch is a raw framebuffer clear, which no fade can dim,
    // so it waits until the screen has finished coming in.
    let term_opacity = if app.screen_fade > 0.99 {
        app.cfg.settings.term_opacity
    } else {
        1.0
    };

    let mut mirror: Option<(u64, Vec<u8>)> = None;
    let mut focus_change: Option<usize> = None;
    let mut dragging: Option<(u64, Rect)> = None;
    let mut dropped: Option<u64> = None;
    let mut maximize: Option<u64> = None;
    let broadcast = app.broadcast;

    for &(index, target) in &placements {
        let session_id = app.sessions[index].id;
        let rect = animated_rect(ui.ctx(), session_id, target, steady);

        let focused = index == app.focused_pane;
        let maximized = app.maximized == Some(session_id);
        let session = &mut app.sessions[index];
        let exited = session.has_exited();
        let bell = session.take_bell();
        let title = session.title();
        let grid = session.size();
        let cwd = session.cwd.to_string_lossy().into_owned();
        let label = session.label.clone();
        let branch = session.branch.clone();
        let is_main = session.is_main;

        let font_size = session.font_size.unwrap_or(default_font_size);
        let term_theme = TermTheme {
            bg: t.term_bg,
            fg: t.term_fg,
            cursor: t.term_cursor,
            selection: t.term_selection,
            ansi: t.ansi,
            font_size,
            corner: CornerRadius {
                nw: 0,
                ne: 0,
                sw: theme::R_CARD,
                se: theme::R_CARD,
            },
        };

        let mut close_clicked = false;
        let mut restart_clicked = false;
        let mut maximize_clicked = false;

        ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt(session_id), |ui| {
            let painter = ui.painter();
            let radius = CornerRadius::same(theme::R_CARD);
            if free {
                // A soft shadow separates the panes from the backdrop.
                painter.add(theme::soft_shadow(t).as_shape(rect, radius));
            }
            painter.rect_filled(rect, radius, t.term_bg);

            // header
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER_H));
            ui.painter().rect_filled(
                header,
                CornerRadius {
                    nw: theme::R_CARD,
                    ne: theme::R_CARD,
                    sw: 0,
                    se: 0,
                },
                t.surface.gamma_multiply(if focused { 1.0 } else { 0.85 }),
            );

            // The header is painted, not built from widgets: a label or a chip
            // on top of the drag area would swallow the gesture.
            // A narrow pane drops the maximize button: three icons would land
            // on top of the name.
            let roomy = rect.width() > 300.0;
            let show_max = free && roomy;
            // Measured, not guessed: each icon button is 32 wide with S2 between
            // them, plus the margin on the right and a gap before the text.
            let icon_count = if show_max { 3.0 } else { 2.0 };
            let buttons_w =
                icon_count * 32.0 + (icon_count - 1.0) * theme::S2 + theme::S2 + theme::S3;
            let content = Rect::from_min_max(
                Pos2::new(header.left() + theme::S3, header.top()),
                Pos2::new(
                    (header.right() - buttons_w).max(header.left() + theme::S3),
                    header.bottom(),
                ),
            );
            let painter = ui.painter().with_clip_rect(content);
            let mid = header.center().y;
            let mut x = content.left();

            super::icons::paint(
                &painter,
                Rect::from_center_size(Pos2::new(x + 7.0, mid), Vec2::splat(14.0)),
                if is_main { Icon::Home } else { Icon::Terminal },
                if focused { t.brand } else { t.text_dim },
            );
            x += 14.0 + theme::S2;

            let name = painter.layout_no_wrap(
                label.clone(),
                theme::font(12.5, Weight::SemiBold),
                Color32::PLACEHOLDER,
            );
            let name_h = name.size().y;
            let name_w = name.size().x;
            painter.galley(
                Pos2::new(x, mid - name_h / 2.0),
                name,
                if focused { t.text } else { t.text_dim },
            );
            x += name_w + theme::S2;

            let mut chips: Vec<(String, Color32, Color32)> = Vec::new();
            if is_main {
                chips.push((tr("wt.chip_official").to_owned(), t.brand, t.brand_tint));
            }
            if !branch.is_empty() {
                let fg = if is_main { t.text_dim } else { t.brand };
                chips.push((branch.clone(), fg, t.surface_alt));
            }
            if exited {
                chips.push((tr("term.chip_exited").to_owned(), t.danger, t.danger_tint));
            }
            if bell {
                chips.push((tr("term.chip_bell").to_owned(), t.warning, t.warning_tint));
            }
            for (text, fg, bg) in &chips {
                x += paint_chip(&painter, x, mid, text, *fg, *bg) + theme::S1;
            }

            // Whole bar: tooltip everywhere, and the drag that moves the pane.
            let sense = if free {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            };
            let bar = ui.interact(header, ui.scope_id().with("header-bar"), sense);
            let mut hover = format!("{} · {}×{} · {:.0}px", cwd, grid.cols, grid.rows, font_size);
            if !title.is_empty() && title != label {
                hover = format!("{title}\n{hover}");
            }
            if is_main {
                hover = format!("{hover}\n{}", tr("term.repo_tooltip"));
            }
            if free {
                hover = format!("{hover}\n{}", tr("term.drag_tooltip"));
            }
            let bar = bar.on_hover_text(hover);
            if free {
                if bar.drag_started() || bar.clicked() {
                    focus_change = Some(index);
                }
                if bar.dragged() {
                    dragging = Some((session_id, rect));
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                } else if bar.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
                if bar.drag_stopped() {
                    dropped = Some(session_id);
                }
                if bar.double_clicked() {
                    maximize = Some(session_id);
                }
            }

            // Buttons come last, so they win the clicks over the drag bar.
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(Rect::from_min_max(
                        Pos2::new(content.right(), header.top() + 5.0),
                        Pos2::new(header.right() - theme::S2, header.bottom() - 5.0),
                    ))
                    .id_salt(("header", session_id))
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    close_clicked =
                        c::icon_button(ui, Icon::Close, tr("term.close_pane"), true).clicked();
                    restart_clicked =
                        c::icon_button(ui, Icon::Refresh, tr("term.restart"), false).clicked();
                    if show_max {
                        let (icon, hint) = if maximized {
                            (Icon::Shrink, tr("term.restore"))
                        } else {
                            (Icon::Expand, tr("term.maximize"))
                        };
                        maximize_clicked = c::icon_button(ui, icon, hint, false).clicked();
                    }
                },
            );

            // body
            let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + HEADER_H), rect.max);
            punch_through(ui, body, t.term_bg, term_opacity);
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(body)
                    .id_salt(("body", session_id)),
                |ui| {
                    let out = show_terminal(ui, session, &term_theme);
                    if (out.zoom - 1.0).abs() > 0.001 {
                        // Ctrl+wheel resizes the text of this pane only.
                        session.font_size = Some((font_size * out.zoom).clamp(7.0, 40.0));
                    }
                    if out.response.has_focus() && !focused {
                        focus_change = Some(index);
                    }
                    if out.response.clicked() {
                        focus_change = Some(index);
                    }
                    if broadcast && !out.sent.is_empty() {
                        mirror = Some((session_id, out.sent));
                    }
                },
            );

            // Outline last: the terminal fills its whole rect, border included.
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(
                    if focused { 1.6 } else { 1.0 },
                    if focused { t.brand } else { t.border_strong },
                ),
                StrokeKind::Inside,
            );
        });

        if close_clicked {
            actions.push(Action::CloseSession(session_id));
        }
        if restart_clicked {
            actions.push(Action::RestartSession(session_id));
        }
        if maximize_clicked {
            maximize = Some(session_id);
        }
    }

    // ---- gutters, drops and the rest --------------------------------------
    if free && app.maximized.is_none() {
        drag_splitters(app, ui, area);
    }
    if free {
        let pointer = ui.ctx().pointer_interact_pos();
        let moving = dragging.map(|(id, _)| id).or(dropped);
        if let (Some(id), Some(pointer)) = (moving, pointer) {
            // Close to the outer border the pane docks against the whole area
            // and takes half of it; otherwise it splits the pane it is over.
            let landing = match tiles::root_side(area, pointer, EDGE_DOCK) {
                Some(side) if placements.len() > 1 => Some((None, area, side)),
                _ => placements
                    .iter()
                    .map(|&(index, rect)| (app.sessions[index].id, rect))
                    .find(|&(other, rect)| other != id && rect.contains(pointer))
                    .map(|(other, rect)| {
                        let side = tiles::drop_side(rect, pointer);
                        (Some(other), rect, side)
                    }),
            };
            if let Some((target, rect, side)) = landing {
                if dropped == Some(id) {
                    apply_drop(app, id, target, side);
                } else {
                    paint_drop_hint(ui, tiles::drop_preview(rect, side), t.brand);
                }
            }
            if let (Some((_, from)), true) = (dragging, dropped.is_none()) {
                paint_ghost(ui, from, pointer, t.brand);
            }
        }
    }
    if let Some(id) = maximize {
        actions.push(Action::MaximizePane(id));
    }
    if let Some(index) = focus_change {
        actions.push(Action::FocusPane(index));
    }
    if let Some((source, bytes)) = mirror {
        for session in app.sessions.iter().filter(|s| s.id != source) {
            session.write(&bytes);
        }
    }
}

/// Pane rectangles from the tiling tree, or the one blown-up pane.
fn tiling_placements(app: &mut App, area: Rect) -> Vec<(usize, Rect)> {
    let alive: Vec<u64> = app.sessions.iter().map(|s| s.id).collect();
    tiles::sync(&mut app.tiles, &alive, area, GAP);
    if app.maximized.is_some_and(|id| !alive.contains(&id)) {
        app.maximized = None;
    }
    if let Some(id) = app.maximized {
        return app
            .sessions
            .iter()
            .position(|s| s.id == id)
            .map(|index| vec![(index, area)])
            .unwrap_or_default();
    }
    let Some(tree) = &app.tiles else {
        return Vec::new();
    };
    tree.layout(area, GAP)
        .into_iter()
        .filter_map(|(id, rect)| {
            app.sessions
                .iter()
                .position(|s| s.id == id)
                .map(|index| (index, rect))
        })
        .collect()
}

/// The gutters between panes: dragging one moves the edge the two panes share,
/// so the neighbour grows by exactly what this one gives up.
fn drag_splitters(app: &mut App, ui: &mut Ui, area: Rect) {
    let Some(tree) = app.tiles.clone() else {
        return;
    };
    let brand = tokens(ui).brand;
    let mut changes: Vec<(Vec<bool>, f32)> = Vec::new();
    for (slot, splitter) in tree.splitters(area, GAP).into_iter().enumerate() {
        let cursor = match splitter.dir {
            Dir::Row => CursorIcon::ResizeHorizontal,
            Dir::Column => CursorIcon::ResizeVertical,
        };
        let response = ui
            .interact(
                splitter.rect.expand(2.0),
                ui.scope_id().with(("gutter", slot)),
                Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.hovered() || response.dragged() {
            let shrink = match splitter.dir {
                Dir::Row => Vec2::new(3.0, 0.0),
                Dir::Column => Vec2::new(0.0, 3.0),
            };
            ui.painter().rect_filled(
                splitter.rect.shrink2(shrink),
                CornerRadius::same(2),
                brand.gamma_multiply(0.5),
            );
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                changes.push((
                    splitter.path.clone(),
                    tiles::ratio_for(splitter.parent, splitter.dir, pointer, GAP),
                ));
            }
            ui.ctx().set_cursor_icon(cursor);
        }
    }
    if let Some(tree) = app.tiles.as_mut() {
        for (path, ratio) in changes {
            tree.set_ratio(&path, ratio);
        }
    }
}

/// Moves `id` next to `target`, taking the room from the target's share. With
/// no target the pane docks against the whole area and takes half of it.
/// The saved commands: one button each, sending the command to every pane, plus
/// a pencil to edit it. The first free slot shows an invitation to fill it.
fn quick_row(app: &mut App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let saved: Vec<(usize, String, String)> = (0..config::QUICK_SLOTS)
        .filter_map(|slot| {
            app.cfg
                .settings
                .quick(slot)
                .map(|quick| (slot, quick.label.clone(), quick.command.clone()))
        })
        .collect();
    let free = (0..config::QUICK_SLOTS).find(|slot| app.cfg.settings.quick(*slot).is_none());

    ui.add_space(theme::S2);
    ui.horizontal_wrapped(|ui| {
        for (slot, label, command) in saved {
            if c::Btn::soft(&label)
                .small()
                .hint(&format!("{command}\n{}", tr("term.quick_run")))
                .show(ui)
                .clicked()
            {
                actions.push(Action::RunQuick(slot));
            }
            if c::icon_button(ui, Icon::Pencil, tr("term.quick_edit"), false).clicked() {
                actions.push(Action::QuickDialog(slot));
            }
            ui.add_space(theme::S1);
        }
        if let Some(slot) = free
            && c::Btn::ghost(tr("term.quick_add"))
                .icon(Icon::Plus)
                .small()
                .show(ui)
                .clicked()
        {
            actions.push(Action::QuickDialog(slot));
        }
    });
}

fn apply_drop(app: &mut App, id: u64, target: Option<u64>, side: Side) {
    let Some(mut tree) = app.tiles.take() else {
        return;
    };
    match target {
        Some(target) if side == Side::Center => tree.swap(id, target),
        Some(target) => {
            tree = tiles::remove(Some(tree), id).unwrap_or(Tile::Leaf(id));
            tree.insert(target, side, id);
        }
        None => {
            tree = tiles::remove(Some(tree), id).unwrap_or(Tile::Leaf(id));
            tree.insert_root(side, id);
        }
    }
    app.tiles = Some(tree);
    app.maximized = None;
}

/// The band a drop would take, drawn under the cursor while dragging.
fn paint_drop_hint(ui: &Ui, rect: Rect, color: Color32) {
    let radius = CornerRadius::same(theme::R_CARD);
    ui.painter()
        .rect_filled(rect.shrink(3.0), radius, color.gamma_multiply(0.22));
    ui.painter().rect_stroke(
        rect.shrink(3.0),
        radius,
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
}

/// A small translucent copy of the pane, hanging off the pointer.
fn paint_ghost(ui: &Ui, from: Rect, pointer: Pos2, color: Color32) {
    let size = Vec2::new(from.width().min(220.0), HEADER_H);
    let rect = Rect::from_center_size(pointer + Vec2::new(0.0, -size.y), size);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(theme::R_INPUT),
        color.gamma_multiply(0.35),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(theme::R_INPUT),
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    );
}

/// Glides a pane between two arrangements. Every edge is animated on its own,
/// so a pane that only grows does not drift sideways on the way.
fn animated_rect(ctx: &egui::Context, id: u64, target: Rect, animate: bool) -> Rect {
    let time = if animate { REFLOW } else { 0.0 };
    let mut out = [0.0f32; 4];
    for (slot, (key, value)) in edges(target).into_iter().enumerate() {
        out[slot] = ctx.animate_value_with_time(Id::unique(("pane-edge", id, key)), value, time);
    }
    if !animate {
        return target;
    }
    let rect = Rect::from_min_max(Pos2::new(out[0], out[1]), Pos2::new(out[2], out[3]));
    if rect.width() < 40.0 || rect.height() < 40.0 {
        target
    } else {
        rect
    }
}

fn edges(rect: Rect) -> [(&'static str, f32); 4] {
    [
        ("x0", rect.left()),
        ("y0", rect.top()),
        ("x1", rect.right()),
        ("y1", rect.bottom()),
    ]
}

/// Makes the terminal area genuinely see-through.
///
/// Painting a translucent colour only blends with the window background that is
/// already there, so the desktop never shows up. The pixels have to be replaced
/// instead: a scissored clear does exactly that, and the two rectangles keep the
/// rounded bottom corners of the pane intact.
fn punch_through(ui: &Ui, body: Rect, color: Color32, opacity: f32) {
    if opacity >= 0.999 {
        return;
    }
    let [r, g, b, _] = color.to_normalized_gamma_f32();
    let alpha = opacity.clamp(0.0, 1.0);
    let radius = theme::R_CARD as f32;
    let top = Rect::from_min_max(body.min, Pos2::new(body.right(), body.bottom() - radius));
    let bottom = Rect::from_min_max(
        Pos2::new(body.left() + radius, body.bottom() - radius),
        Pos2::new(body.right() - radius, body.bottom()),
    );
    for rect in [top, bottom] {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        ui.painter().add(egui::Shape::Callback(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                move |info, painter: &eframe::egui_glow::Painter| {
                    use eframe::glow::HasContext as _;
                    let viewport = info.viewport_in_pixels();
                    let gl = painter.gl();
                    unsafe {
                        gl.enable(eframe::glow::SCISSOR_TEST);
                        gl.scissor(
                            viewport.left_px,
                            viewport.from_bottom_px,
                            viewport.width_px,
                            viewport.height_px,
                        );
                        gl.clear_color(r * alpha, g * alpha, b * alpha, alpha);
                        gl.clear(eframe::glow::COLOR_BUFFER_BIT);
                        gl.disable(eframe::glow::SCISSOR_TEST);
                    }
                },
            )),
        }));
    }
}

/// One painted pill. Returns how much horizontal room it took.
fn paint_chip(
    painter: &egui::Painter,
    left: f32,
    center_y: f32,
    text: &str,
    fg: Color32,
    bg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        theme::font(10.5, Weight::SemiBold),
        Color32::PLACEHOLDER,
    );
    let width = galley.size().x + 16.0;
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(left, center_y - 9.0), Vec2::new(width, 18.0)),
        CornerRadius::same(theme::R_PILL),
        bg,
    );
    painter.galley(
        Pos2::new(left + 8.0, center_y - galley.size().y / 2.0),
        galley,
        fg,
    );
    width
}

/// One tile. The last row shares the full width between the panes it has, so
/// three panes read as "two on top, one wide below" instead of leaving a hole.
fn grid_cell(slot: usize, cols: usize, rows: usize, count: usize, area: Rect) -> Rect {
    let row = (slot / cols).min(rows.saturating_sub(1));
    let col = slot % cols;
    let in_row = (count - row * cols).min(cols).max(1);
    let cell_w = (area.width() - GAP * (in_row as f32 - 1.0)) / in_row as f32;
    let cell_h = (area.height() - GAP * (rows as f32 - 1.0)) / rows as f32;
    Rect::from_min_size(
        Pos2::new(
            area.left() + col as f32 * (cell_w + GAP),
            area.top() + row as f32 * (cell_h + GAP),
        ),
        Vec2::new(cell_w.max(120.0), cell_h.max(120.0)),
    )
}

/// Columns and rows for `n` panes under the chosen layout.
pub fn grid_shape(n: usize, layout: PaneLayout) -> (usize, usize) {
    let n = n.max(1);
    match layout {
        PaneLayout::Tabs => (1, 1),
        PaneLayout::Columns => (n, 1),
        PaneLayout::Rows => (1, n),
        PaneLayout::Grid | PaneLayout::Free => {
            let cols = (n as f32).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            (cols.max(1), rows.max(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 600.0))
    }

    #[test]
    fn grid_is_square_ish() {
        assert_eq!(grid_shape(1, PaneLayout::Grid), (1, 1));
        assert_eq!(grid_shape(2, PaneLayout::Grid), (2, 1));
        assert_eq!(grid_shape(3, PaneLayout::Grid), (2, 2));
        assert_eq!(grid_shape(4, PaneLayout::Grid), (2, 2));
        assert_eq!(grid_shape(5, PaneLayout::Grid), (3, 2));
        assert_eq!(grid_shape(9, PaneLayout::Grid), (3, 3));
    }

    #[test]
    fn columns_rows_and_tabs() {
        assert_eq!(grid_shape(4, PaneLayout::Columns), (4, 1));
        assert_eq!(grid_shape(4, PaneLayout::Rows), (1, 4));
        assert_eq!(grid_shape(4, PaneLayout::Tabs), (1, 1));
    }

    #[test]
    fn every_pane_fits_in_the_grid() {
        for n in 1..40 {
            let (cols, rows) = grid_shape(n, PaneLayout::Grid);
            assert!(cols * rows >= n, "{n} panes do not fit in {cols}x{rows}");
        }
    }

    #[test]
    fn grid_cells_tile_the_area_without_overlap() {
        let area = area();
        let a = grid_cell(0, 2, 2, 4, area);
        let b = grid_cell(1, 2, 2, 4, area);
        assert!(a.right() + GAP <= b.left() + 0.01);
        assert!(area.contains_rect(a) && area.contains_rect(b));

        // Three panes: the third one is alone on its row and takes it all.
        let lonely = grid_cell(2, 2, 2, 3, area);
        assert_eq!(lonely.width(), area.width());
        assert!(lonely.top() > a.bottom());
    }

    #[test]
    fn without_animation_a_pane_lands_straight_on_its_target() {
        let ctx = egui::Context::default();
        let target = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, 200.0));
        assert_eq!(animated_rect(&ctx, 1, target, false), target);
    }
}
