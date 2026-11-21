//! Typed diagrams are a preview (slice S1): made only with the preview on, saved as format
//! version 2 (S2), kept in step with their pages (duplicate, delete, load, import), closed to
//! ordinary layers and edits, and kept out of live sessions and branch merges.

use std::sync::Arc;

use figcraft_doc::use_case::{LAYER_KEY, Model};
use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::{DocState, PointerEvent, PointerKind, Session, ToolId};

/// `doc` as `document.json` text under the plain (version 1) header, which no FigCraft writes for
/// a typed diagram: such a file is refused.
pub(crate) fn typed_json(doc: &Document) -> Value {
    let mut v = serde_json::to_value(doc).unwrap();
    v["format"] = json!(figcraft_format::FORMAT);
    v["version"] = json!(figcraft_format::VERSION);
    v
}

/// A temporary folder of this test's own, named `tag`.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("figcraft-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A design file whose first page holds the Use Case example and whose second is plain: the
/// active tab, open on the plain page. Returns (typed page, plain page).
fn design_with_diagram(s: &mut Session) -> (NodeId, NodeId) {
    let mut doc = Document::new("Mixed");
    crate::use_case::start(&mut doc, crate::use_case::EXAMPLE).unwrap();
    let typed = doc.first_page().unwrap();
    let plain = doc.add_page("Plain");
    let mut ds = DocState::new(doc, None);
    ds.page = plain;
    s.docs.push(ds);
    s.active = s.docs.len() - 1;
    (typed, plain)
}

fn generated(doc: &Document, page: NodeId) -> Vec<NodeId> {
    doc.node(page).unwrap().children.iter().copied().filter(|c| doc.node(*c).unwrap().plugin_data.contains_key(LAYER_KEY)).collect()
}

fn refused(s: &mut Session, cmd: &str, p: Value) -> String {
    let (doc, steps) = (s.ds().unwrap().doc.clone(), s.ds().unwrap().history.undo.len());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} should be refused")).to_string();
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc) || *doc == *s.doc().unwrap(), "{cmd} {p} changed the document");
    assert_eq!(s.ds().unwrap().history.undo.len(), steps, "{cmd} {p} added an undo step");
    e
}

// ---- the preview setting ----

#[test]
fn new_diagram_makes_typed_diagrams_only_with_the_preview_on() {
    let mut s = Session::new();
    s.prefs.diagram_preview = Some(false);
    let (count, active) = (s.docs.len(), s.active);
    let e = s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap_err().to_string();
    assert!(e.contains("preview"), "{e}");
    let e = s.execute("file.newDiagram", &json!({"type": "class"})).unwrap_err().to_string();
    assert!(e.contains("preview"), "{e}");
    assert_eq!((s.docs.len(), s.active), (count, active));
    // A flowchart isn't typed.
    s.execute("file.newDiagram", &json!({})).unwrap();
    assert_eq!(s.execute("pref.diagramPreview", &json!({})).unwrap()["on"], json!(true));
    assert!(s.prefs_unsaved);
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    assert!(s.doc().unwrap().has_typed_records());
    s.execute("file.newDiagram", &json!({"type": "class"})).unwrap();
    assert!(s.doc().unwrap().has_typed_records());
    assert_eq!(s.execute("pref.diagramPreview", &json!({"on": false})).unwrap()["on"], json!(false));
}

#[test]
fn the_preview_is_on_by_default_only_in_development_builds() {
    let prefs = crate::Prefs::default();
    assert_eq!(prefs.diagram_preview_on(), cfg!(debug_assertions));
    let v = serde_json::to_value(&prefs).unwrap();
    assert!(v.get("diagram_preview").is_none(), "the default isn't written: {v}");
}

// ---- saving ----

/// The header version of `path`'s `document.json`.
fn saved_version(path: &std::path::Path) -> u64 {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let doc: Value = serde_json::from_reader(z.by_name("document.json").unwrap()).unwrap();
    doc["version"].as_u64().unwrap()
}

#[test]
fn save_as_keeps_a_diagram_in_the_document_or_only_in_a_version_and_it_opens_again() {
    let dir = temp_dir("typed-save");
    let path = dir.join("d.figcraft");
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": crate::use_case::EXAMPLE})).unwrap();
    let model = s.doc().unwrap().use_cases.clone();
    s.execute("file.saveAs", &json!({"path": path.to_string_lossy()})).unwrap();
    assert_eq!(saved_version(&path), u64::from(figcraft_format::TYPED_VERSION));
    s.execute("file.open", &json!({"path": path.to_string_lossy()})).unwrap();
    assert_eq!(s.doc().unwrap().use_cases, model, "the model comes back as it was saved");
    // The diagram only in a saved version: the page goes, the version keeps it, and the file is
    // still written as version 2 so an older FigCraft refuses it instead of losing that version.
    s.execute("file.saveVersion", &json!({"name": "with diagram"})).unwrap();
    s.execute("page.add", &json!({})).unwrap();
    let typed = s.doc().unwrap().first_page().unwrap();
    s.execute("page.delete", &json!({"id": typed})).unwrap();
    assert!(!s.doc().unwrap().has_typed_records());
    let only_version = dir.join("v.figcraft");
    s.execute("file.saveAs", &json!({"path": only_version.to_string_lossy()})).unwrap();
    assert_eq!(saved_version(&only_version), u64::from(figcraft_format::TYPED_VERSION));
    s.execute("file.open", &json!({"path": only_version.to_string_lossy()})).unwrap();
    let ds = s.ds().unwrap();
    assert!(!ds.doc.has_typed_records());
    assert!(ds.versions.iter().any(|v| v.has_typed_records().unwrap()), "the version keeps the diagram");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- page lifecycle ----

#[test]
fn duplicating_a_diagram_page_copies_its_model_with_fresh_ids_and_new_layers() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    let r = s.execute("page.duplicate", &json!({"id": typed})).unwrap();
    let copy = crate::params::id_of(&r["id"]).unwrap();
    let doc = s.doc().unwrap();
    let (a, b) = (doc.use_cases.get(&typed).unwrap(), doc.use_cases.get(&copy).unwrap());
    assert_eq!(figcraft_doc::use_case::validate(b), Ok(()));
    assert_eq!((a.actors.len(), a.use_cases.len(), a.links.len()), (b.actors.len(), b.use_cases.len(), b.links.len()));
    let ids = |m: &Model| m.actors.iter().map(|x| x.id.0).chain(m.use_cases.iter().map(|x| x.id.0)).collect::<Vec<_>>();
    assert!(ids(b).iter().all(|i| !ids(a).contains(i)), "fresh IDs: {:?} vs {:?}", ids(a), ids(b));
    // The copy's layers are its own, drawn from its model (tagged with its IDs).
    let tags = |p: NodeId| generated(doc, p).iter().map(|c| doc.node(*c).unwrap().plugin_data[LAYER_KEY].clone()).collect::<Vec<_>>();
    assert_eq!(tags(copy).len(), tags(typed).len());
    let first = b.actors[0].id.0;
    assert!(tags(copy).iter().any(|t| t.starts_with(&format!("{first}:"))), "{:?}", tags(copy));
    // One step: undo takes the page and its model away together, and redo brings them back; the
    // source page's generated layers stay as they were, by value, all along (no refit).
    let layers = |s: &Session| {
        let doc = s.doc().unwrap();
        generated(doc, typed).iter().map(|c| doc.node(*c).unwrap().clone()).collect::<Vec<_>>()
    };
    let mut fresh = Session::new();
    design_with_diagram(&mut fresh);
    assert_eq!(layers(&s), layers(&fresh), "duplicating didn't refit the source page");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(!s.doc().unwrap().use_cases.contains_key(&copy));
    assert_eq!(layers(&s), layers(&fresh));
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(s.doc().unwrap().use_cases.contains_key(&copy));
    assert_eq!(layers(&s), layers(&fresh));
}

#[test]
fn a_new_diagram_starts_normalised_so_a_later_edit_refits_nothing() {
    let mut doc = Document::new("d");
    crate::use_case::start(&mut doc, crate::use_case::EXAMPLE).unwrap();
    assert!(doc.take_dirty().is_empty(), "nothing is left for the next command to refit");
}

#[test]
fn deleting_a_diagram_page_removes_its_model_in_the_same_step() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    s.execute("page.delete", &json!({"id": typed})).unwrap();
    assert!(s.doc().unwrap().use_cases.is_empty());
    s.execute("edit.undo", &json!({})).unwrap();
    let doc = s.doc().unwrap();
    assert!(doc.pages.contains(&typed) && doc.use_cases.contains_key(&typed));
}

#[test]
fn opening_a_file_with_a_diagram_is_refused_not_migrated() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase", "template": "useCaseExample"})).unwrap();
    let doc = (*s.ds().unwrap().doc).clone();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, typed_json(&doc).to_string());
    let (count, active) = (s.docs.len(), s.active);
    let e = s.execute("file.open", &json!({"data": data, "name": "d.figcraft"})).unwrap_err().to_string();
    assert!(e.contains(figcraft_format::TYPED_LOAD_REFUSAL), "{e}");
    assert_eq!((s.docs.len(), s.active), (count, active), "no tab opened");
}

#[test]
fn importing_a_file_with_a_diagram_page_is_refused() {
    let mut s = Session::new();
    let (_, _) = design_with_diagram(&mut s);
    // A file FigCraft wrote (version 2): it opens, but its diagram pages can't join another file.
    let saved = figcraft_format::to_json(s.doc().unwrap()).unwrap();
    assert_eq!(saved["version"], figcraft_format::TYPED_VERSION);
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, saved.to_string());
    s.execute("file.new", &json!({})).unwrap();
    let e = refused(&mut s, "file.import", json!({"data": data, "name": "d.figcraft"}));
    assert!(e.contains("can't be imported into another file yet"), "{e}");
    // The same diagram under the plain header is refused as a file no FigCraft wrote.
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, typed_json(&serde_json::from_value(saved).unwrap()).to_string());
    let e = refused(&mut s, "file.import", json!({"data": data, "name": "d.figcraft"}));
    assert!(e.contains(figcraft_format::TYPED_LOAD_REFUSAL), "{e}");
}

// ---- typed pages ----

#[test]
fn ordinary_layers_are_refused_on_a_diagram_page_even_from_another_page_or_a_nested_batch() {
    let mut s = Session::new();
    let (typed, plain) = design_with_diagram(&mut s);
    // The active page is plain: commands naming the diagram page are refused.
    let rect = json!({"type": "rectangle", "parent": typed, "x": 5000, "y": 5000, "width": 10, "height": 10});
    let e = refused(&mut s, "node.create", rect.clone());
    assert!(e.contains("shows a diagram"), "{e}");
    let nested = json!({"commands": [{"command": "command.batch", "params": {"commands": [{"command": "node.create", "params": rect}]}}]});
    refused(&mut s, "command.batch", nested);
    // A layer made on the plain page can't move onto the diagram page.
    let r = s.execute("node.create", &json!({"type": "rectangle", "x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    refused(&mut s, "node.reparent", json!({"ids": [id], "parent": typed}));
    // On the diagram page itself: shapes, SVG import and paste.
    s.execute("page.select", &json!({"id": typed})).unwrap();
    refused(&mut s, "shape.rectangle", json!({"x": 5000, "y": 5000, "width": 10, "height": 10}));
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="5" height="5"/></svg>"#;
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, svg);
    refused(&mut s, "file.import", json!({"data": data, "name": "a.svg", "x": 5000, "y": 5000}));
    s.execute("page.select", &json!({"id": plain})).unwrap();
    s.execute("node.set", &json!({"id": id, "props": {"name": "Copied"}})).unwrap();
    s.set_selection(vec![id]);
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("page.select", &json!({"id": typed})).unwrap();
    refused(&mut s, "edit.paste", json!({}));
}

// ---- the edit gate ----

#[test]
fn ordinary_edits_on_generated_layers_are_refused_with_the_diagram_tools_named() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    let card = generated(s.doc().unwrap(), typed)[0];
    let e = refused(&mut s, "node.set", json!({"id": card, "props": {"x": 12345}}));
    assert!(e.contains("use the diagram tools"), "{e}");
    let inner = s.doc().unwrap().node(card).unwrap().children.first().copied().unwrap();
    let e = refused(&mut s, "node.set", json!({"id": inner, "props": {"name": "Renamed"}}));
    assert!(e.contains("use the diagram tools"), "{e}");
    // A diagram command still works (it changes the model and redraws).
    s.execute("page.select", &json!({"id": typed})).unwrap();
    s.execute("diagram.useCase.addActor", &json!({"name": "Clerk"})).unwrap();
}

#[test]
fn a_diagram_command_waits_for_an_ordinary_edit_through_repeat_and_nested_batches() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    s.execute("diagram.useCase.addActor", &json!({"name": "Guest"})).unwrap();
    s.begin_interaction("Move").unwrap();
    let e = s.execute("diagram.useCase.addActor", &json!({"name": "Clerk"})).unwrap_err().to_string();
    assert!(e.contains("finish the edit in progress"), "{e}");
    let e = s.execute("edit.repeat", &json!({})).unwrap_err().to_string();
    assert!(e.contains("finish the edit in progress"), "{e}");
    let nested = json!({"commands": [{"command": "command.batch", "params": {"commands": [{"command": "diagram.useCase.addActor", "params": {"name": "C"}}]}}]});
    let e = s.execute("command.batch", &nested).unwrap_err().to_string();
    assert!(e.contains("finish the edit in progress"), "{e}");
    assert!(s.ds().unwrap().interaction.is_some(), "the edit stays open");
    s.cancel_interaction().unwrap();
    s.execute("diagram.useCase.addActor", &json!({"name": "Clerk"})).unwrap();
}

// ---- live and merge ----

#[test]
fn sharing_and_merging_are_refused_while_the_history_holds_a_diagram() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    s.execute("page.add", &json!({})).unwrap();
    let typed = s.doc().unwrap().first_page().unwrap();
    s.execute("page.delete", &json!({"id": typed})).unwrap();
    // Only the undo history holds the diagram now.
    assert!(!s.doc().unwrap().has_typed_records());
    for cmd in ["collab.host", "collab.share", "collab.publish", "branch.merge", "branch.updateFromMain"] {
        let e = s.execute(cmd, &json!({})).unwrap_err().to_string();
        assert!(e.contains("can't be shared, published or merged"), "{cmd}: {e}");
    }
}

#[test]
fn hosting_refuses_before_the_transport_starts_when_only_the_history_holds_a_diagram() {
    let (mut s, script) = crate::live_tests::session();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    s.execute("page.add", &json!({})).unwrap();
    let typed = s.doc().unwrap().first_page().unwrap();
    s.execute("page.delete", &json!({"id": typed})).unwrap();
    assert!(!s.doc().unwrap().has_typed_records());
    let e = s.execute("collab.host", &json!({})).unwrap_err().to_string();
    assert!(e.contains("can't be shared"), "{e}");
    assert!(script.lock().unwrap().hosted.is_empty(), "the transport never started");
}

#[test]
fn before_the_welcome_undo_and_redo_refuse_a_diagram_and_keep_the_step() {
    let (mut s, _script) = crate::live_tests::session();
    s.execute("collab.host", &json!({})).unwrap();
    let uid = s.ds().unwrap().uid;
    assert!(!s.live_doc(uid), "no welcome yet");
    let mut typed = Document::new("t");
    crate::use_case::start(&mut typed, "blank").unwrap();
    let page = s.page().unwrap();
    let entry = crate::HistoryEntry { label: "Diagram".into(), doc: Arc::new(typed), selection: vec![], page };
    let ds = s.ds_mut().unwrap();
    ds.history.undo.push(entry.clone());
    ds.history.redo.push(entry);
    for cmd in ["edit.undo", "edit.redo"] {
        let e = s.execute(cmd, &json!({})).unwrap_err().to_string();
        assert!(e.contains("can't be shared"), "{cmd}: {e}");
    }
    let ds = s.ds().unwrap();
    assert_eq!((ds.history.undo.len(), ds.history.redo.len()), (1, 1), "both steps kept");
    assert!(!ds.doc.has_typed_records());
}

#[test]
fn a_version_holding_a_diagram_is_not_restored() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"type": "useCase"})).unwrap();
    s.execute("file.saveVersion", &json!({"name": "v"})).unwrap();
    let e = refused(&mut s, "version.restore", json!({"index": 0}));
    assert!(e.contains("can't be restored"), "{e}");
}

#[test]
fn hosting_is_refused_while_a_plain_version_is_previewed_over_a_diagram() {
    let (mut s, script) = crate::live_tests::session();
    let (typed, _) = design_with_diagram(&mut s);
    let plain = figcraft_format::Version::new("v1", "Plain", "", 1, Arc::new(Document::new("plain")));
    s.ds_mut().unwrap().versions.push(plain);
    s.execute("version.preview", &json!({"index": 0})).unwrap();
    // What's shown is plain; the document itself still has the diagram.
    assert!(!s.ds().unwrap().doc.has_typed_records());
    assert!(s.ds().unwrap().document_itself().use_cases.contains_key(&typed));
    let e = s.execute("collab.host", &json!({})).unwrap_err().to_string();
    assert!(e.contains("can't be shared"), "{e}");
    assert!(script.lock().unwrap().hosted.is_empty(), "the transport never started");
}

// ---- gestures and live previews (outside `execute`) ----

fn press(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(kind, x, y, 1.0)
}

/// A generated text layer of the diagram on `page`.
fn generated_text(doc: &Document, page: NodeId) -> NodeId {
    generated(doc, page)
        .into_iter()
        .flat_map(|c| doc.subtree(c))
        .find(|n| matches!(doc.node(*n).unwrap().kind, figcraft_doc::NodeKind::Text(_)))
        .unwrap()
}

#[test]
fn a_generated_ellipse_is_not_converted_or_edited_as_a_vector() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    s.execute("page.select", &json!({"id": typed})).unwrap();
    let doc = s.ds().unwrap().doc.clone();
    let ellipse = generated(&doc, typed)
        .into_iter()
        .flat_map(|c| doc.subtree(c))
        .find(|n| matches!(doc.node(*n).unwrap().kind, figcraft_doc::NodeKind::Ellipse { .. }))
        .unwrap();
    // Selected (as from the Layers panel), then a double-click inside it.
    s.set_selection(vec![ellipse]);
    let c = doc.world_bounds(ellipse).unwrap().center();
    s.set_tool(ToolId::Move);
    let e = s.pointer(&press(PointerKind::DoubleClick, c.x, c.y)).unwrap_err().to_string();
    assert!(e.contains("use the diagram tools"), "{e}");
    // Enter and the other entries share `begin_many`.
    assert!(crate::canvas::vectoredit::begin_many(&mut s, &[ellipse]).is_err());
    let ds = s.ds().unwrap();
    assert!(Arc::ptr_eq(&doc, &ds.doc), "nothing was converted");
    assert!(ds.vector_edit.is_none() && ds.history.undo.is_empty());
}

#[test]
fn an_open_pen_path_ends_in_its_own_tab_and_never_writes_to_another() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    let plain_tab = s.active;
    s.set_tool(ToolId::Pen);
    for x in [10.0, 100.0] {
        s.pointer(&press(PointerKind::Down, x, 10.0)).unwrap();
        s.pointer(&press(PointerKind::Up, x, 10.0)).unwrap();
    }
    assert!(s.tool.pen.as_ref().is_some_and(|p| p.id.is_some()), "two anchors made the path's layer");
    // A diagram tab comes to the front, open on its diagram page, and the path's layer id names
    // one of its generated layers.
    let (typed, _) = design_with_diagram(&mut s);
    s.ds_mut().unwrap().page = typed;
    let doc = s.ds().unwrap().doc.clone();
    s.tool.pen.as_mut().unwrap().id = Some(generated(&doc, typed)[0]);
    // The next anchor ends the path in its own tab instead, and starts nothing here.
    let e = s.pointer(&press(PointerKind::Down, 5000.0, 5000.0)).unwrap_err().to_string();
    assert!(e.contains("shows a diagram"), "{e}");
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc), "the generated layer wasn't touched");
    assert!(s.tool.pen.is_none() && s.ds().unwrap().interaction.is_none());
    let first = &s.docs[plain_tab];
    assert!(first.interaction.is_none(), "the path was finished where it was drawn");
    assert_eq!(first.history.undo.len(), 1);
}

#[test]
fn the_pen_and_the_pencil_start_nothing_on_a_diagram_page() {
    let mut s = Session::new();
    let (typed, plain) = design_with_diagram(&mut s);
    s.execute("page.select", &json!({"id": typed})).unwrap();
    let doc = s.ds().unwrap().doc.clone();
    s.set_tool(ToolId::Pen);
    let e = s.pointer(&press(PointerKind::Down, 5000.0, 5000.0)).unwrap_err().to_string();
    assert!(e.contains("shows a diagram"), "{e}");
    assert!(s.tool.pen.is_none(), "no anchor was stored");
    s.set_tool(ToolId::Pencil);
    assert!(s.pointer(&press(PointerKind::Down, 5000.0, 5000.0)).is_err());
    assert!(matches!(s.tool.gesture, crate::canvas::tools::Gesture::None));
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc));
    // On the plain page the pen starts a path.
    s.execute("page.select", &json!({"id": plain})).unwrap();
    s.set_tool(ToolId::Pen);
    s.pointer(&press(PointerKind::Down, 10.0, 10.0)).unwrap();
    assert!(s.tool.pen.is_some());
}

#[test]
fn drawing_a_shape_on_a_diagram_page_shows_no_preview_and_leaves_no_step() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    s.execute("page.select", &json!({"id": typed})).unwrap();
    let (doc, steps) = (s.ds().unwrap().doc.clone(), s.ds().unwrap().history.undo.len());
    s.set_tool(ToolId::Rectangle);
    s.pointer(&press(PointerKind::Down, 5000.0, 5000.0)).unwrap();
    let e = s.pointer(&press(PointerKind::Move, 5100.0, 5080.0)).unwrap_err().to_string();
    assert!(e.contains("shows a diagram"), "{e}");
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc), "the refused preview was never shown");
    assert!(s.ds().unwrap().interaction.is_none(), "the gesture ended");
    let _ = s.pointer(&press(PointerKind::Up, 5100.0, 5080.0));
    // A click without a drag makes its shape in the up event: refused there too.
    s.set_tool(ToolId::Rectangle);
    s.pointer(&press(PointerKind::Down, 6000.0, 6000.0)).unwrap();
    assert!(s.pointer(&press(PointerKind::Up, 6000.0, 6000.0)).is_err());
    assert_eq!(*s.doc().unwrap(), *doc, "nothing was added");
    assert_eq!(s.ds().unwrap().history.undo.len(), steps, "no undo step");
    assert!(s.ds().unwrap().interaction.is_none());
}

#[test]
fn a_gesture_commit_is_checked_too() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    let card = generated(s.doc().unwrap(), typed)[0];
    let doc = s.ds().unwrap().doc.clone();
    s.begin_interaction("Move").unwrap();
    // A gesture's last step made outside `preview` (as a frame taking what it encloses is).
    s.ds_mut().unwrap().doc_mut().node_mut(card).unwrap().name = "Moved".into();
    let e = s.commit_interaction().unwrap_err().to_string();
    assert!(e.contains("use the diagram tools"), "{e}");
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc));
    assert!(s.ds().unwrap().history.undo.is_empty() && s.ds().unwrap().interaction.is_none());
}

#[test]
fn the_text_tool_makes_no_text_on_a_diagram_page_and_its_text_is_not_edited() {
    let mut s = Session::new();
    let (typed, plain) = design_with_diagram(&mut s);
    s.execute("page.select", &json!({"id": typed})).unwrap();
    let doc = s.ds().unwrap().doc.clone();
    s.set_tool(ToolId::Text);
    let e = s.pointer(&press(PointerKind::Down, 5000.0, 5000.0)).unwrap_err().to_string();
    assert!(e.contains("shows a diagram"), "{e}");
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc), "no text was made");
    assert!(s.ds().unwrap().text_edit.is_none() && s.ds().unwrap().interaction.is_none());
    let text = generated_text(&doc, typed);
    let e = crate::canvas::textedit::begin(&mut s, text, None, false).unwrap_err().to_string();
    assert!(e.contains("use the diagram tools"), "{e}");
    assert!(s.ds().unwrap().text_edit.is_none() && s.ds().unwrap().interaction.is_none());
    // On the plain page the text tool works as before.
    s.execute("page.select", &json!({"id": plain})).unwrap();
    s.set_tool(ToolId::Text);
    s.pointer(&press(PointerKind::Down, 10.0, 10.0)).unwrap();
    assert!(s.ds().unwrap().text_edit.is_some());
}

#[test]
fn a_live_preview_on_a_generated_layer_is_refused_before_it_shows() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    let card = generated(s.doc().unwrap(), typed)[0];
    let doc = s.ds().unwrap().doc.clone();
    let p = json!({"command": "node.set", "params": {"id": card, "props": {"opacity": 0.2}}});
    let e = s.execute("text.preview", &p).unwrap_err().to_string();
    assert!(e.contains("use the diagram tools"), "{e}");
    assert!(Arc::ptr_eq(&doc, &s.ds().unwrap().doc) && s.ds().unwrap().text_preview.is_none());
}

#[test]
fn reordering_a_diagram_page_is_refused_but_renaming_it_is_not() {
    let mut s = Session::new();
    let (typed, _) = design_with_diagram(&mut s);
    s.execute("page.select", &json!({"id": typed})).unwrap();
    let back = generated(s.doc().unwrap(), typed)[0];
    // `arrange.front` moves a generated root among the page's children: only the page changes.
    let e = refused(&mut s, "arrange.front", json!({"ids": [back]}));
    assert!(e.contains("diagram"), "{e}");
    s.execute("page.rename", &json!({"id": typed, "name": "Booking"})).unwrap();
    assert_eq!(s.doc().unwrap().node(typed).unwrap().name, "Booking");
}

// ---- history that can't be read ----

/// A file whose version history holds `n` autosaves, the first `unreadable` of them damaged.
fn file_with_autosaves(n: usize, unreadable: usize) -> Vec<u8> {
    use std::io::Write;
    let doc = serde_json::to_vec(&figcraft_format::to_json(&Document::new("Plain")).unwrap()).unwrap();
    let entries: Vec<Value> = (0..n).map(|i| json!({"id": format!("v{i}"), "name": "Autosave", "auto": true})).collect();
    let index = serde_json::to_vec(&json!({ "versions": entries })).unwrap();
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut buf);
    let opts = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, bytes: &[u8]| {
        z.start_file(name, opts).unwrap();
        z.write_all(bytes).unwrap();
    };
    put("document.json", &doc);
    put("versions/index.json", &index);
    for i in 0..n {
        put(&format!("versions/v{i}.json"), if i < unreadable { b"{nope" } else { &doc });
    }
    z.finish().unwrap();
    buf.into_inner()
}

fn tab_from(s: &mut Session, bytes: &[u8]) {
    let l = figcraft_format::load_bytes_with_versions(bytes).unwrap();
    s.docs.push(DocState::new(l.doc, None).with_versions(l.versions));
    s.active = s.docs.len() - 1;
}

#[test]
fn an_autosave_never_evicts_a_version_that_cannot_be_read() {
    let n = crate::cmd::file::MAX_AUTOSAVES;
    let mut s = Session::new();
    tab_from(&mut s, &file_with_autosaves(n, 1));
    let ds = s.ds_mut().unwrap();
    assert!(ds.versions[0].has_typed_records().is_err(), "the oldest can't be read");
    // The autosaves are full: the oldest readable one makes room, never the unreadable one.
    assert!(crate::cmd::file::add_version(ds, "Autosave", "", true).is_some());
    let ids: Vec<&str> = ds.versions.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids.first(), Some(&"v0"));
    assert!(!ids.contains(&"v1"), "{ids:?}");
    assert_eq!(ds.versions.len(), n);
    // When only unreadable autosaves could make room, no checkpoint is made.
    let mut s = Session::new();
    tab_from(&mut s, &file_with_autosaves(n, n));
    let ds = s.ds_mut().unwrap();
    assert!(crate::cmd::file::add_version(ds, "Autosave", "", true).is_none());
    assert_eq!(ds.versions.len(), n);
    assert!(ds.versions.iter().all(|v| v.has_typed_records().is_err()));
}
