//! Boards: a "+" beside each side of the one selected sticky or shape (#1597). Hovering one shows
//! where the next one goes; clicking it makes the next of the same kind there, joined by a
//! connector, and starts typing in it (`board.addConnected`). Dragging one carries the preview
//! ("drag the preview to any location on your board", #1919): let go on empty board and the next
//! one is made there, joined by a connector leaving by the handle's side; let go on another sticky
//! or shape (inside a section too) and a connector joins the two; let go off the canvas or over the
//! toolbar, nothing. The bar above the selection stays away while a "+" is dragged, as for any drag.

use egui::{Area, Color32, CornerRadius, Id, Key, LayerId, Order, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, pos2, vec2};
use figcraft_doc::{BoardItem, BoardShape, NodeId};
use figcraft_engine::ToolId;
use figcraft_engine::board::{STICKY, connected_place, sticky_or_shape_at};
use serde_json::json;

use crate::FigApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// How far each "+" sits outside the object, from its edge to its centre (screen points): clear
/// of the selection's handles and of the size label under it.
pub(crate) const REACH: f32 = 36.0;
const RADIUS: f32 = 9.0;
const SIDES: [&str; 4] = ["above", "right", "below", "left"];

/// A drag from a "+": the object and the side it started from, kept while the button is down.
#[derive(Clone)]
struct Drag {
    from: NodeId,
    side: &'static str,
}

fn drag_id() -> Id {
    Id::unique("board_connect_drag")
}

/// The sticky or shape the handles are for: its id, its box on the page and the next one's size.
fn target(app: &FigApp) -> Option<(NodeId, figcraft_geom::Rect, (f64, f64))> {
    let editable = app.session.active_read_only().is_none();
    if app.session.tool.busy() || !editable || app.session.tool.tool != ToolId::Move || !app.session.doc().is_ok_and(figcraft_engine::board::is_board)
    {
        return None;
    }
    let ds = app.session.ds().ok()?;
    let ([id], None) = (ds.selection.as_slice(), &ds.text_edit) else { return None };
    let n = ds.doc.node(*id)?;
    let next = match n.board {
        Some(BoardItem::Sticky { .. }) => (STICKY, STICKY),
        Some(BoardItem::Shape { .. }) => (n.width, n.height),
        _ => return None,
    };
    Some((*id, ds.doc.world_bounds(*id)?, next))
}

/// Handles show for the selection: the bar above it leaves them room.
pub(crate) fn shown(app: &FigApp) -> bool {
    target(app).is_some()
}

/// A "+" is being dragged: the bar above the selection stays away meanwhile, as for any drag, so
/// nothing is made behind it.
pub(crate) fn dragging(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Drag>(drag_id())).is_some()
}

/// The centre of the "+" for `side` of the object's screen box `r`.
fn handle_at(r: Rect, side: &str) -> Pos2 {
    match side {
        "above" => pos2(r.center().x, r.min.y - REACH),
        "right" => pos2(r.max.x + REACH, r.center().y),
        "below" => pos2(r.center().x, r.max.y + REACH),
        _ => pos2(r.min.x - REACH, r.center().y),
    }
}

/// The middle of `side` of the object's screen box `r`, where its connector leaves.
fn side_middle(r: Rect, side: &str) -> Pos2 {
    match side {
        "above" => r.center_top(),
        "right" => r.right_center(),
        "below" => r.center_bottom(),
        _ => r.left_center(),
    }
}

/// A "+" side as `board.connector` names it.
fn connector_side(side: &str) -> &'static str {
    match side {
        "above" => "top",
        "below" => "bottom",
        "left" => "left",
        _ => "right",
    }
}

/// The shape each of the chooser's rows makes (`None`: the source's own), in row order.
const CHOICES: [Option<&str>; 4] = [None, Some("rectangle"), Some("diamond"), Some("roundedRectangle")];

/// A row's name in the interface. Literal keys, so the catalog check sees each one.
fn choice_label(row: usize) -> &'static str {
    match row {
        0 => tr("Same shape"),
        1 => tr("Process"),
        2 => tr("Decision"),
        _ => tr("Terminator"),
    }
}

/// The chooser open where a "+" was let go on empty board (`board.addConnected`'s `shape`): the
/// file and page it is on (node ids repeat from one file to another), the handle it came from,
/// where the next one goes on the page, and the row the keyboard is on.
#[derive(Clone)]
struct Chooser {
    uid: u64,
    page: NodeId,
    from: NodeId,
    side: &'static str,
    at: (f64, f64),
    highlight: usize,
}

fn chooser_id() -> Id {
    Id::unique("board_connect_chooser")
}

/// Whether `id` is a shape (not a sticky): only a shape's next one has a choice of shape.
fn is_shape(app: &FigApp, id: NodeId) -> bool {
    app.session.ds().ok().and_then(|ds| ds.doc.node(id)).is_some_and(|n| matches!(n.board, Some(BoardItem::Shape { .. })))
}

/// What a key or a click does to the open chooser.
enum Step {
    Stay,
    Close,
    Choose(usize),
}

/// The keys the chooser takes while it is open, before the canvas shortcuts (which would nudge
/// the source with the arrows and select its children with Enter).
const CHOOSER_KEYS: [Key; 4] = [Key::ArrowDown, Key::ArrowUp, Key::Enter, Key::Escape];

/// Take this frame's presses of the chooser's keys (no modifier held) out of the input, in the
/// order they came; the canvas shortcuts never see them.
fn take_keys(ctx: &egui::Context) -> Vec<Key> {
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

impl Chooser {
    /// Arrows move the row, Enter makes the choice, Escape leaves nothing made; `None` for a key
    /// that isn't the chooser's.
    fn press(&mut self, key: Key) -> Option<Step> {
        match key {
            Key::ArrowDown => {
                self.highlight = (self.highlight + 1).min(CHOICES.len() - 1);
                Some(Step::Stay)
            }
            Key::ArrowUp => {
                self.highlight = self.highlight.saturating_sub(1);
                Some(Step::Stay)
            }
            Key::Enter => Some(Step::Choose(self.highlight)),
            Key::Escape => Some(Step::Close),
            _ => None,
        }
    }
}

/// Which file and page are in front, to tell the chooser's own from another's.
fn here(app: &FigApp) -> Option<(u64, NodeId)> {
    app.session.ds().ok().map(|ds| (ds.uid, ds.page))
}

/// The chooser that is open, if it is on the file and page in front and its source is still the
/// one selected with handles; any other chooser is closed here (another tab, another page, the
/// source gone or deselected).
fn open_chooser(app: &FigApp, ctx: &egui::Context) -> Option<Chooser> {
    let c = ctx.data(|d| d.get_temp::<Chooser>(chooser_id()))?;
    if here(app) != Some((c.uid, c.page)) || target(app).is_none_or(|(id, _, _)| id != c.from) {
        ctx.data_mut(|d| d.remove::<Chooser>(chooser_id()));
        return None;
    }
    Some(c)
}

/// Keep, close or act on the chooser: a choice makes that shape at the drop point (one undo step,
/// typing started).
fn finish(app: &mut FigApp, ctx: &egui::Context, c: Chooser, step: Step) {
    match step {
        Step::Stay => {
            ctx.data_mut(|d| d.insert_temp(chooser_id(), c));
        }
        Step::Close => {
            ctx.data_mut(|d| d.remove::<Chooser>(chooser_id()));
        }
        Step::Choose(i) => {
            ctx.data_mut(|d| d.remove::<Chooser>(chooser_id()));
            let mut params = json!({"id": c.from, "side": c.side, "at": [c.at.0, c.at.1], "edit": true});
            if let Some(shape) = CHOICES.get(i).copied().flatten() {
                params["shape"] = json!(shape);
            }
            app.run("board.addConnected", params);
        }
    }
}

/// A key from the control channel's `ui.key`, which runs canvas bindings itself instead of going
/// through the window's input: the open chooser takes its keys first there too. True when it did.
pub(crate) fn key(app: &mut FigApp, ctx: &egui::Context, key: Key) -> bool {
    let Some(mut c) = open_chooser(app, ctx) else { return false };
    let Some(step) = c.press(key) else { return false };
    finish(app, ctx, c, step);
    true
}

/// The size the chooser's `row` makes from the shape `from` (`connected_size`, as the engine).
fn row_size(app: &FigApp, from: NodeId, row: usize) -> Option<(f64, f64)> {
    let n = app.session.ds().ok()?.doc.node(from)?;
    let Some(BoardItem::Shape { shape }) = n.board else { return None };
    let choice = CHOICES.get(row).copied().flatten().and_then(BoardShape::from_id);
    Some(figcraft_engine::board::connected_size(shape, (n.width, n.height), choice))
}

/// Draw the chooser and act on it: a row's click or Enter makes that shape where the "+" was let
/// go, Escape or a press elsewhere makes nothing. The highlighted row's shape shows faintly where
/// it will land, at the size it will have. A text field with the keyboard keeps its keys.
fn show_chooser(app: &mut FigApp, ctx: &egui::Context) {
    let Some(mut c) = open_chooser(app, ctx) else { return };
    let mut step = Step::Stay;
    // The first pass already took this frame's keys (egui reruns a discarded pass with them).
    if ctx.current_pass_index() == 0 && !crate::a11y::field_focused(ctx) {
        // In the order they came, up to the first Enter or Escape: Down, Down, Enter in one frame
        // is a Decision, and Escape then Enter makes nothing.
        for k in take_keys(ctx) {
            if let Some(s @ (Step::Close | Step::Choose(_))) = c.press(k) {
                step = s;
                break;
            }
        }
    }
    // Pinned to the drop point on the page, so it goes with the board when the view moves; the
    // menu sits beside the next one's box rather than over it.
    let at = app.to_screen(c.at.0, c.at.1);
    let mut beside = at;
    if let Some(size) = row_size(app, c.from, c.highlight) {
        let px = app.to_screen(size.0, size.1) - app.to_screen(0.0, 0.0);
        let ghost = Rect::from_center_size(at, px);
        let p = ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("board_connect_preview")));
        ghost_box(&p, ghost, crate::theme::canvas_accent(ctx));
        beside = ghost.right_top() + vec2(8.0, 0.0);
    }
    let highlight = c.highlight;
    let response = Area::new(Id::unique("board_connect_chooser_area")).fixed_pos(beside).order(Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            let mut clicked = None;
            for i in 0..CHOICES.len() {
                if ui.selectable_label(i == highlight, choice_label(i)).clicked() {
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
    // Where the press began, not where the chooser is pinned (that is always inside it).
    let pressed_outside =
        ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()).flatten()).is_some_and(|p| !response.response.rect.contains(p));
    if pressed_outside && matches!(step, Step::Stay) {
        step = Step::Close;
    }
    finish(app, ctx, c, step);
}

pub fn show(app: &mut FigApp, ctx: &egui::Context) {
    if ctx.data(|d| d.get_temp::<Chooser>(chooser_id())).is_some() {
        show_chooser(app, ctx);
        return;
    }
    let drag: Option<Drag> = ctx.data(|d| d.get_temp(drag_id()));
    let Some((id, b, next)) = target(app) else {
        // The selection or the tool changed under a drag: it is over.
        if drag.is_some() {
            ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
        }
        return;
    };
    let r = Rect::from_two_pos(app.to_screen(b.x0, b.y0), app.to_screen(b.x1, b.y1));
    // Too small on screen to tell the handles from the object.
    if r.width() < 2.0 * RADIUS || r.height() < 2.0 * RADIUS {
        return;
    }
    let t = Tokens::get(ctx);
    let accent = crate::theme::canvas_accent(ctx);
    // Not over the toolbar at the bottom of the canvas.
    let toolbar = crate::toolbar::pill_rect(app.canvas.rect, crate::toolbar::bar_width(app)).expand(RADIUS);
    let mut picked = None;
    let mut started = None;
    let mut let_go = false;
    for side in SIDES {
        let c = handle_at(r, side);
        if !app.canvas.rect.shrink(RADIUS).contains(c) || toolbar.contains(c) {
            continue;
        }
        let area = Area::new(Id::unique(("board_connect", side))).fixed_pos(c - vec2(RADIUS, RADIUS)).order(Order::Middle);
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
                    preview(app, ctx, b, next, side, accent);
                }
                let resp = resp.on_hover_text(tr("Add a connected shape"));
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
    if let Some(side) = picked {
        app.run("board.addConnected", json!({"id": id, "side": side, "edit": true}));
    }
    if let Some(side) = started {
        ctx.data_mut(|d| d.insert_temp(drag_id(), Drag { from: id, side }));
    }
    // A drag carries the preview: the next one centred on the pointer, or the object under it,
    // which the connector joins instead.
    let Some(Drag { from, side }) = drag else { return };
    // The selection moved to another sticky or shape under the drag: it is over too.
    if from != id {
        ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
        return;
    }
    let Some(pointer) = ctx.input(|i| i.pointer.latest_pos()) else { return };
    let over = object_under(app, from, pointer);
    // Off the canvas (over a panel) or over the floating toolbar there is nothing to show and
    // nothing to make.
    let on_canvas = app.canvas.rect.contains(pointer) && !toolbar.contains(pointer);
    if on_canvas {
        drag_preview(app, ctx, (b, next, side), pointer, over);
    }
    if let_go {
        ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
        if !on_canvas {
            return;
        }
        match over {
            Some(to) => {
                app.run("board.connector", json!({"from": from, "to": to, "fromSide": connector_side(side)}));
            }
            None => {
                let q = app.to_page(pointer);
                if let (true, Some((uid, page))) = (is_shape(app, from), here(app)) {
                    // A shape's next one can be another shape: offer the choice where it lands.
                    ctx.data_mut(|d| d.insert_temp(chooser_id(), Chooser { uid, page, from, side, at: (q.x, q.y), highlight: 0 }));
                } else {
                    app.run("board.addConnected", json!({"id": from, "side": side, "at": [q.x, q.y], "edit": true}));
                }
            }
        }
    } else if !ctx.input(|i| i.pointer.any_down()) {
        // The button came up without the handle hearing of it (the pointer left the window).
        ctx.data_mut(|d| d.remove::<Drag>(drag_id()));
    }
}

/// The sticky or shape under the screen point `p` on the canvas (inside a section or frame too),
/// other than `except`.
fn object_under(app: &FigApp, except: NodeId, p: Pos2) -> Option<NodeId> {
    if !app.canvas.rect.contains(p) {
        return None;
    }
    let ds = app.session.ds().ok()?;
    sticky_or_shape_at(&ds.doc, ds.page, app.to_page(p), Some(except))
}

/// Where the next one goes for `side`, faintly: its box and the connector's line to it.
fn preview(app: &FigApp, ctx: &egui::Context, b: figcraft_geom::Rect, next: (f64, f64), side: &str, accent: Color32) {
    let Some(((x, y), ..)) = connected_place((b.x0, b.y0), (b.width(), b.height()), next, side) else { return };
    let ghost = Rect::from_two_pos(app.to_screen(x, y), app.to_screen(x + next.0, y + next.1));
    let from = Rect::from_two_pos(app.to_screen(b.x0, b.y0), app.to_screen(b.x1, b.y1));
    let (a, z) = match side {
        "above" => (from.center_top(), ghost.center_bottom()),
        "right" => (from.right_center(), ghost.left_center()),
        "below" => (from.center_bottom(), ghost.center_top()),
        _ => (from.left_center(), ghost.right_center()),
    };
    let p = ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("board_connect_preview")));
    let faint = Stroke::new(1.5, accent.gamma_multiply(0.6));
    ghost_box(&p, ghost, accent);
    p.line_segment([a, z], faint);
}

/// The next one's box, faintly: a tint and a dashed outline.
fn ghost_box(p: &egui::Painter, ghost: Rect, accent: Color32) {
    let faint = Stroke::new(1.5, accent.gamma_multiply(0.6));
    p.rect_filled(ghost, CornerRadius::same(8), accent.gamma_multiply(0.08));
    let corners = [ghost.left_top(), ghost.right_top(), ghost.right_bottom(), ghost.left_bottom(), ghost.left_top()];
    p.extend(Shape::dashed_line(&corners, faint, 6.0, 4.0));
}

/// The dragged preview: the next one centred on `pointer` and the connector's line to it from
/// `side` of the object's box `b`, or, over another object, that object outlined and the line to
/// it; the line arrives at the side of the box facing the object.
fn drag_preview(app: &FigApp, ctx: &egui::Context, (b, next, side): (figcraft_geom::Rect, (f64, f64), &str), pointer: Pos2, over: Option<NodeId>) {
    let accent = crate::theme::canvas_accent(ctx);
    let from = Rect::from_two_pos(app.to_screen(b.x0, b.y0), app.to_screen(b.x1, b.y1));
    let p = ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("board_connect_preview")));
    let to_box = match over.and_then(|o| app.session.doc().ok()?.world_bounds(o)) {
        Some(ob) => {
            let r = Rect::from_two_pos(app.to_screen(ob.x0, ob.y0), app.to_screen(ob.x1, ob.y1));
            p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5, accent), StrokeKind::Outside);
            r
        }
        None => {
            let size = app.to_screen(next.0, next.1) - app.to_screen(0.0, 0.0);
            let ghost = Rect::from_center_size(pointer, size);
            ghost_box(&p, ghost, accent);
            ghost
        }
    };
    let d = to_box.center() - from.center();
    let z = if d.x.abs() >= d.y.abs() {
        if d.x >= 0.0 { to_box.left_center() } else { to_box.right_center() }
    } else if d.y >= 0.0 {
        to_box.center_top()
    } else {
        to_box.center_bottom()
    };
    p.line_segment([side_middle(from, side), z], Stroke::new(1.5, accent.gamma_multiply(0.6)));
}
