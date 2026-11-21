//! File ▸ New diagram… (diagram slice 2): the start dialog opens from its request and the home
//! card, Create makes a diagram from the chosen template, Enter creates and Escape cancels, and a
//! diagram's tab shows the diagram mark.

use egui::{Event, Key, Modifiers};
use figcraft_doc::BoardStyle;
use serde_json::json;

use crate::export_preview_tests::{click, labelled};
use crate::icons::{Icon, doc_mark};
use crate::layers_drag_tests::Ui;

fn opened() -> Ui {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.run("dialog.newDiagram", json!({}));
    u.frame(vec![]);
    u.frame(vec![]);
    u
}

fn press(u: &mut Ui, text: &str) {
    let shown = labelled(u);
    let at = shown.iter().rev().find(|(t, _)| t == text).map(|x| x.1).unwrap_or_else(|| panic!("{text:?} in {shown:?}"));
    click(u, at);
}

fn key(u: &mut Ui, key: Key) {
    let ev = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    u.frame(vec![ev(true)]);
    u.frame(vec![ev(false)]);
    u.frame(vec![]);
}

#[test]
fn the_request_opens_the_dialog_with_blank_chosen() {
    let mut u = opened();
    let d = u.app.ui.new_diagram.clone().expect("the dialog is open");
    assert_eq!((d.name.as_str(), d.template), ("Untitled diagram", 0));
    let texts: Vec<String> = labelled(&mut u).into_iter().map(|(t, _)| t).collect();
    for want in ["New diagram", "Flowchart", "Blank", "Flow chart", "Create", "Cancel"] {
        assert!(texts.iter().any(|t| t == want), "{want:?} in {texts:?}");
    }
    // The name field has the keyboard, so typing renames at once.
    assert!(u.ctx.memory(|m| m.focused()).is_some(), "the name field is focused");
}

#[test]
fn create_makes_a_diagram_from_the_chosen_template() {
    let mut u = opened();
    let count = u.app.session.docs.len();
    press(&mut u, "Flow chart");
    assert_eq!(u.app.ui.new_diagram.as_ref().map(|d| d.template), Some(1));
    press(&mut u, "Create");
    assert!(u.app.ui.new_diagram.is_none(), "it closes");
    assert_eq!(u.app.session.docs.len(), count + 1);
    let doc = u.app.session.doc().unwrap();
    assert_eq!(doc.board_style, BoardStyle::Diagram);
    assert!(!doc.node(doc.first_page().unwrap()).unwrap().children.is_empty(), "the template's content");
    assert_eq!(doc_mark(doc), Icon::Diagram);
}

#[test]
fn enter_creates_with_the_typed_name_and_escape_makes_nothing() {
    let mut u = opened();
    let count = u.app.session.docs.len();
    if let Some(d) = u.app.ui.new_diagram.as_mut() {
        d.name = "Review flow".into();
    }
    key(&mut u, Key::Enter);
    assert!(u.app.ui.new_diagram.is_none());
    assert_eq!(u.app.session.docs.len(), count + 1, "exactly one diagram");
    let doc = u.app.session.doc().unwrap();
    assert_eq!((doc.name.as_str(), doc.board_style), ("Review flow", BoardStyle::Diagram));
    assert_eq!(doc.node(doc.first_page().unwrap()).unwrap().children.len(), 0, "Blank, and Enter reached nothing else");

    let mut u = opened();
    let (count, active) = (u.app.session.docs.len(), u.app.session.active);
    key(&mut u, Key::Escape);
    assert!(u.app.ui.new_diagram.is_none());
    assert_eq!((u.app.session.docs.len(), u.app.session.active), (count, active));
}

#[test]
fn typing_and_enter_in_one_frame_keep_every_character() {
    let mut u = opened();
    let enter = |pressed| Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    // The field selected its text when it took the keyboard, so typing replaces it.
    u.frame(vec![Event::Text("Review flow".into()), enter(true)]);
    u.frame(vec![enter(false)]);
    u.frame(vec![]);
    assert!(u.app.ui.new_diagram.is_none(), "Enter in the field creates");
    let doc = u.app.session.doc().unwrap();
    assert_eq!((doc.name.as_str(), doc.board_style), ("Review flow", BoardStyle::Diagram));
}

#[test]
fn enter_that_confirms_an_input_method_candidate_does_not_create() {
    let mut u = opened();
    let enter = |pressed| Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    let preedit = Event::Ime(egui::ImeEvent::Preedit { text: "にほん".into(), active_range_chars: None });
    u.frame(vec![preedit, enter(true), Event::Ime(egui::ImeEvent::Commit("日本".into()))]);
    u.frame(vec![enter(false)]);
    u.frame(vec![]);
    let d = u.app.ui.new_diagram.clone().expect("still open: that Enter was the input method's");
    assert!(d.name.contains("日本"), "the committed text is in the name: {:?}", d.name);
    // The next Enter, with nothing composing, creates.
    key(&mut u, Key::Enter);
    assert!(u.app.ui.new_diagram.is_none());
    assert!(u.app.session.doc().unwrap().name.contains("日本"));
}

#[test]
fn create_clicked_mid_composition_drops_the_unconfirmed_candidate() {
    let mut u = opened();
    let preedit = Event::Ime(egui::ImeEvent::Preedit { text: "にほ".into(), active_range_chars: None });
    u.frame(vec![preedit]);
    u.frame(vec![]);
    press(&mut u, "Create");
    assert!(u.app.ui.new_diagram.is_none());
    let name = u.app.session.doc().unwrap().name.clone();
    assert!(!name.contains("にほ"), "a candidate never confirmed isn't saved: {name:?}");
    assert_eq!(name, "Untitled diagram");
}

#[test]
fn text_typed_with_the_composition_in_one_frame_is_kept() {
    let mut u = opened();
    let preedit = Event::Ime(egui::ImeEvent::Preedit { text: "にほ".into(), active_range_chars: None });
    // Typed (replacing the selected name), then a composition starts, in the same frame.
    u.frame(vec![Event::Text("Draft".into()), preedit]);
    u.frame(vec![]);
    assert!(u.app.ui.new_diagram.as_ref().is_some_and(|d| d.name.contains("にほ")), "the candidate shows in the field");
    press(&mut u, "Create");
    assert_eq!(u.app.session.doc().unwrap().name, "Draft", "only the unconfirmed candidate is dropped");
}

#[test]
fn select_all_during_a_composition_still_drops_only_the_candidate() {
    let mut u = opened();
    let preedit = Event::Ime(egui::ImeEvent::Preedit { text: "にほ".into(), active_range_chars: None });
    u.frame(vec![Event::Text("Draft".into()), preedit]);
    u.frame(vec![]);
    let all = |pressed| Event::Key { key: Key::A, physical_key: None, pressed, repeat: false, modifiers: Modifiers::COMMAND };
    u.frame_with(vec![all(true)], Modifiers::COMMAND);
    u.frame_with(vec![all(false)], Modifiers::COMMAND);
    u.frame(vec![]);
    press(&mut u, "Create");
    assert_eq!(u.app.session.doc().unwrap().name, "Draft");
}

#[test]
fn removing_a_candidate_counts_characters() {
    use crate::new_diagram::without_chars;
    assert_eq!(without_chars("Draftにほ", (5, 7)), "Draft");
    assert_eq!(without_chars("aにほb", (1, 3)), "ab");
    assert_eq!(without_chars("ab", (1, 9)), "a", "a range past the end");
}

#[test]
fn enter_reads_in_order_with_the_composition() {
    let enter = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    let pre = |t: &str| Event::Ime(egui::ImeEvent::Preedit { text: t.into(), active_range_chars: None });
    let commit = Event::Ime(egui::ImeEvent::Commit("x".into()));
    let mut composing = false;
    assert!(!crate::new_diagram::enter_in_field(&[pre("a"), enter.clone()], &mut composing));
    assert!(composing, "still composing into the next frame");
    assert!(!crate::new_diagram::enter_in_field(std::slice::from_ref(&enter), &mut composing), "a later frame's Enter while composing");
    assert!(crate::new_diagram::enter_in_field(&[commit, enter.clone()], &mut composing), "committed, then Enter");
    assert!(crate::new_diagram::enter_in_field(&[pre("a"), pre(""), enter], &mut composing), "a dismissed composition");
}

#[test]
fn enter_on_a_focused_cancel_button_cancels() {
    let mut u = opened();
    let (count, active) = (u.app.session.docs.len(), u.app.session.active);
    // From the name field: Create, then Cancel.
    key(&mut u, Key::Tab);
    key(&mut u, Key::Tab);
    assert!(!u.ctx.memory(|m| m.has_focus(egui::Id::unique("new_diagram_name"))));
    key(&mut u, Key::Enter);
    assert!(u.app.ui.new_diagram.is_none(), "Cancel took the Enter");
    assert_eq!((u.app.session.docs.len(), u.app.session.active), (count, active), "nothing made");
}

#[test]
fn a_narrow_window_stacks_the_templates_and_keeps_the_buttons_on_screen() {
    for (w, h) in [(360.0, 720.0), (360.0, 320.0)] {
        let mut u = opened();
        u.screen = egui::vec2(w, h);
        for _ in 0..3 {
            u.frame(vec![]);
        }
        let shown = labelled(&mut u);
        let at = |text: &str| shown.iter().rev().find(|(t, _)| t == text).map(|x| x.1).unwrap_or_else(|| panic!("{text:?} in {shown:?}"));
        if h > 600.0 {
            let (blank, flow) = (at("Blank"), at("Flow chart"));
            assert!(flow.y > blank.y + 50.0, "one column: {blank:?} {flow:?}");
        }
        for button in ["Create", "Cancel"] {
            let p = at(button);
            assert!(p.x > 0.0 && p.x < w && p.y > 0.0 && p.y < h, "{button} at {p:?} in {w}×{h}");
        }
        // The name field, which has the keyboard, is in view too.
        let field = u.ctx.read_response(egui::Id::unique("new_diagram_name")).expect("the name field").rect;
        assert!(u.ctx.memory(|m| m.has_focus(egui::Id::unique("new_diagram_name"))));
        assert!(field.min.y >= 0.0 && field.max.y <= h && field.min.x >= 0.0 && field.max.x <= w, "{field:?} in {w}×{h}");
        // And they work there.
        press(&mut u, "Create");
        assert!(u.app.ui.new_diagram.is_none(), "{w}×{h}");
    }
}

#[test]
fn a_template_opens_fitted_to_a_small_window() {
    let mut u = opened();
    u.screen = egui::vec2(800.0, 600.0);
    u.frame(vec![]);
    press(&mut u, "Flow chart");
    press(&mut u, "Create");
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let ds = u.app.session.ds().unwrap();
    let b = ds.doc.page_content_bounds(ds.page).expect("the template's content");
    let v = &ds.view;
    assert!(v.vw < 800.0, "the canvas's own size: {}", v.vw);
    assert!(b.width() * v.zoom <= v.vw + 1.0 && b.height() * v.zoom <= v.vh + 1.0, "fits: {b:?} at {} in {}×{}", v.zoom, v.vw, v.vh);
}

#[test]
fn the_home_card_opens_the_dialog_and_create_leaves_home() {
    let mut u = Ui::new(figcraft_engine::Session::new());
    u.app.ui.home = true;
    u.frame(vec![]);
    press(&mut u, "New diagram");
    assert!(u.app.ui.new_diagram.is_some(), "the card asks for the dialog");
    assert!(u.app.ui.home, "home stays behind the dialog");
    press(&mut u, "Create");
    assert!(!u.app.ui.home);
    assert_eq!(u.app.session.doc().unwrap().board_style, BoardStyle::Diagram);
}

#[test]
fn plain_boards_and_design_files_keep_their_marks() {
    let mut s = figcraft_engine::Session::new();
    assert_eq!(doc_mark(s.doc().unwrap()), Icon::PenNib);
    s.execute("file.newBoard", &json!({})).unwrap();
    assert_eq!(doc_mark(s.doc().unwrap()), Icon::Board);
}
