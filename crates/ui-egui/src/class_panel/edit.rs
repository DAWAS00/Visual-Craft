//! Typing members on the canvas, and the drag-out handle.
//!
//! **Inline editor.** A double-click on a member row (or Enter on the selected row) opens a text
//! field over the row; Alt+Shift+A and Alt+Shift+O (on a selected card, Class page only) and Insert
//! open a *draft*: a new row that exists only here until Enter commits it. Enter parses and commits
//! one command (`updateMember`, or `addMember` for a draft) and opens the next draft below; a
//! refused line shows the reason under the field and keeps the text. Escape closes without a
//! change. A draft goes when the page or file changes (the state is per page), on undo or redo,
//! or when its card goes.
//!
//! **Drag-out handle.** The selected card has a "+" on its right edge. Dragging it out and
//! dropping on empty canvas offers only the new elements that can be made from there (a subclass,
//! an interface it implements, a part it contains, ...): one choice is one `addConnected`. Dropping
//! on another card offers the relationships that would join the two, or says why none would.

use egui::{Area, Color32, Context, CornerRadius, Frame, Id, Key, Order, Pos2, Rect, RichText, Stroke, StrokeKind, vec2};
use figcraft_doc::class_diagram::{Classifier, Kind, LinkKind, Model, ROW_HEIGHT};
use figcraft_engine::class_layout::{Section, card_layout};
use figcraft_geom::Point;
use serde_json::{Value, json};

use super::canvas::{card_at, card_box, default_name, joins_cached};
use super::glyphs::{self, Glyph};
use super::{Comp, Place, State, current, link_label, load, save};
use crate::FigApp;
use crate::diagram_kit::chip;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular};

/// An open row editor.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub classifier: u64,
    pub comp: Comp,
    /// The member being edited, or `None` for a draft.
    pub member: Option<u64>,
    /// For a draft: the member it goes after (`None`: at the end).
    pub after: Option<u64>,
    pub text: String,
    pub error: Option<String>,
    /// The undo and redo depths when it opened: an undo or redo since closes a draft.
    pub steps: (usize, usize),
    /// The field takes the keyboard on its first frame.
    pub focus: bool,
}

/// The drag-out handle while it is dragged, or its chooser once dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct Out {
    pub source: u64,
    /// Where it was dropped (page), and on which card; `None` while still dragging.
    pub dropped: Option<(Point, Option<u64>)>,
}

fn steps(app: &FigApp) -> (usize, usize) {
    app.session.ds().map(|d| (d.history.undo.len(), d.history.redo.len())).unwrap_or((0, 0))
}

/// The compartment that `member` of `c` is in.
pub fn comp_of(c: &Classifier, member: u64) -> Option<Comp> {
    if c.attributes.iter().any(|a| a.id.0 == member) {
        Some(Comp::Attribute)
    } else if c.operations.iter().any(|o| o.id.0 == member) {
        Some(Comp::Operation)
    } else if c.literals.iter().any(|l| l.id.0 == member) {
        Some(Comp::Literal)
    } else {
        None
    }
}

fn member_text(c: &Classifier, member: u64) -> Option<String> {
    use figcraft_doc::class_diagram::text::{format_attribute, format_literal, format_operation};
    c.attributes
        .iter()
        .find(|a| a.id.0 == member)
        .map(format_attribute)
        .or_else(|| c.operations.iter().find(|o| o.id.0 == member).map(format_operation))
        .or_else(|| c.literals.iter().find(|l| l.id.0 == member).map(format_literal))
}

/// Opens the editor on an existing row.
pub fn open_row(app: &FigApp, st: &mut State, m: &Model, classifier: u64, member: u64) {
    let Some(c) = m.classifiers.iter().find(|c| c.id.0 == classifier) else { return };
    let (Some(comp), Some(text)) = (comp_of(c, member), member_text(c, member)) else { return };
    st.selected = Some(classifier);
    st.member = Some(member);
    st.editor = Some(Editor { classifier, comp, member: Some(member), after: None, text, error: None, steps: steps(app), focus: true });
}

/// Opens a draft in `comp` of `classifier`, after `after` (or at the end).
pub fn open_draft(app: &FigApp, st: &mut State, classifier: u64, comp: Comp, after: Option<u64>) {
    st.selected = Some(classifier);
    st.editor = Some(Editor { classifier, comp, member: None, after, text: String::new(), error: None, steps: steps(app), focus: true });
}

/// The compartment a new member of `c` goes in by default: literals for an enumeration.
pub fn default_comp(c: &Classifier, want: Comp) -> Comp {
    if c.kind == Kind::Enumeration { Comp::Literal } else { want }
}

/// Where the editor sits on the page: the row it edits, or for a draft the row after `after`
/// (the end of its compartment), one row tall and the card's width.
fn editor_rect(c: &Classifier, e: &Editor) -> Option<figcraft_geom::Rect> {
    let lay = card_layout(c);
    let section = match e.comp {
        Comp::Attribute => Section::Attributes,
        Comp::Operation => Section::Operations,
        Comp::Literal => Section::Literals,
    };
    let rows: Vec<_> = lay.compartments.iter().flat_map(|k| k.rows.iter()).collect();
    if let Some(member) = e.member {
        return rows.iter().find(|r| r.member.0 == member).map(|r| r.rect);
    }
    let comp = lay.compartments.iter().find(|k| k.section == section);
    let anchor = match e.after {
        Some(a) => rows.iter().find(|r| r.member.0 == a).map(|r| r.rect),
        None => comp.and_then(|k| k.rows.last()).map(|r| r.rect),
    };
    Some(match (anchor, comp) {
        (Some(r), _) => figcraft_geom::Rect::new(r.x0, r.y1, r.x1, r.y1 + ROW_HEIGHT),
        (None, Some(k)) => figcraft_geom::Rect::new(k.rect.x0, k.rect.y0, k.rect.x1, k.rect.y0 + ROW_HEIGHT),
        // A hidden or missing compartment: under the card.
        (None, None) => figcraft_geom::Rect::new(c.x, c.y + c.h, c.x + c.w, c.y + c.h + ROW_HEIGHT),
    })
}

/// Shows the editor and the drag-out chooser, and acts on them. Called once a frame while a
/// Class page is in view.
pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(m) = current(app) else { return };
    let mut st = load(ctx, app);
    if let Some(e) = st.editor.clone() {
        editor(app, ctx, &m, &mut st, e);
    }
    if st.out.as_ref().is_some_and(|o| o.dropped.is_some()) {
        chooser(app, ctx, &m, &mut st);
    }
    save(ctx, st);
}

fn editor(app: &mut FigApp, ctx: &Context, m: &Model, st: &mut State, mut e: Editor) {
    // Gone: its card, its row, or (for a draft) an undo or redo since it opened.
    let card = m.classifiers.iter().find(|c| c.id.0 == e.classifier);
    let row_gone = e.member.is_some_and(|id| card.is_none_or(|c| comp_of(c, id).is_none()));
    if card.is_none() || row_gone || (e.member.is_none() && steps(app) != e.steps) || app.session.active_read_only().is_some() {
        st.editor = None;
        return;
    }
    let Some(c) = card else { return };
    let Some(r) = editor_rect(c, &e) else {
        st.editor = None;
        return;
    };
    let rect = Rect::from_two_pos(app.to_screen(r.x0, r.y0), app.to_screen(r.x1, r.y1));
    let t = Tokens::get(ctx);
    let id = Id::unique(("class_row_editor", e.classifier, e.member, e.after));
    let width = rect.width().max(200.0);
    let mut commit = false;
    let mut close = false;
    Area::new(Id::unique("class_row_editor_area")).order(Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, crate::theme::canvas_accent(ctx))).corner_radius(CornerRadius::same(3)).show(ui, |ui| {
            let hint = match e.comp {
                Comp::Attribute => tr("- name : Type [0..1] = default"),
                Comp::Operation => tr("+ name(param: Type) : Return"),
                Comp::Literal => tr("NAME"),
            };
            let edit = egui::TextEdit::singleline(&mut e.text)
                .id(id)
                .hint_text(hint)
                .font(regular(12.0))
                .desired_width(width)
                .min_size(vec2(width, rect.height().max(18.0)));
            let resp = ui.add(edit);
            resp.widget_info(|| egui::WidgetInfo::text_edit(true, e.text.clone(), String::new(), hint));
            if e.focus {
                resp.request_focus();
                e.focus = false;
            }
            if resp.has_focus() || resp.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit = true;
                } else if resp.lost_focus() {
                    // Clicked away: a changed row or a typed draft commits. Anything else that takes
                    // the keyboard away (Escape, which the window may take before the field sees
                    // it) closes with no change.
                    let clicked = ui.input(|i| i.pointer.any_pressed());
                    let changed = match e.member {
                        Some(_) => c_text(c, &e).is_some_and(|old| old != e.text.trim()),
                        None => !e.text.trim().is_empty(),
                    };
                    if clicked && changed && e.error.is_none() {
                        commit = true;
                    } else {
                        close = true;
                    }
                }
            }
            if let Some(err) = &e.error {
                ui.label(RichText::new(err).font(regular(11.0)).color(t.danger));
            }
        });
    });
    if close {
        st.editor = None;
        return;
    }
    if !commit {
        st.editor = Some(e);
        return;
    }
    let text = e.text.trim().to_string();
    if text.is_empty() {
        // An emptied existing row isn't removed by accident: Delete does that.
        st.editor = None;
        return;
    }
    let (cmd, p) = match e.member {
        Some(id) => ("diagram.class.updateMember", json!({"id": id, "text": text})),
        None => {
            // A line with a parameter list typed in the attributes is an operation.
            let comp = if e.comp == Comp::Attribute && text.contains('(') { Comp::Operation } else { e.comp };
            let after = e.after.filter(|_| comp == e.comp);
            let mut p = json!({"classifier": e.classifier, "compartment": comp.param(), "text": text});
            if let Some(a) = after {
                p["after"] = json!(a);
            }
            ("diagram.class.addMember", p)
        }
    };
    match app.session.execute(cmd, &p) {
        Ok(r) => {
            app.handle_ui_requests();
            let done = r.get("id").and_then(Value::as_u64).or(e.member);
            st.member = done;
            // The next draft, right below.
            st.editor = Some(Editor { member: None, after: done, text: String::new(), error: None, steps: steps(app), focus: true, ..e });
        }
        Err(err) => {
            st.editor = Some(Editor { error: Some(reason(&err.to_string())), focus: true, ..e });
        }
    }
}

/// A refusal as the user reads it: the reason, without the command's name in front.
pub fn reason(e: &str) -> String {
    match e.split_once("`: ") {
        Some((head, rest)) if head.starts_with("invalid parameters for `") => rest.to_string(),
        _ => e.to_string(),
    }
}

/// The text of the member an editor edits, as it is now.
fn c_text(c: &Classifier, e: &Editor) -> Option<String> {
    e.member.and_then(|m| member_text(c, m))
}

/// The drag-out handle's centre on the screen, for card `id`.
pub fn handle_pos(app: &FigApp, m: &Model, id: u64) -> Option<Pos2> {
    let (x, y, w, h) = card_box(m, id)?;
    // Clear of the right edge, which resizes the card.
    Some(app.to_screen(x + w, y + h / 2.0) + vec2(22.0, 0.0))
}

/// The handle's radius (screen points): at least the 24-point hit target across.
pub const HANDLE_R: f32 = 12.0;

/// Draws the selected card's handle, and the line while it is dragged.
pub fn draw_handle(app: &FigApp, painter: &egui::Painter, m: &Model, st: &State, pointer: Option<Pos2>) {
    let Some(id) = st.selected.filter(|id| m.classifiers.iter().any(|c| c.id.0 == *id)) else { return };
    let Some(c) = handle_pos(app, m, id) else { return };
    let accent = crate::theme::canvas_accent(painter.ctx());
    let hot = pointer.is_some_and(|p| (p - c).length() <= HANDLE_R);
    painter.circle_filled(c, HANDLE_R * 0.75, if hot { accent } else { accent.gamma_multiply(0.85) });
    let s = HANDLE_R * 0.4;
    painter.line_segment([c - vec2(s, 0.0), c + vec2(s, 0.0)], Stroke::new(2.0, Color32::WHITE));
    painter.line_segment([c - vec2(0.0, s), c + vec2(0.0, s)], Stroke::new(2.0, Color32::WHITE));
    if let (Some(o), Some(p)) = (&st.out, pointer)
        && o.dropped.is_none()
    {
        painter.extend(egui::Shape::dashed_line(&[c, p], Stroke::new(1.5, accent), 6.0, 4.0));
        let ctx = painter.ctx();
        let over = card_at(m, app.to_page(p)).filter(|t| *t != o.source);
        let text = match over {
            Some(t) => {
                let (kinds, reason) = joins_cached(ctx, m, o.source, t);
                if kinds.is_empty() { reason.unwrap_or_default() } else { tr("Drop to choose a relationship").to_string() }
            }
            None => tr("Drop to add a connected element").to_string(),
        };
        chip(painter, p, text, accent);
        if let Some(t) = over
            && let Some(b) = card_box(m, t)
        {
            let r = Rect::from_two_pos(app.to_screen(b.0, b.1), app.to_screen(b.0 + b.2, b.1 + b.3));
            painter.rect_stroke(r.expand(3.0), CornerRadius::same(4), Stroke::new(2.0, accent), StrokeKind::Outside);
        }
    }
}

/// One choice when the handle is dropped on empty canvas: (its label, relation, the new
/// element's kind, direction, the whole end for an aggregation or composition).
pub type Choice = (&'static str, LinkKind, Place, &'static str, Option<&'static str>);

/// The new elements `source` can gain from the handle: only those the rules allow.
pub fn choices(source: &Classifier) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    match source.kind {
        Kind::Class => {
            out.push((tr("Subclass"), LinkKind::Generalization, Place::Class, "in", None));
            out.push((tr("Superclass"), LinkKind::Generalization, Place::Class, "out", None));
            out.push((tr("Implements new interface"), LinkKind::Realization, Place::Interface, "out", None));
        }
        Kind::Interface => {
            out.push((tr("Sub-interface"), LinkKind::Generalization, Place::Interface, "in", None));
            out.push((tr("Implementing class"), LinkKind::Realization, Place::Class, "in", None));
        }
        Kind::Enumeration => {
            out.push((tr("Implements new interface"), LinkKind::Realization, Place::Interface, "out", None));
        }
    }
    out.push((tr("Contains (whole here)"), LinkKind::Composition, Place::Class, "out", Some("source")));
    out.push((tr("Part of (whole there)"), LinkKind::Composition, Place::Class, "out", Some("new")));
    out.push((tr("Has (aggregation)"), LinkKind::Aggregation, Place::Class, "out", Some("source")));
    out.push((tr("Associated class"), LinkKind::Association, Place::Class, "out", None));
    out.push((tr("Uses"), LinkKind::Dependency, Place::Class, "out", None));
    out
}

/// The handle was pressed: start dragging it. `true` when the press was on it.
pub fn press_handle(app: &FigApp, m: &Model, st: &mut State, pos: Pos2) -> bool {
    let Some(id) = st.selected.filter(|id| m.classifiers.iter().any(|c| c.id.0 == *id)) else { return false };
    if handle_pos(app, m, id).is_some_and(|c| (pos - c).length() <= HANDLE_R) {
        st.out = Some(Out { source: id, dropped: None });
        return true;
    }
    false
}

/// The handle was let go at `pos`: open the chooser there.
pub fn drop_handle(app: &FigApp, m: &Model, st: &mut State, pos: Pos2) {
    let Some(o) = st.out.as_mut() else { return };
    let q = app.to_page(pos);
    let on = card_at(m, q).filter(|t| *t != o.source);
    // Let go on the handle itself (a click): nothing.
    if handle_pos(app, m, o.source).is_some_and(|c| (pos - c).length() <= HANDLE_R) {
        st.out = None;
        return;
    }
    o.dropped = Some((q, on));
}

fn chooser(app: &mut FigApp, ctx: &Context, m: &Model, st: &mut State) {
    let Some(Out { source, dropped: Some((q, on)) }) = st.out.clone() else { return };
    let Some(src) = m.classifiers.iter().find(|c| c.id.0 == source) else {
        st.out = None;
        return;
    };
    let t = Tokens::get(ctx);
    let at = app.to_screen(q.x, q.y);
    let mut pick: Option<Value> = None;
    let mut connect: Option<LinkKind> = None;
    let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
    let resp = Area::new(Id::unique("class_out_chooser")).order(Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_min_width(220.0);
            ui.set_max_width(260.0);
            match on {
                Some(target) => {
                    let (kinds, reason) = joins_cached(ctx, m, source, target);
                    ui.label(RichText::new(trf("Join to {name}", &[("name", &name(m, target))])).font(regular(11.0)).color(t.text_secondary));
                    if kinds.is_empty() {
                        ui.label(RichText::new(reason.unwrap_or_default()).font(regular(11.0)).color(t.danger));
                    }
                    for k in kinds {
                        if crate::diagram_kit::row(ui, &t, Glyph::of_link(k.name()).pic(), link_label(k), false, 0.0).clicked() {
                            connect = Some(k);
                        }
                    }
                }
                None => {
                    ui.label(RichText::new(tr("Add a connected element")).font(regular(11.0)).color(t.text_secondary));
                    for (label, rel, kind, dir, whole) in choices(src) {
                        let pic = move |p: &egui::Painter, r: Rect, ink: Color32| {
                            let half = Rect::from_min_size(r.min, vec2(r.width() * 0.5, r.height()));
                            glyphs::draw(p, half, kind.glyph(), ink);
                            glyphs::draw(p, half.translate(vec2(r.width() * 0.5, 0.0)), Glyph::of_link(rel.name()), ink);
                        };
                        if crate::diagram_kit::row(ui, &t, pic, label, false, 8.0).clicked() {
                            let mut p = json!({
                                "source": source,
                                "relation": rel.name(),
                                "kind": kind.param(),
                                "direction": dir,
                                "name": default_name(m, kind),
                                "x": q.x.round(),
                                "y": q.y.round(),
                            });
                            if let Some(w) = whole {
                                p["wholeEnd"] = json!(w);
                            }
                            pick = Some(p);
                        }
                    }
                }
            }
        });
    });
    let inside = ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| resp.response.rect.contains(p));
    if ctx.input(|i| i.pointer.any_pressed()) && !inside && pick.is_none() && connect.is_none() {
        close = true;
    }
    if let Some(p) = pick {
        let quiet = app.session.execute("diagram.class.addConnected", &p).ok();
        // The spot dropped on may be taken: then the element goes to free room nearby.
        let made = quiet.or_else(|| {
            let mut p = p.clone();
            if let Some(o) = p.as_object_mut() {
                o.remove("x");
                o.remove("y");
            }
            app.run("diagram.class.addConnected", p)
        });
        app.handle_ui_requests();
        if let Some(id) = made.as_ref().and_then(|r| r.get("element")).and_then(Value::as_u64) {
            st.selected = Some(id);
            st.member = None;
        }
        close = true;
    }
    if let (Some(k), Some(target)) = (connect, on) {
        if let Some(id) = app
            .run("diagram.class.connect", json!({"kind": k.name(), "from": source, "to": target}))
            .and_then(|r| r.get("id").and_then(Value::as_u64))
        {
            st.selected = Some(id);
            st.member = None;
        }
        close = true;
    }
    if close {
        st.out = None;
    }
}

fn name(m: &Model, id: u64) -> String {
    m.classifiers.iter().find(|c| c.id.0 == id).map_or_else(|| "?".into(), |c| c.name.clone())
}
