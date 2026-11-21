//! Resizing a typed diagram's element with the mouse, as ordinary layers are resized: the selected
//! element has a white square on each corner, and a drag from a square or from an edge gives it a
//! new size. The opposite side stays where it was, Shift on a corner keeps the proportions, and the
//! size never goes below the element's minimum (what its text needs). While the button is down the
//! new box is drawn dashed with its size; when it comes up one command sets it (one undo step).
//!
//! Each diagram says which element is selected, its box and its minimum; this module does the
//! rest. The engine still checks the result (a card can't grow into its neighbour) and a refusal
//! shows its reason, with nothing changed.

use egui::{Color32, Context, CornerRadius, CursorIcon, Id, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::FigApp;
use crate::diagram_kit::{chip, dashed_box};

/// An element's box on the page: (x, y, w, h).
pub type PageBox = (f64, f64, f64, f64);

/// The side of the box a drag moves on each axis: -1 the left (top), 1 the right (bottom), 0 none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grab {
    pub x: i8,
    pub y: i8,
}

/// A resize under way: the element, what was grabbed, where the press was on the screen, its box
/// then and its minimum size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resize {
    pub id: u64,
    pub grab: Grab,
    pub press: Pos2,
    pub start: PageBox,
    pub min: (f64, f64),
}

/// The corner squares' side, as on ordinary layers (screen points).
pub const SQUARE: f32 = 8.0;
/// A press this close to a box corner, on either axis, takes its square (screen points; the
/// square is drawn 3 points out from the corner).
const SQUARE_REACH: f32 = 8.0;
/// A press this close to an edge takes it (screen points).
const EDGE_REACH: f32 = 4.0;
/// A box smaller than this on screen, on either side, has no edges to grab: its squares are
/// enough, and a press inside still moves it.
const SMALL: f32 = 24.0;

fn resize_id() -> Id {
    Id::unique("diagram_resize")
}

fn cursor_id() -> Id {
    Id::unique("diagram_resize_cursor")
}

/// The squares' centres: top left, top right, bottom right, bottom left.
fn corners(r: Rect) -> [(Grab, Pos2); 4] {
    [
        (Grab { x: -1, y: -1 }, r.left_top()),
        (Grab { x: 1, y: -1 }, r.right_top()),
        (Grab { x: 1, y: 1 }, r.right_bottom()),
        (Grab { x: -1, y: 1 }, r.left_bottom()),
    ]
}

/// What a press at screen point `p` grabs on the box drawn at screen rect `r`: a square first,
/// then an edge.
pub fn grab_at(r: Rect, p: Pos2) -> Option<Grab> {
    if !(r.is_finite() && r.width() > 0.0 && r.height() > 0.0) {
        return None;
    }
    if let Some((g, _)) = corners(r).into_iter().find(|(_, c)| (p.x - c.x).abs() <= SQUARE_REACH && (p.y - c.y).abs() <= SQUARE_REACH) {
        return Some(g);
    }
    if r.width() < SMALL || r.height() < SMALL {
        return None;
    }
    let (along_x, along_y) = (p.x > r.left() && p.x < r.right(), p.y > r.top() && p.y < r.bottom());
    let near = |a: f32, b: f32| (a - b).abs() <= EDGE_REACH;
    match () {
        _ if along_y && near(p.x, r.left()) => Some(Grab { x: -1, y: 0 }),
        _ if along_y && near(p.x, r.right()) => Some(Grab { x: 1, y: 0 }),
        _ if along_x && near(p.y, r.top()) => Some(Grab { x: 0, y: -1 }),
        _ if along_x && near(p.y, r.bottom()) => Some(Grab { x: 0, y: 1 }),
        _ => None,
    }
}

/// One axis: the start and length after the grabbed side moves by `d`, never below `min`; the
/// other side stays put. Whole units, except at the minimum (which may be a measured fraction).
fn axis(pos: f64, len: f64, side: i8, d: f64, min: f64) -> (f64, f64) {
    match side {
        1 => (pos, (len + d).round().max(min)),
        -1 => {
            let l = (len - d).round().max(min);
            (pos + len - l, l)
        }
        _ => (pos, len),
    }
}

/// The box after the grabbed sides move by `d` (page units). With `keep` (Shift) a corner keeps
/// the box's proportions, following whichever axis moved further.
pub fn resized(start: PageBox, g: Grab, d: (f64, f64), min: (f64, f64), keep: bool) -> PageBox {
    let (x, y, w, h) = start;
    let mut d = d;
    if keep && g.x != 0 && g.y != 0 && w > 0.0 && h > 0.0 {
        let sx = (w + f64::from(g.x) * d.0) / w;
        let sy = (h + f64::from(g.y) * d.1) / h;
        let s = sx.max(sy).max(min.0 / w).max(min.1 / h);
        d = (f64::from(g.x) * (w * s - w), f64::from(g.y) * (h * s - h));
    }
    let (nx, nw) = axis(x, w, g.x, d.0, min.0);
    let (ny, nh) = axis(y, h, g.y, d.1, min.1);
    (nx, ny, nw, nh)
}

/// The pointer for a grab: a two-headed arrow along the drag.
pub fn cursor(g: Grab) -> CursorIcon {
    match (g.x, g.y) {
        (0, _) => CursorIcon::ResizeVertical,
        (_, 0) => CursorIcon::ResizeHorizontal,
        (1, 1) | (-1, -1) => CursorIcon::ResizeNwSe,
        _ => CursorIcon::ResizeNeSw,
    }
}

/// The element's box on the screen.
pub fn screen_rect(app: &FigApp, b: PageBox) -> Rect {
    Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3))
}

/// A resize is under way (the button is down on a square or an edge).
pub fn active(ctx: &Context) -> Option<Resize> {
    ctx.data(|d| d.get_temp(resize_id()))
}

/// Drops a resize under way (the diagram left the view, or the tool changed).
pub fn cancel(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Resize>(resize_id()));
}

/// A press at `pos` on the selected element `id` (box `b`, minimum `min`): starts a resize when it
/// is on a square or an edge, and says so.
pub fn press(ctx: &Context, app: &FigApp, id: u64, b: PageBox, min: (f64, f64), pos: Pos2) -> bool {
    let Some(grab) = grab_at(screen_rect(app, b), pos) else { return false };
    ctx.data_mut(|d| d.insert_temp(resize_id(), Resize { id, grab, press: pos, start: b, min }));
    true
}

/// The box a resize under way gives, with the pointer at `p`.
pub fn target(app: &FigApp, ctx: &Context, r: &Resize, p: Pos2) -> PageBox {
    let delta = app.to_page(p) - app.to_page(r.press);
    let keep = ctx.input(|i| i.modifiers.shift);
    resized(r.start, r.grab, (delta.x, delta.y), r.min, keep)
}

/// Follows a resize under way. While the button is down: `Some(None)`. When it comes up:
/// `Some(Some((id, box)))` when the box changed (the caller sets it with its command), else
/// `Some(None)`. With no resize under way: `None`.
pub fn follow(app: &FigApp, ctx: &Context) -> Option<Option<(u64, PageBox)>> {
    let r = active(ctx)?;
    let (pos, down) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_down()));
    if down {
        ctx.request_repaint();
        return Some(None);
    }
    cancel(ctx);
    let b = target(app, ctx, &r, pos.unwrap_or(r.press));
    Some((b != r.start).then_some((r.id, b)))
}

/// The resize command's parameters for `id` set to box `b`.
pub fn params(id: u64, b: PageBox) -> serde_json::Value {
    serde_json::json!({"id": id, "x": b.0, "y": b.1, "width": b.2, "height": b.3})
}

/// Draws the selected element's squares (box `b`), the pointer for what it is over, and, while a
/// resize is under way, the new box with its size.
pub fn draw(app: &FigApp, painter: &Painter, b: PageBox, pointer: Option<Pos2>) {
    let ctx = painter.ctx().clone();
    let accent = crate::theme::canvas_accent(&ctx);
    let r = screen_rect(app, b);
    // On the corners of the selection outline, which the diagrams draw 3 points outside the box.
    for (_, c) in corners(r.expand(3.0)) {
        let sq = Rect::from_center_size(c, vec2(SQUARE, SQUARE));
        painter.rect_filled(sq, CornerRadius::same(1), Color32::WHITE);
        painter.rect_stroke(sq, CornerRadius::same(1), Stroke::new(1.0, accent), StrokeKind::Inside);
    }
    let under_way = active(&ctx);
    if let Some(rz) = under_way {
        let p = pointer.or_else(|| ctx.input(|i| i.pointer.hover_pos())).unwrap_or(rz.press);
        let nb = target(app, &ctx, &rz, p);
        let nr = screen_rect(app, nb);
        painter.rect_filled(nr, CornerRadius::same(2), accent.gamma_multiply(0.08));
        dashed_box(painter, nr, accent);
        chip(painter, pos2(nr.center().x, nr.bottom() + 14.0), format!("{} × {}", trim(nb.2), trim(nb.3)), accent);
        set_cursor(&ctx, cursor(rz.grab));
    } else if let Some(g) = pointer.and_then(|p| grab_at(r, p)) {
        set_cursor(&ctx, cursor(g));
    }
}

/// A size as it reads: whole units without a fraction, else one decimal.
fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 { format!("{}", v.round()) } else { format!("{v:.1}") }
}

fn set_cursor(ctx: &Context, c: CursorIcon) {
    ctx.data_mut(|d| d.insert_temp(cursor_id(), c));
}

/// The pointer a diagram's squares or edges asked for this frame, if any (taken once: the canvas
/// shows it in place of the tool's).
pub fn take_cursor(ctx: &Context) -> Option<CursorIcon> {
    ctx.data_mut(|d| d.remove_temp::<CursorIcon>(cursor_id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { min: pos2(100.0, 100.0), max: pos2(300.0, 200.0) };

    #[test]
    fn squares_win_over_edges_and_the_middle_grabs_nothing() {
        assert_eq!(grab_at(R, pos2(101.0, 99.0)), Some(Grab { x: -1, y: -1 }));
        assert_eq!(grab_at(R, pos2(304.0, 204.0)), Some(Grab { x: 1, y: 1 }));
        assert_eq!(grab_at(R, pos2(300.0, 150.0)), Some(Grab { x: 1, y: 0 }));
        assert_eq!(grab_at(R, pos2(200.0, 97.0)), Some(Grab { x: 0, y: -1 }));
        assert_eq!(grab_at(R, pos2(200.0, 150.0)), None);
        assert_eq!(grab_at(R, pos2(320.0, 150.0)), None);
    }

    #[test]
    fn a_small_box_has_only_its_squares() {
        let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(20.0, 20.0));
        assert_eq!(grab_at(small, pos2(20.0, 10.0)), None);
        assert_eq!(grab_at(small, pos2(20.0, 20.0)), Some(Grab { x: 1, y: 1 }));
    }

    #[test]
    fn the_opposite_side_stays_and_the_minimum_holds() {
        let b = (0.0, 0.0, 200.0, 100.0);
        assert_eq!(resized(b, Grab { x: 1, y: 1 }, (40.0, 20.0), (50.0, 50.0), false), (0.0, 0.0, 240.0, 120.0));
        assert_eq!(resized(b, Grab { x: -1, y: 0 }, (30.0, 99.0), (50.0, 50.0), false), (30.0, 0.0, 170.0, 100.0));
        // Past the minimum: it stops there, and the right edge stays at 200.
        assert_eq!(resized(b, Grab { x: -1, y: -1 }, (190.0, 90.0), (62.5, 50.0), false), (137.5, 50.0, 62.5, 50.0));
    }

    #[test]
    fn shift_on_a_corner_keeps_the_proportions() {
        let b = (0.0, 0.0, 200.0, 100.0);
        assert_eq!(resized(b, Grab { x: 1, y: 1 }, (100.0, 10.0), (10.0, 10.0), true), (0.0, 0.0, 300.0, 150.0));
        // On an edge Shift changes nothing.
        assert_eq!(resized(b, Grab { x: 1, y: 0 }, (100.0, 10.0), (10.0, 10.0), true), (0.0, 0.0, 300.0, 100.0));
    }
}
