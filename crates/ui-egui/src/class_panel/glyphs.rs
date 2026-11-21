//! The Class panel's pictures, drawn in code from lines, rectangles and circles (original to
//! FigCraft, like `crate::icons`): one per element kind and relationship kind, plus the small
//! row buttons. The select pointer, trash, warning and check come from the Use Case set.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::use_case_panel::glyphs::{self as uc, Glyph as UcGlyph};

/// What a picture shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Select,
    /// A box with a header and two compartments.
    Class,
    /// A box with a header and a small circle (a provided interface) on its side.
    Interface,
    /// A box with a header and a short list.
    Enumeration,
    /// A plain line between two dots.
    Association,
    /// A line ending in a hollow diamond.
    Aggregation,
    /// A line ending in a filled diamond.
    Composition,
    /// A line ending in a hollow triangle.
    Generalization,
    /// A dashed line ending in a hollow triangle.
    Realization,
    /// A dashed line ending in an open arrowhead.
    Dependency,
    /// An attribute row: a short line with a dot.
    Attribute,
    /// An operation row: a short line with brackets.
    Operation,
    Up,
    Down,
    /// Two opposed arrows.
    Swap,
    Trash,
    Warning,
    Check,
}

impl Glyph {
    /// The picture for relationship kind `name` (`association`, `composition`, …).
    pub fn of_link(name: &str) -> Glyph {
        match name {
            "aggregation" => Glyph::Aggregation,
            "composition" => Glyph::Composition,
            "generalization" => Glyph::Generalization,
            "realization" => Glyph::Realization,
            "dependency" => Glyph::Dependency,
            _ => Glyph::Association,
        }
    }

    /// A drawing function, for the shared kit's widgets.
    pub fn pic(self) -> impl Fn(&Painter, Rect, Color32) {
        move |p: &Painter, r: Rect, ink: Color32| draw(p, r, self, ink)
    }
}

/// Draws `g` filling `r` in `ink` (strokes scale with the box).
pub fn draw(p: &Painter, r: Rect, g: Glyph, ink: Color32) {
    let s = r.width().min(r.height());
    let c = r.center();
    let w = (s / 14.0).max(1.2);
    let line = Stroke::new(w, ink);
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    match g {
        Glyph::Select => uc::draw(p, r, UcGlyph::Select, ink),
        Glyph::Trash => uc::draw(p, r, UcGlyph::Trash, ink),
        Glyph::Warning => uc::draw(p, r, UcGlyph::Warning, ink),
        Glyph::Check => uc::draw(p, r, UcGlyph::Check, ink),
        Glyph::Association => uc::draw(p, r, UcGlyph::Association, ink),
        Glyph::Class | Glyph::Interface | Glyph::Enumeration => {
            let b = Rect::from_min_max(at(-0.34, -0.34), at(0.34, 0.34));
            p.rect_stroke(b, CornerRadius::same(2), line, StrokeKind::Middle);
            let head = b.min.y + s * 0.2;
            p.line_segment([pos2(b.min.x, head), pos2(b.max.x, head)], line);
            match g {
                Glyph::Class => {
                    let mid = b.min.y + s * 0.44;
                    p.line_segment([pos2(b.min.x, mid), pos2(b.max.x, mid)], line);
                }
                Glyph::Interface => {
                    let o = pos2(b.max.x + s * 0.1, c.y + s * 0.08);
                    p.line_segment([pos2(b.max.x, o.y), o], line);
                    p.circle_stroke(o + vec2(s * 0.07, 0.0), s * 0.07, line);
                }
                _ => {
                    for k in [0.0, 0.12, 0.24] {
                        let y = head + s * (0.08 + k);
                        p.circle_filled(pos2(b.min.x + s * 0.12, y), w * 0.7, ink);
                        p.line_segment([pos2(b.min.x + s * 0.2, y), pos2(b.max.x - s * 0.1, y)], Stroke::new(w * 0.8, ink));
                    }
                }
            }
        }
        Glyph::Aggregation | Glyph::Composition => {
            let (a, b) = (at(-0.38, 0.22), at(0.38, -0.22));
            let d = (b - a).normalized();
            let side = Vec2::new(-d.y, d.x) * s * 0.11;
            let (tip, mid, back) = (a, a + d * s * 0.15, a + d * s * 0.3);
            p.line_segment([back, b], line);
            let diamond = vec![tip, mid + side, back, mid - side];
            if g == Glyph::Composition {
                p.add(Shape::convex_polygon(diamond, ink, line));
            } else {
                p.add(Shape::closed_line(diamond, line));
            }
        }
        Glyph::Generalization | Glyph::Realization => {
            let (a, b) = (at(-0.38, 0.24), at(0.36, -0.2));
            let d = (b - a).normalized();
            let base = b - d * s * 0.24;
            if g == Glyph::Realization {
                p.extend(Shape::dashed_line(&[a, base], line, s * 0.1, s * 0.07));
            } else {
                p.line_segment([a, base], line);
            }
            let side = Vec2::new(-d.y, d.x) * s * 0.15;
            p.add(Shape::closed_line(vec![b, base + side, base - side], line));
        }
        Glyph::Dependency => {
            let (a, b) = (at(-0.38, 0.22), at(0.36, -0.2));
            p.extend(Shape::dashed_line(&[a, b], line, s * 0.1, s * 0.07));
            open_arrow(p, a, b, s * 0.22, line);
        }
        Glyph::Attribute => {
            p.circle_filled(at(-0.26, 0.0), w * 1.3, ink);
            p.line_segment([at(-0.1, 0.0), at(0.32, 0.0)], line);
        }
        Glyph::Operation => {
            p.line_segment([at(-0.34, 0.0), at(0.02, 0.0)], line);
            p.add(Shape::line(vec![at(0.14, -0.16), at(0.08, 0.0), at(0.14, 0.16)], line));
            p.add(Shape::line(vec![at(0.24, -0.16), at(0.3, 0.0), at(0.24, 0.16)], line));
        }
        Glyph::Up | Glyph::Down => {
            let dir = if g == Glyph::Up { -1.0 } else { 1.0 };
            p.line_segment([at(0.0, -0.3 * dir), at(0.0, 0.3 * dir)], line);
            p.add(Shape::line(vec![at(-0.2, 0.1 * dir), at(0.0, 0.3 * dir), at(0.2, 0.1 * dir)], line));
        }
        Glyph::Swap => {
            p.line_segment([at(-0.32, -0.12), at(0.32, -0.12)], line);
            open_arrow(p, at(-0.32, -0.12), at(0.32, -0.12), s * 0.16, line);
            p.line_segment([at(0.32, 0.14), at(-0.32, 0.14)], line);
            open_arrow(p, at(0.32, 0.14), at(-0.32, 0.14), s * 0.16, line);
        }
    }
}

/// An open arrowhead at `tip`, pointing away from `from`.
fn open_arrow(p: &Painter, from: Pos2, tip: Pos2, len: f32, line: Stroke) {
    let d = (tip - from).normalized();
    let side = Vec2::new(-d.y, d.x) * len * 0.6;
    let base = tip - d * len;
    p.add(Shape::line(vec![base + side, tip, base - side], line));
}
