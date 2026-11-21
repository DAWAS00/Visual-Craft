//! The Use Case diagram in the window: the New diagram dialog makes one; the panel's picture
//! tools place elements and join them with a press on the canvas; a press on an element selects
//! it and a drag moves it (its layers are locked, so the canvas alone couldn't); Delete removes
//! the selection. Each change is one `diagram.useCase.*` command and one undo step.

use egui::{Event, Key, Modifiers, Pos2};
use figcraft_doc::use_case::{Link, Model};
use serde_json::json;

use crate::export_preview_tests::{click, labelled};
use crate::layers_drag_tests::Ui;
use crate::use_case_panel::canvas::screen_of;
use crate::use_case_panel::{Tool, load};

fn press(u: &mut Ui, text: &str) {
    let shown = labelled(u);
    let at = shown.iter().rev().find(|(t, _)| t == text).map(|x| x.1).unwrap_or_else(|| panic!("{text:?} in {shown:?}"));
    click(u, at);
}

fn texts(u: &mut Ui) -> Vec<String> {
    labelled(u).into_iter().map(|(t, _)| t).collect()
}

fn model(u: &Ui) -> Model {
    crate::use_case_panel::current(&u.app).map(|m| (*m).clone()).expect("a Use Case page in view")
}

fn diagram(template: &str) -> Ui {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.run("file.newDiagram", json!({"type": "useCase", "template": template}));
    u.frame(vec![]);
    u.frame(vec![]);
    u
}

/// The screen point of page point `(x, y)`, checked to be on the canvas clear of the panel.
fn at(u: &Ui, x: f64, y: f64) -> Pos2 {
    let p = screen_of(&u.app, x, y);
    assert!(u.app.canvas.rect.contains(p) && p.x < 1000.0, "{p:?} isn't on the canvas clear of the panel ({:?})", u.app.canvas.rect);
    p
}

fn center_of(u: &Ui, id: u64) -> Pos2 {
    let (x, y, w, h) = crate::use_case_panel::canvas::element_box(&model(u), id).expect("the element");
    at(u, x + w / 2.0, y + h / 2.0)
}

fn key(u: &mut Ui, key: Key) {
    let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    u.frame(vec![ev(true)]);
    u.frame(vec![ev(false)]);
    u.frame(vec![]);
}

/// A small diagram near the page's origin: an actor and a use case side by side.
fn two() -> (Ui, u64, u64) {
    let mut u = diagram("blank");
    let a = u.app.run("diagram.useCase.addActor", json!({"name": "Guest", "x": 0, "y": 0})).unwrap()["id"].as_u64().unwrap();
    let c = u.app.run("diagram.useCase.addUseCase", json!({"name": "Book", "x": 200, "y": 20})).unwrap()["id"].as_u64().unwrap();
    pin_view(&mut u);
    (u, a, c)
}

/// A fixed view: 100 % zoom, centred on page point (300, 150), so page points from (0, 0) to
/// about (400, 400) show on the canvas, clear of the panel at the right.
fn pin_view(u: &mut Ui) {
    let v = &mut u.app.session.ds_mut().unwrap().view;
    (v.zoom, v.cx, v.cy) = (1.0, 300.0, 150.0);
    u.frame(vec![]);
    u.frame(vec![]);
}

#[test]
fn the_dialog_makes_a_use_case_diagram_from_its_example() {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.run("dialog.newDiagram", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    press(&mut u, "Use case");
    assert!(texts(&mut u).iter().any(|t| t == "Example"), "the Use Case templates");
    press(&mut u, "Example");
    let d = u.app.ui.new_diagram.clone().unwrap();
    assert_eq!((d.kind, d.template), (1, 1));
    press(&mut u, "Create");
    assert!(u.app.ui.new_diagram.is_none(), "it closes");
    let m = model(&u);
    assert_eq!((m.use_cases.len(), m.actors.len(), m.links.len()), (4, 2, 5));
}

#[test]
fn the_panel_shows_its_picture_tools_on_a_use_case_page_only() {
    let mut u = diagram("useCaseExample");
    let shown = texts(&mut u);
    for want in
        ["Use case diagram", "Select", "Actor", "System", "Association", "Include", "Extend", "Generalization", "Reserve room", "Nothing unfinished"]
    {
        assert!(shown.iter().any(|t| t == want), "{want:?} in {shown:?}");
    }
    u.app.run("file.newDiagram", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    assert!(!texts(&mut u).iter().any(|t| t == "Use case diagram"), "no panel on a flowchart");
}

#[test]
fn an_add_tool_places_the_element_where_the_canvas_is_pressed() {
    let (mut u, ..) = two();
    let steps = u.app.session.ds().unwrap().history.undo.len();
    press(&mut u, "Actor");
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Place(crate::use_case_panel::Place::Actor));
    let p = at(&u, 120.0, 260.0);
    click(&mut u, p);
    let m = model(&u);
    let made = m.actors.iter().find(|a| a.name == "Actor 1").expect("a new actor");
    let q = u.app.to_page(p);
    assert!((made.x + made.w / 2.0 - q.x).abs() < 2.0 && (made.y + made.h / 2.0 - q.y).abs() < 2.0, "centred on the press: {made:?} {q:?}");
    let st = load(&u.ctx, &u.app);
    assert_eq!((st.tool, st.selected), (Tool::Select, Some(made.id.0)), "selected, and the tool goes back to Select");
    assert_eq!(u.app.session.ds().unwrap().history.undo.len(), steps + 1);
}

#[test]
fn a_press_selects_an_element_and_a_drag_moves_it() {
    let (mut u, actor, _) = two();
    let from = center_of(&u, actor);
    click(&mut u, from);
    assert_eq!(load(&u.ctx, &u.app).selected, Some(actor));
    assert!(texts(&mut u).iter().any(|t| t == "Guest"), "the card shows it");
    let before = model(&u).actors[0].clone();
    let to = from + egui::vec2(0.0, 120.0);
    u.drag_to(from, to);
    u.button(to, false);
    u.frame(vec![]);
    let after = model(&u).actors[0].clone();
    let dy = u.app.to_page(to).y - u.app.to_page(from).y;
    assert_eq!(after.x, before.x);
    assert!((after.y - before.y - dy).abs() <= 1.0, "moved by the drag: {} → {} ({dy})", before.y, after.y);
    // One step: undo puts it back.
    u.app.run("edit.undo", json!({}));
    assert_eq!(model(&u).actors[0].y, before.y);
}

#[test]
fn a_connect_tool_joins_the_two_elements_pressed() {
    let (mut u, actor, case) = two();
    press(&mut u, "Association");
    let (a, c) = (center_of(&u, actor), center_of(&u, case));
    click(&mut u, a);
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Connect { kind: 0, from: Some(actor) });
    click(&mut u, c);
    let m = model(&u);
    assert!(matches!(m.links.first(), Some(Link::Association { actor: x, use_case: y, .. }) if x.0 == actor && y.0 == case));
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Connect { kind: 0, from: None }, "ready for the next one");
    key(&mut u, Key::Escape);
    assert_eq!(load(&u.ctx, &u.app).tool, Tool::Select);
}

#[test]
fn a_refused_connection_changes_nothing() {
    let (mut u, actor, _) = two();
    let other = u.app.run("diagram.useCase.addActor", json!({"name": "Clerk", "x": 0, "y": 200})).unwrap()["id"].as_u64().unwrap();
    u.frame(vec![]);
    let before = model(&u);
    press(&mut u, "Include");
    let (a, b) = (center_of(&u, actor), center_of(&u, other));
    click(&mut u, a);
    click(&mut u, b);
    assert_eq!(model(&u), before);
}

#[test]
fn delete_removes_the_selected_element_and_its_links() {
    let (mut u, actor, case) = two();
    u.app.run("diagram.useCase.connect", json!({"kind": "association", "from": actor, "to": case}));
    u.frame(vec![]);
    let p = center_of(&u, actor);
    click(&mut u, p);
    key(&mut u, Key::Delete);
    let m = model(&u);
    assert!(m.actors.is_empty() && m.links.is_empty(), "{m:?}");
    assert_eq!(load(&u.ctx, &u.app).selected, None);
}

#[test]
fn a_use_case_resizes_from_its_corner_square_and_an_actor_has_none() {
    let (mut u, actor, case) = two();
    // The use case's right side clear of the panel.
    u.app.session.ds_mut().unwrap().view.cx = 420.0;
    u.frame(vec![]);
    u.frame(vec![]);
    let p = center_of(&u, case);
    click(&mut u, p);
    let (x, y, w, h) = crate::use_case_panel::canvas::element_box(&model(&u), case).unwrap();
    let steps = u.app.session.ds().unwrap().history.undo.len();
    let corner = at(&u, x + w, y + h);
    let to = at(&u, x + w + 40.0, y + h + 20.0);
    u.drag_to(corner, to);
    u.button(to, false);
    u.frame(vec![]);
    assert_eq!(crate::use_case_panel::canvas::element_box(&model(&u), case).unwrap(), (x, y, w + 40.0, h + 20.0));
    assert_eq!(u.app.session.ds().unwrap().history.undo.len(), steps + 1);
    assert!(crate::use_case_panel::canvas::resizable(&model(&u), actor).is_none(), "an actor keeps its figure's size");
}

