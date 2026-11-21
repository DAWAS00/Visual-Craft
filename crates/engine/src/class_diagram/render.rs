//! The layers a Class diagram page shows, generated from its model after every change. Each
//! generated layer is locked and carries `figcraft.class` = `"<semantic id>:<role>"` in its plugin
//! data; anything else on the page is left alone. Positions come from [`super::layout`], which
//! the interface also hit-tests with.

use figcraft_color::{Color, Paint};
use figcraft_doc::class_diagram::{Classifier, LAYER_KEY, Link, LinkKind, Model};
use figcraft_doc::{
    Document, Node, NodeId, NodeKind, StrokeAlign, StrokeCap, StrokeJoin, TextAlignH, TextAlignV, TextAutoResize, TextDecoration, VectorNetwork,
};
use figcraft_geom::{Affine, BezPath, Point, Shape, Vec2};

use super::layout::{CardLayout, LinkPath, Marker, card_layout, link_paths, style, whole_navigability};
use super::measure::{NAME_SIZE, PAD, ROW_SIZE, SMALL_SIZE};
use crate::Result;
use crate::use_case::measure::{text_data, text_size};

const INK: Color = Color::from_rgb8_const(0x1E, 0x1E, 0x1E);
const BODY: Color = Color::from_rgb8_const(0xFF, 0xFF, 0xFF);
const HEADER: Color = Color::from_rgb8_const(0xEE, 0xF0, 0xF3);
/// Longest arrowhead, triangle or diamond.
const MARKER: f64 = 12.0;
/// The widest an end label may be.
const LABEL_WIDTH: f64 = 160.0;

/// Replaces `page`'s generated layers with fresh ones for its model (none when it has no model).
pub(crate) fn regenerate(doc: &mut Document, page: NodeId) -> Result<()> {
    let old: Vec<NodeId> = doc
        .node(page)
        .map(|p| p.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| n.plugin_data.contains_key(LAYER_KEY))).collect())
        .unwrap_or_default();
    for id in old {
        doc.remove(id)?;
    }
    let Some(model) = doc.class_diagrams.get(&page).cloned() else { return Ok(()) };
    let mut r = Draw { doc, page };
    // Relationships first, so cards cover their ends.
    for (l, path) in model.links.iter().zip(link_paths(&model)) {
        if let Some(path) = path {
            r.link(&model, l, &path)?;
        }
    }
    for c in &model.classifiers {
        r.card(c, &card_layout(c))?;
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

impl Draw<'_> {
    fn add(&mut self, parent: NodeId, kind: NodeKind, name: &str, at: (f64, f64), size: (f64, f64), f: impl FnOnce(&mut Node)) -> Result<NodeId> {
        let id = self.doc.alloc_id();
        let mut n = Node::new(id, kind, name, size.0.max(0.01), size.1.max(0.01));
        n.transform = Affine::translate(at);
        f(&mut n);
        self.doc.insert(n, parent, None)?;
        Ok(id)
    }

    /// A one-line text box of a fixed size, cut short with an ellipsis when too long.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        parent: NodeId,
        text: &str,
        size: f64,
        at: (f64, f64),
        w: f64,
        h: f64,
        align: TextAlignH,
        f: impl FnOnce(&mut Node),
    ) -> Result<NodeId> {
        let mut td = text_data(text, size, TextAutoResize::Truncate, align);
        td.align_v = TextAlignV::Middle;
        let name: String = text.chars().take(64).collect();
        self.add(parent, NodeKind::Text(td), if name.is_empty() { "Text" } else { &name }, at, (w.max(1.0), h.max(1.0)), |n| {
            n.fills = vec![Paint::solid(INK)];
            f(n);
        })
    }

    /// A vector layer drawing `path` (in `parent`'s space).
    fn path(&mut self, parent: NodeId, path: BezPath, name: &str, f: impl FnOnce(&mut Node)) -> Result<NodeId> {
        let b = path.bounding_box();
        let local = Affine::translate((-b.x0, -b.y0)) * path;
        self.add(parent, NodeKind::Vector { network: VectorNetwork::from_path(&local) }, name, (b.x0, b.y0), (b.width(), b.height()), f)
    }

    /// A card: a group (not a frame, which the canvas would label) holding the box, the header,
    /// the dividers and one text row per member, in card-local coordinates.
    fn card(&mut self, c: &Classifier, lay: &CardLayout) -> Result<()> {
        let (x0, y0) = (lay.bounds.x0, lay.bounds.y0);
        let (w, h) = (lay.bounds.width(), lay.bounds.height());
        let card = self.add(self.page, NodeKind::Group, &c.name, (x0, y0), (w, h), |n| tag(n, c.id.0, "card"))?;
        self.add(card, NodeKind::Rectangle, "Box", (0.0, 0.0), (w, h), |n| outline(n, Some(BODY), 1.5))?;
        let hh = lay.header.height();
        self.add(card, NodeKind::Rectangle, "Header", (0.0, 0.0), (w, hh), |n| outline(n, Some(HEADER), 1.5))?;
        let mut name_top = 0.0;
        if let Some(k) = &lay.keyword {
            let kh = figcraft_doc::class_diagram::KEYWORD_HEIGHT;
            self.text(card, &format!("«{k}»"), SMALL_SIZE, (PAD, 4.0), w - PAD * 2.0, kh, TextAlignH::Center, |_| {})?;
            name_top = kh;
        }
        let style = if c.is_abstract { "Bold Italic" } else { "Bold" };
        self.text(card, &c.name, NAME_SIZE, (PAD, name_top), w - PAD * 2.0, hh - name_top, TextAlignH::Center, |n| {
            if let NodeKind::Text(td) = &mut n.kind {
                td.style.font_style = style.to_string();
            }
        })?;
        for comp in &lay.compartments {
            let top = comp.rect.y0 - y0;
            self.add(card, NodeKind::Line, "Divider", (0.0, top), (w, 0.0), |n| outline(n, None, 1.0))?;
            for row in &comp.rows {
                let underline = row.underline;
                self.text(card, &row.text, ROW_SIZE, (PAD, row.rect.y0 - y0), w - PAD * 2.0, row.rect.height(), TextAlignH::Left, |n| {
                    if let NodeKind::Text(td) = &mut n.kind
                        && underline
                    {
                        td.style.decoration = TextDecoration::Underline;
                    }
                })?;
            }
        }
        Ok(())
    }

    fn link(&mut self, m: &Model, l: &Link, path: &LinkPath) -> Result<()> {
        let id = l.id.0;
        let (Some((p0, u0)), Some((p1, u1))) = (path.start(), path.end()) else { return Ok(()) };
        let (m0, m1, dashed) = style(l);
        let name_of = |c| m.classifier(c).map_or("?", |c| c.name.as_str());
        let (a, b) = l.pair();
        let name = format!("{} → {}", name_of(a), name_of(b));
        // The shaft stops short of a closed marker so its fill covers no line.
        let inset = |mk: Marker| match mk {
            Marker::HollowTriangle => MARKER,
            Marker::HollowDiamond | Marker::FilledDiamond => MARKER * 1.6,
            _ => 0.0,
        };
        let mut pts = path.points.clone();
        let len = |a: Point, b: Point| (b - a).hypot();
        if let (Some(first), Some(second)) = (pts.first().copied(), pts.get(1).copied()) {
            let d = inset(m0).min(len(first, second) / 3.0);
            if let Some(p) = pts.first_mut() {
                *p = first + u0 * d;
            }
        }
        let n = pts.len();
        if n >= 2
            && let (Some(last), Some(prev)) = (pts.get(n - 1).copied(), pts.get(n - 2).copied())
        {
            let d = inset(m1).min(len(prev, last) / 3.0);
            if let Some(p) = pts.last_mut() {
                *p = last + u1 * d;
            }
        }
        let mut shaft = BezPath::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 { shaft.move_to(*p) } else { shaft.line_to(*p) }
        }
        self.path(self.page, shaft, &name, |n| {
            outline(n, None, 1.5);
            (n.stroke.join, n.stroke.cap_start, n.stroke.cap_end) = (StrokeJoin::Miter, StrokeCap::None, StrokeCap::None);
            if dashed {
                n.stroke.dashes = vec![6.0, 4.0];
            }
            tag(n, id, "link");
        })?;
        self.marker(id, m0, p0, u0)?;
        // The whole end's navigability, just past its diamond.
        self.marker(id, whole_navigability(l), p0 + u0 * inset(m0), u0)?;
        self.marker(id, m1, p1, u1)?;
        self.labels(l, path, (p0, u0), (p1, u1))
    }

    /// A marker with its tip at `tip`, pointing out of the line (the line runs along `u`).
    fn marker(&mut self, id: u64, mk: Marker, tip: Point, u: Vec2) -> Result<()> {
        let side = Vec2::new(-u.y, u.x);
        let mut p = BezPath::new();
        let (fill, closed) = match mk {
            Marker::None => return Ok(()),
            Marker::HollowTriangle => {
                let base = tip + u * MARKER;
                p.move_to(tip);
                p.line_to(base + side * MARKER * 0.6);
                p.line_to(base - side * MARKER * 0.6);
                (Some(Color::WHITE), true)
            }
            Marker::HollowDiamond | Marker::FilledDiamond => {
                let mid = tip + u * MARKER * 0.8;
                p.move_to(tip);
                p.line_to(mid + side * MARKER * 0.45);
                p.line_to(tip + u * MARKER * 1.6);
                p.line_to(mid - side * MARKER * 0.45);
                (Some(if mk == Marker::FilledDiamond { INK } else { Color::WHITE }), true)
            }
            Marker::OpenArrow => {
                let base = tip + u * MARKER;
                p.move_to(base + side * MARKER * 0.5);
                p.line_to(tip);
                p.line_to(base - side * MARKER * 0.5);
                (None, false)
            }
            Marker::Cross => {
                let c = tip + u * MARKER;
                let (a, b) = ((u + side) * MARKER * 0.35, (u - side) * MARKER * 0.35);
                p.move_to(c - a);
                p.line_to(c + a);
                p.move_to(c - b);
                p.line_to(c + b);
                (None, false)
            }
        };
        if closed {
            p.close_path();
        }
        self.path(self.page, p, "Marker", |n| {
            outline(n, fill, 1.5);
            n.stroke.join = StrokeJoin::Miter;
            tag(n, id, "marker");
        })?;
        Ok(())
    }

    /// Roles and multiplicities near each end (association kinds), or a dependency's label at the
    /// middle, each kept clear of the line. An end's labels sit past its markers but never beyond
    /// the middle of the end's segment, so on a short link they stay in the gap between the cards
    /// rather than under the far card, which is drawn over them.
    fn labels(&mut self, l: &Link, path: &LinkPath, start: (Point, Vec2), end: (Point, Vec2)) -> Result<()> {
        let id = l.id.0;
        // `along`: how far from `p` along `u` the label's centre may go at most.
        let put = |r: &mut Self, text: &str, p: Point, u: Vec2, along: f64, side: f64| -> Result<()> {
            let normal = Vec2::new(-u.y, u.x);
            let (w, h) = text_size(text, SMALL_SIZE, LABEL_WIDTH);
            let (w, h) = (w + 4.0, h);
            // Half the label's extent along the line, so it starts past the markers.
            let half_along = u.x.abs() * w / 2.0 + u.y.abs() * h / 2.0;
            let d = (MARKER * 1.6 + 4.0 + half_along).min(along).max(0.0);
            let clear = 6.0 + normal.x.abs() * w / 2.0 + normal.y.abs() * h / 2.0;
            let at = p + u * d + normal * side * clear;
            let page = r.page;
            r.text(page, text, SMALL_SIZE, (at.x - w / 2.0, at.y - h / 2.0), w, h, TextAlignH::Center, |n| tag(n, id, "label"))?;
            Ok(())
        };
        if l.kind.is_association() {
            let n = path.points.len();
            let seg = |i: usize, j: usize| match (path.points.get(i), path.points.get(j)) {
                (Some(a), Some(b)) => (*b - *a).hypot(),
                _ => 0.0,
            };
            let halves = [seg(0, 1) / 2.0, seg(n.saturating_sub(2), n.saturating_sub(1)) / 2.0];
            for ((e, (p, u)), along) in l.ends.iter().zip([start, end]).zip(halves) {
                if let Some(role) = &e.role {
                    put(self, role, p, u, along, 1.0)?;
                }
                if let Some(mult) = &e.multiplicity {
                    put(self, mult, p, u, along, -1.0)?;
                }
            }
        } else if l.kind == LinkKind::Dependency
            && let (Some(label), Some(mid), Some((_, u))) = (&l.label, path.middle(), path.start())
        {
            put(self, &format!("«{label}»"), mid, u, 0.0, 1.0)?;
        }
        Ok(())
    }
}
