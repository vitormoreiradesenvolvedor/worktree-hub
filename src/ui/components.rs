//! The design system: cards, pill buttons, round checkboxes, chips, inputs.
//! Everything is painted by hand so the app keeps one visual language.

use egui::epaint;
use egui::{
    Align, Align2, Color32, CornerRadius, Id, Layout, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};

use super::icons::{self, Icon};
use crate::theme::{self, Tokens, Weight};

const TOKENS_ID: &str = "worktree-hub-tokens";

pub fn set_tokens(ctx: &egui::Context, t: Tokens) {
    ctx.data_mut(|d| d.insert_temp(Id::unique(TOKENS_ID), t));
}

pub fn tokens(ui: &Ui) -> Tokens {
    ui.ctx()
        .data(|d| d.get_temp::<Tokens>(Id::unique(TOKENS_ID)))
        .unwrap_or_else(|| theme::tokens(theme::Look::default()))
}

// ---------------------------------------------------------------- containers

/// White (or dark surface) card with soft shadow and generous rounding.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, theme::S4, None, add)
}

pub fn card_with<R>(
    ui: &mut Ui,
    pad: f32,
    accent: Option<Color32>,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, accent.unwrap_or(t.border)))
        .corner_radius(CornerRadius::same(theme::R_CARD))
        .shadow(theme::soft_shadow(t))
        .inner_margin(egui::Margin::same(pad as i8))
        .show(ui, add)
        .inner
}

/// Flat surface without shadow — for nested blocks inside a card.
pub fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.surface_alt)
        .corner_radius(CornerRadius::same(theme::R_INPUT))
        .inner_margin(egui::Margin::same(theme::S3 as i8))
        .show(ui, add)
        .inner
}

pub fn divider(ui: &mut Ui) {
    let t = tokens(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, t.border);
}

pub fn heading(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(text)
            .font(theme::font(20.0, Weight::SemiBold))
            .color(t.text),
    );
}

pub fn section_label(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(theme::font(11.0, Weight::SemiBold))
            .color(t.text_faint),
    );
}

pub fn body(ui: &mut Ui, text: impl Into<String>) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(text.into())
            .font(theme::font(14.0, Weight::Regular))
            .color(t.text),
    );
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    let t = tokens(ui);
    ui.label(
        egui::RichText::new(text.into())
            .font(theme::font(12.5, Weight::Regular))
            .color(t.text_dim),
    );
}

pub fn mono_label(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    ui.label(
        egui::RichText::new(text.into())
            .font(theme::mono(12.0, false))
            .color(color),
    );
}

// ------------------------------------------------------------------- buttons

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BtnKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
    Soft,
}

pub struct Btn<'a> {
    label: &'a str,
    icon: Option<Icon>,
    kind: BtnKind,
    enabled: bool,
    min_width: f32,
    height: f32,
    hint: Option<&'a str>,
}

impl<'a> Btn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            kind: BtnKind::Secondary,
            enabled: true,
            min_width: 0.0,
            height: 38.0,
            hint: None,
        }
    }

    pub fn primary(label: &'a str) -> Self {
        Self::new(label).kind(BtnKind::Primary)
    }

    pub fn ghost(label: &'a str) -> Self {
        Self::new(label).kind(BtnKind::Ghost)
    }

    pub fn danger(label: &'a str) -> Self {
        Self::new(label).kind(BtnKind::Danger)
    }

    pub fn soft(label: &'a str) -> Self {
        Self::new(label).kind(BtnKind::Soft)
    }

    pub fn kind(mut self, kind: BtnKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn small(mut self) -> Self {
        self.height = 30.0;
        self
    }

    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = tokens(ui);
        let font = theme::font(
            if self.height <= 30.0 { 12.5 } else { 13.5 },
            Weight::SemiBold,
        );
        let galley = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font, Color32::PLACEHOLDER);

        let icon_size = (self.height * 0.44).round();
        let icon_space = if self.icon.is_some() {
            icon_size + theme::S2
        } else {
            0.0
        };
        let pad_x = if self.label.is_empty() { 10.0 } else { 16.0 };
        let width = (galley.size().x + icon_space + pad_x * 2.0).max(self.min_width);

        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, self.height), sense);

        let hovered = response.hovered() && self.enabled;
        let pressed = response.is_pointer_button_down_on() && self.enabled;

        let (fill, stroke_color, text_color) = match self.kind {
            BtnKind::Primary => {
                let base = if pressed {
                    t.brand_deep
                } else if hovered {
                    lighten(t.brand, 0.10)
                } else {
                    t.brand
                };
                (base, Color32::TRANSPARENT, t.on_brand)
            }
            BtnKind::Secondary => (
                if pressed || hovered {
                    t.surface_hover
                } else {
                    t.surface
                },
                if hovered { t.brand } else { t.border_strong },
                t.text,
            ),
            BtnKind::Ghost => (
                if hovered {
                    t.surface_hover
                } else {
                    Color32::TRANSPARENT
                },
                Color32::TRANSPARENT,
                if hovered { t.brand } else { t.text_dim },
            ),
            BtnKind::Danger => (
                if pressed || hovered {
                    t.danger
                } else {
                    t.danger_tint
                },
                t.danger,
                if pressed || hovered {
                    Color32::WHITE
                } else {
                    t.danger
                },
            ),
            BtnKind::Soft => (
                if hovered { t.brand } else { t.brand_tint },
                Color32::TRANSPARENT,
                if hovered { t.on_brand } else { t.brand },
            ),
        };

        let (fill, text_color) = if self.enabled {
            (fill, text_color)
        } else {
            (t.surface_alt, t.text_faint)
        };

        let painter = ui.painter();
        let radius = CornerRadius::same(theme::R_PILL);
        if fill != Color32::TRANSPARENT {
            painter.rect_filled(rect, radius, fill);
        }
        if stroke_color != Color32::TRANSPARENT && self.enabled {
            painter.rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, stroke_color),
                StrokeKind::Inside,
            );
        }

        let content_w = galley.size().x + icon_space;
        let mut x = rect.center().x - content_w / 2.0;
        if let Some(icon) = self.icon {
            let icon_rect = Rect::from_center_size(
                egui::Pos2::new(x + icon_size / 2.0, rect.center().y),
                Vec2::splat(icon_size),
            );
            icons::paint(painter, icon_rect, icon, text_color);
            x += icon_size + theme::S2;
        }
        let text_pos = egui::Pos2::new(x, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, text_color);

        let response = if self.enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        };
        match self.hint {
            Some(hint) if self.enabled => response.on_hover_text(hint),
            _ => response,
        }
    }
}

/// Square icon-only button, used in card action rows and pane headers.
pub fn icon_button(ui: &mut Ui, icon: Icon, hint: &str, danger: bool) -> Response {
    let t = tokens(ui);
    let size = Vec2::splat(32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, color) = if danger {
        (
            if hovered {
                t.danger_tint
            } else {
                Color32::TRANSPARENT
            },
            if hovered { t.danger } else { t.text_dim },
        )
    } else {
        (
            if hovered {
                t.brand_tint
            } else {
                Color32::TRANSPARENT
            },
            if hovered { t.brand } else { t.text_dim },
        )
    };

    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), fill);
    }
    icons::paint(painter, rect.shrink(8.0), icon, color);

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(hint)
}

/// Round checkbox: the control that drives the multi-terminal selection.
pub fn round_checkbox(ui: &mut Ui, checked: &mut bool) -> Response {
    let t = tokens(ui);
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }

    let painter = ui.painter();
    let center = rect.center();
    let radius = 11.0;
    if *checked {
        painter.circle_filled(center, radius, t.brand);
        icons::paint(
            painter,
            Rect::from_center_size(center, Vec2::splat(15.0)),
            Icon::Check,
            t.on_brand,
        );
    } else {
        let stroke = if response.hovered() {
            Stroke::new(1.8, t.brand)
        } else {
            Stroke::new(1.6, t.border_strong)
        };
        painter.circle_stroke(center, radius, stroke);
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// iOS-style switch used in dialogs and settings.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let t = tokens(ui);
    let size = Vec2::new(44.0, 26.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
    let painter = ui.painter();
    let fill = lerp_color(t.border_strong, t.brand, how_on);
    painter.rect_filled(rect, CornerRadius::same(13), fill);
    let knob_x = egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), how_on);
    painter.circle_filled(
        egui::Pos2::new(knob_x, rect.center().y),
        10.0,
        Color32::WHITE,
    );

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn toggle_row(ui: &mut Ui, label: &str, description: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            body(ui, label);
            if !description.is_empty() {
                muted(ui, description);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = toggle(ui, on).changed();
        });
    });
    changed
}

// --------------------------------------------------------------------- chips

pub fn chip(ui: &mut Ui, text: &str, fg: Color32, bg: Color32, icon: Option<Icon>) -> Response {
    let font = theme::font(11.5, Weight::SemiBold);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() { 13.0 + 5.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + icon_w + 20.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(theme::R_PILL), bg);
    let mut x = rect.left() + 10.0;
    if let Some(icon) = icon {
        icons::paint(
            painter,
            Rect::from_center_size(egui::Pos2::new(x + 6.5, rect.center().y), Vec2::splat(13.0)),
            icon,
            fg,
        );
        x += 18.0;
    }
    painter.galley(
        egui::Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    response
}

/// Status dot + text, e.g. "3 alterações".
pub fn dot_label(ui: &mut Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        muted(ui, text);
    });
}

// -------------------------------------------------------------------- inputs

pub fn text_field(ui: &mut Ui, value: &mut String, placeholder: &str, width: f32) -> Response {
    let t = tokens(ui);
    let desired = if width > 0.0 {
        width
    } else {
        ui.available_width()
    };
    egui::Frame::new()
        .fill(t.surface_alt)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(theme::R_INPUT))
        .inner_margin(egui::Margin::symmetric(theme::S3 as i8, 9))
        .show(ui, |ui| {
            ui.add_sized(
                Vec2::new(desired - 2.0 * theme::S3, 20.0),
                egui::TextEdit::singleline(value)
                    .frame(egui::Frame::NONE)
                    .hint_text(placeholder)
                    .font(theme::font(13.5, Weight::Regular))
                    .text_color(t.text),
            )
        })
        .inner
}

pub fn search_field(ui: &mut Ui, value: &mut String, placeholder: &str, width: f32) -> Response {
    let t = tokens(ui);
    egui::Frame::new()
        .fill(t.surface_alt)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(theme::R_PILL))
        .inner_margin(egui::Margin::symmetric(theme::S3 as i8, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                icons::paint(ui.painter(), rect, Icon::Search, t.text_faint);
                ui.add_sized(
                    Vec2::new(width - 60.0, 20.0),
                    egui::TextEdit::singleline(value)
                        .frame(egui::Frame::NONE)
                        .hint_text(placeholder)
                        .font(theme::font(13.0, Weight::Regular))
                        .text_color(t.text),
                )
            })
            .inner
        })
        .inner
}

/// Segmented control (layout switcher, theme switcher…).
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    let t = tokens(ui);
    let mut changed = false;

    egui::Frame::new()
        .fill(t.surface_alt)
        .corner_radius(CornerRadius::same(theme::R_PILL))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            // The pill is drawn first and filled in afterwards, so it can slide
            // under the labels instead of blinking from one to the next.
            let pill = ui.painter().add(egui::Shape::Noop);
            let mut chosen: Option<Rect> = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (value, label) in options {
                    let selected = *current == *value;
                    let font = theme::font(12.5, Weight::SemiBold);
                    let galley =
                        ui.painter()
                            .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
                    let size = Vec2::new(galley.size().x + 22.0, 28.0);
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                    if selected {
                        chosen = Some(rect);
                    }

                    let painter = ui.painter();
                    if !selected && response.hovered() {
                        painter.rect_filled(
                            rect,
                            CornerRadius::same(theme::R_PILL),
                            t.surface.gamma_multiply(0.5),
                        );
                    }
                    let color = if selected { t.brand } else { t.text_dim };
                    painter.galley(
                        egui::Pos2::new(
                            rect.center().x - galley.size().x / 2.0,
                            rect.center().y - galley.size().y / 2.0,
                        ),
                        galley,
                        color,
                    );

                    if response.clicked() && !selected {
                        *current = *value;
                        changed = true;
                    }
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
            });
            if let Some(rect) = chosen {
                let rect = slide(ui, "segmented", rect);
                ui.painter().set(
                    pill,
                    epaint::RectShape::filled(rect, CornerRadius::same(theme::R_PILL), t.surface),
                );
            }
        });

    changed
}

/// Animates a rectangle towards `target`, keyed on this `Ui`'s id.
fn slide(ui: &Ui, key: &str, target: Rect) -> Rect {
    let ctx = ui.ctx();
    let id = ui.scope_id().with(key);
    let time = 0.12;
    let left = ctx.animate_value_with_time(id.with("l"), target.left(), time);
    let right = ctx.animate_value_with_time(id.with("r"), target.right(), time);
    let top = ctx.animate_value_with_time(id.with("t"), target.top(), time);
    let bottom = ctx.animate_value_with_time(id.with("b"), target.bottom(), time);
    let rect = Rect::from_min_max(egui::Pos2::new(left, top), egui::Pos2::new(right, bottom));
    if rect.width() < 4.0 || rect.height() < 4.0 {
        target
    } else {
        rect
    }
}

// ------------------------------------------------------------------- states

pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
    let t = tokens(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(theme::S6);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 32.0, t.brand_tint);
        icons::paint(ui.painter(), rect.shrink(20.0), icon, t.brand);
        ui.add_space(theme::S3);
        ui.label(
            egui::RichText::new(title)
                .font(theme::font(16.0, Weight::SemiBold))
                .color(t.text),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(subtitle)
                .font(theme::font(13.0, Weight::Regular))
                .color(t.text_dim),
        );
        ui.add_space(theme::S4);
    });
}

/// Brand-colored gradient header used by the sidebar and dialogs.
pub fn gradient_header(ui: &mut Ui, rect: Rect) {
    let t = tokens(ui);
    // Clipped, otherwise the glow circles spill over the nav below.
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, t.brand_deep);
    // Two translucent circles give a soft "glow" without needing a gradient mesh.
    painter.circle_filled(
        egui::Pos2::new(rect.left() + rect.width() * 0.2, rect.top()),
        rect.width() * 0.75,
        t.brand.gamma_multiply(0.55),
    );
    painter.circle_filled(
        egui::Pos2::new(rect.right(), rect.bottom()),
        rect.width() * 0.5,
        t.brand_soft.gamma_multiply(0.18),
    );
}

pub fn text_with(ui: &mut Ui, text: &str, size: f32, weight: Weight, color: Color32) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::font(size, weight))
            .color(color),
    );
}

/// Cut from the left, so the end of a path stays visible.
pub fn truncated(ui: &mut Ui, text: &str, size: f32, weight: Weight, color: Color32, max_w: f32) {
    shorten(ui, text, size, weight, color, max_w, true);
}

/// Cut at the end, for prose and names, where the start carries the meaning.
pub fn truncated_end(
    ui: &mut Ui,
    text: &str,
    size: f32,
    weight: Weight,
    color: Color32,
    max_w: f32,
) {
    shorten(ui, text, size, weight, color, max_w, false);
}

fn shorten(
    ui: &mut Ui,
    text: &str,
    size: f32,
    weight: Weight,
    color: Color32,
    max_w: f32,
    from_start: bool,
) {
    let font = theme::font(size, weight);
    let mut galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color);
    if galley.size().x > max_w && text.chars().count() > 4 {
        let mut candidate: Vec<char> = text.chars().collect();
        while candidate.len() > 4 {
            if from_start {
                candidate.remove(0);
            } else {
                candidate.pop();
            }
            let kept: String = candidate.iter().collect();
            let shortened = if from_start {
                format!("…{kept}")
            } else {
                format!("{kept}…")
            };
            galley = ui.painter().layout_no_wrap(shortened, font.clone(), color);
            if galley.size().x <= max_w {
                break;
            }
        }
    }
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    if response.hovered() {
        response.on_hover_text(text);
    }
}

pub fn spinner(ui: &mut Ui, size: f32) {
    let t = tokens(ui);
    ui.add(egui::Spinner::new().size(size).color(t.brand));
}

pub fn label_value(ui: &mut Ui, label: &str, value: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        text_with(ui, label, 12.5, Weight::Regular, t.text_dim);
        text_with(ui, value, 12.5, Weight::SemiBold, t.text);
    });
}

/// Collapsing block with our own chevron, instead of egui's default triangle.
pub fn expander<R>(
    ui: &mut Ui,
    id: &str,
    label: &str,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let t = tokens(ui);
    let state_id = Id::unique(("worktree-hub-expander", id));
    let mut open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(state_id))
        .unwrap_or(false);

    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if response.clicked() {
        open = !open;
        ui.ctx().data_mut(|d| d.insert_temp(state_id, open));
    }

    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(theme::R_INPUT), t.surface_hover);
    }
    let chevron = Rect::from_center_size(
        egui::Pos2::new(rect.left() + 12.0, rect.center().y),
        Vec2::splat(14.0),
    );
    icons::paint(
        painter,
        chevron,
        if open {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        if response.hovered() {
            t.brand
        } else {
            t.text_dim
        },
    );
    painter.text(
        egui::Pos2::new(chevron.right() + theme::S2, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font(12.5, Weight::SemiBold),
        if response.hovered() {
            t.brand
        } else {
            t.text_dim
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if open {
        ui.add_space(theme::S2);
        Some(add(ui))
    } else {
        None
    }
}

/// Segmented control whose options are icons (used by the pane layout picker).
pub fn segmented_icons<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    options: &[(T, Icon, &str)],
) -> bool {
    let t = tokens(ui);
    let mut changed = false;

    // Laid out by hand: inside a right-to-left toolbar a `Frame` stretches to
    // the whole strip and the background drifts away from the icons.
    const ITEM: f32 = 34.0;
    const ITEM_H: f32 = 28.0;
    const GAP: f32 = 2.0;
    const PAD: f32 = 3.0;
    let count = options.len() as f32;
    let size = Vec2::new(
        count * ITEM + (count - 1.0).max(0.0) * GAP + 2.0 * PAD,
        ITEM_H + 2.0 * PAD,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(theme::R_PILL), t.surface_alt);

    let pill = ui.painter().add(egui::Shape::Noop);
    let mut chosen: Option<Rect> = None;
    for (slot, (value, icon, hint)) in options.iter().enumerate() {
        let item = Rect::from_min_size(
            egui::Pos2::new(
                rect.left() + PAD + slot as f32 * (ITEM + GAP),
                rect.top() + PAD,
            ),
            Vec2::new(ITEM, ITEM_H),
        );
        let selected = *current == *value;
        let response = ui.interact(item, ui.scope_id().with(("seg", slot)), Sense::click());
        if selected {
            chosen = Some(item);
        } else if response.hovered() {
            ui.painter().rect_filled(
                item,
                CornerRadius::same(theme::R_PILL),
                t.surface.gamma_multiply(0.5),
            );
        }
        icons::paint(
            ui.painter(),
            item.shrink(7.0),
            *icon,
            if selected { t.brand } else { t.text_dim },
        );
        if response.clicked() && !selected {
            *current = *value;
            changed = true;
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*hint);
    }
    if let Some(item) = chosen {
        let item = slide(ui, "segmented-icons", item);
        ui.painter().set(
            pill,
            epaint::RectShape::filled(item, CornerRadius::same(theme::R_PILL), t.surface),
        );
    }

    changed
}

/// A row of icon buttons sharing one pill background: same shape as
/// `segmented_icons`, but every item is an action instead of a choice.
/// Returns the slot that was clicked.
pub fn icon_bar(ui: &mut Ui, items: &[(Icon, &str)]) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let t = tokens(ui);

    const ITEM: f32 = 34.0;
    const ITEM_H: f32 = 28.0;
    const GAP: f32 = 2.0;
    const PAD: f32 = 3.0;
    let count = items.len() as f32;
    let size = Vec2::new(
        count * ITEM + (count - 1.0).max(0.0) * GAP + 2.0 * PAD,
        ITEM_H + 2.0 * PAD,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(theme::R_PILL), t.surface_alt);

    let mut clicked = None;
    for (slot, (icon, hint)) in items.iter().enumerate() {
        let item = Rect::from_min_size(
            egui::Pos2::new(
                rect.left() + PAD + slot as f32 * (ITEM + GAP),
                rect.top() + PAD,
            ),
            Vec2::new(ITEM, ITEM_H),
        );
        let response = ui.interact(item, ui.scope_id().with(("bar", slot)), Sense::click());
        let hovered = response.hovered();
        if hovered {
            ui.painter()
                .rect_filled(item, CornerRadius::same(theme::R_PILL), t.surface);
        }
        icons::paint(
            ui.painter(),
            item.shrink(7.0),
            *icon,
            if hovered { t.brand } else { t.text_dim },
        );
        if response.clicked() {
            clicked = Some(slot);
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*hint);
    }

    clicked
}

// ------------------------------------------------------------------- helpers

pub fn lighten(color: Color32, amount: f32) -> Color32 {
    lerp_color(color, Color32::WHITE, amount)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_endpoints_and_middle() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(255, 255, 255);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 0.5), Color32::from_rgb(128, 128, 128));
    }

    #[test]
    fn lerp_clamps_out_of_range() {
        let a = Color32::from_rgb(10, 20, 30);
        let b = Color32::from_rgb(200, 200, 200);
        assert_eq!(lerp_color(a, b, -2.0), a);
        assert_eq!(lerp_color(a, b, 7.0), b);
    }
}
