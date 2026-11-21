//! `board.addConnected` (#1597): the next sticky or shape on a side of the selected one, of the
//! same kind, size and colour, joined to it by a connector, in one undo step.

use figcraft_doc::{BoardItem, BoardShape, Connector, End, NodeId, Side};
use serde_json::json;

use crate::Session;
use crate::board::CONNECTED_GAP;
use crate::board_tests::{board, id, node};

fn connector(s: &Session, c: NodeId) -> Connector {
    match &node(s, c).board {
        Some(BoardItem::Connector(k)) => k.clone(),
        other => panic!("not a connector: {other:?}"),
    }
}

fn at(s: &Session, n: NodeId) -> (f64, f64) {
    let t = node(s, n).transform.translation();
    (t.x, t.y)
}

#[test]
fn a_shape_gets_the_next_one_joined_on_each_side() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "diamond", "x": 0, "y": 0, "width": 200, "height": 120, "color": "blue"})).unwrap(), "id");
    let colour = |s: &Session, n: NodeId| node(s, node(s, n).children[0]).fills[0].solid_color();
    let g = CONNECTED_GAP;
    for (side, want, ends) in [
        ("right", (200.0 + g, 0.0), (Side::Right, Side::Left)),
        ("below", (0.0, 120.0 + g), (Side::Bottom, Side::Top)),
        ("left", (-200.0 - g, 0.0), (Side::Left, Side::Right)),
        ("above", (0.0, -120.0 - g), (Side::Top, Side::Bottom)),
    ] {
        let r = s.execute("board.addConnected", &json!({"id": a, "side": side})).unwrap();
        let (b, c) = (id(&r, "id"), id(&r, "connector"));
        assert_eq!(at(&s, b), want, "{side}");
        let nb = node(&s, b);
        assert_eq!((nb.width, nb.height), (200.0, 120.0), "{side}: the same size");
        assert!(matches!(nb.board, Some(BoardItem::Shape { shape: BoardShape::Diamond })), "{side}: the same shape");
        assert_eq!(colour(&s, b), colour(&s, a), "{side}: the same colour");
        let k = connector(&s, c);
        assert_eq!((k.from, k.to), (End::Node { node: a, side: ends.0, at: None }, End::Node { node: b, side: ends.1, at: None }), "{side}");
        assert_eq!(k.routing, s.tool.board.routing, "{side}: as the connector tool makes one");
        assert_eq!(s.selection(), vec![b], "{side}: the new one is selected");
    }
}

#[test]
fn a_sticky_gets_a_sticky_of_its_colour_and_typing_goes_on_in_it() {
    let mut s = board();
    let a = id(&s.execute("board.sticky", &json!({"x": 0, "y": 0, "color": "pink", "text": "First"})).unwrap(), "id");
    s.ui_requests.clear();
    // The selection is the default, and editing is asked for.
    s.execute("select.set", &json!({"ids": [a]})).unwrap();
    let r = s.execute("board.addConnected", &json!({"side": "below", "edit": true})).unwrap();
    let b = id(&r, "id");
    assert!(matches!(node(&s, b).board, Some(BoardItem::Sticky { .. })));
    assert_eq!(node(&s, b).fills[0].solid_color(), node(&s, a).fills[0].solid_color());
    assert_eq!(at(&s, b).1, node(&s, a).height + CONNECTED_GAP);
    assert!(s.ui_requests.iter().any(|q| q.kind == "board.editText" && q.data["id"] == json!(b)));
}

#[test]
fn it_is_one_undo_step_and_refuses_what_it_cant_join() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "ellipse", "x": 0, "y": 0})).unwrap(), "id");
    let before = s.doc().unwrap().ids().count();
    let r = s.execute("board.addConnected", &json!({"id": a})).unwrap();
    assert_eq!(at(&s, id(&r, "id")).0, 160.0 + CONNECTED_GAP, "right by default");
    assert!(s.doc().unwrap().ids().count() > before);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().ids().count(), before, "the shape and its connector go together");
    // Not a sticky or shape, not a side, nothing selected.
    let c = id(&s.execute("board.stamp", &json!({"x": 400, "y": 0, "stamp": "star"})).unwrap(), "id");
    assert!(s.execute("board.addConnected", &json!({"id": c})).is_err());
    assert!(s.execute("board.addConnected", &json!({"id": a, "side": "diagonal"})).is_err());
    s.execute("select.set", &json!({"ids": []})).unwrap();
    assert!(s.execute("board.addConnected", &json!({})).is_err());
    // Not on a design file.
    let mut d = Session::new();
    assert!(d.execute("board.addConnected", &json!({})).is_err());
}

#[test]
fn the_next_sticky_shortcut_joins_the_next_shape_and_leaves_stickies_as_they_were() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0, "width": 200, "height": 120})).unwrap(), "id");
    s.execute("select.set", &json!({"ids": [a]})).unwrap();
    let undo = s.ds().unwrap().history.undo.len();
    // Shift+Cmd/Ctrl+Enter on a shape: the next shape on the right, joined by a connector (#1597).
    let r = s.execute("board.nextSticky", &json!({})).unwrap();
    let (b, c) = (id(&r, "id"), id(&r, "connector"));
    assert!(matches!(node(&s, b).board, Some(BoardItem::Shape { shape: BoardShape::Rectangle })));
    assert_eq!(at(&s, b), (200.0 + CONNECTED_GAP, 0.0));
    assert_eq!(connector(&s, c).from, End::Node { node: a, side: Side::Right, at: None });
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1, "one undo step");
    // Another side, as the command takes one.
    let r = s.execute("board.nextSticky", &json!({"id": a, "side": "below"})).unwrap();
    assert_eq!(at(&s, id(&r, "id")), (0.0, 120.0 + CONNECTED_GAP));
    // A sticky still gets a sticky beside it, without a connector.
    let st = id(&s.execute("board.sticky", &json!({"x": 1000, "y": 0})).unwrap(), "id");
    let r = s.execute("board.nextSticky", &json!({"id": st})).unwrap();
    assert!(r.get("connector").is_none(), "{r}");
    assert!(matches!(node(&s, id(&r, "id")).board, Some(BoardItem::Sticky { .. })));
}

/// `at` (#1919; "drag the preview to any location on your board", Build faster with quick create):
/// the next one is centred on that point, wherever it is, and the connector takes the sides facing
/// each other; `at` must be two finite numbers.
#[test]
fn the_next_one_goes_where_it_is_dropped() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "diamond", "x": 0, "y": 0, "width": 200, "height": 120})).unwrap(), "id");
    let r = s.execute("board.addConnected", &json!({"id": a, "side": "below", "at": [700, 60]})).unwrap();
    let (b, c) = (id(&r, "id"), id(&r, "connector"));
    assert_eq!(at(&s, b), (600.0, 0.0), "centred on the point");
    let k = connector(&s, c);
    assert_eq!(
        (k.from, k.to),
        (End::Node { node: a, side: Side::Bottom, at: None }, End::Node { node: b, side: Side::Auto, at: None }),
        "leaves by the handle's side"
    );
    // Inside a frame at (1000, 500): the page point lands in the frame's coordinates.
    let f = id(&s.execute("frame.create", &json!({"x": 1000, "y": 500, "width": 800, "height": 600})).unwrap(), "id");
    let inner = id(&s.execute("board.shape", &json!({"shape": "ellipse", "x": 0, "y": 0, "width": 200, "height": 120, "parent": f})).unwrap(), "id");
    let r = s.execute("board.addConnected", &json!({"id": inner, "at": [1300, 700]})).unwrap();
    let placed = id(&r, "id");
    assert_eq!(node(&s, placed).parent, Some(f), "beside it, in the frame");
    assert_eq!(at(&s, placed), (200.0, 140.0), "centred on the page point, in the frame's coordinates");
    for wrong in [json!([1]), json!("x"), json!([0, "y"]), json!({"x": 1, "y": 2})] {
        assert!(s.execute("board.addConnected", &json!({"id": a, "at": wrong})).is_err(), "{wrong}");
    }
}

/// The layers on the page, to check that a refused command made none.
fn layers(s: &Session) -> usize {
    s.doc().unwrap().ids().count()
}

/// `shape` chooses the next shape (the chooser's Decision): a diamond at least 160 × 120, joined
/// by its connector, and one undo takes both away.
#[test]
fn a_shape_choice_makes_the_next_shape_and_is_one_undo_step() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "roundedRectangle", "x": 0, "y": 0, "width": 160, "height": 100})).unwrap(), "id");
    let before = layers(&s);
    let r = s.execute("board.addConnected", &json!({"id": a, "side": "below", "shape": "diamond"})).unwrap();
    let b = id(&r, "id");
    assert!(matches!(node(&s, b).board, Some(BoardItem::Shape { shape: BoardShape::Diamond })), "a diamond");
    assert_eq!((node(&s, b).width, node(&s, b).height), (160.0, 120.0), "at least 160 by 120");
    assert_eq!(connector(&s, id(&r, "connector")).to.node(), Some(b));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(layers(&s), before, "one undo takes the shape and its connector away");
}

/// Every parameter is optional, so the Actions palette still offers it, and only the documented
/// names are taken (a stray comma in the params doc once made words of it into "parameters").
#[test]
fn it_needs_no_parameters_and_takes_only_its_own() {
    assert!(!crate::cmd::needs_params("board.addConnected"));
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0})).unwrap(), "id");
    let e = s.execute("board.addConnected", &json!({"id": a, "the": 1})).unwrap_err().to_string();
    assert!(e.contains("unknown parameter"), "{e}");
}

/// Refused while a text is being typed (the source is a copy inside an instance, which the
/// executor's checks refuse only after the command ran): the typing stays open and no undo step is
/// added, since the refusal comes before the edit is ended.
#[test]
fn a_refusal_while_typing_leaves_the_typing_open() {
    let mut s = board();
    let f = id(&s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 400, "height": 300, "name": "Card"})).unwrap(), "id");
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 20, "y": 20, "parent": f})).unwrap(), "id");
    let c = id(&s.execute("component.create", &json!({"ids": [f]})).unwrap(), "id");
    let inst = id(&s.execute("instance.create", &json!({"component": c, "x": 600, "y": 0})).unwrap(), "id");
    let copy = {
        let doc = s.doc().unwrap();
        doc.subtree(inst).into_iter().find(|i| doc.node(*i).and_then(|n| n.source) == Some(a)).expect("the shape's copy")
    };
    assert!(matches!(node(&s, copy).board, Some(BoardItem::Shape { .. })), "the copy is a board shape");
    let note = id(&s.execute("board.sticky", &json!({"x": 0, "y": 600, "text": "note"})).unwrap(), "id");
    assert!(crate::board::edit_text(&mut s, note).unwrap(), "typing in the sticky");
    let (undo0, layers0) = (s.undo_labels(usize::MAX), layers(&s));
    let e = s.execute("board.addConnected", &json!({"id": copy, "side": "right"})).unwrap_err().to_string();
    assert!(!e.contains("isn't a sticky or a shape"), "refused for the instance, not the source: {e}");
    assert!(s.ds().unwrap().text_edit.is_some(), "still typing in the sticky");
    assert_eq!((s.undo_labels(usize::MAX), layers(&s)), (undo0, layers0), "no undo step, no layer");
}

/// Typing in a text inside an instance that has been emptied: ending the edit would remove it,
/// which an instance refuses, so the next shape (of an editable shape elsewhere) is refused before
/// the edit ends, and the typing stays open.
#[test]
fn an_emptied_text_in_an_instance_keeps_its_typing_when_refused() {
    let mut s = board();
    let f = id(&s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 400, "height": 300, "name": "Card"})).unwrap(), "id");
    let t = id(&s.execute("text.create", &json!({"text": "Title", "x": 10, "y": 10, "parent": f})).unwrap(), "id");
    let c = id(&s.execute("component.create", &json!({"ids": [f]})).unwrap(), "id");
    let inst = id(&s.execute("instance.create", &json!({"component": c, "x": 600, "y": 0})).unwrap(), "id");
    let copy = {
        let doc = s.doc().unwrap();
        doc.subtree(inst).into_iter().find(|i| doc.node(*i).and_then(|n| n.source) == Some(t)).expect("the text's copy")
    };
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 600})).unwrap(), "id");
    crate::textedit::begin(&mut s, copy, None, false).unwrap();
    crate::textedit::key(&mut s, "SelectAll", false, false, false).unwrap();
    crate::textedit::key(&mut s, "Backspace", false, false, false).unwrap();
    assert_eq!(crate::textedit::end_removes(&s), Some(copy), "ending would remove the emptied copy");
    let (undo0, layers0) = (s.undo_labels(usize::MAX), layers(&s));
    assert!(s.execute("board.addConnected", &json!({"id": a, "side": "right"})).is_err());
    assert!(s.ds().unwrap().text_edit.is_some(), "still typing in the copy");
    assert_eq!((s.undo_labels(usize::MAX), layers(&s)), (undo0, layers0), "no undo step, no layer");
}

/// The Decision's minimum size is for a diamond chosen from another shape: a click on a small
/// diamond's "+" (no choice) still copies its size, and so does choosing a diamond for one.
#[test]
fn a_small_diamond_next_one_keeps_its_size() {
    let mut s = board();
    let d = id(&s.execute("board.shape", &json!({"shape": "diamond", "x": 0, "y": 0, "width": 100, "height": 80})).unwrap(), "id");
    for params in [json!({"id": d, "side": "right"}), json!({"id": d, "side": "below", "shape": "diamond"})] {
        let b = id(&s.execute("board.addConnected", &params).unwrap(), "id");
        assert_eq!((node(&s, b).width, node(&s, b).height), (100.0, 80.0), "{params}");
    }
    assert_eq!(crate::board::connected_size(BoardShape::Rectangle, (100.0, 60.0), Some(BoardShape::Diamond)), (160.0, 120.0));
    assert_eq!(crate::board::connected_size(BoardShape::Rectangle, (100.0, 60.0), None), (100.0, 60.0));
}

/// `label` names the connector, through a label layer on it; an empty label makes none.
#[test]
fn a_label_is_set_on_the_connector_and_an_empty_one_is_none() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0, "width": 160, "height": 100})).unwrap(), "id");
    let r = s.execute("board.addConnected", &json!({"id": a, "side": "right", "shape": "diamond", "label": "  Yes  "})).unwrap();
    let lbl = id(&r, "label");
    assert_eq!(node(&s, lbl).parent, node(&s, id(&r, "connector")).parent, "a sibling of the connector");
    assert!(matches!(node(&s, lbl).board, Some(BoardItem::Label { connector }) if connector == id(&r, "connector")));
    let text = node(&s, node(&s, lbl).children[0]);
    assert!(matches!(&text.kind, figcraft_doc::NodeKind::Text(t) if t.characters == "Yes"), "trimmed text");
    let r = s.execute("board.addConnected", &json!({"id": a, "side": "below", "label": "   "})).unwrap();
    assert!(r.get("label").is_none(), "a blank label makes none");
}

/// Every refusal makes no layer, takes no undo step and changes no selection.
#[test]
fn a_refused_next_one_changes_nothing() {
    let mut s = board();
    let a = id(&s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0, "width": 160, "height": 100})).unwrap(), "id");
    let sticky = id(&s.execute("board.sticky", &json!({"x": 400, "y": 0, "text": "note"})).unwrap(), "id");
    let undo_depth = |s: &Session| s.undo_labels(usize::MAX).len();
    let (layers0, undo0, sel0) = (layers(&s), undo_depth(&s), s.selection());
    let refused = [
        json!({"id": a, "shape": "blob"}),
        json!({"id": sticky, "shape": "diamond"}),
        json!({"id": a, "label": "x".repeat(121)}),
        json!({"id": a, "side": "diagonal"}),
        // Not text: refused, not quietly read as "no shape" or "no label".
        json!({"id": a, "shape": 3}),
        json!({"id": a, "label": ["Yes"]}),
    ];
    for params in refused {
        assert!(s.execute("board.addConnected", &params).is_err(), "{params}");
        assert_eq!((layers(&s), undo_depth(&s), s.selection()), (layers0, undo0, sel0.clone()), "{params} changed something");
    }
    // The longest label that is allowed goes through.
    s.execute("board.addConnected", &json!({"id": a, "label": "x".repeat(120)})).unwrap();
}
