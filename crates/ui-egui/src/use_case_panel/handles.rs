//! Use Case diagrams: a "+" beside each side of the one selected actor or use case, as on boards
//! (`crate::canvas::board_connect`). Hovering one shows where a connected element goes; clicking
//! it makes one there, joined to the selection in one undo step (`diagram.useCase.addConnected`):
//! a use case for an actor (an association), an included use case for a use case. Dragging one
//! carries a line, and the element under the pointer says what the join would be. Let go on another
//! actor or use case and the two are joined, or a menu offers the joins when more than one applies;
//! let go on empty canvas and a menu offers what to make there; let go over a panel, nothing.

use egui::{Area, Color32, Context, CornerRadius, Id, Key, LayerId, Order, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_doc::use_case::Model;
use serde_json::{Value, json};

use super::canvas::{active, chip, default_name, element_at, element_box, join, joins, joins_cached, kind_label};
use super::{Place, Tool, here, load, save};
use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::theme::Tokens;

/// How far each "+" sits outside the element, from its edge to its centre (screen points).
const REACH: f32 = 28.0;
const RADIUS: f32 = 9.0;
const SIDES: [&str; 4] = ["above", "right", "below", "left"];
/// The room between an element and the one a "+" makes beside it (page units).
const GAP: f64 = 60.0;
/// The size of a new actor and of a new use case (page units), as the Add tools place them.
const ACTOR: (f64, f64) = (46.0, 102.0);
const USE_CASE: (f64, f64) = (120.0, 56.0);

/// A drag from a "+": the element and the side it started from, while the button is down.
#[derive(Clone)]
struct Drag {
    from: u64,
    side: &'static str,
}

fn drag_id() -> Id {
    Id::unique("use_case_handle_drag")
}

/// A row of the menu.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Choice {
    /// Join to the element `to` with relationship `kind`.
    Join { kind: &'static str, to: u64 },
    /// Make a new element joined by `addConnected`'s `relation`.
    Make { relation: &'static str },
}

/// The menu open where a "+" was let go: the file and page it is on, the element it came from
/// (and whether that is an actor), the drop point on the page, its rows and the keyboard's row.
#[derive(Clone)]
struct Chooser {
    at: (u64, NodeId),
    from: u64,
    actor: bool,
    drop: (f64, f64),
    choices: Vec<Choice>,
    highlight: usize,
}

fn chooser_id() -> Id {
    Id::unique("use_case_handle_chooser")
}

/// The menu is open: Escape and the arrows are its own, not the canvas's.
pub(crate) fn chooser_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Chooser>(chooser_id())).is_some()
}

fn is_actor(m: &Model, id: u64) -> bool {
    m.actors.iter().any(|a| a.id.0 == id)
}

fn is_case(m: &Model, id: u64) -> bool {
    m.use_cases.iter().any(|u| u.id.0 == id)
}

/// What an element can make with a "+", in menu order: an actor makes a use case it is associated
/// with, or a more general or more specific actor; a use case makes one it includes, one that
/// extends it, or a more general or more specific use case. The first is what a click makes.
fn makes(actor: bool) -> &'static [&'static str] {
    if actor { &["association", "parent", "child"] } else { &["include", "extension", "parent", "child"] }
}

/// The size of the element `relation` makes from an actor (`actor`) or a use case.
fn made_size(actor: bool, relation: &str) -> (f64, f64) {
    if actor && relation != "association" { ACTOR } else { USE_CASE }
}

/// A row's text. Literal keys, so the catalog check sees each one.
fn choice_label(c: Choice, actor: bool) -> String {
    match c {
        Choice::Join { kind, .. } => trf("Join as {kind}", &[("kind", &kind_label(kind))]),
        Choice::Make { relation } => match (relation, actor) {
            ("association", _) => tr("New use case, associated"),
            ("include", _) => tr("New use case, included"),
            ("extension", _) => tr("New use case that extends this one"),
            ("parent", true) => tr("New general actor"),
            ("child", true) => tr("New specific actor"),
            ("parent", false) => tr("New general use case"),
            _ => tr("New specific use case"),
        }
        .to_string(),
    }
}

/// The top left of an element of size `next` beside `side` of the page box `b`, `GAP` away and
/// centred on that side.
fn beside(b: (f64, f64, f64, f64), next: (f64, f64), side: &str) -> (f64, f64) {
    match side {
        "above" => (b.0 + b.2 / 2.0 - next.0 / 2.0, b.1 - GAP - next.1),
        "right" => (b.0 + b.2 + GAP, b.1 + b.3 / 2.0 - next.1 / 2.0),
        "below" => (b.0 + b.2 / 2.0 - next.0 / 2.0, b.1 + b.3 + GAP),
        _ => (b.0 - GAP - next.0, b.1 + b.3 / 2.0 - next.1 / 2.0),
    }
}

/// Makes the element `relation` names from `from`, with its top left at `top_left` when it fits
/// there and at one of a few spots around it when it doesn't. The new element's ID, or `None`
/// (with the engine's reason shown) when none of them fits.
fn make(app: &mut FigApp, m: &Model, from: u64, relation: &str, top_left: (f64, f64)) -> Option<u64> {
    let kind = if is_actor(m, from) && relation != "association" { Place::Actor } else { Place::UseCase };
    let name = default_name(m, kind);
    let mut p = json!({"source": from, "relation": relation, "name": name});
    if relation == "extension" {
        // An extend needs a place in its base: a new extension point named after the new case.
        p["newPoint"] = json!(name);
    }
    if kind == Place::UseCase {
        // A new use case belongs to the system it lands in, and to none when it lands outside
        // every system (otherwise the engine puts it in the source's system, far from the drop).
        let (cx, cy) = (top_left.0 + USE_CASE.0 / 2.0, top_left.1 + USE_CASE.1 / 2.0);
        let inside = m.subjects.iter().find(|s| cx >= s.x && cx <= s.x + s.width && cy >= s.y && cy <= s.y + s.height);
        p["subject"] = inside.map_or(Value::Null, |s| json!(s.id.0));
    }
    // The spot asked for, then eight around it one element's size away: a bounded handful of
    // tries, so a crowded diagram never turns a click into a long search. The first that fits
    // wins; the others are quiet, and only the last refusal is reported.
    let size = if kind == Place::Actor { ACTOR } else { USE_CASE };
    let (dx, dy) = (size.0 + GAP / 2.0, size.1 + GAP / 2.0);
    let offsets = [(0.0, 0.0), (0.0, dy), (0.0, -dy), (dx, 0.0), (-dx, 0.0), (dx, dy), (-dx, dy), (dx, -dy), (-dx, -dy)];
    let at = |(ox, oy): (f64, f64)| {
        let mut q = p.clone();
        q["x"] = json!((top_left.0 + ox).round());
        q["y"] = json!((top_left.1 + oy).round());
        q
    };
    let cmd = "diagram.useCase.addConnected";
    let mut first_error = None;
    let mut made = None;
    for o in offsets {
        match app.session.execute(cmd, &at(o)) {
            Ok(r) => {
                made = Some(r);
                break;
            }
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    // Nothing fitted: the drop spot's own refusal is the one shown, without running it again.
    if let (None, Some(e)) = (&made, &first_error) {
        app.failed(cmd, e);
    }
    app.handle_ui_requests();
    made.as_ref().and_then(|r| r.get("element")).and_then(Value::as_u64)
}

/// The screen box of the page box `b`.
fn screen(app: &FigApp, b: (f64, f64, f64, f64)) -> Rect {
    Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3))
}

/// The centre of the "+" for `side` of the screen box `r`.
fn handle_at(r: Rect, side: &str) -> Pos2 {
    match side {
        "above" => pos2(r.center().x, r.min.y - REACH),
        "right" => pos2(r.max.x + REACH, r.center().y),
        "below" => pos2(r.center().x, r.max.y + REACH),
        _ => pos2(r.min.x - REACH, r.center().y),
    }
}

/// The middle of `side` of the screen box `r`, where a line leaves.
fn side_middle(r: Rect, side: &str) -> Pos2 {
    match side {
        "above" => r.center_top(),
        "right" => r.right_center(),
        "below" => r.center_bottom(),
        _ => r.left_center(),
    }
}

/// The side of `to` facing `from`, where a line arrives.
fn facing(from: Rect, to: Rect) -> Pos2 {
    let d = to.center() - from.center();
    if d.x.abs() >= d.y.abs() {
        if d.x >= 0.0 { to.left_center() } else { to.right_center() }
    } else if d.y >= 0.0 {
        to.center_top()
    } else {
        to.center_bottom()
    }
}

fn preview_painter(ctx: &Context) -> egui::Painter {
    ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("use_case_handle_preview")))
}

/// A new element's box, faintly: a tint and a dashed outline.
fn ghost_box(p: &egui::Painter, ghost: Rect, accent: Color32) {
    let faint = Stroke::new(1.5, accent.gamma_multiply(0.6));
    p.rect_filled(ghost, CornerRadius::same(8), accent.gamma_multiply(0.08));
    let corners = [ghost.left_top(), ghost.right_top(), ghost.right_bottom(), ghost.left_bottom(), ghost.left_top()];
    p.extend(Shape::dashed_line(&corners, faint, 6.0, 4.0));
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    if chooser_open(ctx) {
        show_chooser(app, ctx);
        return;
    }
    let drag: Option<Drag> = ctx.data(|d| d.get_temp(drag_id()));
    let forget = |ctx: &Context| ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
    let Some(m) = active(app) else {
        if drag.is_some() {
            forget(ctx);
        }
        return;
    };
    let mut st = load(ctx, app);
    let target = (st.tool == Tool::Select)
        .then_some(st.selected)
        .flatten()
        .filter(|id| is_actor(&m, *id) || is_case(&m, *id))
        .and_then(|id| Some((id, element_box(&m, id)?)));
    let Some((id, b)) = target else {
        // The selection or the tool changed under a drag: it is over.
        if drag.is_some() {
            forget(ctx);
        }
        return;
    };
    let actor = is_actor(&m, id);
    let r = screen(app, b);
    let t = Tokens::get(ctx);
    let accent = crate::theme::canvas_accent(ctx);
    // Not over the toolbar at the bottom of the canvas.
    let toolbar = crate::toolbar::pill_rect(app.canvas.rect, crate::toolbar::bar_width(app)).expand(RADIUS);
    let first = makes(actor).first().copied().unwrap_or("association");
    let next = made_size(actor, first);
    let (mut picked, mut started, mut let_go) = (None, None, false);
    for side in SIDES {
        let c = handle_at(r, side);
        if !app.canvas.rect.shrink(RADIUS).contains(c) || toolbar.contains(c) {
            continue;
        }
        let area = Area::new(Id::unique(("use_case_handle", side))).fixed_pos(c - vec2(RADIUS, RADIUS)).order(Order::Middle);
        let (clicked, drag_started, drag_stopped) = area
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(2.0 * RADIUS, 2.0 * RADIUS), Sense::click_and_drag());
                let hot = resp.hovered() || resp.dragged();
                let p = ui.painter();
                p.circle_filled(rect.center(), RADIUS, if hot { accent } else { t.panel });
                p.circle_stroke(rect.center(), RADIUS, Stroke::new(1.0, if hot { accent } else { t.border }));
                let ink = Stroke::new(1.5, if hot { Color32::WHITE } else { t.text_secondary });
                let arm = RADIUS * 0.45;
                p.line_segment([rect.center() - vec2(arm, 0.0), rect.center() + vec2(arm, 0.0)], ink);
                p.line_segment([rect.center() - vec2(0.0, arm), rect.center() + vec2(0.0, arm)], ink);
                // Where a click puts the next one; a drag shows its own preview.
                if hot && drag.is_none() && !resp.dragged() {
                    let ghost = screen(app, {
                        let (x, y) = beside(b, next, side);
                        (x, y, next.0, next.1)
                    });
                    let painter = preview_painter(ctx);
                    ghost_box(&painter, ghost, accent);
                    painter.line_segment([side_middle(r, side), facing(r, ghost)], Stroke::new(1.5, accent.gamma_multiply(0.6)));
                }
                let resp = resp.on_hover_text(tr("Click to add a connected element, or drag to an element to connect"));
                (resp.clicked(), resp.drag_started(), resp.drag_stopped())
            })
            .inner;
        if clicked {
            picked = Some(side);
        }
        if drag_started {
            started = Some(side);
        }
        let_go |= drag_stopped;
    }
    if let Some(side) = picked
        && let Some(new) = make(app, &m, id, first, beside(b, next, side))
    {
        st.selected = Some(new);
    }
    if let Some(side) = started {
        ctx.data_mut(|d| d.insert_temp(drag_id(), Drag { from: id, side }));
    }
    if let Some(Drag { from, side }) = drag {
        dragged(app, ctx, &m, &mut st, (from, side, id), (r, actor), let_go, toolbar);
    }
    save(ctx, st);
}

/// A "+" being dragged: the line and what a drop would do, then the drop itself.
#[allow(clippy::too_many_arguments)]
fn dragged(
    app: &mut FigApp,
    ctx: &Context,
    m: &Model,
    st: &mut super::State,
    (from, side, selected): (u64, &'static str, u64),
    (r, actor): (Rect, bool),
    let_go: bool,
    toolbar: Rect,
) {
    let forget = |ctx: &Context| ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
    // The selection moved to another element under the drag: it is over.
    if from != selected {
        forget(ctx);
        return;
    }
    // Where the button came up when it did: a touch backend can send the release and then
    // `PointerGone` in one frame, which leaves no latest position.
    let released_at = ctx.input(|i| {
        i.events.iter().rev().find_map(|e| match e {
            egui::Event::PointerButton { pos, pressed: false, .. } => Some(*pos),
            _ => None,
        })
    });
    let Some(pointer) = released_at.filter(|_| let_go).or_else(|| ctx.input(|i| i.pointer.latest_pos())) else {
        if let_go || !ctx.input(|i| i.pointer.any_down()) {
            forget(ctx);
        }
        return;
    };
    // Back over the element it started from: letting go there cancels, as in the flowchart.
    let on_source = element_at(m, app.to_page(pointer)) == Some(from);
    // Over a panel or a window (the Use Case panel floats over the canvas) or the toolbar, there
    // is nothing to show and nothing to make.
    let over_window = ctx.layer_id_at(pointer).is_some_and(|l| l.order != Order::Background);
    let on_canvas = app.canvas.rect.contains(pointer) && !toolbar.contains(pointer) && !over_window;
    let over = element_at(m, app.to_page(pointer)).filter(|o| *o != from && (is_actor(m, *o) || is_case(m, *o)));
    if on_canvas && !on_source {
        drag_preview(app, ctx, m, (from, side), r, actor, pointer, over);
    }
    if !let_go {
        if !ctx.input(|i| i.pointer.any_down()) {
            // The button came up without the handle hearing of it (the pointer left the window).
            forget(ctx);
        }
        return;
    }
    forget(ctx);
    if !on_canvas || on_source {
        return;
    }
    let q = app.to_page(pointer);
    let choices: Vec<Choice> = match over {
        Some(to) => joins_cached(ctx, m, from, to).0.into_iter().map(|kind| Choice::Join { kind, to }).collect(),
        None => makes(actor).iter().map(|relation| Choice::Make { relation }).collect(),
    };
    match (over, choices.as_slice()) {
        // Nothing joins the two: the drag already said so, and nothing happens.
        (_, []) => {}
        // One join only: made at once.
        (Some(to), [Choice::Join { kind, .. }]) => {
            if let Some(l) = join(app, m, kind, from, to) {
                st.selected = Some(l);
            }
        }
        _ => {
            if let Some(at) = here(app) {
                ctx.data_mut(|d| d.insert_temp(chooser_id(), Chooser { at, from, actor, drop: (q.x, q.y), choices, highlight: 0 }));
            }
        }
    }
}

/// The dragged line from `side` of the element, and what letting go would do: over an element,
/// it is outlined and labelled with the join (red when nothing joins the two); over empty canvas,
/// the next element's box follows the pointer.
#[allow(clippy::too_many_arguments)]
fn drag_preview(app: &FigApp, ctx: &Context, m: &Model, (from, side): (u64, &str), r: Rect, actor: bool, pointer: Pos2, over: Option<u64>) {
    let accent = crate::theme::canvas_accent(ctx);
    let danger = Tokens::get(ctx).danger;
    let p = preview_painter(ctx);
    let (to_box, ink, text) = match over.and_then(|o| Some((o, element_box(m, o)?))) {
        Some((o, ob)) => {
            let (kinds, reason) = joins_cached(ctx, m, from, o);
            let names: Vec<String> = kinds.iter().map(|k| kind_label(k)).collect();
            let (ink, text) = match kinds.as_slice() {
                [] => (danger, reason.clone().unwrap_or_default()),
                [k] => (accent, trf("Joins as {kind}", &[("kind", &kind_label(k))])),
                _ => (accent, trf("Let go to choose: {kinds}", &[("kinds", &names.join(", "))])),
            };
            let ob = screen(app, ob);
            p.rect_stroke(ob.expand(3.0), CornerRadius::same(4), Stroke::new(2.0, ink), StrokeKind::Outside);
            (ob, ink, text)
        }
        None => {
            let next = made_size(actor, makes(actor).first().copied().unwrap_or("association"));
            let size = app.to_screen(next.0, next.1) - app.to_screen(0.0, 0.0);
            let ghost = Rect::from_center_size(pointer, size);
            ghost_box(&p, ghost, accent);
            (ghost, accent, tr("Let go to choose what to add").to_string())
        }
    };
    p.line_segment([side_middle(r, side), facing(r, to_box)], Stroke::new(1.5, ink.gamma_multiply(0.8)));
    chip(&p, pointer, text, ink);
}

/// What a key or a click does to the open menu.
enum Step {
    Stay,
    Close,
    Choose(usize),
}

/// The keys the menu takes while it is open, before the canvas's.
const CHOOSER_KEYS: [Key; 4] = [Key::ArrowDown, Key::ArrowUp, Key::Enter, Key::Escape];

/// This frame's presses of the menu's keys (no modifier held), taken out of the input in order.
fn take_keys(ctx: &Context) -> Vec<Key> {
    ctx.input_mut(|i| {
        let mut keys = Vec::new();
        i.events.retain(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.is_none() && CHOOSER_KEYS.contains(key) => {
                keys.push(*key);
                false
            }
            _ => true,
        });
        keys
    })
}

fn pending_id() -> Id {
    Id::unique("use_case_handle_keys")
}

/// Before the canvas shortcuts each frame: while the menu is open its keys are taken out of the
/// input and kept for it, so the arrows don't nudge or scroll the canvas and Escape doesn't drop
/// the selection. The first pass only (egui reruns a discarded pass with the same input).
pub(crate) fn take_menu_keys(ctx: &Context) {
    if !chooser_open(ctx) || ctx.current_pass_index() != 0 || crate::a11y::field_focused(ctx) {
        return;
    }
    let keys = take_keys(ctx);
    if !keys.is_empty() {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Key>>(pending_id()).extend(keys));
    }
}

/// Draw the menu and act on it: a row's click or Enter does that row, Escape or a press elsewhere
/// does nothing. The highlighted row's new element shows faintly where it will land.
fn show_chooser(app: &mut FigApp, ctx: &Context) {
    let Some(mut c) = ctx.data(|d| d.get_temp::<Chooser>(chooser_id())) else { return };
    // Closing also drops any key kept for it, so a stale Enter can't choose in the next menu.
    let close = |ctx: &Context| {
        ctx.data_mut(|d| {
            d.remove::<Chooser>(chooser_id());
            d.remove::<Vec<Key>>(pending_id());
        });
    };
    // Another tab or page, or the source gone: the menu is over.
    let Some(m) = active(app).filter(|m| here(app) == Some(c.at) && element_box(m, c.from).is_some()) else {
        close(ctx);
        return;
    };
    let mut step = Step::Stay;
    // The keys `take_menu_keys` kept this frame, in the order they came, up to the first Enter or
    // Escape: Down, Enter in one frame is the second row, and Escape then Enter does nothing.
    let keys = ctx.data_mut(|d| d.remove_temp::<Vec<Key>>(pending_id())).unwrap_or_default();
    for k in keys {
        match k {
            Key::ArrowDown => c.highlight = (c.highlight + 1).min(c.choices.len().saturating_sub(1)),
            Key::ArrowUp => c.highlight = c.highlight.saturating_sub(1),
            Key::Enter => {
                step = Step::Choose(c.highlight);
                break;
            }
            _ => {
                step = Step::Close;
                break;
            }
        }
    }
    let at = app.to_screen(c.drop.0, c.drop.1);
    let accent = crate::theme::canvas_accent(ctx);
    let mut beside_at = at + vec2(12.0, 12.0);
    match c.choices.get(c.highlight).copied() {
        Some(Choice::Make { relation }) => {
            let next = made_size(c.actor, relation);
            let size = app.to_screen(next.0, next.1) - app.to_screen(0.0, 0.0);
            let ghost = Rect::from_center_size(at, size);
            ghost_box(&preview_painter(ctx), ghost, accent);
            beside_at = ghost.right_top() + vec2(8.0, 0.0);
        }
        Some(Choice::Join { to, .. }) => {
            if let Some(ob) = element_box(&m, to) {
                preview_painter(ctx).rect_stroke(screen(app, ob).expand(3.0), CornerRadius::same(4), Stroke::new(2.0, accent), StrokeKind::Outside);
            }
        }
        None => {}
    }
    let (highlight, actor) = (c.highlight, c.actor);
    let choices = c.choices.clone();
    let response = Area::new(Id::unique("use_case_handle_chooser_area")).fixed_pos(beside_at).order(Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            let mut clicked = None;
            for (i, choice) in choices.iter().enumerate() {
                if ui.selectable_label(i == highlight, choice_label(*choice, actor)).clicked() {
                    clicked = Some(i);
                }
            }
            clicked
        })
    });
    // A key that came in the same frame (Escape, Enter) has the last word over a row's click.
    if let (Some(i), Step::Stay) = (response.inner.inner, &step) {
        step = Step::Choose(i);
    }
    // Where the press began, not where the menu is pinned.
    let pressed_outside =
        ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()).flatten()).is_some_and(|p| !response.response.rect.contains(p));
    if pressed_outside && matches!(step, Step::Stay) {
        step = Step::Close;
    }
    match step {
        Step::Stay => {
            ctx.data_mut(|d| d.insert_temp(chooser_id(), c));
        }
        Step::Close => close(ctx),
        Step::Choose(i) => {
            close(ctx);
            let Some(choice) = c.choices.get(i).copied() else { return };
            let made = match choice {
                // Checked again: the diagram may have changed while the menu was open.
                Choice::Join { kind, to } if joins(&m, c.from, to).0.contains(&kind) => join(app, &m, kind, c.from, to),
                Choice::Join { .. } => None,
                Choice::Make { relation } => {
                    let next = made_size(c.actor, relation);
                    make(app, &m, c.from, relation, (c.drop.0 - next.0 / 2.0, c.drop.1 - next.1 / 2.0))
                }
            };
            let mut st = load(ctx, app);
            if let Some(id) = made {
                st.selected = Some(id);
            }
            save(ctx, st);
        }
    }
}
