//! The Use Case panel's pictures, drawn in code from circles, lines and rectangles (original to
//! FigCraft, like `crate::icons`): one for each element and relationship kind, so the tools and
//! rows show what they make.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

/// What a picture shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// The select tool: an arrow pointer.
    Select,
    /// A person actor: a stick figure.
    Actor,
    /// A system actor: a box with a small figure badge.
    ExternalSystem,
    /// A use case: an ellipse.
    UseCase,
    /// A system boundary: a rectangle with a title bar.
    System,
    /// A plain line between two dots.
    Association,
    /// A dashed open arrow marked "i".
    Include,
    /// A dashed open arrow marked "e".
    Extend,
    /// A line ending in a hollow triangle.
    Generalization,
    Trash,
    Warning,
    Check,
}

/// Draws `g` filling `r` in `ink` (strokes scale with the box).
pub fn draw(p: &Painter, r: Rect, g: Glyph, ink: Color32) {
    let s = r.width().min(r.height());
    let c = r.center();
    let w = (s / 14.0).max(1.2);
    let line = Stroke::new(w, ink);
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    match g {
        Glyph::Select => {
            let pts = vec![at(-0.22, -0.36), at(-0.22, 0.26), at(-0.06, 0.12), at(0.06, 0.38), at(0.16, 0.33), at(0.04, 0.08), at(0.24, 0.08)];
            p.add(Shape::closed_line(pts, line));
        }
        Glyph::Actor => figure(p, c, s * 0.9, line),
        Glyph::ExternalSystem => {
            let b = Rect::from_center_size(at(0.0, 0.06), vec2(s * 0.78, s * 0.6));
            p.rect_stroke(b, CornerRadius::same(2), line, StrokeKind::Middle);
            figure(p, at(0.0, 0.06), s * 0.42, Stroke::new(w * 0.8, ink));
        }
        Glyph::UseCase => {
            p.add(Shape::ellipse_stroke(c, vec2(s * 0.44, s * 0.26), line));
        }
        Glyph::System => {
            let b = Rect::from_center_size(c, vec2(s * 0.84, s * 0.7));
            p.rect_stroke(b, CornerRadius::same(2), line, StrokeKind::Middle);
            p.line_segment([pos2(b.min.x, b.min.y + s * 0.18), pos2(b.max.x, b.min.y + s * 0.18)], line);
            p.add(Shape::ellipse_stroke(at(0.0, 0.1), vec2(s * 0.2, s * 0.11), Stroke::new(w * 0.8, ink)));
        }
        Glyph::Association => {
            let (a, b) = (at(-0.36, 0.18), at(0.36, -0.18));
            p.line_segment([a, b], line);
            p.circle_filled(a, w * 1.6, ink);
            p.circle_filled(b, w * 1.6, ink);
        }
        Glyph::Include | Glyph::Extend => {
            let (a, b) = (at(-0.38, 0.22), at(0.36, -0.2));
            p.extend(Shape::dashed_line(&[a, b], line, s * 0.1, s * 0.07));
            open_arrow(p, a, b, s * 0.2, line);
            let letter = if g == Glyph::Include { "i" } else { "e" };
            p.text(at(-0.16, -0.2), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(s * 0.36), ink);
        }
        Glyph::Generalization => {
            let (a, b) = (at(-0.38, 0.24), at(0.36, -0.2));
            let d = (b - a).normalized();
            let base = b - d * s * 0.24;
            p.line_segment([a, base], line);
            let side = Vec2::new(-d.y, d.x) * s * 0.15;
            p.add(Shape::closed_line(vec![b, base + side, base - side], line));
        }
        Glyph::Trash => {
            let body = Rect::from_min_max(at(-0.22, -0.18), at(0.22, 0.36));
            p.rect_stroke(body, CornerRadius::same(2), line, StrokeKind::Middle);
            p.line_segment([at(-0.32, -0.24), at(0.32, -0.24)], line);
            p.line_segment([at(-0.08, -0.32), at(0.08, -0.32)], line);
            for x in [-0.08, 0.08] {
                p.line_segment([at(x, -0.06), at(x, 0.24)], line);
            }
        }
        Glyph::Warning => {
            p.add(Shape::closed_line(vec![at(0.0, -0.36), at(0.36, 0.3), at(-0.36, 0.3)], line));
            p.line_segment([at(0.0, -0.1), at(0.0, 0.1)], line);
            p.circle_filled(at(0.0, 0.2), w * 0.8, ink);
        }
        Glyph::Check => {
            p.circle_stroke(c, s * 0.38, line);
            p.add(Shape::line(vec![at(-0.18, 0.0), at(-0.04, 0.14), at(0.2, -0.12)], line));
        }
    }
}

/// A stick figure `h` tall centred on `c`.
fn figure(p: &Painter, c: Pos2, h: f32, line: Stroke) {
    let at = |x: f32, y: f32| pos2(c.x + x * h, c.y + y * h);
    p.circle_stroke(at(0.0, -0.33), h * 0.12, line);
    p.line_segment([at(0.0, -0.21), at(0.0, 0.12)], line);
    p.line_segment([at(-0.22, -0.08), at(0.22, -0.08)], line);
    p.line_segment([at(0.0, 0.12), at(-0.17, 0.42)], line);
    p.line_segment([at(0.0, 0.12), at(0.17, 0.42)], line);
}

/// An open arrowhead at `tip`, pointing away from `from`.
fn open_arrow(p: &Painter, from: Pos2, tip: Pos2, len: f32, line: Stroke) {
    let d = (tip - from).normalized();
    let side = Vec2::new(-d.y, d.x) * len * 0.6;
    let base = tip - d * len;
    p.add(Shape::line(vec![base + side, tip, base - side], line));
}
