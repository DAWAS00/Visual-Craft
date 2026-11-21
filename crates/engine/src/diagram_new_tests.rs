//! `file.newDiagram` and `dialog.newDiagram` (diagram slice 2): a board shown as a diagram, its
//! style kept by the native file and by a live replacement, and refusals that leave the tabs alone.

use std::sync::Arc;

use figcraft_doc::{BoardStyle, Document, FileKind};
use serde_json::json;

use crate::Session;

fn layers(s: &Session) -> usize {
    let doc = s.doc().unwrap();
    doc.node(doc.first_page().unwrap()).unwrap().children.len()
}

#[test]
fn a_blank_diagram_is_an_empty_board_shown_as_a_diagram() {
    let mut s = Session::new();
    let before = s.docs.len();
    let r = s.execute("file.newDiagram", &json!({})).unwrap();
    assert_eq!(s.docs.len(), before + 1);
    assert_eq!(r["index"], json!(s.active));
    assert_eq!((r["kind"].as_str(), r["style"].as_str(), r["template"].as_str()), (Some("board"), Some("diagram"), Some("blank")));
    let doc = s.doc().unwrap();
    assert_eq!((doc.kind, doc.board_style, doc.name.as_str()), (FileKind::Board, BoardStyle::Diagram, "Untitled diagram"));
    assert_eq!(layers(&s), 0);
    // Board commands work on it as on any board.
    s.execute("board.shape", &json!({"shape": "rectangle", "x": 0, "y": 0})).unwrap();
}

#[test]
fn the_flow_chart_template_is_the_starting_state_so_undo_keeps_it() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"name": "  Review flow ", "template": "flowChart"})).unwrap();
    assert_eq!(s.doc().unwrap().name, "Review flow");
    let made = layers(&s);
    assert!(made >= 6, "the template's shapes and connectors: {made}");
    let _ = s.execute("edit.undo", &json!({}));
    assert_eq!(layers(&s), made, "undo has nothing to take back in a new tab");
    assert!(!s.ds().unwrap().is_dirty(), "a new diagram starts with no unsaved changes");
    // The default view size, so a canvas's first sizing fits it again to its real size.
    let view = &s.ds().unwrap().view;
    assert_eq!((view.vw, view.vh), (1200.0, 800.0));
}

#[test]
fn refusals_change_no_tab() {
    let mut s = Session::new();
    s.execute("file.newBoard", &json!({})).unwrap();
    let (count, active) = (s.docs.len(), s.active);
    for p in [
        json!({"type": "mindMap"}),
        json!({"type": "uml"}),
        json!({"template": "kanbanish"}),
        json!({"name": 3}),
        json!({"type": ["flowchart"]}),
        json!({"template": true}),
    ] {
        let e = s.execute("file.newDiagram", &p).unwrap_err().to_string();
        assert_eq!((s.docs.len(), s.active), (count, active), "{p}: {e}");
    }
    let e = s.execute("file.newDiagram", &json!({"type": "mindMap"})).unwrap_err().to_string();
    assert!(e.contains("aren't available yet"), "{e}");
    // Type and template names are taken in any case.
    s.execute("file.newDiagram", &json!({"type": "Flowchart", "template": "FLOWCHART"})).unwrap();
}

#[test]
fn the_dialog_command_only_asks_for_the_dialog() {
    let mut s = Session::new();
    let count = s.docs.len();
    s.execute("dialog.newDiagram", &json!({})).unwrap();
    assert_eq!(s.docs.len(), count);
    assert_eq!(s.ui_requests.last().map(|r| r.kind.as_str()), Some("dialog.newDiagram"));
}

#[test]
fn the_native_file_keeps_the_style_and_a_plain_board_leaves_it_out() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"template": "flowChart"})).unwrap();
    let v = figcraft_format::to_json(s.doc().unwrap()).unwrap();
    assert_eq!(v["board_style"], json!("diagram"));
    let (back, _) = figcraft_format::from_json(&v.to_string()).unwrap();
    assert_eq!((back.kind, back.board_style), (FileKind::Board, BoardStyle::Diagram));
    // Through the whole archive too.
    let bytes = crate::cmd::file::file_bytes(s.ds().unwrap()).unwrap();
    let (opened, _) = crate::cmd::file::load_bytes(&bytes, "d.figcraft").unwrap();
    assert_eq!(opened.board_style, BoardStyle::Diagram);

    s.execute("file.newBoard", &json!({})).unwrap();
    let v = figcraft_format::to_json(s.doc().unwrap()).unwrap();
    assert!(v.get("board_style").is_none(), "a plain board writes no style: {v}");
}

#[test]
fn files_without_a_style_or_with_an_unknown_one_open_as_plain_boards() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({})).unwrap();
    let mut v = figcraft_format::to_json(s.doc().unwrap()).unwrap();
    // A file from before diagrams.
    v.as_object_mut().unwrap().remove("board_style");
    let (legacy, _) = figcraft_format::from_json(&v.to_string()).unwrap();
    assert_eq!((legacy.kind, legacy.board_style), (FileKind::Board, BoardStyle::Plain));
    // A style from a later version.
    v["board_style"] = json!("swimlanes");
    let (later, _) = figcraft_format::from_json(&v.to_string()).unwrap();
    assert_eq!((later.kind, later.board_style), (FileKind::Board, BoardStyle::Plain));
}

#[test]
fn a_live_replacement_keeps_this_tabs_style() {
    let mut s = Session::new();
    s.execute("file.newDiagram", &json!({"template": "flowChart"})).unwrap();
    let uid = s.ds().unwrap().uid;
    // The session's copy never has the style (it isn't sent), and others' edits arrive in it.
    let mut remote: Document = (*s.ds().unwrap().doc).clone();
    remote.board_style = BoardStyle::Plain;
    remote.name = "Renamed by a peer".into();
    s.apply_remote(uid, Arc::new(remote));
    let doc = s.doc().unwrap();
    assert_eq!((doc.name.as_str(), doc.board_style), ("Renamed by a peer", BoardStyle::Diagram));
}
