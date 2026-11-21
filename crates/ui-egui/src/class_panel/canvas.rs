//! Pointing at a Class diagram on the canvas. Its drawn layers are locked (they are made from the
//! model), so the canvas can't pick them; instead, with the Move tool:
//! - a press on a card selects it (and the member row under the press), and a drag moves it
//!   (`diagram.class.move` when the button comes up: one undo step);
//! - the selected card has a square on each corner: a drag from one, or from an edge, resizes it
//!   (`diagram.class.resize` when the button comes up, never below what its text needs);
//! - a press near a relationship selects it;
//! - with an Add tool, a press places a new classifier there;
//! - with a Connect tool, a press on one card and then another joins them.
//!
//! The rows and link paths come from the engine's layout, the same one the renderer draws with,
//! so a press lands on what is drawn. Delete removes the selected row or element, Escape drops the
//! tool and then the selection. Presses on empty canvas, the wheel and pinch go on to the canvas.

use egui::{Context, CornerRadius, Key, Modifiers, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use figcraft_doc::class_diagram::{ClassifierId, LinkKind, Model, connection_kinds};
use figcraft_engine::ToolId;
use figcraft_engine::class_layout::{card_layout, link_paths};
use figcraft_geom::Point;
use serde_json::{Value, json};

use super::Comp;
use super::edit;
use super::{Place, State, Tool, current, link_label, load, save};
use crate::FigApp;
use crate::diagram_kit::{chip, dashed_box};
use crate::i18n::{tr, trf};
use crate::theme::Tokens;

/// A press moves a card only once the pointer has gone this far (screen points).
const DRAG_SLOP: f32 = 4.0;
/// A press this close to a relationship's line picks it (screen points).
const LINK_REACH: f32 = 6.0;
/// The size a new card is drawn at while placing (page units).
const NEW_CARD: (f64, f64) = (140.0, 64.0);

/// A drag of a card: where it started on the screen and the card's position then.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    id: u64,
    press: Pos2,
    origin: (f64, f64),
}

fn drag_id() -> egui::Id {
    egui::Id::unique("class_canvas_drag")
}

/// The canvas acts on the diagram: a Class page in view, the Move tool, and the file editable.
pub(super) fn active(app: &FigApp) -> Option<std::sync::Arc<Model>> {
    if app.session.tool.tool != ToolId::Move || app.session.tool.busy() || app.session.active_read_only().is_some() {
        return None;
    }
    current(app)
}

/// A card's box on the page: (x, y, w, h).
pub fn card_box(m: &Model, id: u64) -> Option<(f64, f64, f64, f64)> {
    m.classifiers.iter().find(|c| c.id.0 == id).map(|c| (c.x, c.y, c.w, c.h))
}

/// The card at page point `q`, topmost (last drawn) first.
pub fn card_at(m: &Model, q: Point) -> Option<u64> {
    m.classifiers.iter().rev().find(|c| q.x >= c.x && q.x <= c.x + c.w && q.y >= c.y && q.y <= c.y + c.h).map(|c| c.id.0)
}

/// A resize under way belongs to a card of the Class page in view (else to another diagram).
fn card_box_any(app: &FigApp, id: u64) -> bool {
    current(app).is_some_and(|m| card_box(&m, id).is_some())
}

/// The member row of card `id` at page point `q`.
pub fn row_at(m: &Model, id: u64, q: Point) -> Option<u64> {
    let c = m.classifiers.iter().find(|c| c.id.0 == id)?;
    card_layout(c).row_at(q).map(|r| r.member.0)
}

/// The relationship whose path passes within `reach` (page units) of `q`.
pub fn link_at(m: &Model, q: Point, reach: f64) -> Option<u64> {
    m.links.iter().zip(link_paths(m)).rev().find_map(|(l, p)| p.filter(|p| p.distance(q) <= reach).map(|_| l.id.0))
}

/// Handles a press, drag or release on the canvas; `true` when the diagram took it (the canvas
/// then doesn't see it).
pub fn input(app: &mut FigApp, ui: &egui::Ui, resp: &egui::Response) -> bool {
    let ctx = ui.ctx().clone();
    let drag: Option<Drag> = ctx.data(|d| d.get_temp(drag_id()));
    let Some(model) = active(app) else {
        if drag.is_some() {
            ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
        }
        if crate::diagram_resize::active(&ctx).is_some_and(|r| card_box_any(app, r.id)) {
            crate::diagram_resize::cancel(&ctx);
        }
        return false;
    };
    // A resize under way from a square or an edge.
    if crate::diagram_resize::active(&ctx).is_some_and(|r| card_box(&model, r.id).is_some()) {
        if let Some(Some((id, b))) = crate::diagram_resize::follow(app, &ctx) {
            app.run("diagram.class.resize", crate::diagram_resize::params(id, b));
        }
        return true;
    }
    let (pos, pressed, down, double) = ctx.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.primary_down(),
            i.pointer.button_double_clicked(egui::PointerButton::Primary),
        )
    });
    // The drag-out handle, while it is dragged: its drop opens the chooser.
    let mut st = load(&ctx, app);
    if st.out.as_ref().is_some_and(|o| o.dropped.is_none()) {
        if !down {
            match pos {
                Some(p) => edit::drop_handle(app, &model, &mut st, p),
                None => st.out = None,
            }
        }
        ctx.request_repaint();
        save(&ctx, st);
        return true;
    }
    if let Some(d) = drag {
        if down {
            ctx.request_repaint();
            return true;
        }
        ctx.data_mut(|m| m.remove::<Drag>(drag_id()));
        let end = pos.unwrap_or(d.press);
        if (end - d.press).length() >= DRAG_SLOP {
            let delta = app.to_page(end) - app.to_page(d.press);
            app.run("diagram.class.move", json!({"id": d.id, "x": (d.origin.0 + delta.x).round(), "y": (d.origin.1 + delta.y).round()}));
        } else if double && let Some(member) = row_at(&model, d.id, app.to_page(end)) {
            // A double-click (egui reports it on the release) on a row types over it.
            edit::open_row(app, &mut st, &model, d.id, member);
            save(&ctx, st);
        }
        return true;
    }
    let Some(pos) = pos.filter(|p| pressed && resp.hovered() && app.canvas.rect.contains(*p)) else { return false };
    let q = app.to_page(pos);
    if st.tool == Tool::Select && edit::press_handle(app, &model, &mut st, pos) {
        save(&ctx, st);
        return true;
    }
    // A square or an edge of the selected card starts a resize.
    if st.tool == Tool::Select
        && let Some(c) = st.selected.and_then(|id| model.classifiers.iter().find(|c| c.id.0 == id))
        && crate::diagram_resize::press(&ctx, app, c.id.0, (c.x, c.y, c.w, c.h), (c.min_w, c.min_h), pos)
    {
        st.member = None;
        save(&ctx, st);
        return true;
    }
    // A double-click on a row types over it.
    if double
        && st.tool == Tool::Select
        && let Some(id) = card_at(&model, q)
        && let Some(member) = row_at(&model, id, q)
    {
        ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
        edit::open_row(app, &mut st, &model, id, member);
        save(&ctx, st);
        return true;
    }
    let took = match st.tool {
        Tool::Place(kind) => {
            place(app, &model, &mut st, kind, q);
            true
        }
        Tool::Connect { kind, from } => {
            connect(app, &model, &mut st, kind, from, q);
            true
        }
        Tool::Select => select(app, &ctx, &model, &mut st, pos, q),
    };
    save(&ctx, st);
    took
}

/// A press with the select tool: a card (its row, and the start of a drag), a relationship, or
/// nothing (the press goes on to the canvas).
fn select(app: &FigApp, ctx: &Context, m: &Model, st: &mut State, pos: Pos2, q: Point) -> bool {
    if let Some(id) = card_at(m, q) {
        st.selected = Some(id);
        st.member = row_at(m, id, q);
        if let Some((x, y, ..)) = card_box(m, id) {
            ctx.data_mut(|d| d.insert_temp(drag_id(), Drag { id, press: pos, origin: (x, y) }));
        }
        return true;
    }
    let zoom = app.session.ds().map(|d| d.view.zoom).unwrap_or(1.0).max(1e-6);
    if let Some(id) = link_at(m, q, f64::from(LINK_REACH) / zoom) {
        st.selected = Some(id);
        st.member = None;
        return true;
    }
    st.selected = None;
    st.member = None;
    false
}

/// The next default name for `kind`: "Class 3" when "Class 1" and "Class 2" are taken.
pub fn default_name(m: &Model, kind: Place) -> String {
    let base = tr(kind.label());
    let taken = |n: &str| m.classifiers.iter().any(|c| c.name == n);
    (1..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
}

/// Places a new `kind` centred on page point `q`; where it doesn't fit it goes to the nearest
/// free room instead.
pub(super) fn place(app: &mut FigApp, m: &Model, st: &mut State, kind: Place, q: Point) {
    let p = json!({"kind": kind.param(), "name": default_name(m, kind)});
    let mut at = p.clone();
    at["x"] = json!((q.x - NEW_CARD.0 / 2.0).round());
    at["y"] = json!((q.y - NEW_CARD.1 / 2.0).round());
    // Quietly: a spot that doesn't fit isn't an error, the card goes to free room nearby.
    let made = app.session.execute("diagram.class.addClassifier", &at).ok().or_else(|| app.run("diagram.class.addClassifier", p));
    app.handle_ui_requests();
    if let Some(id) = made.as_ref().and_then(|r| r.get("id")).and_then(Value::as_u64) {
        st.selected = Some(id);
        st.member = None;
    }
    st.tool = Tool::Select;
}

/// A press with a Connect tool: the first card starts the relationship, the second ends it. A
/// self-association is made by pressing the same card twice.
fn connect(app: &mut FigApp, m: &Model, st: &mut State, kind: LinkKind, from: Option<u64>, q: Point) {
    let Some(at) = card_at(m, q) else { return };
    let Some(from) = from else {
        st.tool = Tool::Connect { kind, from: Some(at) };
        st.selected = Some(at);
        st.member = None;
        return;
    };
    let made = app.run("diagram.class.connect", json!({"kind": kind.name(), "from": from, "to": at}));
    if let Some(id) = made.as_ref().and_then(|r| r.get("id")).and_then(Value::as_u64) {
        st.selected = Some(id);
    }
    // Ready for the next one of the same kind.
    st.tool = Tool::Connect { kind, from: None };
}

/// The kinds that would join `from` to `to`, and why not when none would; kept for the last model
/// and pair asked about (the hover hint asks every frame).
pub fn joins_cached(ctx: &Context, m: &Model, from: u64, to: u64) -> (Vec<LinkKind>, Option<String>) {
    type Cached = (Model, u64, u64, Vec<LinkKind>, Option<String>);
    let key = egui::Id::unique("class_joins");
    if let Some((cm, cf, ct, kinds, reason)) = ctx.data(|d| d.get_temp::<std::sync::Arc<Cached>>(key)).as_deref()
        && (*cf, *ct) == (from, to)
        && cm == m
    {
        return (kinds.clone(), reason.clone());
    }
    let mut reason = None;
    let kinds: Vec<LinkKind> = connection_kinds(m, ClassifierId(from), ClassifierId(to))
        .into_iter()
        .filter(|k| match figcraft_engine::class_check_connect(m, &json!({"kind": k.name(), "from": from, "to": to})) {
            Ok(()) => true,
            Err(e) => {
                reason.get_or_insert(e);
                false
            }
        })
        .collect();
    let reason = if kinds.is_empty() { Some(reason.unwrap_or_else(|| tr("Nothing can join these two").to_string())) } else { None };
    ctx.data_mut(|d| d.insert_temp(key, std::sync::Arc::new((m.clone(), from, to, kinds.clone(), reason.clone()))));
    (kinds, reason)
}

/// The keyboard on a Class page, taken before the shortcuts while no field has the keyboard:
/// - Delete removes the selected row or element; Escape drops the tool, then the row, then the
///   selection;
/// - Alt+Shift+A / Alt+Shift+O add an attribute / operation to the selected card (a literal on an
///   enumeration), Insert adds a row after the selected one, Enter or F2 types over the selected
///   row. With no card selected those keys keep their usual commands (Alt+Shift+A removes auto
///   layout elsewhere);
/// - `[` and `]` step the selection through the cards and relationships;
/// - the arrows move the selected card by 8 (Shift: 1), Alt+arrows resize it.
pub fn keys(app: &mut FigApp, ctx: &Context) {
    let Some(m) = active(app) else { return };
    if crate::a11y::field_focused(ctx) {
        return;
    }
    let mut st = load(ctx, app);
    let card = st.selected.and_then(|id| m.classifiers.iter().find(|c| c.id.0 == id)).cloned();
    if let Some(c) = &card {
        let alt_shift = Modifiers::ALT | Modifiers::SHIFT;
        let comp = st.member.and_then(|r| edit::comp_of(c, r));
        if ctx.input_mut(|i| i.consume_key(alt_shift, Key::A)) {
            let k = edit::default_comp(c, Comp::Attribute);
            edit::open_draft(app, &mut st, c.id.0, k, None);
        } else if ctx.input_mut(|i| i.consume_key(alt_shift, Key::O)) {
            let k = edit::default_comp(c, Comp::Operation);
            edit::open_draft(app, &mut st, c.id.0, k, None);
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Insert)) {
            let k = comp.unwrap_or_else(|| edit::default_comp(c, Comp::Attribute));
            let after = st.member.filter(|_| comp.is_some());
            edit::open_draft(app, &mut st, c.id.0, k, after);
        } else if let Some(r) = st.member.filter(|_| comp.is_some())
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::F2))
        {
            edit::open_row(app, &mut st, &m, c.id.0, r);
        } else if let Some((dx, dy, resize)) = arrow(ctx) {
            if resize {
                app.run("diagram.class.resize", json!({"id": c.id.0, "width": (c.w + dx).max(1.0), "height": (c.h + dy).max(1.0)}));
            } else {
                app.run("diagram.class.move", json!({"id": c.id.0, "x": c.x + dx, "y": c.y + dy}));
            }
        }
    }
    // `[` and `]`: the previous or next card or relationship, in model order.
    let order: Vec<u64> = m.classifiers.iter().map(|c| c.id.0).chain(m.links.iter().map(|l| l.id.0)).collect();
    let step = if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::CloseBracket)) {
        Some(1isize)
    } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::OpenBracket)) {
        Some(-1)
    } else {
        None
    };
    if let Some(step) = step
        && !order.is_empty()
    {
        let n = order.len() as isize;
        let at = st.selected.and_then(|s| order.iter().position(|x| *x == s)).map_or(if step > 0 { -1 } else { 0 }, |i| i as isize);
        st.selected = order.get((at + step).rem_euclid(n) as usize).copied();
        st.member = None;
    }
    if st.tool != Tool::Select && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        st.tool = Tool::Select;
    } else if let Some(id) = st.selected {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if st.member.is_some() {
                st.member = None;
            } else {
                st.selected = None;
            }
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace)) {
            if let Some(member) = st.member.take() {
                app.run("diagram.class.removeMember", json!({"id": member}));
            } else {
                app.run("diagram.class.remove", json!({"ids": [id]}));
                st.selected = None;
            }
        }
    }
    save(ctx, st);
}

/// An arrow key this frame: the move (or, with Alt, the resize) it asks for.
fn arrow(ctx: &Context) -> Option<(f64, f64, bool)> {
    for (mods, step, resize) in [(Modifiers::SHIFT, 1.0, false), (Modifiers::ALT, 8.0, true), (Modifiers::NONE, 8.0, false)] {
        for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
            if ctx.input_mut(|i| i.consume_key(mods, key)) {
                return Some((dx * step, dy * step, resize));
            }
        }
    }
    None
}

/// The selection's outline and row, the dragged card's ghost, the Connect tool's line and what a
/// press would pick, over the canvas.
pub fn draw(app: &FigApp, painter: &Painter) {
    let Some(m) = active(app) else { return };
    let ctx = painter.ctx().clone();
    let st = load(&ctx, app);
    let accent = crate::theme::canvas_accent(&ctx);
    let screen = |b: (f64, f64, f64, f64)| Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3));
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| app.canvas.rect.contains(*p));
    let hover = pointer.and_then(|p| card_at(&m, app.to_page(p)));
    if let Some(h) = hover.filter(|h| Some(*h) != st.selected)
        && let Some(b) = card_box(&m, h)
    {
        painter.rect_stroke(screen(b).expand(3.0), CornerRadius::same(4), Stroke::new(1.0, accent.gamma_multiply(0.6)), StrokeKind::Outside);
    }
    if let Some(sel) = st.selected {
        if let Some(b) = card_box(&m, sel) {
            painter.rect_stroke(screen(b).expand(3.0), CornerRadius::same(4), Stroke::new(2.0, accent), StrokeKind::Outside);
            if let Some(member) = st.member
                && let Some(c) = m.classifiers.iter().find(|c| c.id.0 == sel)
                && let Some(r) = card_layout(c).compartments.iter().flat_map(|k| k.rows.iter()).find(|r| r.member.0 == member)
            {
                let rr = screen((r.rect.x0, r.rect.y0, r.rect.width(), r.rect.height()));
                painter.rect_filled(rr, CornerRadius::same(2), accent.gamma_multiply(0.15));
                painter.rect_stroke(rr, CornerRadius::same(2), Stroke::new(1.0, accent), StrokeKind::Inside);
            }
        } else if let Some(i) = m.links.iter().position(|l| l.id.0 == sel)
            && let Some(Some(p)) = link_paths(&m).into_iter().nth(i)
        {
            let pts: Vec<Pos2> = p.points.iter().map(|q| app.to_screen(q.x, q.y)).collect();
            painter.add(Shape::line(pts, Stroke::new(4.0, accent.gamma_multiply(0.5))));
        }
    }
    let drag: Option<Drag> = ctx.data(|d| d.get_temp(drag_id()));
    if let (Some(d), Some(p)) = (drag, pointer)
        && (p - d.press).length() >= DRAG_SLOP
        && let Some(b) = card_box(&m, d.id)
    {
        let ghost = screen(b).translate(p - d.press);
        painter.rect_filled(ghost, CornerRadius::same(4), accent.gamma_multiply(0.08));
        dashed_box(painter, ghost, accent);
    }
    if st.tool == Tool::Select {
        if let Some(b) = st.selected.and_then(|id| card_box(&m, id)) {
            crate::diagram_resize::draw(app, painter, b, pointer);
        }
        edit::draw_handle(app, painter, &m, &st, pointer);
    }
    match st.tool {
        Tool::Connect { kind, from: Some(f) } => {
            if let (Some(b), Some(p)) = (card_box(&m, f), pointer) {
                let a = app.to_screen(b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
                painter.extend(Shape::dashed_line(&[a, p], Stroke::new(1.5, accent), 6.0, 4.0));
                painter.circle_filled(p, 3.0, accent);
            }
            if let (Some(h), Some(p)) = (hover, pointer) {
                connection_hint(painter, app, &m, f, h, kind, p);
            }
        }
        Tool::Place(_) => {
            if let Some(p) = pointer {
                let zoom = app.session.ds().map(|d| d.view.zoom).unwrap_or(1.0) as f32;
                let size = vec2(NEW_CARD.0 as f32, NEW_CARD.1 as f32) * zoom;
                dashed_box(painter, Rect::from_center_size(p, size), accent);
            }
        }
        _ => {}
    }
}

/// While a Connect tool has its first card, the card under the pointer says what a press there
/// would make: outlined in the accent colour when the tool joins them, in red with the reason or
/// the kinds that do apply when it doesn't.
fn connection_hint(painter: &Painter, app: &FigApp, m: &Model, from: u64, to: u64, kind: LinkKind, at: Pos2) {
    let Some(b) = card_box(m, to) else { return };
    let ctx = painter.ctx().clone();
    let t = Tokens::get(&ctx);
    let (joins, reason) = joins_cached(&ctx, m, from, to);
    let (ink, text) = if joins.contains(&kind) {
        (crate::theme::canvas_accent(&ctx), trf("Joins as {kind}", &[("kind", &link_label(kind))]))
    } else if joins.is_empty() {
        (t.danger, reason.unwrap_or_else(|| tr("Nothing can join these two").to_string()))
    } else {
        let here: Vec<&str> = joins.iter().map(|k| link_label(*k)).collect();
        (t.danger, trf("{tool} doesn't join these. Here: {kinds}", &[("tool", &link_label(kind)), ("kinds", &here.join(", "))]))
    };
    let screen = Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3));
    painter.rect_stroke(screen.expand(3.0), CornerRadius::same(4), Stroke::new(2.0, ink), StrokeKind::Outside);
    chip(painter, at, text, ink);
}

/// The screen point of page point `(x, y)`, for tests.
#[cfg(test)]
pub(crate) fn screen_of(app: &FigApp, x: f64, y: f64) -> Pos2 {
    app.to_screen(x, y)
}
