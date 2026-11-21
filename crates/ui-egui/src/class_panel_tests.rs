//! The Class diagram in the window: the New diagram dialog makes one; the panel's picture tools
//! place cards and join them with a press on the canvas; a press on a card selects it (and its
//! member row) and a drag moves it; the panel edits members as UML text; Delete removes the
//! selected row or card. Each change is one `diagram.class.*` command and one undo step.

use egui::{Event, Key, Modifiers, Pos2};
use figcraft_doc::class_diagram::{LinkKind, Model};
use serde_json::json;

use crate::class_panel::canvas::screen_of;
use crate::class_panel::{Place, Tool, load};
use crate::export_preview_tests::{click, labelled};
use crate::layers_drag_tests::Ui;

fn press(u: &mut Ui, text: &str) {
    let shown = labelled(u);
    let at = shown.iter().rev().find(|(t, _)| t == text).map(|x| x.1).unwrap_or_else(|| panic!("{text:?} in {shown:?}"));
    click(u, at);
}

fn texts(u: &mut Ui) -> Vec<String> {
    labelled(u).into_iter().map(|(t, _)| t).collect()
}

fn model(u: &Ui) -> Model {
    crate::class_panel::current(&u.app).map(|m| (*m).clone()).expect("a Class page in view")
}

fn undo_len(u: &Ui) -> usize {
    u.app.session.ds().unwrap().history.undo.len()
}

fn diagram(template: &str) -> Ui {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.run("file.newDiagram", json!({"type": "class", "template": template}));
    u.frame(vec![]);
    u.frame(vec![]);
    u
}

/// The screen point of page point `(x, y)`, checked to be on the canvas clear of the panel.
fn at(u: &Ui, x: f64, y: f64) -> Pos2 {
    let p = screen_of(&u.app, x, y);
    assert!(u.app.canvas.rect.contains(p) && p.x < 900.0, "{p:?} isn't on the canvas clear of the panel ({:?})", u.app.canvas.rect);
    p
}

fn center_of(u: &Ui, id: u64) -> Pos2 {
    let (x, y, w, h) = crate::class_panel::canvas::card_box(&model(u), id).expect("the card");
    at(u, x + w / 2.0, y + h / 2.0)
}

fn key(u: &mut Ui, key: Key) {
    let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    u.frame(vec![ev(true)]);
    u.frame(vec![ev(false)]);
    u.frame(vec![]);
}

/// Two classes side by side near the page's origin, in a fixed view: 100 % zoom centred on
/// (300, 150), so page points from (0, 0) to about (400, 400) show clear of the panel.
fn two() -> (Ui, u64, u64) {
    let mut u = diagram("blank");
    let a = u.app.run("diagram.class.addClassifier", json!({"name": "Shelf", "x": 0, "y": 0})).unwrap()["id"].as_u64().unwrap();
    let b = u.app.run("diagram.class.addClassifier", json!({"name": "Book", "x": 0, "y": 200})).unwrap()["id"].as_u64().unwrap();
    let v = &mut u.app.session.ds_mut().unwrap().view;
    (v.zoom, v.cx, v.cy) = (1.0, 300.0, 150.0);
    u.frame(vec![]);
    u.frame(vec![]);
    (u, a, b)
}

#[test]
fn the_dialog_makes_a_class_diagram_from_library_loans() {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.run("dialog.newDiagram", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    press(&mut u, "Class");
    press(&mut u, "Library loans");
    let d = u.app.ui.new_diagram.clone().unwrap();
    assert_eq!((d.kind, d.template), (2, 1));
    press(&mut u, "Create");
    assert!(u.app.ui.new_diagram.is_none(), "it closes");
    let m = model(&u);
    assert!(m.classifiers.iter().any(|c| c.name == "Book") && m.links.len() >= 6, "{} links", m.links.len());
}

#[test]
fn the_panel_shows_its_picture_tools_on_a_class_page_only() {
    let mut u = diagram("libraryLoans");
    let shown = texts(&mut u);
    for want in [
        "Class diagram",
        "Select",
        "Class",
        "Interface",
        "Enumeration",
        "Association",
        "Aggregation",
        "Composition",
        "Generalization",
        "Realization",
        "Dependency",
        "Nothing unfinished",
        "Library",
    ] {
        assert!(shown.iter().any(|t| t == want), "{want:?} in {shown:?}");
    }
    u.app.run("file.newDiagram", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(!texts(&mut u).iter().any(|t| t == "Class diagram"), "no panel on a flowchart");
}

#[test]
fn an_add_tool_places_a_class_where_the_canvas_is_pressed() {
    let (mut u, ..) = two();
    let steps = undo_len(&u);
    press(&mut u, "Interface");
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Place(Place::Interface));
    let p = at(&u, 160.0, 280.0);
    click(&mut u, p);
    let m = model(&u);
    let made = m.classifiers.iter().find(|c| c.name == "Interface 1").expect("a new interface");
    assert_eq!(made.kind, figcraft_doc::class_diagram::Kind::Interface);
    let st = load(&u.ctx, &u.app);
    assert_eq!((st.tool, st.selected), (Tool::Select, Some(made.id.0)), "selected, and the tool goes back to Select");
    assert_eq!(undo_len(&u), steps + 1);
}

#[test]
fn a_press_selects_a_card_and_its_row_and_a_drag_moves_it() {
    let (mut u, shelf, _) = two();
    let row =
        u.app.run("diagram.class.addMember", json!({"classifier": shelf, "compartment": "attribute", "text": "- label : String"})).unwrap()["id"]
            .as_u64()
            .unwrap();
    u.frame(vec![]);
    let c = model(&u).classifiers[0].clone();
    let r = figcraft_engine::class_layout::card_layout(&c).compartments.iter().flat_map(|k| k.rows.clone()).next().unwrap();
    let on_row = at(&u, r.rect.center().x, r.rect.center().y);
    click(&mut u, on_row);
    let st = load(&u.ctx, &u.app);
    assert_eq!((st.selected, st.member), (Some(shelf), Some(row)), "the card and the row under the press");
    assert!(texts(&mut u).iter().any(|t| t.starts_with("Class Shelf, 1 attributes")), "the card summary");
    let from = center_of(&u, shelf);
    let to = from + egui::vec2(160.0, 0.0);
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    let after = model(&u).classifiers[0].clone();
    let dx = u.app.to_page(to).x - u.app.to_page(from).x;
    assert!((after.x - c.x - dx).abs() <= 1.0, "moved by the drag: {} → {} ({dx})", c.x, after.x);
    u.app.run("edit.undo", json!({}));
    assert_eq!(model(&u).classifiers[0].x, c.x, "one step");
}

#[test]
fn a_connect_tool_joins_the_two_cards_pressed() {
    let (mut u, shelf, book) = two();
    press(&mut u, "Aggregation");
    let (a, b) = (center_of(&u, shelf), center_of(&u, book));
    click(&mut u, a);
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Connect { kind: LinkKind::Aggregation, from: Some(shelf) });
    click(&mut u, b);
    let m = model(&u);
    let l = m.links.first().unwrap_or_else(|| {
        panic!(
            "a relationship: {:?} {:?} b={b:?} {:?}",
            u.app.ui.last_error,
            load(&u.ctx, &u.app),
            m.classifiers.iter().map(|c| (c.x, c.y, c.w, c.h)).collect::<Vec<_>>()
        )
    });
    assert_eq!((l.kind, l.ends[0].classifier.0, l.ends[1].classifier.0), (LinkKind::Aggregation, shelf, book));
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Connect { kind: LinkKind::Aggregation, from: None }, "ready for the next one");
    key(&mut u, Key::Escape);
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Select);
    // A press near the line selects the relationship.
    let mid = at(&u, 40.0, 140.0);
    click(&mut u, mid);
    assert_eq!(load(&u.ctx, &u.app).selected, Some(l.id.0));
}

#[test]
fn a_refused_connection_changes_nothing() {
    let (mut u, shelf, book) = two();
    u.app.run("diagram.class.connect", json!({"kind": "generalization", "from": shelf, "to": book}));
    u.frame(vec![]);
    let before = model(&u);
    press(&mut u, "Generalization");
    let (a, b) = (center_of(&u, book), center_of(&u, shelf));
    click(&mut u, a);
    click(&mut u, b);
    assert_eq!(model(&u), before, "a cycle is refused");
}

#[test]
fn delete_removes_the_selected_row_then_the_card_and_its_links() {
    let (mut u, shelf, book) = two();
    u.app.run("diagram.class.addMember", json!({"classifier": shelf, "compartment": "attribute", "text": "x : Int"}));
    u.app.run("diagram.class.connect", json!({"kind": "association", "from": shelf, "to": book}));
    u.frame(vec![]);
    let c = model(&u).classifiers[0].clone();
    let r = figcraft_engine::class_layout::card_layout(&c).compartments.iter().flat_map(|k| k.rows.clone()).next().unwrap();
    let p = at(&u, r.rect.center().x, r.rect.center().y);
    click(&mut u, p);
    key(&mut u, Key::Delete);
    let m = model(&u);
    assert!(m.classifiers[0].attributes.is_empty(), "the row first");
    assert_eq!(m.classifiers.len(), 2);
    key(&mut u, Key::Delete);
    let m = model(&u);
    assert!(m.classifiers.len() == 1 && m.links.is_empty(), "{m:?}");
    assert_eq!(load(&u.ctx, &u.app).selected, None);
}

fn chord(u: &mut Ui, key: Key, modifiers: Modifiers) {
    let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
    u.frame(vec![ev(true)]);
    u.frame(vec![ev(false)]);
    u.frame(vec![]);
}

fn type_text(u: &mut Ui, text: &str) {
    u.frame(vec![Event::Text(text.to_string())]);
    u.frame(vec![]);
}

fn select_card(u: &mut Ui, id: u64) {
    let p = center_of(u, id);
    click(u, p);
}

#[test]
fn alt_shift_a_opens_a_draft_that_enter_commits_and_the_next_draft_follows() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    let steps = undo_len(&u);
    chord(&mut u, Key::A, Modifiers::ALT | Modifiers::SHIFT);
    assert!(load(&u.ctx, &u.app).editor.is_some(), "a draft opens on the card");
    assert_eq!(undo_len(&u), steps, "a draft is not a change");
    type_text(&mut u, "- balance : Money = 0");
    key(&mut u, Key::Enter);
    let c = model(&u).classifiers[0].clone();
    let a = c.attributes.first().expect("the attribute");
    assert_eq!((a.name.as_str(), a.type_name.as_deref(), a.default.as_deref()), ("balance", Some("Money"), Some("0")));
    assert_eq!(undo_len(&u), steps + 1, "one step");
    let e = load(&u.ctx, &u.app).editor.expect("the next draft opens below");
    assert_eq!((e.member, e.after), (None, Some(a.id.0)));
    // Escape closes the empty draft with no change.
    key(&mut u, Key::Escape);
    assert!(load(&u.ctx, &u.app).editor.is_none());
    assert_eq!(undo_len(&u), steps + 1);
}

#[test]
fn a_bad_line_shows_why_and_keeps_the_text() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    chord(&mut u, Key::O, Modifiers::ALT | Modifiers::SHIFT);
    type_text(&mut u, "renew(days Int");
    key(&mut u, Key::Enter);
    let e = load(&u.ctx, &u.app).editor.expect("still open");
    assert!(e.error.is_some(), "the reason shows");
    assert_eq!(e.text, "renew(days Int");
    assert!(model(&u).classifiers[0].operations.is_empty());
}

#[test]
fn a_draft_goes_on_undo() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    chord(&mut u, Key::A, Modifiers::ALT | Modifiers::SHIFT);
    assert!(load(&u.ctx, &u.app).editor.is_some());
    u.app.run("edit.undo", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(load(&u.ctx, &u.app).editor.is_none());
}

#[test]
fn a_double_click_on_a_row_types_over_it() {
    let (mut u, shelf, _) = two();
    u.app.run("diagram.class.addMember", json!({"classifier": shelf, "compartment": "attribute", "text": "x : Int"}));
    u.frame(vec![]);
    let c = model(&u).classifiers[0].clone();
    let r = figcraft_engine::class_layout::card_layout(&c).compartments.iter().flat_map(|k| k.rows.clone()).next().unwrap();
    let p = at(&u, r.rect.center().x, r.rect.center().y);
    u.frame(vec![Event::PointerMoved(p)]);
    for _ in 0..2 {
        for pressed in [true, false] {
            u.frame(vec![Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
        }
    }
    u.frame(vec![]);
    let e = load(&u.ctx, &u.app).editor.expect("the row editor");
    assert_eq!((e.member, e.text.as_str()), (Some(r.member.0), "+ x : Int"));
}

#[test]
fn brackets_step_and_arrows_move_the_selection() {
    let (mut u, shelf, book) = two();
    key(&mut u, Key::CloseBracket);
    assert_eq!(load(&u.ctx, &u.app).selected, Some(shelf));
    key(&mut u, Key::CloseBracket);
    assert_eq!(load(&u.ctx, &u.app).selected, Some(book));
    let before = model(&u).classifiers[1].clone();
    key(&mut u, Key::ArrowDown);
    chord(&mut u, Key::ArrowRight, Modifiers::SHIFT);
    let after = model(&u).classifiers[1].clone();
    assert_eq!((after.x - before.x, after.y - before.y), (1.0, 8.0));
}

#[test]
fn the_drag_out_handle_makes_an_interface_the_class_implements() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    let handle = crate::class_panel::edit::handle_pos(&u.app, &model(&u), shelf).unwrap();
    let drop = at(&u, 260.0, 120.0);
    u.drag_to(handle, drop);
    u.button(drop, false);
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(load(&u.ctx, &u.app).out.as_ref().is_some_and(|o| o.dropped.is_some()), "the chooser opens");
    let steps = undo_len(&u);
    press(&mut u, "Implements new interface");
    let m = model(&u);
    let new = m
        .classifiers
        .iter()
        .find(|c| c.kind == figcraft_doc::class_diagram::Kind::Interface)
        .unwrap_or_else(|| panic!("a new interface: {:?} {:?}", u.app.ui.last_error, load(&u.ctx, &u.app)));
    let l = m.links.iter().find(|l| l.kind == LinkKind::Realization).expect("a realization");
    assert_eq!((l.ends[0].classifier.0, l.ends[1].classifier), (shelf, new.id));
    assert_eq!(undo_len(&u), steps + 1, "one step");
}

#[test]
fn an_operation_typed_in_an_attribute_draft_goes_to_the_operations() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    chord(&mut u, Key::A, Modifiers::ALT | Modifiers::SHIFT);
    type_text(&mut u, "+ renew(days: Int) : Bool");
    key(&mut u, Key::Enter);
    let c = model(&u).classifiers[0].clone();
    assert!(c.attributes.is_empty() && c.operations.len() == 1, "{c:?}");
}

#[test]
fn escape_closes_a_refused_draft_without_trying_again() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    chord(&mut u, Key::O, Modifiers::ALT | Modifiers::SHIFT);
    type_text(&mut u, "two words(");
    key(&mut u, Key::Enter);
    let e = load(&u.ctx, &u.app).editor.expect("still open");
    let why = e.error.expect("the reason");
    assert!(!why.starts_with("invalid parameters"), "the reason alone: {why}");
    let steps = undo_len(&u);
    key(&mut u, Key::Escape);
    assert!(load(&u.ctx, &u.app).editor.is_none(), "closed");
    assert_eq!(undo_len(&u), steps);
}

#[test]
fn the_element_the_handle_makes_is_selected() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    let handle = crate::class_panel::edit::handle_pos(&u.app, &model(&u), shelf).unwrap();
    let drop = at(&u, 260.0, 120.0);
    u.drag_to(handle, drop);
    u.button(drop, false);
    u.frame(vec![]);
    u.frame(vec![]);
    press(&mut u, "Subclass");
    u.frame(vec![]);
    let made = model(&u).classifiers.iter().find(|c| c.name == "Class 1").map(|c| c.id.0).expect("a subclass");
    assert_eq!(load(&u.ctx, &u.app).selected, Some(made));
}

#[test]
fn a_corner_square_resizes_the_selected_card_in_one_step() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    let (x, y, w, h) = crate::class_panel::canvas::card_box(&model(&u), shelf).unwrap();
    let steps = undo_len(&u);
    let corner = at(&u, x + w, y + h);
    let to = at(&u, x + w + 60.0, y + h + 30.0);
    u.drag_to(corner, to);
    assert_eq!(crate::class_panel::canvas::card_box(&model(&u), shelf).unwrap(), (x, y, w, h), "nothing changes while dragging");
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(crate::class_panel::canvas::card_box(&model(&u), shelf).unwrap(), (x, y, w + 60.0, h + 30.0));
    assert_eq!(undo_len(&u), steps + 1, "one undo step");
}

#[test]
fn an_edge_stops_at_what_the_text_needs_and_the_other_side_stays() {
    let (mut u, shelf, _) = two();
    select_card(&mut u, shelf);
    let c = model(&u).classifiers.iter().find(|c| c.id.0 == shelf).cloned().unwrap();
    let left = at(&u, c.x, c.y + c.h / 2.0);
    let far = at(&u, c.x + c.w + 100.0, c.y + c.h / 2.0);
    u.drag_to(left, far);
    u.button(far, false);
    u.frame(vec![]);
    let (x, _, w, _) = crate::class_panel::canvas::card_box(&model(&u), shelf).unwrap();
    assert_eq!(w, c.min_w, "never below its minimum");
    assert!((x + w - (c.x + c.w)).abs() < 1e-9, "the right edge stays at {}", c.x + c.w);
}
