//! Pointing at a Use Case diagram on the canvas. Its drawn layers are locked (they are made from
//! the model), so the canvas can't pick them; instead, with the Move tool:
//! - a press on an actor, use case or system selects it, and a drag moves it
//!   (`diagram.useCase.move` when the button comes up: one undo step);
//! - a press near a relationship selects it;
//! - with an Add tool, a press places a new element there;
//! - with a Connect tool, a press on one element and then another joins them.
//!
//! Delete removes the selection, Escape drops the tool and then the selection. Presses on empty
//! canvas, the wheel and pinch go on to the canvas as usual.

use egui::{Color32, Context, CornerRadius, Key, Modifiers, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use figcraft_doc::use_case::{GenEnds, Link, Model};
use figcraft_engine::ToolId;
use figcraft_geom::Point;
use serde_json::{Value, json};

use super::{KINDS, Place, State, Tool, current, load, save};
use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular};

/// A press moves an element only once the pointer has gone this far (screen points).
const DRAG_SLOP: f32 = 4.0;
/// A press this close to a relationship's line picks it (screen points).
const LINK_REACH: f32 = 6.0;
/// A subject's smallest size (page units), as the model allows.
const SUBJECT_MIN: (f64, f64) = (80.0, 80.0);

/// A drag of an element: where it started on the page and its position then.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    id: u64,
    press: Pos2,
    origin: (f64, f64),
}

fn drag_id() -> egui::Id {
    egui::Id::unique("use_case_canvas_drag")
}

/// The canvas acts on the diagram: a Use Case page in view, the Move tool, and the file editable.
pub(super) fn active(app: &FigApp) -> Option<std::sync::Arc<Model>> {
    if app.session.tool.tool != ToolId::Move || app.session.tool.busy() || app.session.active_read_only().is_some() {
        return None;
    }
    current(app)
}

/// An element's box on the page: (x, y, w, h).
pub fn element_box(m: &Model, id: u64) -> Option<(f64, f64, f64, f64)> {
    m.actors
        .iter()
        .find(|a| a.id.0 == id)
        .map(|a| (a.x, a.y, a.w, a.h))
        .or_else(|| m.use_cases.iter().find(|u| u.id.0 == id).map(|u| (u.x, u.y, u.w, u.h)))
        .or_else(|| m.subjects.iter().find(|s| s.id.0 == id).map(|s| (s.x, s.y, s.width, s.height)))
}

/// The box and minimum size of element `id` when it can be resized: a use case (never below what
/// its name and extension points need) or a subject. An actor keeps its figure's size.
pub fn resizable(m: &Model, id: u64) -> Option<(crate::diagram_resize::PageBox, (f64, f64))> {
    if let Some(u) = m.use_cases.iter().find(|u| u.id.0 == id) {
        return Some(((u.x, u.y, u.w, u.h), (u.min_w, u.min_h)));
    }
    m.subjects.iter().find(|s| s.id.0 == id).map(|s| ((s.x, s.y, s.width, s.height), SUBJECT_MIN))
}

/// The element at page point `q`, topmost first as drawn: actors, then use cases (inside the
/// ellipse), then systems.
pub fn element_at(m: &Model, q: Point) -> Option<u64> {
    let inside = |x: f64, y: f64, w: f64, h: f64| q.x >= x && q.x <= x + w && q.y >= y && q.y <= y + h;
    if let Some(a) = m.actors.iter().rev().find(|a| inside(a.x, a.y, a.w, a.h)) {
        return Some(a.id.0);
    }
    let in_ellipse = |x: f64, y: f64, w: f64, h: f64| {
        let (rx, ry) = (w / 2.0, h / 2.0);
        rx > 0.0 && ry > 0.0 && ((q.x - x - rx) / rx).powi(2) + ((q.y - y - ry) / ry).powi(2) <= 1.0
    };
    if let Some(u) = m.use_cases.iter().rev().find(|u| in_ellipse(u.x, u.y, u.w, u.h)) {
        return Some(u.id.0);
    }
    m.subjects.iter().rev().find(|s| inside(s.x, s.y, s.width, s.height)).map(|s| s.id.0)
}

/// A relationship's two ends' IDs.
pub fn link_ends(l: &Link) -> (u64, u64) {
    match l {
        Link::Association { actor, use_case, .. } => (actor.0, use_case.0),
        Link::Include { including, included, .. } => (including.0, included.0),
        Link::Extend { extending, base, .. } => (extending.0, base.0),
        Link::Generalization { ends: GenEnds::Actors { specific, general }, .. } => (specific.0, general.0),
        Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => (specific.0, general.0),
    }
}

fn center(b: (f64, f64, f64, f64)) -> Point {
    Point::new(b.0 + b.2 / 2.0, b.1 + b.3 / 2.0)
}

/// The relationship whose line passes within `reach` (page units) of `q`.
fn link_at(m: &Model, q: Point, reach: f64) -> Option<u64> {
    m.links.iter().rev().find_map(|l| {
        let (a, b) = link_ends(l);
        let (a, b) = (center(element_box(m, a)?), center(element_box(m, b)?));
        let d = b - a;
        let len2 = d.hypot2();
        let t = if len2 > 0.0 { ((q - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
        (((a + d * t) - q).hypot() <= reach).then_some(l.id().0)
    })
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
        if crate::diagram_resize::active(&ctx).is_some_and(|r| current(app).is_some_and(|m| resizable(&m, r.id).is_some())) {
            crate::diagram_resize::cancel(&ctx);
        }
        return false;
    };
    // A resize under way from a square or an edge.
    if crate::diagram_resize::active(&ctx).is_some_and(|r| resizable(&model, r.id).is_some()) {
        if let Some(Some((id, b))) = crate::diagram_resize::follow(app, &ctx) {
            app.run("diagram.useCase.resize", crate::diagram_resize::params(id, b));
        }
        return true;
    }
    let (pos, pressed, down) =
        ctx.input(|i| (i.pointer.hover_pos(), i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.primary_down()));
    if let Some(d) = drag {
        if down {
            ctx.request_repaint();
            return true;
        }
        ctx.data_mut(|m| m.remove::<Drag>(drag_id()));
        let end = pos.unwrap_or(d.press);
        if (end - d.press).length() >= DRAG_SLOP {
            let delta = app.to_page(end) - app.to_page(d.press);
            let (dx, dy) = (delta.x, delta.y);
            app.run("diagram.useCase.move", json!({"id": d.id, "x": (d.origin.0 + dx).round(), "y": (d.origin.1 + dy).round()}));
        }
        return true;
    }
    let Some(pos) = pos.filter(|p| pressed && resp.hovered() && app.canvas.rect.contains(*p)) else { return false };
    let q = app.to_page(pos);
    let mut st = load(&ctx, app);
    // A square or an edge of the selected subject or use case starts a resize.
    if st.tool == Tool::Select
        && let Some(id) = st.selected
        && let Some((b, min)) = resizable(&model, id)
        && crate::diagram_resize::press(&ctx, app, id, b, min, pos)
    {
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

/// A press with the select tool: an element (and the start of a drag), a relationship, or
/// nothing (the press goes on to the canvas).
fn select(app: &FigApp, ctx: &Context, m: &Model, st: &mut State, pos: Pos2, q: Point) -> bool {
    if let Some(id) = element_at(m, q) {
        st.selected = Some(id);
        if let Some((x, y, ..)) = element_box(m, id) {
            ctx.data_mut(|d| d.insert_temp(drag_id(), Drag { id, press: pos, origin: (x, y) }));
        }
        return true;
    }
    let zoom = app.session.ds().map(|d| d.view.zoom).unwrap_or(1.0).max(1e-6);
    if let Some(id) = link_at(m, q, f64::from(LINK_REACH) / zoom) {
        st.selected = Some(id);
        return true;
    }
    st.selected = None;
    false
}

/// The next default name for `kind`: "Actor 3" when "Actor 1" and "Actor 2" are taken.
pub(super) fn default_name(m: &Model, kind: Place) -> String {
    let base = crate::i18n::tr(kind.label());
    let taken = |n: &str| m.actors.iter().any(|a| a.name == n) || m.use_cases.iter().any(|u| u.name == n) || m.subjects.iter().any(|s| s.name == n);
    (1..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
}

/// Places a new `kind` at page point `q` (centred on it), inside the system there for a use case;
/// where it doesn't fit it goes to the nearest free room instead.
fn place(app: &mut FigApp, m: &Model, st: &mut State, kind: Place, q: Point) {
    let name = default_name(m, kind);
    // Centred using the size the engine will give it (it depends on the name).
    let half = |actor| {
        let (w, h) = figcraft_engine::use_case_new_element_size(actor, &name);
        (w / 2.0, h / 2.0)
    };
    let (cmd, half, mut p) = match kind {
        Place::Actor => ("diagram.useCase.addActor", half(true), json!({"name": name})),
        Place::UseCase => {
            let subject = m.subjects.iter().find(|s| q.x >= s.x && q.x <= s.x + s.width && q.y >= s.y && q.y <= s.y + s.height).map(|s| s.id.0);
            ("diagram.useCase.addUseCase", half(false), json!({"name": name, "subject": subject}))
        }
        Place::System => ("diagram.useCase.addSubject", (0.0, 0.0), json!({"name": name})),
    };
    let mut at = p.clone();
    at["x"] = json!((q.x - half.0).round());
    at["y"] = json!((q.y - half.1).round());
    // Quietly: a spot that doesn't fit isn't an error, the element goes to free room nearby.
    let made = app.session.execute(cmd, &at).ok().or_else(|| {
        if let Some(o) = p.as_object_mut() {
            o.remove("x");
            o.remove("y");
        }
        app.run(cmd, p)
    });
    app.handle_ui_requests();
    if let Some(id) = made.as_ref().and_then(|r| r.get("id")).and_then(Value::as_u64) {
        st.selected = Some(id);
    }
    st.tool = Tool::Select;
}

/// A press with a Connect tool: the first element starts the relationship, the second ends it.
fn connect(app: &mut FigApp, m: &Model, st: &mut State, kind: usize, from: Option<u64>, q: Point) {
    let Some(at) = element_at(m, q).filter(|id| !m.subjects.iter().any(|s| s.id.0 == *id)) else {
        return;
    };
    let Some(from) = from.filter(|f| *f != at) else {
        st.tool = Tool::Connect { kind, from: Some(at) };
        st.selected = Some(at);
        return;
    };
    let Some((k, ..)) = KINDS.get(kind) else { return };
    if let Some(id) = join(app, m, k, from, at) {
        st.selected = Some(id);
    }
    // Ready for the next one of the same kind.
    st.tool = Tool::Connect { kind, from: None };
}

/// Joins `from` to `to` with relationship `kind` (`association`, `include`, `extend` or
/// `generalization`), one undo step. An extend uses the base's first extension point, or a new
/// one named after the extending use case. The new relationship's ID, or `None` when refused.
pub(super) fn join(app: &mut FigApp, m: &Model, kind: &str, from: u64, to: u64) -> Option<u64> {
    let p = join_params(m, kind, from, to);
    app.run("diagram.useCase.connect", p).as_ref().and_then(|r| r.get("id")).and_then(Value::as_u64)
}

/// The kinds that would really join `from` to `to`: the type rules, no duplicate or cycle, and
/// the engine's own check of the exact join `join` would send (an extend's new point can grow
/// its base past its system, say). When none would, why not.
pub(super) fn joins(m: &Model, from: u64, to: u64) -> (Vec<&'static str>, Option<String>) {
    let fits = figcraft_doc::use_case::connection_kinds(m, from, to);
    if fits.is_empty() {
        return (fits, Some(why_not(m, from, to)));
    }
    let mut refused = None;
    let kinds: Vec<&'static str> = fits
        .into_iter()
        .filter(|k| match figcraft_engine::use_case_check_connect(m, &join_params(m, k, from, to)) {
            Ok(()) => true,
            Err(e) => {
                refused.get_or_insert(e);
                false
            }
        })
        .collect();
    let reason = if kinds.is_empty() { refused } else { None };
    (kinds, reason)
}

/// `joins`, kept for the last model and pair asked about: the hover hints ask every frame, and
/// each answer clones and validates the model once per kind.
pub(super) fn joins_cached(ctx: &egui::Context, m: &Model, from: u64, to: u64) -> (Vec<&'static str>, Option<String>) {
    type Cached = (Model, u64, u64, Vec<&'static str>, Option<String>);
    let key = egui::Id::unique("use_case_joins");
    if let Some((cm, cf, ct, kinds, reason)) = ctx.data(|d| d.get_temp::<std::sync::Arc<Cached>>(key)).as_deref()
        && (*cf, *ct) == (from, to)
        && cm == m
    {
        return (kinds.clone(), reason.clone());
    }
    let (kinds, reason) = joins(m, from, to);
    ctx.data_mut(|d| d.insert_temp(key, std::sync::Arc::new((m.clone(), from, to, kinds.clone(), reason.clone()))));
    (kinds, reason)
}

/// The `diagram.useCase.connect` parameters for joining `from` to `to` by `kind`.
fn join_params(m: &Model, kind: &str, from: u64, to: u64) -> Value {
    let mut p = json!({"kind": kind, "from": from, "to": to});
    if kind == "extend" {
        let base = m.use_cases.iter().find(|u| u.id.0 == to);
        match base.and_then(|b| b.points.first()) {
            Some(point) => p["points"] = json!([point.id.0]),
            None => {
                // A case name may be longer than a point name can be: cut to the point's limit.
                let name = m.use_cases.iter().find(|u| u.id.0 == from).map_or("extension", |u| u.name.as_str());
                let name: String = name.chars().take(figcraft_doc::use_case::MAX_POINT_NAME).collect();
                p["newPoint"] = json!(name.trim_end());
            }
        }
    }
    p
}

/// A relationship kind's name in the interface (`include` â†’ "Include").
pub(super) fn kind_label(name: &str) -> String {
    KINDS.iter().find(|k| k.0 == name).map_or(name, |k| tr(k.1)).to_string()
}

/// A small label beside the pointer, outlined in `ink`: what a press or a drop there does.
pub(super) fn chip(painter: &Painter, at: Pos2, text: String, ink: Color32) {
    let t = Tokens::get(painter.ctx());
    let galley = painter.layout_no_wrap(text, regular(11.0), t.text);
    let r = Rect::from_min_size(at + vec2(14.0, 14.0), galley.size() + vec2(12.0, 6.0));
    painter.rect_filled(r, CornerRadius::same(6), t.panel);
    painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, ink), StrokeKind::Inside);
    painter.galley(r.min + vec2(6.0, 3.0), galley, t.text);
}

/// Delete removes the selection and Escape drops the tool, then the selection: taken before the
/// shortcuts while a Use Case page is in view and no field has the keyboard.
pub fn keys(app: &mut FigApp, ctx: &Context) {
    // An open "+" menu takes Escape, Enter and the arrows itself, before the canvas shortcuts.
    super::handles::take_menu_keys(ctx);
    if active(app).is_none() || crate::a11y::field_focused(ctx) || super::handles::chooser_open(ctx) {
        return;
    }
    let mut st = load(ctx, app);
    if st.tool != Tool::Select && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        st.tool = Tool::Select;
    } else if let Some(id) = st.selected {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            st.selected = None;
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace)) {
            app.run("diagram.useCase.remove", json!({"ids": [id]}));
            st.selected = None;
        }
    }
    save(ctx, st);
}

/// The selection's outline, the dragged element's ghost, the Connect tool's line and what a
/// press would pick, over the canvas.
pub fn draw(app: &FigApp, painter: &Painter) {
    let Some(m) = active(app) else { return };
    let ctx = painter.ctx().clone();
    let st = load(&ctx, app);
    let accent = crate::theme::canvas_accent(&ctx);
    let screen = |b: (f64, f64, f64, f64)| Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3));
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| app.canvas.rect.contains(*p));
    let hover = pointer.and_then(|p| element_at(&m, app.to_page(p)));
    // What a press picks: a faint outline.
    if let Some(h) = hover.filter(|h| Some(*h) != st.selected)
        && let Some(b) = element_box(&m, h)
    {
        painter.rect_stroke(screen(b).expand(3.0), CornerRadius::same(4), Stroke::new(1.0, accent.gamma_multiply(0.6)), StrokeKind::Outside);
    }
    if let Some(sel) = st.selected {
        if let Some(b) = element_box(&m, sel) {
            painter.rect_stroke(screen(b).expand(3.0), CornerRadius::same(4), Stroke::new(2.0, accent), StrokeKind::Outside);
        } else if let Some(l) = m.links.iter().find(|l| l.id().0 == sel) {
            let (a, b) = link_ends(l);
            if let (Some(a), Some(b)) = (element_box(&m, a), element_box(&m, b)) {
                let (a, b) = (center(a), center(b));
                painter.line_segment([app.to_screen(a.x, a.y), app.to_screen(b.x, b.y)], Stroke::new(4.0, accent.gamma_multiply(0.5)));
            }
        }
    }
    let drag: Option<Drag> = ctx.data(|d| d.get_temp(drag_id()));
    if let (Some(d), Some(p)) = (drag, pointer)
        && (p - d.press).length() >= DRAG_SLOP
        && let Some(b) = element_box(&m, d.id)
    {
        let ghost = screen(b).translate(p - d.press);
        painter.rect_filled(ghost, CornerRadius::same(4), accent.gamma_multiply(0.08));
        dashed_box(painter, ghost, accent);
    }
    if st.tool == Tool::Select
        && let Some((b, _)) = st.selected.and_then(|id| resizable(&m, id))
    {
        crate::diagram_resize::draw(app, painter, b, pointer);
    }
    match st.tool {
        Tool::Connect { kind, from: Some(f) } => {
            if let (Some(b), Some(p)) = (element_box(&m, f), pointer) {
                let c = center(b);
                let a = app.to_screen(c.x, c.y);
                painter.extend(Shape::dashed_line(&[a, p], Stroke::new(1.5, accent), 6.0, 4.0));
                painter.circle_filled(p, 3.0, accent);
            }
            if let (Some(h), Some(p)) = (hover.filter(|h| *h != f), pointer) {
                connection_hint(painter, app, &m, f, h, kind, p);
            }
        }
        Tool::Place(kind) => {
            if let Some(p) = pointer {
                let zoom = app.session.ds().map(|d| d.view.zoom).unwrap_or(1.0) as f32;
                let size = match kind {
                    Place::Actor => vec2(46.0, 102.0),
                    Place::UseCase => vec2(120.0, 56.0),
                    Place::System => vec2(420.0, 520.0),
                } * zoom;
                let at = if kind == Place::System { Rect::from_min_size(p, size) } else { Rect::from_center_size(p, size) };
                dashed_box(painter, at, accent);
            }
        }
        _ => {}
    }
}

/// While a Connect tool has its first element, the element under the pointer says what a press
/// there would make: outlined in the accent colour and labelled with the join when the tool makes
/// it, outlined in red and labelled with the joins that do apply when it doesn't.
fn connection_hint(painter: &Painter, app: &FigApp, m: &Model, from: u64, to: u64, kind: usize, at: Pos2) {
    let Some(b) = element_box(m, to) else { return };
    let ctx = painter.ctx().clone();
    let t = Tokens::get(&ctx);
    let (joins, reason) = joins_cached(&ctx, m, from, to);
    let label = kind_label;
    let chosen = KINDS.get(kind).map_or("", |k| k.0);
    let (ink, text) = if joins.contains(&chosen) {
        (crate::theme::canvas_accent(&ctx), trf("Joins as {kind}", &[("kind", &label(chosen))]))
    } else if joins.is_empty() {
        (t.danger, reason.unwrap_or_else(|| why_not(m, from, to)))
    } else {
        let here: Vec<String> = joins.iter().map(|j| label(j)).collect();
        (t.danger, trf("{tool} doesn't join these. Here: {kinds}", &[("tool", &label(chosen)), ("kinds", &here.join(", "))]))
    };
    let screen = Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3));
    painter.rect_stroke(screen.expand(3.0), CornerRadius::same(4), Stroke::new(2.0, ink), StrokeKind::Outside);
    chip(painter, at, text, ink);
}

/// Why nothing joins `from` to `to`: a link that's already there, a loop it would close, or
/// two elements no relationship joins.
pub(super) fn why_not(m: &Model, from: u64, to: u64) -> String {
    use figcraft_doc::use_case::{Blocked, connection_options};
    match connection_options(m, from, to).iter().find_map(|(_, b)| *b) {
        Some(Blocked::AlreadyJoined) => tr("These two are already joined").to_string(),
        Some(Blocked::Cycle) => tr("That would make a loop").to_string(),
        None => tr("Nothing can join these two").to_string(),
    }
}

fn dashed_box(p: &Painter, r: Rect, ink: Color32) {
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&corners, Stroke::new(1.5, ink), 6.0, 4.0));
}

/// The screen point of page point `(x, y)`, for tests.
#[cfg(test)]
pub(crate) fn screen_of(app: &FigApp, x: f64, y: f64) -> Pos2 {
    app.to_screen(x, y)
}
