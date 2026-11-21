//! File ▸ New diagram…: the start dialog. The diagram's type (Flowchart or Use case; Mind map
//! comes with its own slice), what it starts from (Blank or the type's template; the flowchart's
//! with a picture of itself) and its name. Create runs `file.newDiagram`; the dialog never edits a document itself.
//! Use case and Class are typed diagrams, a preview: offered only while `pref.diagramPreview` is on.

use egui::{Align2, Context, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use super::{FigApp, Tokens, regular, semibold, tr, tr_noop, window};
use crate::a11y;
use crate::icons::{self, Icon};

/// The diagram types: (the `type` parameter, its label).
pub const TYPES: [(&str, &str); 3] = [("flowchart", tr_noop("Flowchart")), ("useCase", tr_noop("Use case")), ("class", tr_noop("Class"))];
/// What a flowchart starts from: (the `template` parameter, its label).
pub const TEMPLATES: [(&str, &str); 2] = [("blank", tr_noop("Blank")), ("flowChart", tr_noop("Flow chart"))];
/// What a Use Case diagram starts from.
pub const USE_CASE_TEMPLATES: [(&str, &str); 2] = [("blank", tr_noop("Blank")), ("useCaseExample", tr_noop("Example"))];
/// What a Class diagram starts from.
pub const CLASS_TEMPLATES: [(&str, &str); 2] = [("blank", tr_noop("Blank")), ("libraryLoans", tr_noop("Library loans"))];

/// The templates of type `kind` (an index into [`TYPES`]).
pub fn templates(kind: usize) -> &'static [(&'static str, &'static str)] {
    match kind {
        1 => &USE_CASE_TEMPLATES,
        2 => &CLASS_TEMPLATES,
        _ => &TEMPLATES,
    }
}
const CARD: egui::Vec2 = vec2(150.0, 112.0);
const PICTURE_H: f32 = 82.0;

/// The dialog while it is open.
#[derive(Clone, Debug)]
pub struct NewDiagramUi {
    pub name: String,
    /// Index into [`TYPES`]: Flowchart first, and chosen when the dialog opens.
    pub kind: usize,
    /// Index into the type's [`templates`]: Blank first, and chosen when the dialog opens.
    pub template: usize,
    /// The name field takes the keyboard on the dialog's first frame.
    focus_name: bool,
    /// An input method is composing in the name field: Enter is its own (it confirms a candidate)
    /// until the composition is committed or dismissed.
    composing: bool,
    /// Where the composition's candidate is in the name (characters, start..end), while one is in
    /// progress: the field shows it selected. Taken out of the name when the field loses the
    /// keyboard mid-composition (Create clicked, say), so a candidate never confirmed isn't saved
    /// and everything else typed stays.
    preedit: Option<(usize, usize)>,
}

impl Default for NewDiagramUi {
    fn default() -> Self {
        Self { name: tr("Untitled diagram").to_string(), kind: 0, template: 0, focus_name: true, composing: false, preedit: None }
    }
}

/// Opens the dialog (`dialog.newDiagram`), starting afresh.
pub fn open(app: &mut FigApp) {
    app.ui.new_diagram = Some(NewDiagramUi::default());
}

/// The parameters Create sends: the name as typed (the engine trims it and names an empty one).
pub fn params(d: &NewDiagramUi) -> serde_json::Value {
    let template = templates(d.kind).get(d.template).map_or("blank", |t| t.0);
    let kind = TYPES.get(d.kind).map_or("flowchart", |t| t.0);
    json!({"name": d.name, "type": kind, "template": template})
}

/// Create: makes the diagram, closes the dialog and leaves the home screen. A refusal (shown as a
/// message by `app.run`) keeps the dialog open as it was, and the current tab stays in front.
pub fn create(app: &mut FigApp) {
    let Some(d) = app.ui.new_diagram.clone() else { return };
    if app.run("file.newDiagram", params(&d)).is_some() {
        app.ui.new_diagram = None;
        app.ui.home = false;
    }
}

/// The templates' pictures, drawn the first time the dialog opens.
/// Shared, so reading them each frame copies a pointer, not the list.
#[derive(Clone, Default)]
struct Pictures(std::sync::Arc<Vec<Option<egui::TextureHandle>>>);

fn pictures_id() -> egui::Id {
    egui::Id::unique("new_diagram_pictures")
}

fn name_id() -> egui::Id {
    egui::Id::unique("new_diagram_name")
}

/// Below this window width the template cards stack in one column.
const NARROW: f32 = 640.0;

/// Enter creates while the dialog is open and nothing has the keyboard, taken before the
/// canvas's shortcuts see it (see [`super::keys`]). A focused control keeps Enter for itself (a
/// card picks its template, Cancel cancels), and in the name field Enter is read after the field's
/// typing (see [`enter_in_field`]).
pub(super) fn enter(app: &mut FigApp, ctx: &Context) {
    if app.ui.new_diagram.is_some()
        && ctx.memory(|m| m.focused().is_none())
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
    {
        create(app);
    }
}

/// Whether an Enter in this frame's `events` creates, read in order after the name field has taken
/// them: an Enter that arrives while an input method is composing confirms its candidate instead.
/// `composing` carries the composition from frame to frame.
pub fn enter_in_field(events: &[egui::Event], composing: &mut bool) -> bool {
    let mut go = false;
    for e in events {
        match e {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => *composing = !text.is_empty(),
            egui::Event::Ime(egui::ImeEvent::Commit(_)) => *composing = false,
            egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } if modifiers.is_none() && !*composing => go = true,
            _ => {}
        }
    }
    go
}

pub(super) fn show(app: &mut FigApp, ctx: &Context) {
    // Taken out while drawn and put back if it stays open.
    let Some(mut d) = app.ui.new_diagram.take() else { return };
    let t = Tokens::get(ctx);
    let mut pictures: Pictures = ctx.data(|m| m.get_temp(pictures_id())).unwrap_or_default();
    if pictures.0.len() != TEMPLATES.len() {
        let drawn = TEMPLATES.iter().map(|(id, _)| if *id == "blank" { None } else { crate::board_picker::picture(ctx, id) }).collect();
        pictures = Pictures(std::sync::Arc::new(drawn));
        ctx.data_mut(|m| m.insert_temp(pictures_id(), pictures.clone()));
    }
    let narrow = ctx.content_rect().width() < NARROW;
    // Typed diagrams (Use case, Class) are offered only while their preview is on.
    let preview = app.session.prefs.diagram_preview_on();
    if !preview && d.kind != 0 {
        (d.kind, d.template) = (0, 0);
    }
    let mut open = true;
    let (mut go, mut cancel) = (false, false);
    // The type and templates scroll when the window is too short for them; the name field (which
    // has the keyboard) and the buttons stay below them, in view.
    let form_h = (ctx.content_rect().height() - 230.0).max(40.0);
    window(tr("New diagram"), ctx).open(&mut open).show(ctx, |ui| {
        ui.set_width(if narrow { CARD.x } else { 2.0 * CARD.x + 8.0 });
        egui::ScrollArea::vertical().max_height(form_h).show(ui, |ui| form(ui, ctx, &t, &mut d, &pictures, preview));
        ui.add_space(12.0);
        name_field(ui, &t, &mut d);
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if crate::widgets::button(ui, tr("Create"), true, true).clicked() {
                go = true;
            }
            if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
                cancel = true;
            }
        });
    });
    // Enter in the name field, after it took this frame's typing.
    if ctx.memory(|m| m.has_focus(name_id())) {
        go |= ctx.input(|i| enter_in_field(&i.events, &mut d.composing));
        // The candidate's place, read only on a frame that sent one: right after the field put it
        // in (and selected it), before any later key can move the selection.
        let sent = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) if !text.is_empty())));
        if !d.composing {
            d.preedit = None;
        } else if sent {
            d.preedit = preedit_range(ctx);
        }
    } else if std::mem::take(&mut d.composing) {
        // The keyboard left mid-composition: the candidate was never confirmed, so drop it.
        if let Some(range) = d.preedit.take() {
            d.name = without_chars(&d.name, range);
        }
    }
    if !open || cancel {
        app.ui.new_diagram = None;
        return;
    }
    app.ui.new_diagram = Some(d);
    if go {
        create(app);
    }
}

/// The name field's selection, which is the candidate while an input method composes.
fn preedit_range(ctx: &Context) -> Option<(usize, usize)> {
    let range = egui::text_edit::TextEditState::load(ctx, name_id())?.cursor.char_range()?;
    let (a, b) = (range.primary.index, range.secondary.index);
    Some((a.min(b).into(), a.max(b).into()))
}

/// `text` without its characters `start..end` (as far as it has them).
pub fn without_chars(text: &str, (start, end): (usize, usize)) -> String {
    text.chars().enumerate().filter(|(i, _)| *i < start || *i >= end).map(|(_, c)| c).collect()
}

/// The dialog's choices: type and template.
fn form(ui: &mut egui::Ui, ctx: &Context, t: &Tokens, d: &mut NewDiagramUi, pictures: &Pictures, preview: bool) {
    let section = |ui: &mut egui::Ui, text: &str| {
        ui.label(RichText::new(text).font(semibold(11.0)).color(t.text_secondary));
        ui.add_space(4.0);
    };
    section(ui, tr("Type"));
    for (i, (_, label)) in TYPES.iter().enumerate() {
        if i != 0 && !preview {
            continue;
        }
        if crate::widgets::radio(ui, d.kind == i, tr(label)) && d.kind != i {
            d.kind = i;
            d.template = 0;
        }
    }
    ui.add_space(12.0);
    section(ui, tr("Template"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (i, (_, label)) in templates(d.kind).iter().enumerate() {
            let label = tr(label);
            let (r, resp) = ui.allocate_exact_size(CARD, Sense::click());
            a11y::radio(&resp, label, d.template == i);
            let p = ui.painter();
            let on = d.template == i;
            p.rect_filled(r, CornerRadius::same(8), if resp.hovered() { t.hover } else { t.panel });
            p.rect_stroke(
                r,
                CornerRadius::same(8),
                Stroke::new(if on { 2.0 } else { 1.0 }, if on { t.accent } else { t.border }),
                StrokeKind::Inside,
            );
            if resp.has_focus() {
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(2.0, t.focus), StrokeKind::Outside);
            }
            let pic = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(CARD.x - 12.0, PICTURE_H));
            p.rect_filled(pic, CornerRadius::same(6), crate::theme::c32(figcraft_doc::BOARD_PAGE));
            // Only the flowchart templates have pictures; the others show the diagram mark.
            let picture = if d.kind == 0 { pictures.0.get(i).and_then(Option::as_ref) } else { None };
            match picture {
                Some(tex) => {
                    let s = tex.size_vec2() / ctx.pixels_per_point();
                    let k = ((pic.width() - 12.0) / s.x).min((pic.height() - 12.0) / s.y).min(1.0);
                    p.image(
                        tex.id(),
                        Rect::from_center_size(pic.center(), s * k),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                // Blank (and a template whose picture couldn't be drawn): the diagram mark.
                None => icons::paint(p, Rect::from_center_size(pic.center(), vec2(24.0, 24.0)), Icon::Diagram, egui::Color32::from_gray(140)),
            }
            p.text(pos2(r.min.x + 8.0, pic.max.y + 13.0), Align2::LEFT_CENTER, label, regular(12.0), t.text);
            if resp.clicked() {
                d.template = i;
            }
        }
    });
}

/// The name field (the keyboard starts here).
fn name_field(ui: &mut egui::Ui, t: &Tokens, d: &mut NewDiagramUi) {
    ui.label(RichText::new(tr("Name")).font(semibold(11.0)).color(t.text_secondary));
    ui.add_space(4.0);
    // Enter doesn't take the keyboard away from the field: the dialog reads it (see `show`).
    let field = egui::TextEdit::singleline(&mut d.name).id(name_id()).desired_width(f32::INFINITY).return_key(None);
    let resp = ui.add(field);
    a11y::text(&resp, tr("Name"), &d.name);
    if std::mem::take(&mut d.focus_name) {
        resp.request_focus();
    }
    crate::widgets::select_all_on_focus(ui, &resp, &d.name);
}
