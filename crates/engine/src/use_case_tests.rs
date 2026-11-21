//! Use Case diagram commands (`diagram.useCase.*`) and `file.newDiagram {type: useCase}`: the
//! model and its generated layers change together, a refusal changes nothing and adds no undo
//! step, and a diagram is saved as format version 2 and opens again with its model.

use figcraft_doc::use_case::{LAYER_KEY, Link, Model};
use serde_json::{Value, json};

use crate::Session;

fn blank() -> Session {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    s
}

fn model(s: &Session) -> Model {
    let doc = s.doc().unwrap();
    (**doc.use_cases.get(&s.page().unwrap()).unwrap()).clone()
}

fn undo_len(s: &Session) -> usize {
    s.ds().unwrap().history.undo.len()
}

fn run(s: &mut Session, cmd: &str, p: Value) -> u64 {
    let r = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    r.get("id").or_else(|| r.get("element")).and_then(Value::as_u64).unwrap_or(0)
}

/// The generated layers on the page: (plugin data value, locked).
fn layers(s: &Session) -> Vec<(String, bool)> {
    let doc = s.doc().unwrap();
    let page = doc.node(s.page().unwrap()).unwrap();
    page.children.iter().filter_map(|c| doc.node(*c)).filter_map(|n| n.plugin_data.get(LAYER_KEY).map(|v| (v.clone(), n.locked))).collect()
}

/// Asserts `cmd` is refused and leaves the model and the undo history as they were.
fn refused(s: &mut Session, cmd: &str, p: Value) -> String {
    let (before, steps) = (model(s), undo_len(s));
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} should be refused")).to_string();
    assert_eq!(model(s), before, "{cmd} {p} changed the model");
    assert_eq!(undo_len(s), steps, "{cmd} {p} added an undo step");
    e
}

#[test]
fn a_blank_use_case_diagram_is_an_empty_model_on_its_page() {
    let mut s = Session::new();
    let r = s.execute("file.newDiagram", &json!({"type": "useCase", "name": "Booking"})).unwrap();
    assert_eq!((r["type"].as_str(), r["template"].as_str()), (Some("useCase"), Some("blank")));
    assert_eq!(model(&s), Model::default());
    assert!(layers(&s).is_empty());
    let read = s.execute("diagram.useCase.read", &json!({})).unwrap();
    assert_eq!(read["schema"], json!(1));
}

#[test]
fn the_example_template_is_the_room_booking_diagram() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let m = model(&s);
    assert_eq!((m.subjects.len(), m.use_cases.len(), m.actors.len(), m.links.len()), (1, 4, 2, 5));
    assert!(figcraft_doc::use_case::check(&m).is_empty(), "{:?}", figcraft_doc::use_case::check(&m));
    let ls = layers(&s);
    assert!(ls.iter().all(|(_, locked)| *locked), "generated layers are locked");
    for role in ["boundary", "card", "link", "marker", "label"] {
        assert!(ls.iter().any(|(v, _)| v.ends_with(&format!(":{role}"))), "a {role} layer: {ls:?}");
    }
    // The template is where the tab starts: nothing to undo.
    assert_eq!(undo_len(&s), 0);
}

#[test]
fn other_diagram_types_keep_their_templates() {
    let mut s = Session::new();
    let e = s.execute("file.newDiagram", &json!({"type": "useCase", "template": "flowChart"})).unwrap_err().to_string();
    assert!(e.contains("useCaseExample"), "{e}");
    let e = s.execute("file.newDiagram", &json!({"template": "useCaseExample"})).unwrap_err().to_string();
    assert!(e.contains("flowChart"), "{e}");
}

#[test]
fn elements_find_free_room_when_no_position_is_given() {
    let mut s = blank();
    let sys = run(&mut s, "diagram.useCase.addSubject", json!({"name": "Shop"}));
    let a = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Buy", "subject": sys}));
    let b = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Pay", "subject": sys}));
    let actor = run(&mut s, "diagram.useCase.addActor", json!({"name": "Customer"}));
    let m = model(&s);
    assert_eq!(figcraft_doc::use_case::validate(&m), Ok(()));
    assert!(m.use_cases.iter().all(|u| u.subject.map(|x| x.0) == Some(sys)));
    assert_ne!((a, b), (b, a));
    assert!(m.actors.iter().any(|x| x.id.0 == actor));
    assert_eq!(undo_len(&s), 4, "one undo step per command");
}

#[test]
fn each_command_redraws_and_undo_restores_model_and_layers() {
    let mut s = blank();
    let actor = run(&mut s, "diagram.useCase.addActor", json!({"name": "Guest", "x": 40, "y": 40}));
    let case = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Book", "x": 300, "y": 40}));
    let (m1, l1) = (model(&s), layers(&s));
    run(&mut s, "diagram.useCase.connect", json!({"kind": "association", "from": case, "to": actor}));
    assert_eq!(layers(&s).iter().filter(|(v, _)| v.ends_with(":link")).count(), 1);
    // Drawn either way, stored actor first.
    assert!(matches!(model(&s).links.first(), Some(Link::Association { actor: a, .. }) if a.0 == actor));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!((model(&s), layers(&s)), (m1, l1));
}

#[test]
fn invalid_edits_are_refused_and_change_nothing() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let m = model(&s);
    let id_of = |name: &str| -> u64 {
        m.actors.iter().find(|a| a.name == name).map(|a| a.id.0).or_else(|| m.use_cases.iter().find(|u| u.name == name).map(|u| u.id.0)).unwrap()
    };
    let (guest, member, reserve, check) = (id_of("Guest"), id_of("Member"), id_of("Reserve room"), id_of("Check availability"));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "association", "from": guest, "to": member}));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "include", "from": guest, "to": reserve}));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "generalization", "from": member, "to": reserve}));
    let e = refused(&mut s, "diagram.useCase.connect", json!({"kind": "include", "from": check, "to": reserve}));
    assert!(e.contains("cycle"), "{e}");
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "include", "from": reserve, "to": reserve}));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "include", "from": reserve, "to": check}));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "extend", "from": check, "to": reserve}));
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "include", "from": reserve, "to": check, "condition": "x"}));
    // An extend with a new point that is refused leaves no point behind.
    refused(&mut s, "diagram.useCase.connect", json!({"kind": "extend", "from": reserve, "to": reserve, "newPoint": "Somewhere"}));
    let point = model(&s).use_cases.iter().flat_map(|u| u.points.iter()).map(|p| p.id.0).next().unwrap();
    let e = refused(&mut s, "diagram.useCase.removeExtensionPoint", json!({"id": point}));
    assert!(e.contains("extend"), "{e}");
    refused(&mut s, "diagram.useCase.move", json!({"id": reserve, "x": 2000, "y": 2000}));
    // Taken out of the subject but left inside its rectangle.
    refused(&mut s, "diagram.useCase.setSubject", json!({"useCase": reserve, "subject": null, "x": 340, "y": 100}));
    refused(&mut s, "diagram.useCase.move", json!({"id": guest, "x": 400, "y": 200}));
    refused(&mut s, "diagram.useCase.addActor", json!({"name": "  "}));
    refused(&mut s, "diagram.useCase.remove", json!({"ids": [99_999]}));
}

#[test]
fn moving_a_subject_moves_its_use_cases_but_resizing_does_not() {
    let mut s = blank();
    let sys = run(&mut s, "diagram.useCase.addSubject", json!({"name": "Shop", "x": 200, "y": 0, "width": 500, "height": 400}));
    let case = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Buy", "subject": sys, "x": 260, "y": 60}));
    run(&mut s, "diagram.useCase.move", json!({"id": sys, "x": 300, "y": 100}));
    let at = |s: &Session| model(s).use_cases.iter().find(|u| u.id.0 == case).map(|u| (u.x, u.y)).unwrap();
    assert_eq!(at(&s), (360.0, 160.0));
    run(&mut s, "diagram.useCase.resize", json!({"id": sys, "x": 280, "y": 100, "width": 520, "height": 400}));
    assert_eq!(at(&s), (360.0, 160.0));
}

#[test]
fn add_connected_extension_points_back_to_its_base_in_one_step() {
    let mut s = blank();
    let base = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Return book", "x": 300, "y": 40}));
    let steps = undo_len(&s);
    let r = s
        .execute(
            "diagram.useCase.addConnected",
            &json!({"source": base, "relation": "extension", "name": "Pay fine", "newPoint": "Late return", "condition": "overdue"}),
        )
        .unwrap();
    let new = r["element"].as_u64().unwrap();
    let point = r["point"].as_u64().unwrap();
    let m = model(&s);
    assert!(
        matches!(m.links.first(), Some(Link::Extend { extending, base: b, points, .. }) if extending.0 == new && b.0 == base && points.iter().any(|p| p.0 == point))
    );
    assert!(m.use_cases.iter().find(|u| u.id.0 == base).unwrap().points.iter().any(|p| p.id.0 == point));
    assert_eq!(undo_len(&s), steps + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    let m = model(&s);
    assert_eq!((m.use_cases.len(), m.links.len()), (1, 0));
    assert!(m.use_cases.iter().all(|u| u.points.is_empty()));
}

#[test]
fn removing_an_actor_takes_its_links_and_removing_a_subject_keeps_its_cases() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let m = model(&s);
    let guest = m.actors.iter().find(|a| a.name == "Guest").unwrap().id.0;
    let sys = m.subjects[0].id.0;
    run(&mut s, "diagram.useCase.remove", json!({"ids": [guest, sys]}));
    let m = model(&s);
    assert_eq!(m.actors.len(), 1);
    // The association and Member's generalization to Guest went with Guest.
    assert_eq!(m.links.len(), 3);
    assert!(m.subjects.is_empty());
    assert_eq!(m.use_cases.len(), 4);
    assert!(m.use_cases.iter().all(|u| u.subject.is_none()));
}

#[test]
fn a_rename_to_the_same_name_changes_nothing() {
    let mut s = blank();
    let case = run(&mut s, "diagram.useCase.addUseCase", json!({"name": "Buy", "x": 40, "y": 40}));
    let steps = undo_len(&s);
    let r = s.execute("diagram.useCase.updateElement", &json!({"id": case, "name": " Buy "})).unwrap();
    assert_eq!(r["changed"], json!(false));
    assert_eq!(undo_len(&s), steps);
    let r = s.execute("diagram.useCase.updateElement", &json!({"id": case, "name": "Buy a much longer thing to measure"})).unwrap();
    assert_eq!(r["changed"], json!(true));
    let u = model(&s).use_cases[0].clone();
    assert!(u.w >= u.min_w && u.min_w > 120.0, "{u:?}");
}

#[test]
fn other_layers_on_the_page_are_left_alone() {
    let mut s = blank();
    let page = s.page().unwrap();
    let doc = s.doc_mut().unwrap();
    let own = doc.create(page, figcraft_doc::NodeKind::Rectangle, "Mine", 0.0, 900.0, 50.0, 50.0).unwrap();
    run(&mut s, "diagram.useCase.addActor", json!({"name": "Guest", "x": 40, "y": 40}));
    run(&mut s, "diagram.useCase.updateElement", json!({"id": 1, "name": "Visitor"}));
    assert!(s.doc().unwrap().node(own).is_some());
}

#[test]
fn the_diagram_is_saved_and_opened_and_a_broken_model_is_a_problem() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let bytes = crate::cmd::file::file_bytes(s.ds().unwrap()).unwrap();
    let (opened, issues) = crate::cmd::file::load_bytes(&bytes, "d.figcraft").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(opened.use_cases, s.doc().unwrap().use_cases);
    // Under the plain header (which no FigCraft writes for a diagram) it isn't opened: typed
    // records aren't migrated.
    let mut v = crate::typed_safety_tests::typed_json(s.doc().unwrap());
    let e = crate::cmd::file::load_bytes(v.to_string().as_bytes(), "d.figcraft").unwrap_err().to_string();
    assert!(e.contains(figcraft_format::TYPED_LOAD_REFUSAL), "{e}");
    // In memory, the document's JSON (written aside) reads back with its model.
    let page = s.page().unwrap();
    let back: figcraft_doc::Document = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(back.use_cases.get(&page).map(|m| (**m).clone()), Some(model(&s)));
    assert_eq!(back.use_case_problem(), None);
    // An include cycle written in by hand is a problem the reader reports.
    let key = page.0.to_string();
    let links = v["use_cases"][&key]["links"].as_array_mut().unwrap();
    let include = links.iter().find(|l| l["kind"] == "include").cloned().unwrap();
    links.push(json!({"kind": "include", "id": 9_999, "including": include["included"], "included": include["including"]}));
    let broken: figcraft_doc::Document = serde_json::from_value(v).unwrap();
    let e = broken.use_case_problem().unwrap();
    assert!(e.contains("cycle"), "{e}");
}

#[test]
fn commands_refuse_a_page_that_is_not_a_use_case_diagram() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({})).unwrap();
    let e = s.execute("diagram.useCase.addActor", &json!({"name": "Guest"})).unwrap_err().to_string();
    assert!(e.contains("isn't a Use Case diagram"), "{e}");
}

#[test]
fn the_check_reports_unfinished_work() {
    let mut s = blank();
    run(&mut s, "diagram.useCase.addActor", json!({"name": "Guest", "x": 40, "y": 40}));
    let r = s.execute("diagram.useCase.check", &json!({})).unwrap();
    assert_eq!(r["warnings"].as_array().map(Vec::len), Some(1));
}

#[test]
fn taking_a_use_case_out_of_its_subject_moves_it_to_free_room() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let id = model(&s).use_cases[0].id.0;
    run(&mut s, "diagram.useCase.setSubject", json!({"useCase": id, "subject": null}));
    let m = model(&s);
    let u = m.use_cases.iter().find(|u| u.id.0 == id).unwrap();
    assert!(u.subject.is_none());
    assert!(u.x >= m.subjects[0].x + m.subjects[0].width, "{u:?}");
    let sys = m.subjects[0].id.0;
    run(&mut s, "diagram.useCase.setSubject", json!({"useCase": id, "subject": sys}));
    assert_eq!(model(&s).use_cases.iter().find(|u| u.id.0 == id).unwrap().subject.map(|x| x.0), Some(sys));
}

#[test]
fn add_connected_with_a_null_subject_makes_the_case_where_asked_in_no_system() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let guest = model(&s).actors.iter().find(|a| a.name == "Guest").unwrap().id.0;
    let r = s
        .execute(
            "diagram.useCase.addConnected",
            &json!({"source": guest, "relation": "association", "name": "Ask for help", "x": -2000, "y": -2000, "subject": null}),
        )
        .unwrap();
    let new = r["element"].as_u64().unwrap();
    let m = model(&s);
    let case = m.use_cases.iter().find(|u| u.id.0 == new).unwrap();
    assert_eq!((case.x, case.y, case.subject), (-2000.0, -2000.0, None));
}

#[test]
fn check_connect_answers_as_connect_would_without_changing_anything() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let m = model(&s);
    let guest = m.actors.iter().find(|a| a.name == "Guest").unwrap().id.0;
    let joined = m.links.iter().find_map(|l| match l {
        figcraft_doc::use_case::Link::Association { actor, use_case, .. } if actor.0 == guest => Some(use_case.0),
        _ => None,
    });
    let joined = joined.unwrap();
    let free = m
        .use_cases
        .iter()
        .map(|u| u.id.0)
        .find(|u| crate::use_case_check_connect(&m, &json!({"kind": "association", "from": guest, "to": u})).is_ok());
    assert!(crate::use_case_check_connect(&m, &json!({"kind": "association", "from": guest, "to": joined})).is_err(), "a duplicate is refused");
    assert!(crate::use_case_check_connect(&m, &json!({"kind": "include", "from": guest, "to": joined})).is_err(), "an actor can't include");
    assert_eq!(model(&s), m, "nothing changed");
    if let Some(u) = free {
        s.execute("diagram.useCase.connect", &json!({"kind": "association", "from": guest, "to": u})).unwrap();
    }
}

#[test]
fn check_connect_refuses_an_unknown_parameter_as_dispatch_does() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let m = model(&s);
    let (a, b) = (m.use_cases[0].id.0, m.use_cases[1].id.0);
    let ok = crate::use_case_check_connect(&m, &json!({"kind": "generalization", "from": a, "to": b}));
    let odd = crate::use_case_check_connect(&m, &json!({"kind": "generalization", "from": a, "to": b, "colour": "red"}));
    assert!(ok.is_ok() || ok.as_ref().is_err_and(|e| !e.contains("unknown parameter")));
    assert!(odd.is_err_and(|e| e.contains("unknown parameter")));
}
