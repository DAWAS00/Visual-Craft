//! The Class diagram panel, shown beside the canvas while the page in view is a Class diagram. Its
//! tools are pictures: pick an element kind (class, interface, enumeration) and click the canvas
//! to place one, or pick a relationship and click two cards to join them ([`canvas`]). The card
//! or relationship selected on the canvas shows below to rename and edit: a card's members are
//! rows of UML text (`- balance : Money = 0`), each with its visibility, order and Remove; a
//! relationship's ends have their role, multiplicity and navigability. Then come the diagram's
//! unfinished parts and its lists. Every change runs one `diagram.class.*` command; the panel
//! never edits the document itself.

pub mod canvas;
pub mod edit;
pub mod glyphs;

use std::sync::Arc;

use egui::{Context, CornerRadius, Id, RichText, Sense, Ui, vec2};
use figcraft_doc::NodeId;
use figcraft_doc::class_diagram::text::{format_attribute, format_literal, format_operation};
use figcraft_doc::class_diagram::{ALL_LINK_KINDS, Classifier, Kind, Link, LinkKind, Model, Navigable, Visibility};
use serde_json::{Value, json};

use self::glyphs::Glyph;
use crate::FigApp;
use crate::diagram_kit::{field, icon_button, note, row, section, tile};
use crate::i18n::{tr, tr_noop, trf};
use crate::theme::{Tokens, regular, semibold};

/// What an Add tool places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Class,
    Interface,
    Enumeration,
}

impl Place {
    pub const ALL: [Place; 3] = [Place::Class, Place::Interface, Place::Enumeration];

    pub fn of(kind: Kind) -> Place {
        match kind {
            Kind::Class => Place::Class,
            Kind::Interface => Place::Interface,
            Kind::Enumeration => Place::Enumeration,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Place::Class => tr_noop("Class"),
            Place::Interface => tr_noop("Interface"),
            Place::Enumeration => tr_noop("Enumeration"),
        }
    }

    /// The `kind` parameter.
    pub fn param(self) -> &'static str {
        match self {
            Place::Class => "class",
            Place::Interface => "interface",
            Place::Enumeration => "enumeration",
        }
    }

    pub fn glyph(self) -> Glyph {
        match self {
            Place::Class => Glyph::Class,
            Place::Interface => Glyph::Interface,
            Place::Enumeration => Glyph::Enumeration,
        }
    }
}

/// A relationship kind's name in the interface.
pub fn link_label(k: LinkKind) -> &'static str {
    match k {
        LinkKind::Association => tr("Association"),
        LinkKind::Aggregation => tr("Aggregation"),
        LinkKind::Composition => tr("Composition"),
        LinkKind::Generalization => tr("Generalization"),
        LinkKind::Realization => tr("Realization"),
        LinkKind::Dependency => tr("Dependency"),
    }
}

/// What a press on the diagram does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Select a card, a member row or a relationship; drag a card to move it.
    #[default]
    Select,
    /// Place a new classifier where the press is.
    Place(Place),
    /// Join two cards with relationship `kind`; `from` once the first is picked.
    Connect { kind: LinkKind, from: Option<u64> },
}

/// The panel's and the canvas's shared state, for one file and page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub tool: Tool,
    /// The selected classifier or relationship.
    pub selected: Option<u64>,
    /// The selected member row, within the selected classifier.
    pub member: Option<u64>,
    /// The file and page this state is for: another one starts afresh.
    pub at: Option<(u64, NodeId)>,
    /// The open row editor or draft on the canvas.
    pub editor: Option<edit::Editor>,
    /// The drag-out handle, while dragged or dropped.
    pub out: Option<edit::Out>,
}

pub(crate) fn state_id() -> Id {
    Id::unique("class_state")
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

/// The Class model of the page in view, if it is one.
pub fn current(app: &FigApp) -> Option<Arc<Model>> {
    let page = app.session.page().ok()?;
    app.session.doc().ok()?.class_diagrams.get(&page).cloned()
}

/// The commands the panel asked for this frame, run after it is drawn.
type Actions = Vec<(&'static str, Value)>;

/// A classifier's name by ID ("?" when it is gone).
fn name_of(m: &Model, id: u64) -> String {
    m.classifiers.iter().find(|c| c.id.0 == id).map_or_else(|| "?".into(), |c| c.name.clone())
}

fn link_row_text(m: &Model, l: &Link) -> String {
    let (a, b) = l.pair();
    format!("{} → {}", name_of(m, a.0), name_of(m, b.0))
}

/// Drops a selection that no longer exists (undone, or removed elsewhere).
fn prune(m: &Model, st: &mut State) {
    let card = st.selected.and_then(|id| m.classifiers.iter().find(|c| c.id.0 == id));
    if st.selected.is_some() && card.is_none() && !m.links.iter().any(|l| Some(l.id.0) == st.selected) {
        st.selected = None;
    }
    let has = |c: &Classifier, id: u64| {
        c.attributes.iter().any(|a| a.id.0 == id) || c.operations.iter().any(|o| o.id.0 == id) || c.literals.iter().any(|l| l.id.0 == id)
    };
    if st.member.is_some_and(|id| !card.is_some_and(|c| has(c, id))) {
        st.member = None;
    }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    if current(app).is_none() {
        return;
    }
    // The editor and the chooser first: what they make is then in the model the panel shows (and
    // a new element they select isn't dropped as gone).
    edit::show(app, ctx);
    let Some(model) = current(app) else { return };
    let mut st = load(ctx, app);
    prune(&model, &mut st);
    let mut actions: Actions = Vec::new();
    let t = Tokens::get(ctx);
    let size_id = Id::unique("class_panel_size");
    let (size, min, max, start) = crate::diagram_kit::panel_size(ctx, size_id, 320.0);
    let mut resized = size;
    let read_only = app.session.active_read_only().is_some();
    crate::dialogs::floating(tr("Class diagram"), ctx).id(Id::unique("class_panel")).default_pos(start).fixed_size(size).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if read_only {
                note(ui, &t, tr("This file is read-only"), 11.0);
            }
            ui.add_enabled_ui(!read_only, |ui| {
                tools(ui, &t, &mut st);
                selected(ui, &t, &model, &mut st, &mut actions);
            });
            unfinished(ui, &t, &model, &mut st);
            lists(ui, &t, &model, &mut st);
        });
        resized = crate::diagram_kit::grip(ui, &t, Id::unique("class_panel_grip"), size, min, max);
    });
    ctx.data_mut(|d| d.insert_temp(size_id, resized));
    save(ctx, st);
    for (cmd, params) in actions {
        app.run(cmd, params);
    }
}

/// The tools: Select and the three element kinds, then the six relationships, and a line saying
/// what a press on the canvas does now.
fn tools(ui: &mut Ui, t: &Tokens, st: &mut State) {
    section(ui, t, tr("Add"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if tile(ui, t, Glyph::Select.pic(), tr("Select"), st.tool == Tool::Select).clicked() {
            st.tool = Tool::Select;
        }
        for kind in Place::ALL {
            let on = st.tool == Tool::Place(kind);
            if tile(ui, t, kind.glyph().pic(), tr(kind.label()), on).clicked() {
                st.tool = if on { Tool::Select } else { Tool::Place(kind) };
            }
        }
    });
    section(ui, t, tr("Connect"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for k in ALL_LINK_KINDS {
            let on = matches!(st.tool, Tool::Connect { kind, .. } if kind == k);
            if tile(ui, t, Glyph::of_link(k.name()).pic(), link_label(k), on).clicked() {
                st.tool = if on { Tool::Select } else { Tool::Connect { kind: k, from: None } };
            }
        }
    });
    ui.add_space(6.0);
    let hint = match st.tool {
        Tool::Select => tr("Click a card or a row to select it, or drag a card to move it"),
        Tool::Place(_) => tr("Click the canvas to place it"),
        Tool::Connect { from: None, kind: LinkKind::Generalization } => tr("Click the subclass first"),
        Tool::Connect { from: None, kind: LinkKind::Realization } => tr("Click the class that implements it first"),
        Tool::Connect { from: None, kind: LinkKind::Aggregation | LinkKind::Composition } => tr("Click the whole first"),
        Tool::Connect { from: None, .. } => tr("Click the first card"),
        Tool::Connect { from: Some(_), .. } => tr("Click the card to connect it to (Esc stops)"),
    };
    note(ui, t, hint, 11.0);
}

/// The selected card or relationship.
fn selected(ui: &mut Ui, t: &Tokens, m: &Model, st: &mut State, actions: &mut Actions) {
    section(ui, t, tr("Selected"));
    let Some(id) = st.selected else {
        note(ui, t, tr("Nothing selected: click a card on the canvas"), 11.0);
        return;
    };
    egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some(c) = m.classifiers.iter().find(|c| c.id.0 == id) {
            card(ui, t, m, c, st, actions);
        } else if let Some(l) = m.links.iter().find(|l| l.id.0 == id) {
            link_card(ui, t, m, l, st, actions);
        }
    });
}

/// The heading of a selected card or relationship: its picture, what it is and Remove.
fn heading(ui: &mut Ui, t: &Tokens, glyph: Glyph, label: &str, id: u64, st: &mut State, actions: &mut Actions) {
    ui.horizontal(|ui| {
        let (g, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        glyphs::draw(ui.painter(), g, glyph, t.accent_text);
        ui.label(RichText::new(label).font(semibold(12.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, t, Glyph::Trash.pic(), tr("Remove"), true).clicked() {
                actions.push(("diagram.class.remove", json!({"ids": [id]})));
                st.selected = None;
                st.member = None;
            }
        });
    });
}

/// What a screen reader says for a card: "Class Book, 3 attributes, 2 operations".
pub fn card_summary(c: &Classifier) -> String {
    let kind = tr(Place::of(c.kind).label());
    let mut parts = vec![format!("{kind} {}", c.name)];
    if c.kind == Kind::Enumeration {
        parts.push(trf("{n} literals", &[("n", &c.literals.len().to_string())]));
    } else {
        parts.push(trf("{n} attributes", &[("n", &c.attributes.len().to_string())]));
        parts.push(trf("{n} operations", &[("n", &c.operations.len().to_string())]));
    }
    parts.join(", ")
}

fn card(ui: &mut Ui, t: &Tokens, m: &Model, c: &Classifier, st: &mut State, actions: &mut Actions) {
    let id = c.id.0;
    let place = Place::of(c.kind);
    heading(ui, t, place.glyph(), tr(place.label()), id, st, actions);
    ui.label(RichText::new(card_summary(c)).font(regular(10.0)).color(t.text_secondary));
    ui.add_space(6.0);
    if let Some(new) = field(ui, t, tr("Name"), Id::unique(("class_name", id)), &c.name) {
        actions.push(("diagram.class.updateClassifier", json!({"id": id, "name": new.trim()})));
    }
    ui.add_space(4.0);
    if let Some(new) = field(ui, t, tr("Stereotype"), Id::unique(("class_stereotype", id)), c.stereotype.as_deref().unwrap_or("")) {
        let v = new.trim();
        actions.push(("diagram.class.updateClassifier", json!({"id": id, "stereotype": if v.is_empty() { Value::Null } else { json!(v) }})));
    }
    // A class and an interface can become each other; an enumeration stays one.
    if c.kind != Kind::Enumeration {
        ui.add_space(6.0);
        note(ui, t, tr("Kind"), 10.0);
        ui.horizontal(|ui| {
            for p in [Place::Class, Place::Interface] {
                if tile(ui, t, p.glyph().pic(), tr(p.label()), place == p).clicked() && place != p {
                    actions.push(("diagram.class.updateClassifier", json!({"id": id, "kind": p.param()})));
                }
            }
        });
    }
    if c.kind == Kind::Class {
        let mut on = c.is_abstract;
        if ui.checkbox(&mut on, tr("Abstract")).changed() {
            actions.push(("diagram.class.updateClassifier", json!({"id": id, "abstract": on})));
        }
    }
    if c.kind == Kind::Enumeration {
        compartment(ui, t, c, Comp::Literal, st, actions);
    } else {
        compartment(ui, t, c, Comp::Attribute, st, actions);
        compartment(ui, t, c, Comp::Operation, st, actions);
    }
    // Its relationships.
    let mine: Vec<&Link> = m.links.iter().filter(|l| l.touches(c.id)).collect();
    if !mine.is_empty() {
        ui.add_space(8.0);
        note(ui, t, tr("Relationships"), 10.0);
        for l in mine {
            if row(ui, t, Glyph::of_link(l.kind.name()).pic(), &link_row_text(m, l), false, 0.0).clicked() {
                st.selected = Some(l.id.0);
                st.member = None;
            }
        }
    }
}

/// A card's compartment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comp {
    Attribute,
    Operation,
    Literal,
}

impl Comp {
    pub fn param(self) -> &'static str {
        match self {
            Comp::Attribute => "attribute",
            Comp::Operation => "operation",
            Comp::Literal => "literal",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Comp::Attribute => tr("Attributes"),
            Comp::Operation => tr("Operations"),
            Comp::Literal => tr("Literals"),
        }
    }

    fn add_hint(self) -> &'static str {
        match self {
            Comp::Attribute => tr("Add attribute, e.g. - title : String"),
            Comp::Operation => tr("Add operation, e.g. + renew(days: Int) : Bool"),
            Comp::Literal => tr("Add literal"),
        }
    }

    fn glyph(self) -> Glyph {
        if self == Comp::Operation { Glyph::Operation } else { Glyph::Attribute }
    }
}

/// One member as the panel shows it: ID, text, visibility (none for a literal), static.
struct MemberRow {
    id: u64,
    text: String,
    visibility: Option<Visibility>,
}

fn members(c: &Classifier, k: Comp) -> Vec<MemberRow> {
    match k {
        Comp::Attribute => c.attributes.iter().map(|a| MemberRow { id: a.id.0, text: format_attribute(a), visibility: Some(a.visibility) }).collect(),
        Comp::Operation => c.operations.iter().map(|o| MemberRow { id: o.id.0, text: format_operation(o), visibility: Some(o.visibility) }).collect(),
        Comp::Literal => c.literals.iter().map(|l| MemberRow { id: l.id.0, text: format_literal(l), visibility: None }).collect(),
    }
}

/// The visibility choices: (value, its marker, its name for screen readers).
const VISIBILITIES: [(Visibility, &str, &str); 4] = [
    (Visibility::Public, "+", tr_noop("Public")),
    (Visibility::Private, "-", tr_noop("Private")),
    (Visibility::Protected, "#", tr_noop("Protected")),
    (Visibility::Package, "~", tr_noop("Package")),
];

/// A compartment: one row per member (its UML text to edit, visibility, order, Remove), then a
/// field that adds one.
fn compartment(ui: &mut Ui, t: &Tokens, c: &Classifier, k: Comp, st: &mut State, actions: &mut Actions) {
    ui.add_space(8.0);
    ui.label(RichText::new(k.title()).font(semibold(11.0)).color(t.text));
    let rows = members(c, k);
    let n = rows.len();
    for (i, r) in rows.iter().enumerate() {
        let picked = st.member == Some(r.id);
        let frame = if picked { egui::Frame::NONE.fill(t.selected_row).corner_radius(CornerRadius::same(5)) } else { egui::Frame::NONE };
        frame.inner_margin(egui::Margin::symmetric(2, 2)).show(ui, |ui| {
            let w = ui.available_width();
            if let Some(new) = crate::widgets::text_field_named(ui, Id::unique(("class_member", r.id)), &r.text, w, k.title())
                && new.trim() != r.text
            {
                actions.push(("diagram.class.updateMember", json!({"id": r.id, "text": new.trim()})));
                st.member = Some(r.id);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if let Some(vis) = r.visibility {
                    for (v, mark, name) in VISIBILITIES {
                        let on = vis == v;
                        let resp = ui.add_sized([24.0, 22.0], egui::Button::selectable(on, RichText::new(mark).font(regular(12.0))));
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tr(name)));
                        if resp.on_hover_text(tr(name)).clicked() && !on {
                            actions.push(("diagram.class.updateMember", json!({"id": r.id, "visibility": mark})));
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, t, Glyph::Trash.pic(), tr("Remove"), true).clicked() {
                        actions.push(("diagram.class.removeMember", json!({"id": r.id})));
                        st.member = None;
                    }
                    let next = rows.get(i + 1).map(|x| x.id);
                    if ui.add_enabled_ui(next.is_some(), |ui| icon_button(ui, t, Glyph::Down.pic(), tr("Move down"), false)).inner.clicked()
                        && let Some(after) = next
                    {
                        actions.push(("diagram.class.moveMember", json!({"id": r.id, "after": after})));
                    }
                    let prev = i.checked_sub(1).and_then(|j| rows.get(j)).map(|x| x.id);
                    if ui.add_enabled_ui(prev.is_some(), |ui| icon_button(ui, t, Glyph::Up.pic(), tr("Move up"), false)).inner.clicked()
                        && let Some(before) = prev
                    {
                        actions.push(("diagram.class.moveMember", json!({"id": r.id, "before": before})));
                    }
                });
            });
        });
        let _ = n;
    }
    // A new member: typed as UML text, added on Enter.
    let w = ui.available_width();
    ui.horizontal(|ui| {
        let (g, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        glyphs::draw(ui.painter(), g, k.glyph(), t.icon);
        let id = Id::unique(("class_new_member", c.id.0, k.param()));
        if let Some(text) = crate::widgets::text_field_named(ui, id, "", w - 26.0, k.add_hint())
            && !text.trim().is_empty()
        {
            let after = st.member.filter(|m| rows.iter().any(|r| r.id == *m));
            let mut p = json!({"classifier": c.id.0, "compartment": k.param(), "text": text.trim()});
            if let Some(a) = after {
                p["after"] = json!(a);
            }
            actions.push(("diagram.class.addMember", p));
        }
    });
    if rows.is_empty() {
        note(ui, t, k.add_hint(), 10.0);
    }
}

/// A relationship: its ends (each a button that selects that card), the kind among the
/// association kinds, each end's role, multiplicity and navigability, a dependency's label, and
/// Swap ends.
fn link_card(ui: &mut Ui, t: &Tokens, m: &Model, l: &Link, st: &mut State, actions: &mut Actions) {
    let id = l.id.0;
    heading(ui, t, Glyph::of_link(l.kind.name()), link_label(l.kind), id, st, actions);
    let (a, b) = l.pair();
    for (i, e) in [a.0, b.0].into_iter().enumerate() {
        let full = name_of(m, e);
        let text = if i == 0 { full.clone() } else { format!("→ {full}") };
        let button = egui::Button::new(text).wrap_mode(egui::TextWrapMode::Truncate);
        if ui.add_sized([ui.available_width(), 22.0], button).on_hover_text(full).clicked() {
            st.selected = Some(e);
        }
    }
    if l.kind.is_association() {
        ui.add_space(6.0);
        note(ui, t, tr("Kind"), 10.0);
        ui.horizontal_wrapped(|ui| {
            for k in [LinkKind::Association, LinkKind::Aggregation, LinkKind::Composition] {
                if tile(ui, t, Glyph::of_link(k.name()).pic(), link_label(k), l.kind == k).clicked() && l.kind != k {
                    actions.push(("diagram.class.updateLink", json!({"id": id, "kind": k.name()})));
                }
            }
        });
        let ends = [tr("End A"), tr("End B")];
        for (i, end) in l.ends.iter().enumerate() {
            ui.add_space(8.0);
            let whole = i == 0 && matches!(l.kind, LinkKind::Aggregation | LinkKind::Composition);
            let title =
                format!("{} · {}{}", ends.get(i).copied().unwrap_or(""), name_of(m, end.classifier.0), if whole { tr(" (the whole)") } else { "" });
            ui.label(RichText::new(title).font(semibold(11.0)).color(t.text));
            let patch = |key: &str, v: Value| {
                let mut both = [json!({}), json!({})];
                if let Some(slot) = both.get_mut(i) {
                    slot[key] = v;
                }
                json!({"id": id, "ends": both})
            };
            let clear = |s: &str| if s.trim().is_empty() { Value::Null } else { json!(s.trim()) };
            if let Some(new) = field(ui, t, tr("Role"), Id::unique(("class_role", id, i)), end.role.as_deref().unwrap_or("")) {
                actions.push(("diagram.class.updateLink", patch("role", clear(&new))));
            }
            if let Some(new) = field(ui, t, tr("Multiplicity"), Id::unique(("class_mult", id, i)), end.multiplicity.as_deref().unwrap_or("")) {
                actions.push(("diagram.class.updateLink", patch("multiplicity", clear(&new))));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for m in ["1", "0..1", "*", "1..*"] {
                    let on = end.multiplicity.as_deref() == Some(m);
                    if ui.add(egui::Button::selectable(on, RichText::new(m).font(regular(11.0)))).clicked() && !on {
                        actions.push(("diagram.class.updateLink", patch("multiplicity", json!(m))));
                    }
                }
            });
            ui.horizontal(|ui| {
                note(ui, t, tr("Navigable"), 10.0);
                for (n, label, v) in
                    [(Navigable::Unspecified, tr("Unset"), "unspecified"), (Navigable::Yes, tr("Yes"), "yes"), (Navigable::No, tr("No"), "no")]
                {
                    let on = end.navigable == n;
                    if ui.add(egui::Button::selectable(on, RichText::new(label).font(regular(11.0)))).clicked() && !on {
                        actions.push(("diagram.class.updateLink", patch("navigable", json!(v))));
                    }
                }
            });
        }
    }
    if l.kind == LinkKind::Dependency
        && let Some(new) = field(ui, t, tr("Label"), Id::unique(("class_label", id)), l.label.as_deref().unwrap_or(""))
    {
        let v = new.trim();
        actions.push(("diagram.class.updateLink", json!({"id": id, "label": if v.is_empty() { Value::Null } else { json!(v) }})));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if icon_button(ui, t, Glyph::Swap.pic(), tr("Swap ends"), false).clicked() {
            actions.push(("diagram.class.swapEnds", json!({"id": id})));
        }
        let what = match l.kind {
            LinkKind::Aggregation | LinkKind::Composition => tr("Swap ends (the other card becomes the whole)"),
            _ => tr("Swap ends"),
        };
        note(ui, t, what, 10.0);
    });
}

/// What is unfinished: a click selects what it is about.
fn unfinished(ui: &mut Ui, t: &Tokens, m: &Model, st: &mut State) {
    section(ui, t, tr("Unfinished"));
    let warnings = figcraft_doc::class_diagram::check(m);
    if warnings.is_empty() {
        ui.horizontal(|ui| {
            let (g, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            glyphs::draw(ui.painter(), g, Glyph::Check, t.success);
            ui.label(RichText::new(tr("Nothing unfinished")).font(regular(11.0)).color(t.text));
        });
        return;
    }
    for w in &warnings {
        let (about, text) = figcraft_engine::class_warning_text(m, w);
        if row(ui, t, Glyph::Warning.pic(), &text, st.selected == Some(about) || st.member == Some(about), 0.0).clicked() {
            if m.classifiers.iter().any(|c| c.id.0 == about) {
                st.selected = Some(about);
                st.member = None;
            } else if let Some(c) = m.owner_of(figcraft_doc::class_diagram::MemberId(about)) {
                st.selected = Some(c.id.0);
                st.member = Some(about);
            }
        }
    }
}

/// The diagram's classifiers and relationships: a click selects.
fn lists(ui: &mut Ui, t: &Tokens, m: &Model, st: &mut State) {
    if m.classifiers.is_empty() {
        section(ui, t, tr("Diagram"));
        note(ui, t, tr("Pick Class, then click the canvas"), 11.0);
        return;
    }
    section(ui, t, tr("Diagram"));
    for c in &m.classifiers {
        if row(ui, t, Place::of(c.kind).glyph().pic(), &c.name, st.selected == Some(c.id.0), 0.0).clicked() {
            st.selected = Some(c.id.0);
            st.member = None;
        }
    }
    if !m.links.is_empty() {
        section(ui, t, tr("Relationships"));
        for l in &m.links {
            if row(ui, t, Glyph::of_link(l.kind.name()).pic(), &link_row_text(m, l), st.selected == Some(l.id.0), 0.0).clicked() {
                st.selected = Some(l.id.0);
                st.member = None;
            }
        }
    }
}
