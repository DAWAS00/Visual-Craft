//! Class diagram commands (`diagram.class.*`) and `file.newDiagram {type: class}`: the model and
//! its generated layers change together, a refusal changes nothing and adds no undo step, and the
//! layout the interface hit-tests with matches what is drawn.

use std::collections::HashSet;

use figcraft_doc::class_diagram::{self as cd, ClassifierId, Kind, LAYER_KEY, LinkKind, Model, Navigable};
use serde_json::{Value, json};

use crate::Session;
use crate::class_diagram::layout::{card_layout, link_paths};

fn blank() -> Session {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "class"})).unwrap();
    s
}

fn library() -> Session {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "class", "template": "libraryLoans"})).unwrap();
    s
}

fn model(s: &Session) -> Model {
    let doc = s.doc().unwrap();
    (**doc.class_diagrams.get(&s.page().unwrap()).unwrap()).clone()
}

fn undo_len(s: &Session) -> usize {
    s.ds().unwrap().history.undo.len()
}

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn new_id(s: &mut Session, cmd: &str, p: Value) -> u64 {
    run(s, cmd, p)["id"].as_u64().unwrap()
}

/// The generated layers on the page: their plugin data values.
fn layers(s: &Session) -> Vec<String> {
    let doc = s.doc().unwrap();
    let page = doc.node(s.page().unwrap()).unwrap();
    page.children.iter().filter_map(|c| doc.node(*c)).filter_map(|n| n.plugin_data.get(LAYER_KEY).cloned()).collect()
}

/// Every generated layer: the tagged page children and everything inside them (a card's box,
/// header, texts and dividers).
fn layer_count(s: &Session) -> usize {
    fn subtree(doc: &figcraft_doc::Document, id: figcraft_doc::NodeId) -> usize {
        1 + doc.node(id).map_or(0, |n| n.children.iter().map(|c| subtree(doc, *c)).sum())
    }
    let doc = s.doc().unwrap();
    let page = doc.node(s.page().unwrap()).unwrap();
    page.children.iter().filter(|c| doc.node(**c).is_some_and(|n| n.plugin_data.contains_key(LAYER_KEY))).map(|c| subtree(doc, *c)).sum()
}

/// What a refusal or a no-op must leave alone: the model, the layers and the undo depth.
fn state(s: &Session) -> (Model, Vec<String>, usize, usize) {
    (model(s), layers(s), layer_count(s), undo_len(s))
}

/// Asserts `cmd` is refused and leaves the model, the layers and the undo history as they were.
fn refused(s: &mut Session, cmd: &str, p: Value) -> String {
    let before = state(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} should be refused")).to_string();
    assert_eq!(state(s), before, "{cmd} {p} changed the model, its layers or the undo history");
    e
}

/// Asserts `cmd` answers `changed: false` and leaves everything as it was.
fn no_op(s: &mut Session, cmd: &str, p: Value) {
    let before = state(s);
    let r = run(s, cmd, p.clone());
    assert_eq!(r["changed"].as_bool(), Some(false), "{cmd} {p}");
    assert_eq!(state(s), before, "{cmd} {p} changed the model, its layers or the undo history");
}

/// The page-space boxes of link `id`'s generated layers with `role`.
fn link_layers(s: &Session, id: u64, role: &str) -> Vec<figcraft_geom::Rect> {
    let doc = s.doc().unwrap();
    let page = doc.node(s.page().unwrap()).unwrap();
    let want = format!("{id}:{role}");
    page.children
        .iter()
        .filter(|c| doc.node(**c).is_some_and(|n| n.plugin_data.get(LAYER_KEY) == Some(&want)))
        .map(|c| doc.world_bounds(*c).unwrap())
        .collect()
}

/// Whether `p` lies on the edge of `r`.
fn on_edge(r: figcraft_geom::Rect, p: figcraft_geom::Point) -> bool {
    let e = 1e-6;
    let inside = p.x >= r.x0 - e && p.x <= r.x1 + e && p.y >= r.y0 - e && p.y <= r.y1 + e;
    let edge = [(p.x - r.x0).abs(), (p.x - r.x1).abs(), (p.y - r.y0).abs(), (p.y - r.y1).abs()].into_iter().fold(f64::INFINITY, f64::min);
    inside && edge <= e
}

fn card_box(c: &cd::Classifier) -> figcraft_geom::Rect {
    figcraft_geom::Rect::new(c.x, c.y, c.x + c.w, c.y + c.h)
}

#[test]
fn a_blank_class_diagram_is_an_empty_model_on_its_page() {
    let mut s = Session::new();
    let r = s.execute("file.newDiagram", &json!({"type": "class", "name": "Shop"})).unwrap();
    assert_eq!((r["type"].as_str(), r["template"].as_str()), (Some("class"), Some("blank")));
    assert_eq!(model(&s), Model::default());
    assert!(layers(&s).is_empty());
    assert_eq!(run(&mut s, "diagram.class.read", json!({}))["schema"], json!(1));
}

#[test]
fn library_loans_has_all_six_relationships_and_nothing_unfinished() {
    let mut s = library();
    let m = model(&s);
    assert_eq!(cd::validate(&m), Ok(()));
    let kinds: HashSet<LinkKind> = m.links.iter().map(|l| l.kind).collect();
    assert_eq!(kinds.len(), 6, "{kinds:?}");
    assert_eq!(run(&mut s, "diagram.class.check", json!({}))["warnings"], json!([]));
    assert!(m.classifiers.iter().any(|c| c.kind == Kind::Interface));
    assert!(m.classifiers.iter().any(|c| c.kind == Kind::Enumeration && c.literals.len() == 3));
    // A card and a link layer for every element and relationship, all locked.
    let tags = layers(&s);
    for c in &m.classifiers {
        assert!(tags.contains(&format!("{}:card", c.id)), "card {}", c.name);
    }
    for l in &m.links {
        assert!(tags.contains(&format!("{}:link", l.id)), "link {}", l.id);
    }
    // Counting every generated layer, not only the page's children (a card is a group of many).
    assert!(layer_count(&s) <= cd::layer_estimate(&m), "the renderer stays within the estimate the model is capped by");
    assert_eq!(undo_len(&s), 0, "the template is the starting state");
}

#[test]
fn every_change_is_one_undo_step_and_undo_restores_the_model() {
    let mut s = blank();
    let book = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Book"}));
    let steps = undo_len(&s);
    let title = new_id(&mut s, "diagram.class.addMember", json!({"classifier": book, "compartment": "attribute", "text": "- title : String"}));
    assert_eq!(undo_len(&s), steps + 1);
    let before = model(&s);
    run(&mut s, "diagram.class.updateMember", json!({"id": title, "text": "- title : Text [0..1]"}));
    let a = &model(&s).classifiers[0].attributes[0];
    assert_eq!((a.type_name.as_deref(), a.multiplicity.as_deref()), (Some("Text"), Some("0..1")));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(model(&s), before);
    // The same text again changes nothing and adds no step.
    no_op(&mut s, "diagram.class.updateMember", json!({"id": title, "text": "- title : String"}));
}

#[test]
fn members_by_text_and_by_fields_and_their_order() {
    let mut s = blank();
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Account"}));
    let a = new_id(
        &mut s,
        "diagram.class.addMember",
        json!({"classifier": c, "compartment": "attribute", "name": "balance", "type": "Money", "visibility": "private"}),
    );
    let b = new_id(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "attribute", "text": "+ static count : Int = 0"}));
    let first = new_id(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "attribute", "text": "id : Int", "after": a}));
    let names = |s: &Session| model(s).classifiers[0].attributes.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&s), ["balance", "id", "count"]);
    run(&mut s, "diagram.class.moveMember", json!({"id": b, "before": a}));
    assert_eq!(names(&s), ["count", "balance", "id"]);
    run(&mut s, "diagram.class.moveMember", json!({"id": b, "after": first}));
    assert_eq!(names(&s), ["balance", "id", "count"]);
    let op =
        new_id(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "operation", "text": "+ deposit(amount: Money) : Bool"}));
    let e = refused(&mut s, "diagram.class.moveMember", json!({"id": op, "before": a}));
    assert!(e.contains("own compartment"), "{e}");
    let e = refused(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "attribute", "text": "two words : T"}));
    assert!(e.contains("at character"), "{e}");
    refused(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "literal", "text": "Open"}));
    refused(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "attribute", "text": "x", "name": "y"}));
    run(&mut s, "diagram.class.removeMember", json!({"id": op}));
    assert!(model(&s).classifiers[0].operations.is_empty());
}

#[test]
fn a_card_grows_with_its_text_and_never_into_a_neighbour() {
    let mut s = blank();
    let a = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "A", "x": 0, "y": 0}));
    let h0 = model(&s).classifiers[0].h;
    new_id(&mut s, "diagram.class.addMember", json!({"classifier": a, "compartment": "attribute", "text": "x : Int"}));
    new_id(&mut s, "diagram.class.addMember", json!({"classifier": a, "compartment": "attribute", "text": "y : Int"}));
    assert!(model(&s).classifiers[0].h > h0);
    let c = model(&s).classifiers[0].clone();
    // A neighbour just below: one more row would reach it.
    new_id(&mut s, "diagram.class.addClassifier", json!({"name": "B", "x": 0, "y": c.h + cd::CARD_GAP}));
    let e = refused(&mut s, "diagram.class.addMember", json!({"classifier": a, "compartment": "attribute", "text": "z : Int"}));
    assert!(e.contains("closer than"), "{e}");
    refused(&mut s, "diagram.class.resize", json!({"id": a, "width": 10, "height": 10}));
}

#[test]
fn relationships_follow_the_rules_and_edit_in_place() {
    let mut s = blank();
    let shape = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Shape", "abstract": true}));
    let drawable = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Drawable", "kind": "interface"}));
    let circle = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Circle"}));
    new_id(&mut s, "diagram.class.connect", json!({"kind": "generalization", "from": circle, "to": shape}));
    new_id(&mut s, "diagram.class.connect", json!({"kind": "realization", "from": shape, "to": drawable}));
    refused(&mut s, "diagram.class.connect", json!({"kind": "generalization", "from": shape, "to": circle}));
    refused(&mut s, "diagram.class.connect", json!({"kind": "realization", "from": drawable, "to": shape}));
    refused(&mut s, "diagram.class.connect", json!({"kind": "generalization", "from": circle, "to": drawable}));
    let owns = new_id(
        &mut s,
        "diagram.class.connect",
        json!({"kind": "aggregation", "from": shape, "to": circle, "ends": [{"multiplicity": "1"}, {"role": "parts", "multiplicity": "*", "navigable": "yes"}]}),
    );
    run(&mut s, "diagram.class.updateLink", json!({"id": owns, "kind": "composition"}));
    let l = model(&s).links.iter().find(|l| l.id.0 == owns).cloned().unwrap();
    assert_eq!((l.kind, l.ends[1].role.as_deref(), l.ends[1].navigable), (LinkKind::Composition, Some("parts"), Navigable::Yes));
    let e = refused(&mut s, "diagram.class.updateLink", json!({"id": owns, "kind": "dependency"}));
    assert!(e.contains("connect again"), "{e}");
    // Swapped, the whole would be the `*` end: refused until that multiplicity goes.
    let e = refused(&mut s, "diagram.class.swapEnds", json!({"id": owns}));
    assert!(e.contains("whole"), "{e}");
    run(&mut s, "diagram.class.updateLink", json!({"id": owns, "ends": [{}, {"multiplicity": null}]}));
    run(&mut s, "diagram.class.swapEnds", json!({"id": owns}));
    let l = model(&s).links.iter().find(|l| l.id.0 == owns).cloned().unwrap();
    assert_eq!(l.ends[0].classifier, ClassifierId(circle));
    // Removing a classifier takes its relationships.
    run(&mut s, "diagram.class.remove", json!({"ids": [circle]}));
    let m = model(&s);
    assert!(m.links.iter().all(|l| !l.touches(ClassifierId(circle))));
    assert_eq!(m.links.len(), 1);
}

#[test]
fn add_connected_makes_the_element_and_its_relationship_in_one_step() {
    let mut s = blank();
    let book = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Book"}));
    let steps = undo_len(&s);
    let r = run(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": book, "relation": "realization", "kind": "interface", "direction": "out", "name": "Lendable"}),
    );
    assert_eq!(undo_len(&s), steps + 1);
    let m = model(&s);
    let new = m.classifier(ClassifierId(r["element"].as_u64().unwrap())).unwrap();
    assert_eq!(new.kind, Kind::Interface, "a class realizes a new interface");
    let link = m.links.iter().find(|l| l.id.0 == r["link"].as_u64().unwrap()).unwrap();
    assert_eq!(link.pair(), (ClassifierId(book), new.id));
    let r = run(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": book, "relation": "generalization", "kind": "class", "direction": "in", "name": "Ebook"}),
    );
    let m = model(&s);
    let sub = m.classifier(ClassifierId(r["element"].as_u64().unwrap())).unwrap();
    let link = m.links.iter().find(|l| l.id.0 == r["link"].as_u64().unwrap()).unwrap();
    assert_eq!(link.pair(), (sub.id, ClassifierId(book)), "a new subclass of Book");
    // The new element's kind and the direction are explicit.
    let e = refused(&mut s, "diagram.class.addConnected", json!({"source": book, "relation": "association", "direction": "out", "name": "X"}));
    assert!(e.contains("`kind`"), "{e}");
    let e = refused(&mut s, "diagram.class.addConnected", json!({"source": book, "relation": "association", "kind": "class", "name": "X"}));
    assert!(e.contains("`direction`"), "{e}");
}

#[test]
fn add_connected_whole_end_decides_the_whole_of_a_composition() {
    let mut s = blank();
    let shelf = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Shelf"}));
    // The new Library is the whole: the first end.
    let r = run(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": shelf, "relation": "composition", "kind": "class", "direction": "in", "wholeEnd": "new", "name": "Library"}),
    );
    let m = model(&s);
    let library = ClassifierId(r["element"].as_u64().unwrap());
    let link = m.links.iter().find(|l| l.id.0 == r["link"].as_u64().unwrap()).unwrap();
    assert_eq!((link.kind, link.pair()), (LinkKind::Composition, (library, ClassifierId(shelf))));
    // The source as the whole.
    let r = run(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": shelf, "relation": "aggregation", "kind": "class", "direction": "out", "wholeEnd": "source", "name": "Book"}),
    );
    let m = model(&s);
    let link = m.links.iter().find(|l| l.id.0 == r["link"].as_u64().unwrap()).unwrap();
    assert_eq!(link.pair(), (ClassifierId(shelf), ClassifierId(r["element"].as_u64().unwrap())));
    // A whole end that disagrees with the direction, or on a kind without a whole, is refused.
    let e = refused(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": shelf, "relation": "composition", "kind": "class", "direction": "in", "wholeEnd": "source", "name": "Case"}),
    );
    assert!(e.contains("direction"), "{e}");
    let e = refused(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": shelf, "relation": "association", "kind": "class", "direction": "out", "wholeEnd": "source", "name": "Case"}),
    );
    assert!(e.contains("aggregation or a composition"), "{e}");
}

#[test]
fn a_kind_change_that_would_lose_literals_is_refused() {
    let mut s = blank();
    let e = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Status", "kind": "enumeration"}));
    new_id(&mut s, "diagram.class.addMember", json!({"classifier": e, "compartment": "literal", "text": "Open"}));
    let why = refused(&mut s, "diagram.class.updateClassifier", json!({"id": e, "kind": "class"}));
    assert!(why.contains("enumeration"), "{why}");
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Shape"}));
    run(&mut s, "diagram.class.updateClassifier", json!({"id": c, "kind": "interface", "stereotype": "service"}));
    let m = model(&s);
    let shape = m.classifier(ClassifierId(c)).unwrap();
    assert_eq!((shape.kind, shape.stereotype.as_deref()), (Kind::Interface, Some("service")));
}

#[test]
fn page_duplicate_copies_the_model_with_fresh_ids() {
    let mut s = library();
    let before = model(&s);
    run(&mut s, "page.duplicate", json!({}));
    let doc = s.doc().unwrap();
    assert_eq!(doc.class_diagrams.len(), 2);
    let ids = |m: &Model| m.classifiers.iter().map(|c| c.id.0).collect::<HashSet<_>>();
    let copies: Vec<_> = doc.class_diagrams.values().collect();
    assert!(ids(copies[0]).is_disjoint(&ids(copies[1])));
    assert!(copies.iter().any(|m| m.classifiers.len() == before.classifiers.len()));
}

#[test]
fn the_layout_rows_are_where_the_text_is_drawn() {
    let s = library();
    let m = model(&s);
    let book = m.classifiers.iter().find(|c| c.name == "Book").unwrap();
    let lay = card_layout(book);
    let rows: Vec<_> = lay.compartments.iter().flat_map(|c| c.rows.iter()).collect();
    assert_eq!(rows.len(), book.member_count());
    for r in &rows {
        assert!(lay.bounds.contains(r.rect.center()));
        assert_eq!(lay.row_at(r.rect.center()).map(|x| x.member), Some(r.member));
    }
    // Every relationship between two cards has a path that starts and ends on them.
    for (l, p) in m.links.iter().zip(link_paths(&m)) {
        let p = p.unwrap_or_else(|| panic!("link {} has no path", l.id));
        assert!(p.points.len() >= 2);
    }
}

#[test]
fn a_self_association_is_drawn_as_a_loop() {
    let mut s = blank();
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Employee", "x": 100, "y": 100}));
    new_id(&mut s, "diagram.class.connect", json!({"kind": "association", "from": c, "to": c, "ends": [{"role": "manager"}, {"role": "reports"}]}));
    let m = model(&s);
    let p = link_paths(&m).into_iter().next().flatten().unwrap();
    assert_eq!(p.points.len(), 4, "a three-segment loop");
    let r = card_box(&m.classifiers[0]);
    assert!(on_edge(r, p.points[0]) && on_edge(r, p.points[3]), "it leaves and meets the card's edge: {:?}", p.points);
    assert!(layers(&s).iter().any(|t| t.ends_with(":link")));
}

#[test]
fn class_commands_are_refused_on_another_kind_of_page() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    let e = s.execute("diagram.class.addClassifier", &json!({"name": "A"})).unwrap_err().to_string();
    assert!(e.contains("isn't a Class diagram"), "{e}");
}

#[test]
fn a_class_diagram_is_saved_and_opens_again_with_its_model() {
    let s = library();
    let doc = s.doc().unwrap();
    assert!(doc.has_typed_records());
    let bytes = figcraft_format::save_bytes(doc, None).unwrap();
    let (back, issues) = figcraft_format::load_bytes(&bytes).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(back.class_diagrams, doc.class_diagrams);
    assert_eq!(back.class_diagram_problem(), None);
}

#[test]
fn parallel_links_stay_on_their_cards() {
    let mut s = blank();
    // Two empty classes, 80 high, side by side: seven associations offset 14 apart would put the
    // outer ones past the cards' top and bottom edges.
    let a = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "A", "x": 0, "y": 0}));
    let b = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "B", "x": 300, "y": 0}));
    assert_eq!(model(&s).classifiers[0].h, 80.0);
    for k in 0..7 {
        new_id(&mut s, "diagram.class.connect", json!({"kind": "association", "from": a, "to": b, "ends": [{"role": format!("r{k}")}, {}]}));
    }
    let m = model(&s);
    let (ra, rb) = (card_box(&m.classifiers[0]), card_box(&m.classifiers[1]));
    for (l, p) in m.links.iter().zip(link_paths(&m)) {
        let p = p.unwrap_or_else(|| panic!("link {} has no path", l.id));
        let (first, last) = (p.points[0], *p.points.last().unwrap());
        assert!(on_edge(ra, first), "link {} starts off A: {first:?}", l.id);
        assert!(on_edge(rb, last), "link {} ends off B: {last:?}", l.id);
    }
    let ys: HashSet<i64> = link_paths(&m).iter().flatten().map(|p| (p.points[0].y * 1000.0) as i64).collect();
    assert_eq!(ys.len(), 7, "the links stay apart");
}

#[test]
fn end_labels_stay_between_two_close_cards() {
    let mut s = blank();
    let a = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "A", "x": 0, "y": 0}));
    let w = model(&s).classifiers[0].w;
    // The closest two cards may be.
    let b = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "B", "x": w + cd::CARD_GAP, "y": 0}));
    let l = new_id(
        &mut s,
        "diagram.class.connect",
        json!({"kind": "association", "from": a, "to": b, "ends": [{"role": "a", "multiplicity": "1"}, {"role": "b", "multiplicity": "*"}]}),
    );
    let m = model(&s);
    let (ra, rb) = (card_box(&m.classifiers[0]), card_box(&m.classifiers[1]));
    let labels = link_layers(&s, l, "label");
    assert_eq!(labels.len(), 4);
    for r in labels {
        // Not under either card (the cards are drawn over the labels).
        assert!(r.x0 >= ra.x1 - 1e-6 && r.x1 <= rb.x0 + 1e-6, "label {r:?} reaches under a card ({ra:?}, {rb:?})");
    }
}

#[test]
fn a_row_that_fits_a_card_resized_taller_is_added() {
    let mut s = blank();
    let a = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "A", "x": 0, "y": 0}));
    let c = model(&s).classifiers[0].clone();
    let tall = c.h + 40.0;
    run(&mut s, "diagram.class.resize", json!({"id": a, "width": c.w, "height": tall}));
    // A neighbour at the legal gap below the taller card.
    new_id(&mut s, "diagram.class.addClassifier", json!({"name": "B", "x": 0, "y": tall + cd::CARD_GAP}));
    // One more row (18) fits in the 40 the resize gave.
    new_id(&mut s, "diagram.class.addMember", json!({"classifier": a, "compartment": "attribute", "text": "x : Int"}));
    let c = model(&s).classifiers[0].clone();
    assert_eq!(c.h, tall, "the card keeps its height");
    assert_eq!(c.attributes.len(), 1);
}

#[test]
fn placement_reports_the_real_problem_without_searching() {
    let mut s = blank();
    let book = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Book"}));
    let e = refused(&mut s, "diagram.class.addClassifier", json!({"name": "Bad: name"}));
    assert!(e.contains("isn't usable"), "{e}");
    // Over the cap: refused before the text is measured.
    let e = refused(&mut s, "diagram.class.addClassifier", json!({"name": "x".repeat(5000)}));
    assert!(e.contains("longer than"), "{e}");
    let e = refused(&mut s, "diagram.class.addMember", json!({"classifier": book, "compartment": "attribute", "name": "y".repeat(5000)}));
    assert!(e.contains("longer than"), "{e}");
    // A relationship the rules refuse: a class realizes only an interface.
    let e = refused(
        &mut s,
        "diagram.class.addConnected",
        json!({"source": book, "relation": "realization", "kind": "class", "direction": "out", "name": "Lendable"}),
    );
    assert!(e.contains("realizes an interface"), "{e}");
}

#[test]
fn null_clears_parameters_and_navigability() {
    let mut s = blank();
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Account", "x": 0, "y": 0}));
    let d = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Bank", "x": 400, "y": 0}));
    let op = new_id(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "operation", "text": "+ deposit(amount: Money)"}));
    run(&mut s, "diagram.class.updateMember", json!({"id": op, "params": null}));
    assert!(model(&s).classifiers[0].operations[0].params.is_empty(), "null clears the parameters");
    let l = new_id(&mut s, "diagram.class.connect", json!({"kind": "association", "from": c, "to": d, "ends": [{"navigable": "yes"}, {}]}));
    run(&mut s, "diagram.class.updateLink", json!({"id": l, "ends": [{"navigable": null}, {}]}));
    assert_eq!(model(&s).links[0].ends[0].navigable, Navigable::Unspecified, "null clears the navigability");
    // Omitted leaves it.
    run(&mut s, "diagram.class.updateLink", json!({"id": l, "ends": [{"navigable": "no"}, {}]}));
    no_op(&mut s, "diagram.class.updateLink", json!({"id": l, "ends": [{}, {}]}));
    assert_eq!(model(&s).links[0].ends[0].navigable, Navigable::No);
}

#[test]
fn only_class_and_interface_change_into_one_another() {
    let mut s = blank();
    let e = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Status", "kind": "enumeration"}));
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Shape"}));
    // Out of an enumeration, even an empty one.
    for kind in ["class", "interface"] {
        let why = refused(&mut s, "diagram.class.updateClassifier", json!({"id": e, "kind": kind}));
        assert!(why.contains("enumeration"), "{why}");
    }
    // Into an enumeration.
    let why = refused(&mut s, "diagram.class.updateClassifier", json!({"id": c, "kind": "enumeration"}));
    assert!(why.contains("enumeration"), "{why}");
    run(&mut s, "diagram.class.updateClassifier", json!({"id": c, "kind": "interface"}));
    run(&mut s, "diagram.class.updateClassifier", json!({"id": c, "kind": "class"}));
    assert_eq!(model(&s).classifier(ClassifierId(c)).unwrap().kind, Kind::Class);
}

#[test]
fn a_patch_that_changes_nothing_keeps_the_stored_sizes_and_adds_no_step() {
    let mut s = blank();
    let c = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Book", "x": 0, "y": 0}));
    let t = new_id(&mut s, "diagram.class.addMember", json!({"classifier": c, "compartment": "attribute", "text": "- title : String"}));
    // Sizes measured with other fonts than this machine's: a remeasure would change them.
    let page = s.page().unwrap();
    let mut m = model(&s);
    (m.classifiers[0].w, m.classifiers[0].min_w) = (300.0, 300.0);
    s.doc_mut().unwrap().class_diagrams.insert(page, std::sync::Arc::new(m));
    no_op(&mut s, "diagram.class.updateClassifier", json!({"id": c, "name": "Book", "abstract": false}));
    no_op(&mut s, "diagram.class.updateMember", json!({"id": t, "text": "- title : String"}));
    no_op(&mut s, "diagram.class.updateMember", json!({"id": t, "type": "String"}));
    assert_eq!(model(&s).classifiers[0].min_w, 300.0);
}

#[test]
fn the_whole_ends_navigability_and_abstract_are_drawn() {
    let mut s = blank();
    let whole = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Order", "abstract": true, "x": 0, "y": 0}));
    let part = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Line", "x": 300, "y": 0}));
    let l = new_id(
        &mut s,
        "diagram.class.connect",
        json!({"kind": "aggregation", "from": whole, "to": part, "ends": [{"navigable": "no"}, {"navigable": "yes"}]}),
    );
    // The diamond, the whole end's cross and the part end's arrow.
    assert_eq!(link_layers(&s, l, "marker").len(), 3);
    // «abstract» is written out as well as the name being italic, with room made for its line.
    let m = model(&s);
    let card = m.classifier(ClassifierId(whole)).unwrap();
    assert_eq!(crate::class_diagram::layout::keyword(card).as_deref(), Some("abstract"));
    assert_eq!(card.min_h, card.floor().1 + cd::KEYWORD_HEIGHT);
    let doc = s.doc().unwrap();
    let page = doc.node(s.page().unwrap()).unwrap();
    let tag = format!("{whole}:card");
    let group = page.children.iter().find(|c| doc.node(**c).unwrap().plugin_data.get(LAYER_KEY) == Some(&tag)).unwrap();
    let names: Vec<String> = doc.node(*group).unwrap().children.iter().map(|c| doc.node(*c).unwrap().name.clone()).collect();
    assert!(names.iter().any(|n| n == "«abstract»"), "{names:?}");
    // The rows sit below the taller header, inside the card.
    let lay = card_layout(card);
    assert!(lay.compartments.last().unwrap().rect.y1 <= lay.bounds.y1 + 1e-6);
    // Not abstract any more: the line goes and the card fits again.
    run(&mut s, "diagram.class.updateClassifier", json!({"id": whole, "abstract": false}));
    let m = model(&s);
    let card = m.classifier(ClassifierId(whole)).unwrap();
    assert_eq!((crate::class_diagram::layout::keyword(card), card.min_h), (None, card.floor().1));
    // With another keyword, «abstract» joins its line.
    let i = new_id(&mut s, "diagram.class.addClassifier", json!({"name": "Shape", "kind": "interface", "abstract": true}));
    let m = model(&s);
    let card = m.classifier(ClassifierId(i)).unwrap();
    assert_eq!(crate::class_diagram::layout::keyword(card).as_deref(), Some("interface, abstract"));
    assert_eq!(card.min_h, card.floor().1);
}
