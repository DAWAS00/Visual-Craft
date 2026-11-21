//! The "+" handles beside a board's selected sticky or shape (#1597), drawn headless: four, only
//! for one sticky or shape with the Move tool, clear of the bar above it; clicking one makes the
//! next shape on that side joined by a connector and starts typing in it; one undo takes both away.

use egui::{Event, Id, LayerId, Modifiers, Order, PointerButton, Pos2, Rect, pos2};
use figcraft_doc::{BoardItem, NodeId};
use figcraft_engine::Session;
use figcraft_engine::tools::ToolId;
use serde_json::{Value, json};

use crate::board_connect::REACH;
use crate::layers_drag_tests::Ui;

fn id(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn board_with_shape() -> (Ui, NodeId) {
    let mut s = Session::new();
    s.execute("file.newBoard", &json!({})).unwrap();
    let a = id(&s.execute("board.shape", &json!({"shape": "roundedRectangle", "x": 0, "y": 0, "width": 160, "height": 100})).unwrap());
    let mut u = Ui::new(s);
    u.frame(vec![]);
    // The shape in the middle of the canvas at 100%.
    let view = &mut u.app.session.ds_mut().unwrap().view;
    (view.zoom, view.cx, view.cy) = (1.0, 80.0, 50.0);
    u.frame(vec![]);
    (u, a)
}

/// Where the "+" for a side shows, if it does.
fn handle(u: &Ui, side: &str) -> Option<Rect> {
    let layer = LayerId::new(Order::Middle, Id::unique(("board_connect", side)));
    u.ctx.memory(|m| m.areas().visible_last_frame(&layer)).then(|| u.ctx.memory(|m| m.area_rect(layer.id))).flatten()
}

/// Where the bar above the selection shows, if it does.
fn bar(u: &Ui) -> Option<Rect> {
    let layer = LayerId::new(Order::Middle, Id::unique("board_bar"));
    u.ctx.memory(|m| m.areas().visible_last_frame(&layer)).then(|| u.ctx.memory(|m| m.area_rect(layer.id))).flatten()
}

fn click(u: &mut Ui, p: Pos2) {
    for pressed in [true, false] {
        u.frame(vec![Event::PointerMoved(p), Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
    }
    u.frame(vec![]);
}

fn screen_box(u: &Ui, n: NodeId) -> Rect {
    let b = u.app.session.doc().unwrap().world_bounds(n).unwrap();
    Rect::from_two_pos(u.app.to_screen(b.x0, b.y0), u.app.to_screen(b.x1, b.y1))
}

#[test]
fn four_handles_show_for_one_shape_and_clear_of_the_bar() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": []})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(handle(&u, "right").is_none(), "nothing selected");
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let r = screen_box(&u, a);
    for (side, want) in [
        ("above", pos2(r.center().x, r.min.y - REACH)),
        ("right", pos2(r.max.x + REACH, r.center().y)),
        ("below", pos2(r.center().x, r.max.y + REACH)),
        ("left", pos2(r.min.x - REACH, r.center().y)),
    ] {
        let h = handle(&u, side).unwrap_or_else(|| panic!("{side}: no handle"));
        assert!((h.center() - want).length() < 1.0, "{side}: {h:?}");
    }
    let bar = u.ctx.memory(|m| m.area_rect(Id::unique("board_bar"))).expect("the bar shows");
    assert!(!bar.intersects(handle(&u, "above").unwrap()), "the bar leaves the handle above free");
    // Another tool, or two layers selected: none.
    u.app.session.set_tool(ToolId::Hand);
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(handle(&u, "right").is_none(), "another tool");
    u.app.session.set_tool(ToolId::Move);
    let b = id(&u.app.session.execute("board.sticky", &json!({"x": 600, "y": 0})).unwrap());
    u.app.session.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(handle(&u, "right").is_none(), "two layers");
}

#[test]
fn clicking_a_handle_adds_the_next_shape_joined_and_typing() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let before = u.app.session.doc().unwrap().ids().count();
    let at = handle(&u, "below").expect("a handle below").center();
    click(&mut u, at);
    let doc = u.app.session.doc().unwrap();
    let made: Vec<NodeId> = doc.ids().filter(|i| doc.node(*i).is_some_and(|n| n.board.is_some())).collect();
    let next = made.iter().copied().find(|i| *i != a && matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { .. }))).expect("a new shape");
    let line = made.iter().copied().find(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Connector(_)))).expect("a connector");
    assert!(screen_box(&u, next).min.y > screen_box(&u, a).max.y, "below it");
    assert!(matches!(&doc.node(line).unwrap().board, Some(BoardItem::Connector(c)) if c.from.node() == Some(a) && c.to.node() == Some(next)));
    assert!(u.app.session.ds().unwrap().text_edit.is_some(), "typing in the new shape");
    figcraft_engine::textedit::end(&mut u.app.session).unwrap();
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "undo takes the shape and its connector away");
}

/// While the stamp tool is over an object, or a stamp is dragged over one, that object is outlined:
/// the one the stamp will stick to (the same rule `board.stamp` attaches by). Nothing off it, and
/// nothing once the stamp is let go.
#[test]
fn the_object_a_stamp_will_stick_to_is_highlighted() {
    let (mut u, a) = board_with_shape();
    let target = |u: &Ui| u.app.session.tool.overlay.stamp_target;
    u.app.run("tool.stamp", json!({"stamp": "heart"}));
    let centre = screen_box(&u, a).center();
    let off = centre + egui::vec2(400.0, 0.0);
    u.frame(vec![Event::PointerMoved(centre)]);
    assert_eq!(target(&u), Some(a), "the stamp tool over the shape");
    u.frame(vec![Event::PointerMoved(off)]);
    assert_eq!(target(&u), None, "over the empty board");
    // Placed where the outline was: it sticks to that shape.
    click(&mut u, centre);
    let stamp = u.app.session.ds().unwrap().selection[0];
    let on = |u: &Ui| match &u.app.session.doc().unwrap().node(stamp).unwrap().board {
        Some(BoardItem::Stamp { on, .. }) => *on,
        other => panic!("not a stamp: {other:?}"),
    };
    assert_eq!(on(&u), Some(a));
    // Dragged with the Move tool: the shape outlined while the stamp is over it, nothing off it.
    u.app.run("tool.set", json!({"tool": "move"}));
    u.drag_to(centre, centre + egui::vec2(20.0, 10.0));
    assert_eq!(target(&u), Some(a), "a stamp dragged over the shape");
    // Let go there, it sticks to the shape and stays on the page, not in the shape's layout.
    let there = centre + egui::vec2(20.0, 10.0);
    u.button(there, false);
    let page = u.app.session.ds().unwrap().page;
    assert_eq!(u.app.session.doc().unwrap().node(stamp).unwrap().parent, Some(page), "a shape takes no layers in");
    assert_eq!((on(&u), target(&u)), (Some(a), None));
    u.drag_to(there, there + egui::vec2(10.0, 0.0));
    u.frame(vec![Event::PointerMoved(off)]);
    assert_eq!(target(&u), None, "dragged off it");
    u.button(off, false);
    assert_eq!((target(&u), on(&u)), (None, None), "let go off the shape: on nothing, and no outline");
}

/// Dragging a "+" carries the preview ("Click to select and create the next shape in that
/// direction, or drag the preview to any location on your board", Build faster with quick create;
/// #1919): let go on empty board and the next shape is made there, centred on the pointer, joined
/// by a connector, with typing started; one undo takes both away.
#[test]
fn dragging_a_handle_drops_the_next_shape_where_it_is_let_go() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let before = u.app.session.doc().unwrap().ids().count();
    let from = handle(&u, "below").expect("a handle below").center();
    let to = from + egui::vec2(260.0, 180.0);
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "nothing is made until a shape is chosen");
    u.frame(vec![key(egui::Key::Enter)]);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    let next = doc.ids().find(|i| *i != a && matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { .. }))).expect("a new shape");
    let line = doc.ids().find(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Connector(_)))).expect("a connector");
    let centre = screen_box(&u, next).center();
    assert!((centre - to).length() < 1.0, "centred where it was let go: {centre:?}, not {to:?}");
    assert!(matches!(&doc.node(line).unwrap().board, Some(BoardItem::Connector(c)) if c.from.node() == Some(a) && c.to.node() == Some(next)));
    assert!(u.app.session.ds().unwrap().text_edit.is_some(), "typing in the new shape");
    figcraft_engine::textedit::end(&mut u.app.session).unwrap();
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "undo takes the shape and its connector away");
}

/// A "+" dragged onto another sticky or shape joins the two with a connector instead of making a
/// new shape (#1919).
#[test]
fn dropping_a_handle_on_another_shape_joins_them() {
    let (mut u, a) = board_with_shape();
    let b = id(&u.app.session.execute("board.shape", &json!({"shape": "ellipse", "x": 400, "y": 300, "width": 160, "height": 100})).unwrap());
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let shapes = |u: &Ui| {
        let doc = u.app.session.doc().unwrap();
        doc.ids().filter(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { .. }))).count()
    };
    assert_eq!(shapes(&u), 2);
    let from = handle(&u, "right").expect("a handle right").center();
    let to = screen_box(&u, b).center();
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(shapes(&u), 2, "no new shape");
    let doc = u.app.session.doc().unwrap();
    let line = doc.ids().find(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Connector(_)))).expect("a connector");
    assert!(matches!(&doc.node(line).unwrap().board, Some(BoardItem::Connector(c)) if c.from.node() == Some(a) && c.to.node() == Some(b)));
}

/// A sticky or shape inside a section or frame takes the drop too (review): the connector joins it.
#[test]
fn dropping_a_handle_on_a_shape_inside_a_frame_joins_it() {
    let (mut u, a) = board_with_shape();
    let f = id(&u.app.session.execute("frame.create", &json!({"x": 300, "y": 200, "width": 500, "height": 400})).unwrap());
    let b = id(&u
        .app
        .session
        .execute("board.shape", &json!({"shape": "ellipse", "x": 100, "y": 100, "width": 160, "height": 100, "parent": f}))
        .unwrap());
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let from = handle(&u, "right").expect("a handle right").center();
    let to = screen_box(&u, b).center();
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    let line = doc.ids().find(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Connector(_)))).expect("a connector");
    assert!(
        matches!(&doc.node(line).unwrap().board, Some(BoardItem::Connector(c)) if c.from.node() == Some(a) && c.to.node() == Some(b)),
        "joined to the shape in the frame, not the frame"
    );
}

/// A handle let go off the canvas, over a panel, makes nothing (review).
#[test]
fn letting_a_handle_go_over_a_panel_makes_nothing() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let before = u.app.session.doc().unwrap().ids().count();
    let from = handle(&u, "left").expect("a handle left").center();
    // The left sidebar.
    let to = Pos2::new(u.app.canvas.rect.min.x - 60.0, from.y);
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "nothing made");
}

/// A handle let go over the floating toolbar at the bottom of the canvas makes nothing either
/// (review): the toolbar lies inside the canvas rectangle.
#[test]
fn letting_a_handle_go_over_the_toolbar_makes_nothing() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let before = u.app.session.doc().unwrap().ids().count();
    let from = handle(&u, "below").expect("a handle below").center();
    let to = crate::toolbar::pill_rect(u.app.canvas.rect, crate::toolbar::bar_width(&u.app)).center();
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "nothing made behind the toolbar");
}

/// The selection moving to another shape under a drag ends the drag (review): letting go later,
/// even with the first shape selected again, makes nothing.
#[test]
fn a_drag_ends_when_the_selection_moves_to_another_shape() {
    let (mut u, a) = board_with_shape();
    let b = id(&u.app.session.execute("board.shape", &json!({"shape": "ellipse", "x": 600, "y": 0, "width": 160, "height": 100})).unwrap());
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let before = u.app.session.doc().unwrap().ids().count();
    let from = handle(&u, "below").expect("a handle below").center();
    let to = from + egui::vec2(0.0, 160.0);
    u.drag_to(from, to);
    u.app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    u.frame(vec![Event::PointerMoved(to)]);
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![Event::PointerMoved(to)]);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(u.app.session.doc().unwrap().ids().count(), before, "the drag was over");
}

/// The bar above the selection stays away while a handle is dragged, as for any drag (review):
/// letting go where it was makes the next one there, not behind it, and the bar is back after.
#[test]
fn the_bar_stays_away_while_a_handle_is_dragged() {
    let (mut u, a) = board_with_shape();
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let r = bar(&u).expect("the bar shows");
    let from = handle(&u, "above").expect("a handle above").center();
    let to = r.center();
    u.drag_to(from, to);
    assert!(bar(&u).is_none(), "no bar while a handle is dragged");
    u.button(to, false);
    u.frame(vec![]);
    u.frame(vec![key(egui::Key::Enter)]);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    let next = doc.ids().find(|i| *i != a && matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { .. }))).expect("a new shape");
    let line = doc.ids().find(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Connector(_)))).expect("a connector");
    let centre = screen_box(&u, next).center();
    assert!((centre - to).length() < 1.0, "centred where the bar was: {centre:?}, not {to:?}");
    assert!(matches!(&doc.node(line).unwrap().board, Some(BoardItem::Connector(c)) if c.from.node() == Some(a) && c.to.node() == Some(next)));
    // Typing in the new one keeps the bar away, as before; done typing, it is back.
    figcraft_engine::textedit::end(&mut u.app.session).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(bar(&u).is_some(), "the bar is back for the new selection");
}

/// A key pressed (its release is not sent).
fn key(k: egui::Key) -> Event {
    Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// Drag a "+" below `a` and let go at the point `to`, on empty board: the chooser opens there.
fn drop_below(u: &mut Ui, a: NodeId) -> Pos2 {
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![]);
    let from = handle(u, "below").expect("a handle below").center();
    // Above the floating toolbar at the bottom of the canvas (a drop there is refused).
    let to = from + egui::vec2(260.0, 60.0);
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    to
}

/// The page's shapes and connectors, to check that a dismissed chooser made nothing.
fn layers(u: &Ui) -> usize {
    u.app.session.doc().unwrap().ids().count()
}

/// Escape closes the chooser and nothing is made or moved; Down before it only moves the row.
#[test]
fn escape_leaves_the_chooser_and_the_source_where_it_was() {
    let (mut u, a) = board_with_shape();
    let before = (layers(&u), u.app.session.doc().unwrap().node(a).unwrap().transform, u.app.session.undo_labels(usize::MAX).len());
    drop_below(&mut u, a);
    u.frame(vec![key(egui::Key::ArrowDown)]);
    u.frame(vec![key(egui::Key::Escape)]);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    assert_eq!(layers(&u), before.0, "nothing made");
    assert_eq!(doc.node(a).unwrap().transform, before.1, "the source did not move (the arrow was the chooser's)");
    assert_eq!(u.app.session.undo_labels(usize::MAX).len(), before.2, "no undo step");
}

/// Down twice and Enter makes a Decision (a diamond at least 160 by 120), centred where the "+"
/// was let go, joined by a connector, typing started; one undo takes both away.
#[test]
fn the_keyboard_chooses_a_decision_where_it_was_let_go() {
    let (mut u, a) = board_with_shape();
    let before = layers(&u);
    let to = drop_below(&mut u, a);
    u.frame(vec![key(egui::Key::ArrowDown)]);
    u.frame(vec![key(egui::Key::ArrowDown)]);
    u.frame(vec![key(egui::Key::Enter)]);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    let next = doc
        .ids()
        .find(|i| *i != a && matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { shape: figcraft_doc::BoardShape::Diamond })))
        .expect("a diamond");
    let n = doc.node(next).unwrap();
    assert!(n.width >= 160.0 && n.height >= 120.0, "at least 160 by 120");
    assert!((screen_box(&u, next).center() - to).length() < 1.0, "centred where it was let go");
    assert!(u.app.session.ds().unwrap().text_edit.is_some(), "typing in the new shape");
    figcraft_engine::textedit::end(&mut u.app.session).unwrap();
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(layers(&u), before, "undo takes the shape, its connector and the chosen shape away");
}

/// Whether the chooser showed last frame.
fn chooser_shown(u: &Ui) -> bool {
    let layer = LayerId::new(Order::Foreground, Id::unique("board_connect_chooser_area"));
    u.ctx.memory(|m| m.areas().visible_last_frame(&layer))
}

/// A press outside the chooser closes it, even one that keeps the source selected (on the source
/// itself), and Enter after it is no longer the chooser's.
#[test]
fn a_press_outside_closes_the_chooser_and_makes_nothing() {
    let (mut u, a) = board_with_shape();
    let before = layers(&u);
    drop_below(&mut u, a);
    assert!(chooser_shown(&u), "the chooser opened");
    let on_source = screen_box(&u, a).center();
    click(&mut u, on_source);
    u.frame(vec![]);
    assert!(!chooser_shown(&u), "the press outside closed it");
    assert_eq!(u.app.session.selection(), vec![a], "the source is still selected");
    assert!(handle(&u, "below").is_some(), "its handles are back");
    u.frame(vec![key(egui::Key::Enter)]);
    u.frame(vec![]);
    let doc = u.app.session.doc().unwrap();
    assert!(!doc.ids().any(|i| i != a && matches!(doc.node(i).unwrap().board, Some(BoardItem::Shape { .. }))), "no shape made");
    assert_eq!(layers(&u), before, "nothing made, nothing taken away");
}

/// Keys that come in one frame count in their order: Down, Down, Enter is a Decision, and Escape
/// before Enter makes nothing.
#[test]
fn keys_in_one_frame_count_in_their_order() {
    let diamonds = |u: &Ui| {
        let doc = u.app.session.doc().unwrap();
        doc.ids().filter(|i| matches!(doc.node(*i).unwrap().board, Some(BoardItem::Shape { shape: figcraft_doc::BoardShape::Diamond }))).count()
    };
    let (mut u, a) = board_with_shape();
    let before = layers(&u);
    drop_below(&mut u, a);
    u.frame(vec![key(egui::Key::Escape), key(egui::Key::Enter)]);
    u.frame(vec![]);
    assert_eq!(layers(&u), before, "Escape came first: nothing made");
    drop_below(&mut u, a);
    u.frame(vec![key(egui::Key::ArrowDown), key(egui::Key::ArrowDown), key(egui::Key::Enter)]);
    u.frame(vec![]);
    assert_eq!(diamonds(&u), 1, "Down, Down, Enter: the Decision");
}

/// A click on a row makes that shape: the third row, Decision, makes a diamond, and the canvas
/// under the chooser starts nothing.
#[test]
fn clicking_a_row_makes_that_shape() {
    let (mut u, a) = board_with_shape();
    drop_below(&mut u, a);
    u.frame(vec![]);
    let area = u.ctx.memory(|m| m.area_rect(Id::unique("board_connect_chooser_area"))).expect("the chooser's box");
    // Four rows of equal height: the middle of the third.
    let row = pos2(area.center().x, area.min.y + area.height() * 2.5 / 4.0);
    u.frame(vec![Event::PointerMoved(row)]);
    click(&mut u, row);
    let doc = u.app.session.doc().unwrap();
    assert!(
        doc.ids().any(|i| matches!(doc.node(i).unwrap().board, Some(BoardItem::Shape { shape: figcraft_doc::BoardShape::Diamond }))),
        "the click made the Decision"
    );
    assert!(!chooser_shown(&u), "and closed the chooser");
}

/// Escape and a row's click in the same frame: Escape wins and nothing is made.
#[test]
fn escape_wins_over_a_row_click_in_the_same_frame() {
    let (mut u, a) = board_with_shape();
    let before = layers(&u);
    drop_below(&mut u, a);
    // A new area's first frame is egui's sizing pass: read where it settles after it.
    u.frame(vec![]);
    let area = u.ctx.memory(|m| m.area_rect(Id::unique("board_connect_chooser_area"))).expect("the chooser's box");
    let row = pos2(area.center().x, area.min.y + 14.0);
    // Over the row first, as a mouse is, then pressed.
    u.frame(vec![Event::PointerMoved(row)]);
    u.frame(vec![Event::PointerButton { pos: row, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    assert!(!u.app.session.tool.busy(), "the canvas under the chooser started nothing");
    assert!(chooser_shown(&u), "a press on a row keeps it open");
    u.frame(vec![
        key(egui::Key::Escape),
        Event::PointerButton { pos: row, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
    ]);
    u.frame(vec![]);
    assert_eq!(layers(&u), before, "nothing made");
}

/// A chooser belongs to its file: in another tab, where a shape with the same id is selected,
/// Enter makes nothing there.
#[test]
fn the_chooser_stays_with_its_file() {
    let (mut u, a) = board_with_shape();
    drop_below(&mut u, a);
    assert!(chooser_shown(&u), "the chooser opened");
    // Another board, its first shape getting the same id as the source, selected.
    u.app.session.execute("file.newBoard", &json!({})).unwrap();
    let b = id(&u.app.session.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0, "width": 160, "height": 100})).unwrap());
    assert_eq!(b, a, "the same id in the other file");
    u.app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    let before = layers(&u);
    u.frame(vec![]);
    u.frame(vec![key(egui::Key::Enter)]);
    u.frame(vec![]);
    assert!(!chooser_shown(&u), "closed in the other file");
    assert_eq!(layers(&u), before, "nothing made in the other file");
}

/// The control channel's `ui.key` (which runs canvas bindings itself) gives the open chooser its
/// keys too: Down, Down, Enter makes a Decision and the source is not nudged.
#[test]
fn ui_key_goes_to_the_open_chooser_before_the_canvas() {
    let (mut u, a) = board_with_shape();
    let at = u.app.session.doc().unwrap().node(a).unwrap().transform;
    drop_below(&mut u, a);
    for k in ["ArrowDown", "ArrowDown", "Enter"] {
        crate::control::handle(&mut u.app, &u.ctx, "ui.key", &json!({"key": k})).unwrap();
        u.frame(vec![]);
    }
    let doc = u.app.session.doc().unwrap();
    assert_eq!(doc.node(a).unwrap().transform, at, "the arrows were the chooser's, not a nudge");
    assert!(
        doc.ids().any(|i| matches!(doc.node(i).unwrap().board, Some(BoardItem::Shape { shape: figcraft_doc::BoardShape::Diamond }))),
        "Enter made the Decision"
    );
}

/// A sticky's next one is still made where it is let go, with no chooser (only shapes choose).
#[test]
fn a_sticky_still_makes_its_next_sticky_where_it_is_let_go() {
    let mut s = Session::new();
    s.execute("file.newBoard", &json!({})).unwrap();
    let a = id(&s.execute("board.sticky", &json!({"x": 0, "y": 0, "text": "Idea"})).unwrap());
    let mut u = Ui::new(s);
    u.frame(vec![]);
    let view = &mut u.app.session.ds_mut().unwrap().view;
    (view.zoom, view.cx, view.cy) = (1.0, 80.0, 50.0);
    u.frame(vec![]);
    let before = layers(&u);
    drop_below(&mut u, a);
    let doc = u.app.session.doc().unwrap();
    assert!(doc.ids().any(|i| matches!(doc.node(i).unwrap().board, Some(BoardItem::Sticky { .. })) && i != a), "a sticky was made at once");
    assert!(layers(&u) > before, "made without a choice");
}
