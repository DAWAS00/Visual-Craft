//! The layers a Use Case page shows, generated from its model after every change. Each generated
//! layer is locked and carries `figcraft.useCase` = `"<semantic id>:<role>"` in its plugin data;
//! anything else on the page is left alone.

use std::collections::HashMap;

use figcraft_color::{Color, Paint};
use figcraft_doc::use_case::{Actor, GenEnds, LAYER_KEY, Link, Model, Presentation, Subject, UseCase};
use figcraft_doc::{Document, Node, NodeId, NodeKind, StrokeAlign, StrokeCap, StrokeJoin, TextAlignH, TextAlignV, TextAutoResize, VectorNetwork};
use figcraft_geom::{Affine, BezPath, Point, Shape, Vec2};

use super::measure::{ACTOR_TEXT_WIDTH, CASE_TEXT_WIDTH, FIGURE_H, NAME_SIZE, POINT_SIZE, POINTS_HEADING, SMALL_SIZE, text_data, text_size};
use crate::Result;

const INK: Color = Color::from_rgb8_const(0x1E, 0x1E, 0x1E);
const SUBJECT_FILL: Color = Color::from_rgb8_const(0xFA, 0xFA, 0xF7);
const CASE_FILL: Color = Color::from_rgb8_const(0xFF, 0xF4, 0xD6);
const ACTOR_FILL: Color = Color::from_rgb8_const(0xE3, 0xEE, 0xFF);
/// Links between the same two elements sit this far apart.
const PARALLEL: f64 = 12.0;
/// Longest arrowhead or triangle.
const MARKER: f64 = 12.0;

/// Replaces `page`'s generated layers with fresh ones for its model (none when it has no model).
pub(crate) fn regenerate(doc: &mut Document, page: NodeId) -> Result<()> {
    let old: Vec<NodeId> = doc
        .node(page)
        .map(|p| p.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| n.plugin_data.contains_key(LAYER_KEY))).collect())
        .unwrap_or_default();
    for id in old {
        doc.remove(id)?;
    }
    let Some(model) = doc.use_cases.get(&page).cloned() else { return Ok(()) };
    let mut r = Draw { doc, page };
    for s in &model.subjects {
        r.subject(s)?;
    }
    r.links(&model)?;
    for u in &model.use_cases {
        r.use_case(u)?;
    }
    for a in &model.actors {
        r.actor(a)?;
    }
    Ok(())
}

struct Draw<'a> {
    doc: &'a mut Document,
    page: NodeId,
}

fn tag(n: &mut Node, id: u64, role: &str) {
    n.locked = true;
    n.plugin_data.insert(LAYER_KEY.to_string(), format!("{id}:{role}"));
}

fn outline(n: &mut Node, fill: Option<Color>, weight: f64) {
    n.fills = fill.map(Paint::solid).into_iter().collect();
    n.strokes = vec![Paint::solid(INK)];
    n.stroke.weight = weight;
    n.stroke.align = StrokeAlign::Center;
}

fn ellipse_kind() -> NodeKind {
    NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 }
}

impl Draw<'_> {
    fn add(&mut self, parent: NodeId, kind: NodeKind, name: &str, at: (f64, f64), size: (f64, f64), f: impl FnOnce(&mut Node)) -> Result<NodeId> {
        let id = self.doc.alloc_id();
        let mut n = Node::new(id, kind, name, size.0.max(0.01), size.1.max(0.01));
        n.transform = Affine::translate(at);
        f(&mut n);
        self.doc.insert(n, parent, None)?;
        Ok(id)
    }

    /// A text box of a fixed size: centred both ways, cut short with an ellipsis when too long.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, parent: NodeId, text: &str, size: f64, at: (f64, f64), w: f64, h: f64, align: TextAlignH) -> Result<NodeId> {
        let mut td = text_data(text, size, TextAutoResize::Truncate, align);
        td.align_v = TextAlignV::Middle;
        let name: String = text.chars().take(64).collect();
        self.add(parent, NodeKind::Text(td), if name.is_empty() { "Text" } else { &name }, at, (w, h), |n| n.fills = vec![Paint::solid(INK)])
    }

    /// A group holding one element's layers, at its box. A group, not a frame: the canvas labels
    /// top-level frames with their names, and the element already shows its name.
    fn card(&mut self, id: u64, name: &str, (x, y, w, h): (f64, f64, f64, f64), role: &str) -> Result<NodeId> {
        self.add(self.page, NodeKind::Group, name, (x, y), (w, h), |n| tag(n, id, role))
    }

    fn subject(&mut self, s: &Subject) -> Result<()> {
        let card = self.card(s.id.0, &s.name, (s.x, s.y, s.width, s.height), "boundary")?;
        self.add(card, NodeKind::Rectangle, "Boundary", (0.0, 0.0), (s.width, s.height), |n| outline(n, Some(SUBJECT_FILL), 1.5))?;
        let (_, h) = text_size(&s.name, NAME_SIZE, (s.width - 24.0).max(1.0));
        self.text(card, &s.name, NAME_SIZE, (12.0, 8.0), (s.width - 24.0).max(1.0), h.min(s.height - 16.0).max(1.0), TextAlignH::Left)?;
        Ok(())
    }

    fn use_case(&mut self, u: &UseCase) -> Result<()> {
        let card = self.card(u.id.0, &u.name, (u.x, u.y, u.w, u.h), "card")?;
        self.add(card, ellipse_kind(), "Use case", (0.0, 0.0), (u.w, u.h), |n| outline(n, Some(CASE_FILL), 1.5))?;
        // The text sits in the ellipse's inner rectangle (its sides at 1/√2 of the radii).
        let (iw, ih) = (u.w / std::f64::consts::SQRT_2, u.h / std::f64::consts::SQRT_2);
        let (ix, iy) = ((u.w - iw) / 2.0, (u.h - ih) / 2.0);
        if u.points.is_empty() {
            self.text(card, &u.name, NAME_SIZE, (ix, iy), iw, ih, TextAlignH::Center)?;
            return Ok(());
        }
        let (_, name_h) = text_size(&u.name, NAME_SIZE, CASE_TEXT_WIDTH.min(iw));
        let name_h = name_h.min(ih / 2.0);
        self.text(card, &u.name, NAME_SIZE, (ix, iy), iw, name_h, TextAlignH::Center)?;
        let y = iy + name_h + 4.0;
        self.add(card, NodeKind::Line, "Divider", (ix, y), (iw, 0.0), |n| outline(n, None, 1.0))?;
        let mut lines = vec![POINTS_HEADING.to_string()];
        lines.extend(u.points.iter().map(|p| p.name.clone()));
        let rest = (iy + ih - y - 4.0).max(1.0);
        let row = (rest / lines.len() as f64).max(1.0);
        for (i, line) in lines.iter().enumerate() {
            let size = if i == 0 { SMALL_SIZE } else { POINT_SIZE };
            self.text(card, line, size, (ix, y + 4.0 + row * i as f64), iw, row, TextAlignH::Center)?;
        }
        Ok(())
    }

    fn actor(&mut self, a: &Actor) -> Result<()> {
        let card = self.card(a.id.0, &a.name, (a.x, a.y, a.w, a.h), "card")?;
        match a.presentation {
            Presentation::Person => {
                let cx = a.w / 2.0;
                self.add(card, ellipse_kind(), "Head", (cx - 9.0, 2.0), (18.0, 18.0), |n| outline(n, Some(ACTOR_FILL), 2.0))?;
                let mut body = BezPath::new();
                for (p, q) in [((0.0, 20.0), (0.0, 50.0)), ((-18.0, 30.0), (18.0, 30.0)), ((0.0, 50.0), (-15.0, 76.0)), ((0.0, 50.0), (15.0, 76.0))] {
                    body.move_to((cx + p.0, p.1));
                    body.line_to((cx + q.0, q.1));
                }
                self.path(card, body, "Body", |n| {
                    outline(n, None, 2.0);
                    (n.stroke.cap_start, n.stroke.cap_end, n.stroke.join) = (StrokeCap::Round, StrokeCap::Round, StrokeJoin::Round);
                })?;
                let top = FIGURE_H + 2.0;
                self.text(card, &a.name, NAME_SIZE, (0.0, top), a.w, (a.h - top).max(1.0), TextAlignH::Center)?;
            }
            Presentation::System => {
                self.add(card, NodeKind::Rectangle, "Box", (0.0, 0.0), (a.w, a.h), |n| outline(n, Some(ACTOR_FILL), 1.5))?;
                let (_, kh) = text_size("«actor»", SMALL_SIZE, ACTOR_TEXT_WIDTH);
                self.text(card, "«actor»", SMALL_SIZE, (8.0, 6.0), (a.w - 16.0).max(1.0), kh, TextAlignH::Center)?;
                self.text(card, &a.name, NAME_SIZE, (8.0, 6.0 + kh), (a.w - 16.0).max(1.0), (a.h - kh - 12.0).max(1.0), TextAlignH::Center)?;
            }
        }
        Ok(())
    }

    /// A vector layer drawing `path` (in `parent`'s space).
    fn path(&mut self, parent: NodeId, path: BezPath, name: &str, f: impl FnOnce(&mut Node)) -> Result<NodeId> {
        let b = path.bounding_box();
        let local = Affine::translate((-b.x0, -b.y0)) * path;
        self.add(parent, NodeKind::Vector { network: VectorNetwork::from_path(&local) }, name, (b.x0, b.y0), (b.width(), b.height()), f)
    }

    fn links(&mut self, m: &Model) -> Result<()> {
        let boxes = Boxes::new(m);
        // Links between the same two elements, so each gets its own offset.
        let mut pairs: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
        for (i, l) in m.links.iter().enumerate() {
            let (a, b) = ends(l);
            pairs.entry((a.min(b), a.max(b))).or_default().push(i);
        }
        for (i, l) in m.links.iter().enumerate() {
            let (a, b) = ends(l);
            let group = pairs.get(&(a.min(b), a.max(b)));
            let n = group.map_or(1, Vec::len);
            let k = group.and_then(|g| g.iter().position(|j| *j == i)).unwrap_or(0);
            let offset = (k as f64 - (n as f64 - 1.0) / 2.0) * PARALLEL;
            if let (Some(sa), Some(sb)) = (boxes.get(a), boxes.get(b)) {
                let name = format!("{} → {}", name_of(m, a), name_of(m, b));
                self.link(m, l, (sa, sb), offset, &name)?;
            }
        }
        Ok(())
    }

    fn link(&mut self, m: &Model, l: &Link, (a, b): (Outline, Outline), offset: f64, name: &str) -> Result<()> {
        let id = l.id().0;
        let d = b.center - a.center;
        let len = d.hypot();
        if len <= f64::EPSILON {
            return Ok(());
        }
        // The same perpendicular for both directions of a pair, so offsets don't cross.
        let flip = if (a.center.x, a.center.y) <= (b.center.x, b.center.y) { 1.0 } else { -1.0 };
        let perp = Vec2::new(-d.y, d.x) / len * offset * flip;
        let (ca, cb) = (a.center + perp, b.center + perp);
        let (p0, p1) = (a.clip(ca, cb), b.clip(cb, ca));
        let span = p1 - p0;
        let span_len = span.hypot();
        if span_len <= f64::EPSILON {
            return Ok(());
        }
        let u = span / span_len;
        let marker = MARKER.min(span_len / 3.0);
        let (dashed, arrow, triangle) = match l {
            Link::Association { .. } => (false, false, false),
            Link::Include { .. } | Link::Extend { .. } => (true, true, false),
            Link::Generalization { .. } => (false, false, true),
        };
        let shaft_end = if triangle { p1 - u * marker } else { p1 };
        let mut shaft = BezPath::new();
        shaft.move_to(p0);
        shaft.line_to(shaft_end);
        self.path(self.page, shaft, name, |n| {
            outline(n, None, 1.5);
            if dashed {
                n.stroke.dashes = vec![6.0, 4.0];
            }
            if arrow {
                n.stroke.cap_end = StrokeCap::LineArrow;
            }
            tag(n, id, "link");
        })?;
        if triangle {
            let base = p1 - u * marker;
            let side = Vec2::new(-u.y, u.x) * marker * 0.6;
            let mut tri = BezPath::new();
            tri.move_to(p1);
            tri.line_to(base + side);
            tri.line_to(base - side);
            tri.close_path();
            self.path(self.page, tri, "Arrowhead", |n| {
                outline(n, Some(Color::WHITE), 1.5);
                n.stroke.join = StrokeJoin::Miter;
                tag(n, id, "marker");
            })?;
        }
        self.labels(m, l, p0, p1, u)
    }

    fn labels(&mut self, m: &Model, l: &Link, p0: Point, p1: Point, u: Vec2) -> Result<()> {
        let id = l.id().0;
        let normal = Vec2::new(-u.y, u.x);
        let mid = p0.midpoint(p1);
        // A label beside the line at `at`, on `side` (1 or -1) of it: its centre moves out by half
        // its extent across the line, so a wide label beside a steep line doesn't cover it.
        let put = |r: &mut Self, text: &str, size: f64, at: Point, side: f64| -> Result<()> {
            let (w, h) = text_size(text, size, CASE_TEXT_WIDTH);
            let clear = 6.0 + normal.x.abs() * (w / 2.0 + 2.0) + normal.y.abs() * h / 2.0;
            let at = at + normal * side * clear;
            let t = r.text(r.page, text, size, (at.x - w / 2.0 - 2.0, at.y - h / 2.0), w + 4.0, h, TextAlignH::Center)?;
            if let Some(n) = r.doc.node_mut(t) {
                tag(n, id, "label");
            }
            Ok(())
        };
        match l {
            Link::Include { .. } => put(self, "«include»", SMALL_SIZE, mid, 1.0),
            Link::Extend { points, condition, base, .. } => {
                put(self, "«extend»", SMALL_SIZE, mid, 1.0)?;
                let names: Vec<String> = m
                    .use_cases
                    .iter()
                    .filter(|c| c.id == *base)
                    .flat_map(|c| c.points.iter().filter(|p| points.contains(&p.id)).map(|p| p.name.clone()))
                    .collect();
                let mut detail = String::new();
                if let Some(c) = condition.as_deref().filter(|c| !c.is_empty()) {
                    detail.push_str(&format!("[{c}] "));
                }
                if !names.is_empty() {
                    detail.push_str(&format!("({})", names.join(", ")));
                }
                if detail.is_empty() { Ok(()) } else { put(self, detail.trim(), SMALL_SIZE, mid, -1.0) }
            }
            Link::Association { actor_multiplicity, use_case_multiplicity, .. } => {
                // `p0` is the actor's end (the association is drawn from the actor).
                if let Some(m) = actor_multiplicity {
                    put(self, m, SMALL_SIZE, p0 + (p1 - p0) * 0.15, 1.0)?;
                }
                if let Some(m) = use_case_multiplicity {
                    put(self, m, SMALL_SIZE, p1 - (p1 - p0) * 0.15, 1.0)?;
                }
                Ok(())
            }
            Link::Generalization { .. } => Ok(()),
        }
    }
}

/// An actor's or use case's name, for a link layer's name.
fn name_of(m: &Model, id: u64) -> &str {
    m.actors
        .iter()
        .find(|a| a.id.0 == id)
        .map(|a| a.name.as_str())
        .or_else(|| m.use_cases.iter().find(|u| u.id.0 == id).map(|u| u.name.as_str()))
        .unwrap_or("?")
}

/// A link's two ends, in drawing order (from → to; actor → use case for an association).
fn ends(l: &Link) -> (u64, u64) {
    match l {
        Link::Association { actor, use_case, .. } => (actor.0, use_case.0),
        Link::Include { including, included, .. } => (including.0, included.0),
        Link::Extend { extending, base, .. } => (extending.0, base.0),
        Link::Generalization { ends: GenEnds::Actors { specific, general }, .. } => (specific.0, general.0),
        Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => (specific.0, general.0),
    }
}

/// An element's outline, for clipping links to it.
#[derive(Clone, Copy)]
struct Outline {
    center: Point,
    rx: f64,
    ry: f64,
    ellipse: bool,
}

impl Outline {
    /// Where the line from `from` (inside) towards `to` leaves the outline, measured around a
    /// centre moved to `from` so offset links stay parallel.
    fn clip(self, from: Point, to: Point) -> Point {
        let d = to - from;
        let t = if self.ellipse {
            let k = ((d.x / self.rx).powi(2) + (d.y / self.ry).powi(2)).sqrt();
            if k > 0.0 { 1.0 / k } else { 0.0 }
        } else {
            let tx = if d.x.abs() > 0.0 { self.rx / d.x.abs() } else { f64::INFINITY };
            let ty = if d.y.abs() > 0.0 { self.ry / d.y.abs() } else { f64::INFINITY };
            tx.min(ty)
        };
        if t.is_finite() { from + d * t.min(1.0) } else { from }
    }
}

struct Boxes(HashMap<u64, Outline>);

impl Boxes {
    fn new(m: &Model) -> Self {
        let mut map = HashMap::new();
        for a in &m.actors {
            map.insert(a.id.0, Outline { center: Point::new(a.x + a.w / 2.0, a.y + a.h / 2.0), rx: a.w / 2.0, ry: a.h / 2.0, ellipse: false });
        }
        for u in &m.use_cases {
            map.insert(u.id.0, Outline { center: Point::new(u.x + u.w / 2.0, u.y + u.h / 2.0), rx: u.w / 2.0, ry: u.h / 2.0, ellipse: true });
        }
        Self(map)
    }
    fn get(&self, id: u64) -> Option<Outline> {
        self.0.get(&id).copied()
    }
}
