//! Where a Class diagram's parts sit, computed from the model alone. The renderer draws from this
//! and the interface hit-tests with it, so a click always lands on the row that is drawn.
//!
//! Rows have a fixed height ([`ROW_HEIGHT`]) and the header a fixed height, so the layout never
//! depends on fonts: text that doesn't fit a row's width is cut short, and a card's measured
//! minimum only ever widens it.

use std::collections::HashMap;

use figcraft_doc::class_diagram::text::{format_attribute, format_literal, format_operation};
use figcraft_doc::class_diagram::{
    COMPARTMENT_PADDING, Classifier, ClassifierId, HEADER_HEIGHT, KEYWORD_HEIGHT, Kind, Link, LinkKind, MemberId, Model, ROW_HEIGHT,
};
use figcraft_geom::{Point, Rect, Vec2};

/// Which compartment of a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Literals,
    Attributes,
    Operations,
}

/// One member's row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub member: MemberId,
    /// In page coordinates.
    pub rect: Rect,
    /// What the row shows: the member's canonical text.
    pub text: String,
    /// A static member is underlined.
    pub underline: bool,
}

/// One compartment: its box and its rows (none when it is empty).
#[derive(Clone, Debug, PartialEq)]
pub struct Compartment {
    pub section: Section,
    pub rect: Rect,
    pub rows: Vec<Row>,
}

/// A card's parts.
#[derive(Clone, Debug, PartialEq)]
pub struct CardLayout {
    pub id: ClassifierId,
    pub bounds: Rect,
    /// The header: the keyword line (if any) above the name.
    pub header: Rect,
    /// The keyword line's text, without guillemets («» are added when drawn).
    pub keyword: Option<String>,
    pub compartments: Vec<Compartment>,
}

impl CardLayout {
    /// The row under `p` (page coordinates), if any.
    pub fn row_at(&self, p: Point) -> Option<&Row> {
        self.compartments.iter().flat_map(|c| c.rows.iter()).find(|r| r.rect.contains(p))
    }

    /// The compartment under `p`, if any.
    pub fn section_at(&self, p: Point) -> Option<Section> {
        self.compartments.iter().find(|c| c.rect.contains(p)).map(|c| c.section)
    }
}

/// The keyword line's text: the kind's keyword, the stereotype and, with `with_abstract`,
/// `abstract`, joined with commas; `None` when there are none.
pub(crate) fn keyword_text(c: &Classifier, with_abstract: bool) -> Option<String> {
    let parts: Vec<&str> = c.kind.keyword().into_iter().chain(c.stereotype.as_deref()).chain(with_abstract.then_some("abstract")).collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Whether an abstract classifier's keyword line says «abstract» as well as the name being drawn
/// in italic (which a substituted font may not show). With another keyword it joins that line;
/// alone it takes a line of its own, which the measured minimum height makes room for. A card
/// whose stored minimum has no room for it (written by hand) keeps the italic only, so the
/// compartments never run past the card.
pub fn abstract_line(c: &Classifier) -> bool {
    c.is_abstract && (keyword_text(c, false).is_some() || c.min_h >= c.floor().1 + KEYWORD_HEIGHT)
}

/// The keyword line a classifier shows: its kind's keyword, its stereotype and «abstract» (see
/// [`abstract_line`]).
pub fn keyword(c: &Classifier) -> Option<String> {
    keyword_text(c, abstract_line(c))
}

/// One row before layout: the member, its text and whether it is underlined (static).
pub type RowText = (MemberId, String, bool);

/// The compartments a card shows, in order, with their rows' texts (matching
/// [`Classifier::floor`]).
pub fn sections(c: &Classifier) -> Vec<(Section, Vec<RowText>)> {
    let attrs = || c.attributes.iter().map(|a| (a.id, format_attribute(a), a.is_static)).collect();
    let ops = || c.operations.iter().map(|o| (o.id, format_operation(o), o.is_static)).collect();
    let mut out = Vec::new();
    if c.kind == Kind::Enumeration {
        out.push((Section::Literals, c.literals.iter().map(|l| (l.id, format_literal(l), false)).collect()));
    }
    if c.show_attributes && (c.kind != Kind::Enumeration || !c.attributes.is_empty()) {
        out.push((Section::Attributes, attrs()));
    }
    if c.show_operations && (c.kind != Kind::Enumeration || !c.operations.is_empty()) {
        out.push((Section::Operations, ops()));
    }
    out
}

/// The parts of card `c`. Any height beyond the floor goes to the last compartment.
pub fn card_layout(c: &Classifier) -> CardLayout {
    let bounds = Rect::new(c.x, c.y, c.x + c.w, c.y + c.h);
    let keyword = keyword(c);
    let header_h = HEADER_HEIGHT + if keyword.is_some() { KEYWORD_HEIGHT } else { 0.0 };
    let header = Rect::new(c.x, c.y, c.x + c.w, (c.y + header_h).min(bounds.y1));
    let parts = sections(c);
    let count = parts.len();
    let mut y = header.y1;
    let mut compartments = Vec::with_capacity(count);
    for (i, (section, rows)) in parts.into_iter().enumerate() {
        let natural = COMPARTMENT_PADDING * 2.0 + rows.len().max(1) as f64 * ROW_HEIGHT;
        let h = if i + 1 == count { (bounds.y1 - y).max(natural) } else { natural };
        let top = y + COMPARTMENT_PADDING;
        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(k, (member, text, underline))| {
                let ry = top + k as f64 * ROW_HEIGHT;
                Row { member, rect: Rect::new(c.x, ry, c.x + c.w, ry + ROW_HEIGHT), text, underline }
            })
            .collect();
        compartments.push(Compartment { section, rect: Rect::new(c.x, y, c.x + c.w, y + h), rows });
        y += h;
    }
    CardLayout { id: c.id, bounds, header, keyword, compartments }
}

/// How a relationship is drawn: the polyline from `ends[0]`'s side to `ends[1]`'s side, already
/// clipped to the two cards.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkPath {
    pub points: Vec<Point>,
}

impl LinkPath {
    /// The first segment's start and the unit direction pointing into the line from there.
    pub fn start(&self) -> Option<(Point, Vec2)> {
        let a = *self.points.first()?;
        let b = *self.points.get(1)?;
        unit(b - a).map(|u| (a, u))
    }

    /// The last point and the unit direction pointing into the line from there.
    pub fn end(&self) -> Option<(Point, Vec2)> {
        let n = self.points.len();
        let a = *self.points.get(n.checked_sub(1)?)?;
        let b = *self.points.get(n.checked_sub(2)?)?;
        unit(b - a).map(|u| (a, u))
    }

    /// The middle of the path, by length.
    pub fn middle(&self) -> Option<Point> {
        let total: f64 = self.points.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
        let mut left = total / 2.0;
        for w in self.points.windows(2) {
            let d = (w[1] - w[0]).hypot();
            if d >= left && d > 0.0 {
                return Some(w[0] + (w[1] - w[0]) * (left / d));
            }
            left -= d;
        }
        self.points.first().copied()
    }

    /// The distance from `p` to the path.
    pub fn distance(&self, p: Point) -> f64 {
        self.points.windows(2).map(|w| segment_distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
    }
}

fn unit(v: Vec2) -> Option<Vec2> {
    let len = v.hypot();
    (len > f64::EPSILON && len.is_finite()).then(|| v / len)
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let d = b - a;
    let len2 = d.hypot2();
    let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + d * t)).hypot()
}

/// Relationships between the same two cards sit this far apart.
pub const PARALLEL: f64 = 14.0;
/// A self-association's loop reaches this far off the card, more for each further loop.
pub const LOOP: f64 = 32.0;

/// The stretch (`t0`, `t1`) of the line `o + u·t` that lies inside `r`, if it crosses it.
fn span(r: Rect, o: Point, u: Vec2) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for (o, u, lo, hi) in [(o.x, u.x, r.x0, r.x1), (o.y, u.y, r.y0, r.y1)] {
        if u.abs() <= f64::EPSILON {
            if o < lo || o > hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - o) / u, (hi - o) / u);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    (t0.is_finite() && t1.is_finite() && t0 <= t1).then_some((t0, t1))
}

/// How far a line along `u` may sit from `r`'s centre and still cross it with some length: the
/// box's half extent across `u`, less a margin so an offset line never just grazes a corner.
fn reach_across(r: Rect, u: Vec2) -> f64 {
    let half = r.width() / 2.0 * u.y.abs() + r.height() / 2.0 * u.x.abs();
    (half * 0.8).max(0.0)
}

/// Every relationship's path, by link index. A link whose cards are missing or overlap gets none.
pub fn link_paths(m: &Model) -> Vec<Option<LinkPath>> {
    let boxes: HashMap<ClassifierId, Rect> = m.classifiers.iter().map(|c| (c.id, Rect::new(c.x, c.y, c.x + c.w, c.y + c.h))).collect();
    let mut pairs: HashMap<(ClassifierId, ClassifierId), Vec<usize>> = HashMap::new();
    for (i, l) in m.links.iter().enumerate() {
        let (a, b) = l.pair();
        pairs.entry((a.min(b), a.max(b))).or_default().push(i);
    }
    m.links
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let (a, b) = l.pair();
            let group = pairs.get(&(a.min(b), a.max(b)));
            let n = group.map_or(1, Vec::len);
            let k = group.and_then(|g| g.iter().position(|j| *j == i)).unwrap_or(0);
            let (ra, rb) = (boxes.get(&a)?, boxes.get(&b)?);
            if a == b { Some(self_loop(*ra, k)) } else { straight(*ra, *rb, k, n, a < b) }
        })
        .collect()
}

/// The path of the link at `index`, if it is drawn.
pub fn link_path(m: &Model, index: usize) -> Option<LinkPath> {
    link_paths(m).into_iter().nth(index).flatten()
}

/// The `k`th of `n` straight links between boxes `a` and `b`: parallel lines [`PARALLEL`] apart,
/// packed closer when that would take the outer ones off a box, each running from the edge of `a`
/// to the edge of `b`.
fn straight(a: Rect, b: Rect, k: usize, n: usize, forward: bool) -> Option<LinkPath> {
    let d = b.center() - a.center();
    let u = unit(d)?;
    let room = reach_across(a, u).min(reach_across(b, u));
    let spacing = if n > 1 { PARALLEL.min(2.0 * room / (n as f64 - 1.0)) } else { 0.0 };
    let offset = (k as f64 - (n as f64 - 1.0) / 2.0) * spacing;
    // The same perpendicular for both directions of a pair, so offsets don't cross.
    let flip = if forward { 1.0 } else { -1.0 };
    let o = a.center() + Vec2::new(-u.y, u.x) * offset * flip;
    // Out of `a` where the line leaves it, into `b` where it enters.
    let (_, out_of_a) = span(a, o, u)?;
    let (into_b, _) = span(b, o, u)?;
    let (p0, p1) = (o + u * out_of_a, o + u * into_b);
    if into_b <= out_of_a {
        return None;
    }
    unit(p1 - p0)?;
    Some(LinkPath { points: vec![p0, p1] })
}

/// A three-segment loop off the card's top-right corner: up from the top edge, across the corner
/// and back into the right edge. The `k`th loop on a card sits further out.
fn self_loop(r: Rect, k: usize) -> LinkPath {
    let reach = LOOP + k as f64 * PARALLEL;
    // The same inset on both edges keeps the corner `reach - inset` clear of the middle segment.
    let inset = (reach * 0.75).min(r.width() / 2.0).min(r.height() / 2.0);
    let start = Point::new(r.x1 - inset, r.y0);
    LinkPath { points: vec![start, Point::new(start.x, r.y0 - reach), Point::new(r.x1 + reach, r.y0 + inset), Point::new(r.x1, r.y0 + inset)] }
}

/// What is drawn at each end and along a relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    None,
    HollowDiamond,
    FilledDiamond,
    HollowTriangle,
    OpenArrow,
    /// A navigability "not navigable" cross.
    Cross,
}

fn nav(n: figcraft_doc::class_diagram::Navigable) -> Marker {
    use figcraft_doc::class_diagram::Navigable;
    match n {
        Navigable::Yes => Marker::OpenArrow,
        Navigable::No => Marker::Cross,
        Navigable::Unspecified => Marker::None,
    }
}

/// The navigability marker of an aggregation's or composition's whole end (`ends[0]`), drawn just
/// past its diamond; [`Marker::None`] for every other kind (their `ends[0]` marker is in
/// [`style`]).
pub fn whole_navigability(l: &Link) -> Marker {
    match l.kind {
        LinkKind::Aggregation | LinkKind::Composition => nav(l.ends[0].navigable),
        _ => Marker::None,
    }
}

/// The marker at each end (`ends[0]`, `ends[1]`) and whether the line is dashed.
pub fn style(l: &Link) -> (Marker, Marker, bool) {
    match l.kind {
        LinkKind::Association => (nav(l.ends[0].navigable), nav(l.ends[1].navigable), false),
        LinkKind::Aggregation => (Marker::HollowDiamond, nav(l.ends[1].navigable), false),
        LinkKind::Composition => (Marker::FilledDiamond, nav(l.ends[1].navigable), false),
        LinkKind::Generalization => (Marker::None, Marker::HollowTriangle, false),
        LinkKind::Realization => (Marker::None, Marker::HollowTriangle, true),
        LinkKind::Dependency => (Marker::None, Marker::OpenArrow, true),
    }
}
