//! The Use Case diagram panel, shown beside the canvas while the page in view is a Use Case
//! diagram. Its tools are pictures: pick an element (actor, use case, system) and click the canvas
//! to place it, or pick a relationship and click two elements to join them ([`canvas`]). The
//! element or relationship selected on the canvas shows in a card to rename and edit; below it
//! come the diagram's unfinished parts, then its elements and relationships, each with its
//! picture. Every change runs one `diagram.useCase.*` command; the panel never edits the document
//! itself.

pub mod canvas;
pub mod glyphs;
pub mod handles;

use std::sync::Arc;

use egui::{Align2, Context, CornerRadius, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_doc::use_case::{Link, Model, Presentation, Warning};
use serde_json::{Value, json};

use self::canvas::link_ends;
use self::glyphs::Glyph;
use crate::FigApp;
use crate::i18n::{tr, tr_noop};
use crate::theme::{Tokens, regular, semibold};

/// The relationship kinds: (the `kind` parameter, its label, its picture).
pub const KINDS: [(&str, &str, Glyph); 4] = [
    ("association", tr_noop("Association"), Glyph::Association),
    ("include", tr_noop("Include"), Glyph::Include),
    ("extend", tr_noop("Extend"), Glyph::Extend),
    ("generalization", tr_noop("Generalization"), Glyph::Generalization),
];

/// What an Add tool places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Actor,
    UseCase,
    System,
}

impl Place {
    pub const ALL: [Place; 3] = [Place::Actor, Place::UseCase, Place::System];

    pub fn label(self) -> &'static str {
        match self {
            Place::Actor => tr_noop("Actor"),
            Place::UseCase => tr_noop("Use case"),
            Place::System => tr_noop("System"),
        }
    }

    pub fn glyph(self) -> Glyph {
        match self {
            Place::Actor => Glyph::Actor,
            Place::UseCase => Glyph::UseCase,
            Place::System => Glyph::System,
        }
    }
}

/// What a press on the diagram does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Select an element or relationship; drag an element to move it.
    #[default]
    Select,
    /// Place a new element where the press is.
    Place(Place),
    /// Join two elements with relationship `kind` (an index into [`KINDS`]); `from` once the
    /// first is picked.
    Connect { kind: usize, from: Option<u64> },
}

/// The panel's and the canvas's shared state, for one file and page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub tool: Tool,
    /// The selected element or relationship.
    pub selected: Option<u64>,
    /// The file and page this state is for: another one starts afresh.
    pub at: Option<(u64, NodeId)>,
    /// A new extension point's name, for the selected use case.
    pub point_draft: String,
}

pub(crate) fn state_id() -> Id {
    Id::unique("use_case_state")
}

fn here(app: &FigApp) -> Option<(u64, NodeId)> {
    app.session.ds().ok().map(|ds| (ds.uid, ds.page))
}

/// The state for the file and page in front.
pub fn load(ctx: &Context, app: &FigApp) -> State {
    let at = here(app);
    match ctx.data(|d| d.get_temp::<State>(state_id())) {
        Some(st) if st.at == at => st,
        _ => State { at, ..State::default() },
    }
}

pub fn save(ctx: &Context, st: State) {
    ctx.data_mut(|d| d.insert_temp(state_id(), st));
}

/// The Use Case model of the page in view, if it is one.
pub fn current(app: &FigApp) -> Option<Arc<Model>> {
    let page = app.session.page().ok()?;
    app.session.doc().ok()?.use_cases.get(&page).cloned()
}

/// The commands the panel asked for this frame, run after it is drawn.
type Actions = Vec<(&'static str, Value)>;

/// What a row or card is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum What {
    Actor(Presentation),
    UseCase,
    System,
    Link(usize),
}

fn what(m: &Model, id: u64) -> Option<What> {
    if let Some(a) = m.actors.iter().find(|a| a.id.0 == id) {
        return Some(What::Actor(a.presentation));
    }
    if m.use_cases.iter().any(|u| u.id.0 == id) {
        return Some(What::UseCase);
    }
    if m.subjects.iter().any(|s| s.id.0 == id) {
        return Some(What::System);
    }
    m.links.iter().find(|l| l.id().0 == id).map(|l| What::Link(kind_index(l)))
}

fn kind_index(l: &Link) -> usize {
    match l {
        Link::Association { .. } => 0,
        Link::Include { .. } => 1,
        Link::Extend { .. } => 2,
        Link::Generalization { .. } => 3,
    }
}

impl What {
    fn glyph(self) -> Glyph {
        match self {
            What::Actor(Presentation::Person) => Glyph::Actor,
            What::Actor(Presentation::System) => Glyph::ExternalSystem,
            What::UseCase => Glyph::UseCase,
            What::System => Glyph::System,
            What::Link(k) => KINDS.get(k).map_or(Glyph::Association, |k| k.2),
        }
    }

    fn label(self) -> &'static str {
        match self {
            What::Actor(_) => tr("Actor"),
            What::UseCase => tr("Use case"),
            What::System => tr("System"),
            What::Link(k) => tr(KINDS.get(k).map_or("Association", |k| k.1)),
        }
    }
}

/// A name of an element, by ID ("?" when it is gone).
fn name_of(m: &Model, id: u64) -> String {
    m.actors
        .iter()
        .find(|a| a.id.0 == id)
        .map(|a| a.name.clone())
        .or_else(|| m.use_cases.iter().find(|u| u.id.0 == id).map(|u| u.name.clone()))
        .or_else(|| m.subjects.iter().find(|x| x.id.0 == id).map(|x| x.name.clone()))
        .unwrap_or_else(|| "?".into())
}

fn warning_id(w: &Warning) -> u64 {
    match w {
        Warning::Unassigned(u) | Warning::UseCaseUnreached(u) => u.0,
        Warning::EmptySubject(x) => x.0,
        Warning::ActorUnconnected(a) => a.0,
    }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(model) = current(app) else { return };
    let mut st = load(ctx, app);
    // A selection undone or removed elsewhere is gone.
    if st.selected.is_some_and(|id| what(&model, id).is_none()) {
        st.selected = None;
    }
    let warnings = figcraft_doc::use_case::check(&model);
    let mut actions: Actions = Vec::new();
    let t = Tokens::get(ctx);
    // The panel opens at the top right and then stays where the user drags it. Its size is ours,
    // not egui's: egui grows a resizable window to fit its content, and the content (a wide row
    // in the selected card, say) would then stretch the panel. So the window has a fixed size that
    // we keep, and the grip in its corner changes that size.
    let content = ctx.content_rect();
    let min = vec2(180.0, 200.0);
    let max = vec2((content.width() - 32.0).max(min.x), (content.height() - 120.0).max(min.y));
    let size_id = Id::unique("use_case_panel_size");
    let stored = ctx.data(|d| d.get_temp::<egui::Vec2>(size_id));
    let size = stored.unwrap_or_else(|| vec2(296.0, (content.height() - 220.0).max(160.0))).clamp(min, max);
    let start = egui::pos2((content.right() - size.x - 16.0).max(content.left()), content.top() + 96.0);
    let mut resized = size;
    crate::dialogs::floating(tr("Use case diagram"), ctx).id(Id::unique("use_case_panel")).default_pos(start).fixed_size(size).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            tools(ui, &t, &mut st);
            selected_card(ui, &t, &model, &mut st, &mut actions);
            // What is unfinished matters more than the full list, so it comes first.
            unfinished(ui, &t, &model, &warnings, &mut st);
            lists(ui, &t, &model, &mut st);
        });
        resized = grip(ui, &t, size, min, max);
    });
    ctx.data_mut(|d| d.insert_temp(size_id, resized));
    save(ctx, st);
    for (cmd, params) in actions {
        app.run(cmd, params);
    }
}

/// The resize grip in the panel's bottom-right corner. A drag changes the panel's size; the result
/// is the size after this frame's drag, kept within `min` and `max`.
fn grip(ui: &mut Ui, t: &Tokens, size: egui::Vec2, min: egui::Vec2, max: egui::Vec2) -> egui::Vec2 {
    const SIDE: f32 = 16.0;
    let corner = ui.max_rect().max;
    let rect = Rect::from_min_max(corner - vec2(SIDE, SIDE), corner);
    let resp = ui.interact(rect, Id::unique("use_case_panel_grip"), Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    let painter = ui.painter();
    for k in [4.0, 8.0] {
        painter.line_segment([corner - vec2(k, 2.0), corner - vec2(2.0, k)], Stroke::new(1.0, t.icon));
    }
    (size + resp.drag_delta()).clamp(min, max)
}

fn section(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(text).font(semibold(11.0)).color(t.text));
    ui.add_space(4.0);
}

/// A tool: its picture over its name; outlined in the accent colour while it is the tool.
fn tile(ui: &mut Ui, t: &Tokens, glyph: Glyph, label: &str, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 60.0), Sense::click());
    let p = ui.painter();
    let fill = if on {
        t.accent.gamma_multiply(0.18)
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    if on {
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, t.accent), StrokeKind::Inside);
    }
    let ink = if on { t.accent_text } else { t.icon };
    glyphs::draw(p, Rect::from_center_size(pos2(rect.center().x, rect.min.y + 23.0), vec2(30.0, 30.0)), glyph, ink);
    p.text(pos2(rect.center().x, rect.max.y - 10.0), Align2::CENTER_CENTER, label, regular(10.0), if on { t.accent_text } else { t.text });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    resp
}

/// The tools: Select and the three elements, then the four relationships, and a line saying
/// what a press on the canvas does now.
fn tools(ui: &mut Ui, t: &Tokens, st: &mut State) {
    section(ui, t, tr("Add"));
    // Wrapped, so a narrow panel shows the tiles in more rows instead of cutting them off.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if tile(ui, t, Glyph::Select, tr("Select"), st.tool == Tool::Select).clicked() {
            st.tool = Tool::Select;
        }
        for kind in Place::ALL {
            let on = st.tool == Tool::Place(kind);
            if tile(ui, t, kind.glyph(), tr(kind.label()), on).clicked() {
                st.tool = if on { Tool::Select } else { Tool::Place(kind) };
            }
        }
    });
    section(ui, t, tr("Connect"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, (_, label, glyph)) in KINDS.iter().enumerate() {
            let on = matches!(st.tool, Tool::Connect { kind, .. } if kind == i);
            if tile(ui, t, *glyph, tr(label), on).clicked() {
                st.tool = if on { Tool::Select } else { Tool::Connect { kind: i, from: None } };
            }
        }
    });
    ui.add_space(6.0);
    let hint = match st.tool {
        Tool::Select => tr("Click an element to select it, or drag it to move it"),
        Tool::Place(_) => tr("Click the canvas to place it"),
        Tool::Connect { from: None, .. } => tr("Click the first element"),
        Tool::Connect { from: Some(_), .. } => tr("Click the element to connect it to (Esc stops)"),
    };
    ui.label(RichText::new(hint).font(regular(11.0)).color(t.text_secondary));
}

/// A small picture button (remove, connect from here), named for screen readers.
fn icon_button(ui: &mut Ui, t: &Tokens, glyph: Glyph, name: &str, danger: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    let ink = if danger && resp.hovered() { t.danger } else { t.icon };
    glyphs::draw(ui.painter(), rect.shrink(3.0), glyph, ink);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    resp.on_hover_text(name)
}

/// A list row: picture and text across the panel; highlighted when selected.
fn row(ui: &mut Ui, t: &Tokens, glyph: Glyph, text: &str, selected: bool, indent: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, CornerRadius::same(5), t.selected_row);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    let g = Rect::from_center_size(pos2(rect.min.x + indent + 12.0, rect.center().y), vec2(18.0, 18.0));
    glyphs::draw(&p, g, glyph, t.icon);
    p.text(pos2(g.max.x + 8.0, rect.center().y), Align2::LEFT_CENTER, text, regular(11.0), t.text);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, text));
    resp
}

/// The selected element's or relationship's card: what it is, its name and what can change.
fn selected_card(ui: &mut Ui, t: &Tokens, m: &Model, st: &mut State, actions: &mut Actions) {
    section(ui, t, tr("Selected"));
    let Some((id, w)) = st.selected.and_then(|id| what(m, id).map(|w| (id, w))) else {
        ui.label(RichText::new(tr("Nothing selected: click an element on the canvas")).font(regular(11.0)).color(t.text_secondary));
        return;
    };
    egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (g, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
            glyphs::draw(ui.painter(), g, w.glyph(), t.accent_text);
            ui.label(RichText::new(w.label()).font(semibold(12.0)).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, t, Glyph::Trash, tr("Remove"), true).clicked() {
                    actions.push(("diagram.useCase.remove", json!({"ids": [id]})));
                    st.selected = None;
                }
            });
        });
        ui.add_space(6.0);
        match w {
            What::Link(_) => link_card(ui, t, m, id, actions, st),
            _ => element_card(ui, t, m, id, w, st, actions),
        }
    });
}

/// A labelled text field: the new text when it is committed changed.
fn field(ui: &mut Ui, t: &Tokens, label: &str, salt: (&str, u64), value: &str) -> Option<String> {
    ui.label(RichText::new(label).font(regular(10.0)).color(t.text_secondary));
    let w = ui.available_width();
    let new = crate::widgets::text_field_named(ui, Id::unique(salt), value, w, label);
    new.filter(|n| n.trim() != value)
}

fn element_card(ui: &mut Ui, t: &Tokens, m: &Model, id: u64, w: What, st: &mut State, actions: &mut Actions) {
    let name = name_of(m, id);
    if let Some(new) = field(ui, t, tr("Name"), ("use_case_name", id), &name) {
        actions.push(("diagram.useCase.updateElement", json!({"id": id, "name": new})));
    }
    match w {
        What::Actor(p) => {
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Shown as")).font(regular(10.0)).color(t.text_secondary));
            ui.horizontal(|ui| {
                for (pres, glyph, label, v) in [
                    (Presentation::Person, Glyph::Actor, tr("Person"), "person"),
                    (Presentation::System, Glyph::ExternalSystem, tr("External system"), "system"),
                ] {
                    if tile(ui, t, glyph, label, p == pres).clicked() && p != pres {
                        actions.push(("diagram.useCase.updateElement", json!({"id": id, "presentation": v})));
                    }
                }
            });
        }
        What::UseCase => use_case_extras(ui, t, m, id, st, actions),
        _ => {}
    }
    ui.add_space(8.0);
    // Start a relationship here: the next press on the canvas picks where it goes.
    if !matches!(w, What::System) {
        ui.label(RichText::new(tr("Connect from here")).font(regular(10.0)).color(t.text_secondary));
        // Only the joins this element can start, to some other element in the diagram.
        let starts = start_kinds(ui.ctx(), m, id);
        ui.horizontal_wrapped(|ui| {
            for (i, (name, label, glyph)) in KINDS.iter().enumerate() {
                if !starts.contains(name) {
                    continue;
                }
                if icon_button(ui, t, *glyph, tr(label), false).clicked() {
                    st.tool = Tool::Connect { kind: i, from: Some(id) };
                }
            }
        });
    }
    // Its relationships.
    let mine: Vec<&Link> = m
        .links
        .iter()
        .filter(|l| {
            let (a, b) = link_ends(l);
            a == id || b == id
        })
        .collect();
    if !mine.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new(tr("Relationships")).font(regular(10.0)).color(t.text_secondary));
        for l in mine {
            let (a, b) = link_ends(l);
            let glyph = KINDS.get(kind_index(l)).map_or(Glyph::Association, |k| k.2);
            if row(ui, t, glyph, &format!("{} → {}", name_of(m, a), name_of(m, b)), false, 0.0).clicked() {
                st.selected = Some(l.id().0);
            }
        }
    }
}

/// A use case's system and extension points.
fn use_case_extras(ui: &mut Ui, t: &Tokens, m: &Model, id: u64, st: &mut State, actions: &mut Actions) {
    let Some(u) = m.use_cases.iter().find(|u| u.id.0 == id) else { return };
    ui.add_space(6.0);
    ui.label(RichText::new(tr("System")).font(regular(10.0)).color(t.text_secondary));
    let mut subject = u.subject.map(|s| s.0);
    let label = subject.map_or_else(|| tr("No system").to_string(), |s| name_of(m, s));
    let before = subject;
    craft_ui::ComboBox::from_id_salt(("use_case_subject", id)).selected_text(label).width(ui.available_width()).show_ui(ui, |ui| {
        ui.selectable_value(&mut subject, None, tr("No system"));
        for s in &m.subjects {
            ui.selectable_value(&mut subject, Some(s.id.0), &s.name);
        }
    });
    if subject != before {
        actions.push(("diagram.useCase.setSubject", json!({"useCase": id, "subject": subject})));
    }
    ui.add_space(6.0);
    ui.label(RichText::new(tr("Extension points")).font(regular(10.0)).color(t.text_secondary));
    for p in &u.points {
        ui.horizontal(|ui| {
            let w = ui.available_width() - 30.0;
            if let Some(new) = crate::widgets::text_field_named(ui, Id::unique(("use_case_point", p.id.0)), &p.name, w, tr("Extension points"))
                && new.trim() != p.name
            {
                actions.push(("diagram.useCase.updateExtensionPoint", json!({"id": p.id.0, "name": new})));
            }
            if icon_button(ui, t, Glyph::Trash, tr("Remove"), true).clicked() {
                actions.push(("diagram.useCase.removeExtensionPoint", json!({"id": p.id.0})));
            }
        });
    }
    ui.horizontal(|ui| {
        let w = ui.available_width() - 76.0;
        ui.add(
            egui::TextEdit::singleline(&mut st.point_draft).id(Id::unique(("use_case_new_point", id))).hint_text(tr("New point")).desired_width(w),
        );
        let name = st.point_draft.trim().to_string();
        if crate::widgets::button(ui, tr("Add point"), false, !name.is_empty()).clicked() {
            actions.push(("diagram.useCase.addExtensionPoint", json!({"useCase": id, "name": name})));
            st.point_draft.clear();
        }
    });
}

/// The kinds `id` can start to some other element, worked out once per model and element and
/// kept until either changes (each check walks the links, so not every frame).
fn start_kinds(ctx: &egui::Context, m: &Model, id: u64) -> Vec<&'static str> {
    type Cached = (Model, u64, Vec<&'static str>);
    let key = Id::unique("use_case_start_kinds");
    if let Some((cm, cid, kinds)) = ctx.data(|d| d.get_temp::<std::sync::Arc<Cached>>(key)).as_deref()
        && *cid == id
        && cm == m
    {
        return kinds.clone();
    }
    let mut kinds: Vec<&'static str> = Vec::new();
    for o in m.actors.iter().map(|a| a.id.0).chain(m.use_cases.iter().map(|u| u.id.0)) {
        for k in figcraft_doc::use_case::connection_kinds(m, id, o) {
            if !kinds.contains(&k) {
                kinds.push(k);
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(key, std::sync::Arc::new((m.clone(), id, kinds.clone()))));
    kinds
}

/// A relationship's ends and what it carries: an extend's points and condition, an
/// association's multiplicities.
fn link_card(ui: &mut Ui, t: &Tokens, m: &Model, id: u64, actions: &mut Actions, st: &mut State) {
    let Some(l) = m.links.iter().find(|l| l.id().0 == id) else { return };
    let (a, b) = link_ends(l);
    // One end per line, each cut to the panel's width with the full name on hover, so long or
    // wide names never widen the panel past its size.
    for (i, e) in [a, b].into_iter().enumerate() {
        let full = name_of(m, e);
        let text = if i == 0 { full.clone() } else { format!("→ {full}") };
        let button = egui::Button::new(text).wrap_mode(egui::TextWrapMode::Truncate);
        if ui.add_sized([ui.available_width(), 22.0], button).on_hover_text(full).clicked() {
            st.selected = Some(e);
        }
    }
    match l {
        Link::Extend { points, condition, base, .. } => {
            if let Some(new) = field(ui, t, tr("Condition"), ("use_case_condition", id), condition.as_deref().unwrap_or("")) {
                actions.push(("diagram.useCase.updateLink", json!({"id": id, "condition": new.trim()})));
            }
            let base_points = m.use_cases.iter().find(|u| u.id == *base).map(|u| u.points.clone()).unwrap_or_default();
            if !base_points.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Extension points")).font(regular(10.0)).color(t.text_secondary));
                for p in &base_points {
                    let mut on = points.contains(&p.id);
                    if ui.checkbox(&mut on, &p.name).changed() {
                        let mut next: Vec<u64> = points.iter().map(|x| x.0).filter(|x| *x != p.id.0).collect();
                        if on {
                            next.push(p.id.0);
                        }
                        actions.push(("diagram.useCase.updateLink", json!({"id": id, "points": next})));
                    }
                }
            }
        }
        Link::Association { actor_multiplicity, use_case_multiplicity, .. } => {
            if let Some(new) = field(ui, t, tr("Multiplicity at the actor"), ("use_case_mult_a", id), actor_multiplicity.as_deref().unwrap_or("")) {
                actions.push(("diagram.useCase.updateLink", json!({"id": id, "actorMultiplicity": new.trim()})));
            }
            if let Some(new) =
                field(ui, t, tr("Multiplicity at the use case"), ("use_case_mult_u", id), use_case_multiplicity.as_deref().unwrap_or(""))
            {
                actions.push(("diagram.useCase.updateLink", json!({"id": id, "useCaseMultiplicity": new.trim()})));
            }
        }
        _ => {}
    }
}

/// The diagram's systems (with their use cases under them), use cases outside any system,
/// actors and relationships: a click selects.
fn lists(ui: &mut Ui, t: &Tokens, m: &Model, st: &mut State) {
    if m.subjects.is_empty() && m.use_cases.is_empty() && m.actors.is_empty() {
        return;
    }
    section(ui, t, tr("Diagram"));
    let mut pick = |ui: &mut Ui, glyph: Glyph, text: &str, id: u64, indent: f32| {
        if row(ui, t, glyph, text, st.selected == Some(id), indent).clicked() {
            st.selected = Some(id);
        }
    };
    for s in &m.subjects {
        pick(ui, Glyph::System, &s.name, s.id.0, 0.0);
        for u in m.use_cases.iter().filter(|u| u.subject == Some(s.id)) {
            pick(ui, Glyph::UseCase, &u.name, u.id.0, 18.0);
        }
    }
    for u in m.use_cases.iter().filter(|u| u.subject.is_none()) {
        pick(ui, Glyph::UseCase, &u.name, u.id.0, 0.0);
    }
    for a in &m.actors {
        let glyph = if a.presentation == Presentation::System { Glyph::ExternalSystem } else { Glyph::Actor };
        pick(ui, glyph, &a.name, a.id.0, 0.0);
    }
    if !m.links.is_empty() {
        section(ui, t, tr("Relationships"));
        for l in &m.links {
            let (a, b) = link_ends(l);
            let glyph = KINDS.get(kind_index(l)).map_or(Glyph::Association, |k| k.2);
            pick(ui, glyph, &format!("{} → {}", name_of(m, a), name_of(m, b)), l.id().0, 0.0);
        }
    }
}

/// What is unfinished: a click selects the element it is about.
fn unfinished(ui: &mut Ui, t: &Tokens, m: &Model, warnings: &[Warning], st: &mut State) {
    section(ui, t, tr("Unfinished"));
    if warnings.is_empty() {
        ui.horizontal(|ui| {
            let (g, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            glyphs::draw(ui.painter(), g, Glyph::Check, t.success);
            ui.label(RichText::new(tr("Nothing unfinished")).font(regular(11.0)).color(t.text));
        });
        return;
    }
    for w in warnings {
        let id = warning_id(w);
        if row(ui, t, Glyph::Warning, &w.message(), st.selected == Some(id), 0.0).clicked() && what(m, id).is_some() {
            st.selected = Some(id);
        }
    }
}
