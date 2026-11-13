//! Hand-drawn line icons. Keeping them procedural means no image decoding, no
//! icon font, and a stroke weight that always matches the text next to them.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Plus,
    Trash,
    Terminal,
    Folder,
    Code,
    Copy,
    Refresh,
    Search,
    Branch,
    Settings,
    Grid,
    Free,
    Columns,
    Rows,
    Tabs,
    Broadcast,
    Lock,
    Warning,
    Check,
    Close,
    ChevronRight,
    ChevronDown,
    ChevronLeft,
    ArrowUp,
    Expand,
    Shrink,
    Info,
    External,
    Dot,
    Home,
    Pencil,
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height());
    let origin = rect.center() - Vec2::splat(s / 2.0);
    let w = (s * 0.11).clamp(1.3, 2.4);
    let stroke = Stroke::new(w, color);
    let p = |x: f32, y: f32| Pos2::new(origin.x + x * s, origin.y + y * s);
    let line = |a: Pos2, b: Pos2| {
        painter.line_segment([a, b], stroke);
    };
    let path = |pts: Vec<Pos2>| {
        painter.add(egui::Shape::line(pts, stroke));
    };

    match icon {
        Icon::Plus => {
            line(p(0.5, 0.16), p(0.5, 0.84));
            line(p(0.16, 0.5), p(0.84, 0.5));
        }
        Icon::Close => {
            line(p(0.22, 0.22), p(0.78, 0.78));
            line(p(0.78, 0.22), p(0.22, 0.78));
        }
        Icon::Check => {
            path(vec![p(0.18, 0.54), p(0.42, 0.76), p(0.84, 0.26)]);
        }
        Icon::Trash => {
            line(p(0.14, 0.26), p(0.86, 0.26));
            line(p(0.38, 0.26), p(0.42, 0.14));
            line(p(0.62, 0.26), p(0.58, 0.14));
            line(p(0.42, 0.14), p(0.58, 0.14));
            path(vec![
                p(0.22, 0.26),
                p(0.28, 0.88),
                p(0.72, 0.88),
                p(0.78, 0.26),
            ]);
            line(p(0.42, 0.40), p(0.44, 0.74));
            line(p(0.58, 0.40), p(0.56, 0.74));
        }
        Icon::Terminal => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.10, 0.18), p(0.90, 0.82)),
                CornerRadius::same((s * 0.14) as u8),
                stroke,
                StrokeKind::Inside,
            );
            path(vec![p(0.26, 0.38), p(0.40, 0.50), p(0.26, 0.62)]);
            line(p(0.50, 0.64), p(0.72, 0.64));
        }
        Icon::Folder => {
            path(vec![
                p(0.10, 0.80),
                p(0.10, 0.26),
                p(0.42, 0.26),
                p(0.50, 0.38),
                p(0.90, 0.38),
                p(0.90, 0.80),
                p(0.10, 0.80),
            ]);
        }
        Icon::Code => {
            path(vec![p(0.38, 0.24), p(0.14, 0.50), p(0.38, 0.76)]);
            path(vec![p(0.62, 0.24), p(0.86, 0.50), p(0.62, 0.76)]);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.12, 0.12), p(0.62, 0.62)),
                CornerRadius::same((s * 0.1) as u8),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.38, 0.38), p(0.88, 0.88)),
                CornerRadius::same((s * 0.1) as u8),
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Refresh => {
            // Two half turns chasing each other, each with its own head: reads
            // as "again" at any size, unlike a single arc with a loose tip.
            arc(painter, rect.center(), s * 0.30, 3.05, 5.95, stroke);
            path(vec![p(0.80, 0.14), p(0.80, 0.34), p(0.60, 0.33)]);
            arc(painter, rect.center(), s * 0.30, -0.10, 2.80, stroke);
            path(vec![p(0.20, 0.86), p(0.20, 0.66), p(0.40, 0.67)]);
        }
        Icon::Search => {
            painter.circle_stroke(p(0.44, 0.44), s * 0.26, stroke);
            line(p(0.64, 0.64), p(0.86, 0.86));
        }
        Icon::Branch => {
            painter.circle_stroke(p(0.28, 0.22), s * 0.12, stroke);
            painter.circle_stroke(p(0.28, 0.78), s * 0.12, stroke);
            painter.circle_stroke(p(0.76, 0.40), s * 0.12, stroke);
            line(p(0.28, 0.34), p(0.28, 0.66));
            path(vec![p(0.40, 0.40), p(0.64, 0.40)]);
            path(vec![p(0.28, 0.52), p(0.40, 0.40)]);
        }
        Icon::Settings => {
            painter.circle_stroke(rect.center(), s * 0.18, stroke);
            for i in 0..6 {
                let angle = std::f32::consts::TAU * i as f32 / 6.0;
                let (sin, cos) = angle.sin_cos();
                let inner = rect.center() + Vec2::new(cos, sin) * s * 0.26;
                let outer = rect.center() + Vec2::new(cos, sin) * s * 0.42;
                line(inner, outer);
            }
        }
        Icon::Grid => {
            for (x, y) in [(0.12, 0.12), (0.54, 0.12), (0.12, 0.54), (0.54, 0.54)] {
                painter.rect_stroke(
                    Rect::from_min_max(p(x, y), p(x + 0.34, y + 0.34)),
                    CornerRadius::same((s * 0.08) as u8),
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
        // Two overlapping windows: panes you place yourself.
        Icon::Free => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.10, 0.14), p(0.62, 0.60)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.40, 0.42), p(0.90, 0.86)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Columns => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.12, 0.14), p(0.46, 0.86)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.54, 0.14), p(0.88, 0.86)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Rows => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.14, 0.12), p(0.86, 0.46)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.14, 0.54), p(0.86, 0.88)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Tabs => {
            line(p(0.12, 0.34), p(0.88, 0.34));
            painter.rect_stroke(
                Rect::from_min_max(p(0.12, 0.16), p(0.46, 0.34)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.12, 0.34), p(0.88, 0.86)),
                CornerRadius::same((s * 0.08) as u8),
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Broadcast => {
            painter.circle_filled(rect.center(), s * 0.09, color);
            arc(painter, rect.center(), s * 0.24, -0.9, 0.9, stroke);
            arc(painter, rect.center(), s * 0.24, 2.24, 4.04, stroke);
            arc(painter, rect.center(), s * 0.40, -0.9, 0.9, stroke);
            arc(painter, rect.center(), s * 0.40, 2.24, 4.04, stroke);
        }
        Icon::Lock => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.22, 0.46), p(0.78, 0.88)),
                CornerRadius::same((s * 0.12) as u8),
                stroke,
                StrokeKind::Inside,
            );
            // From π to 2π, i.e. over the top of the body (y grows downwards).
            arc(
                painter,
                p(0.50, 0.46),
                s * 0.18,
                std::f32::consts::PI,
                std::f32::consts::TAU,
                stroke,
            );
        }
        Icon::Warning => {
            path(vec![
                p(0.50, 0.14),
                p(0.90, 0.84),
                p(0.10, 0.84),
                p(0.50, 0.14),
            ]);
            line(p(0.50, 0.40), p(0.50, 0.62));
            painter.circle_filled(p(0.50, 0.74), w * 0.6, color);
        }
        Icon::ChevronRight => {
            path(vec![p(0.36, 0.20), p(0.66, 0.50), p(0.36, 0.80)]);
        }
        Icon::ChevronLeft => {
            path(vec![p(0.64, 0.20), p(0.34, 0.50), p(0.64, 0.80)]);
        }
        Icon::ChevronDown => {
            path(vec![p(0.20, 0.38), p(0.50, 0.66), p(0.80, 0.38)]);
        }
        Icon::Info => {
            painter.circle_stroke(p(0.5, 0.5), s * 0.38, stroke);
            line(p(0.5, 0.45), p(0.5, 0.72));
            painter.circle_filled(p(0.5, 0.3), s * 0.05, color);
        }
        Icon::Expand => {
            // Four arrows pushing out of the corners.
            line(p(0.14, 0.14), p(0.40, 0.40));
            path(vec![p(0.14, 0.38), p(0.14, 0.14), p(0.38, 0.14)]);
            line(p(0.86, 0.14), p(0.60, 0.40));
            path(vec![p(0.62, 0.14), p(0.86, 0.14), p(0.86, 0.38)]);
            line(p(0.14, 0.86), p(0.40, 0.60));
            path(vec![p(0.14, 0.62), p(0.14, 0.86), p(0.38, 0.86)]);
            line(p(0.86, 0.86), p(0.60, 0.60));
            path(vec![p(0.62, 0.86), p(0.86, 0.86), p(0.86, 0.62)]);
        }
        Icon::Shrink => {
            // The same arrows, pulled back in.
            line(p(0.10, 0.10), p(0.38, 0.38));
            path(vec![p(0.38, 0.14), p(0.38, 0.38), p(0.14, 0.38)]);
            line(p(0.90, 0.10), p(0.62, 0.38));
            path(vec![p(0.86, 0.38), p(0.62, 0.38), p(0.62, 0.14)]);
            line(p(0.10, 0.90), p(0.38, 0.62));
            path(vec![p(0.14, 0.62), p(0.38, 0.62), p(0.38, 0.86)]);
            line(p(0.90, 0.90), p(0.62, 0.62));
            path(vec![p(0.62, 0.86), p(0.62, 0.62), p(0.86, 0.62)]);
        }
        Icon::ArrowUp => {
            line(p(0.5, 0.82), p(0.5, 0.20));
            path(vec![p(0.28, 0.42), p(0.50, 0.20), p(0.72, 0.42)]);
        }
        Icon::External => {
            path(vec![
                p(0.46, 0.18),
                p(0.16, 0.18),
                p(0.16, 0.84),
                p(0.82, 0.84),
                p(0.82, 0.54),
            ]);
            line(p(0.52, 0.48), p(0.86, 0.14));
            path(vec![p(0.62, 0.14), p(0.86, 0.14), p(0.86, 0.38)]);
        }
        Icon::Dot => {
            painter.circle_filled(rect.center(), s * 0.18, color);
        }
        Icon::Home => {
            path(vec![p(0.12, 0.48), p(0.50, 0.14), p(0.88, 0.48)]);
            path(vec![
                p(0.22, 0.44),
                p(0.22, 0.86),
                p(0.78, 0.86),
                p(0.78, 0.44),
            ]);
        }
        Icon::Pencil => {
            // Nib pointing at the lower left, body running to the upper right.
            path(vec![
                p(0.18, 0.82),
                p(0.30, 0.78),
                p(0.82, 0.26),
                p(0.70, 0.14),
                p(0.18, 0.66),
                p(0.18, 0.82),
            ]);
            line(p(0.60, 0.24), p(0.72, 0.36));
        }
    }
}

fn arc(painter: &Painter, center: Pos2, radius: f32, from: f32, to: f32, stroke: Stroke) {
    const SEGMENTS: usize = 18;
    let mut points = Vec::with_capacity(SEGMENTS + 1);
    for i in 0..=SEGMENTS {
        let t = from + (to - from) * i as f32 / SEGMENTS as f32;
        let (sin, cos) = t.sin_cos();
        points.push(center + Vec2::new(cos, sin) * radius);
    }
    painter.add(egui::Shape::line(points, stroke));
}
