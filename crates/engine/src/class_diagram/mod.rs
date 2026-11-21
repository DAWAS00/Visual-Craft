//! UML Class diagram commands (`diagram.class.*`; `docs/diagram-type-specifications.md` § 2).
//!
//! A Class diagram page keeps its model in [`Document::class_diagrams`]; every command builds a
//! whole new model, validates it ([`figcraft_doc::class_diagram::validate`]) and only then stores
//! it and regenerates the page's layers ([`render`]). A refused command changes nothing, and a
//! command that changes nothing adds no undo step.

mod elements;
mod example;
pub mod layout;
mod lifecycle;
mod links;
mod measure;
mod members;
mod params;
pub(crate) mod render;
mod specs;

pub(crate) use example::start;
pub(crate) use lifecycle::duplicate_page;
pub use links::{check_connect, warning_text};
pub use specs::specs;

use std::sync::Arc;

use figcraft_doc::NodeId;
use figcraft_doc::class_diagram::{self as cd, Classifier, ClassifierId, Kind, Model};
use serde_json::Value;

use crate::diagram_params::{id, req_text};
use crate::error::bad;
use crate::{Result, Session};

/// The `type` of a Class diagram in `file.newDiagram`.
pub(crate) const TYPE_ID: &str = "class";
/// Its example template.
pub(crate) const EXAMPLE: &str = "libraryLoans";
/// Placement tries spots this far apart.
const STEP: f64 = 40.0;

// ---- the page and its model ----

/// The Class diagram page a command acts on (`page`, or the page in view) and its model.
pub(super) fn target(s: &Session, cmd: &str, p: &Value) -> Result<(NodeId, Arc<Model>)> {
    let page = match id(cmd, p, "page")? {
        Some(n) => NodeId(n),
        None => s.page()?,
    };
    let model = s.doc()?.class_diagrams.get(&page).cloned().ok_or_else(|| bad(cmd, format!("page {page} isn't a Class diagram")))?;
    Ok((page, model))
}

/// Stores `after` and redraws the page, unless it equals `before` (`false`: nothing changed).
pub(super) fn commit(s: &mut Session, cmd: &str, page: NodeId, before: &Model, after: Model) -> Result<bool> {
    if *before == after {
        return Ok(false);
    }
    cd::validate(&after).map_err(|e| bad(cmd, e.to_string()))?;
    let doc = s.doc_mut()?;
    doc.class_diagrams.insert(page, Arc::new(after));
    render::regenerate(doc, page)?;
    Ok(true)
}

pub(super) fn fresh(cmd: &str, m: &Model) -> Result<u64> {
    m.next_id().ok_or_else(|| bad(cmd, "the diagram has run out of IDs"))
}

pub(super) fn classifier_index(cmd: &str, m: &Model, id: u64) -> Result<usize> {
    m.classifiers
        .iter()
        .position(|c| c.id.0 == id)
        .ok_or_else(|| bad(cmd, format!("there is no class, interface or enumeration {id} on this diagram")))
}

pub(super) fn link_index(cmd: &str, m: &Model, id: u64) -> Result<usize> {
    m.links.iter().position(|l| l.id.0 == id).ok_or_else(|| bad(cmd, format!("there is no relationship {id} on this diagram")))
}

/// Which list a member is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Compartment {
    Attribute,
    Operation,
    Literal,
}

pub(super) fn compartment(cmd: &str, p: &Value) -> Result<Compartment> {
    match req_text(cmd, p, "compartment")?.as_str() {
        "attribute" | "attributes" => Ok(Compartment::Attribute),
        "operation" | "operations" => Ok(Compartment::Operation),
        "literal" | "literals" => Ok(Compartment::Literal),
        other => Err(bad(cmd, format!("`compartment` is attribute, operation or literal, not `{other}`"))),
    }
}

/// The classifier holding member `id`, its compartment and the member's index there.
pub(super) fn member_at(cmd: &str, m: &Model, id: u64) -> Result<(usize, Compartment, usize)> {
    for (ci, c) in m.classifiers.iter().enumerate() {
        if let Some(i) = c.attributes.iter().position(|a| a.id.0 == id) {
            return Ok((ci, Compartment::Attribute, i));
        }
        if let Some(i) = c.operations.iter().position(|o| o.id.0 == id) {
            return Ok((ci, Compartment::Operation, i));
        }
        if let Some(i) = c.literals.iter().position(|l| l.id.0 == id) {
            return Ok((ci, Compartment::Literal, i));
        }
    }
    Err(bad(cmd, format!("there is no member {id} on this diagram")))
}

pub(super) fn len_of(c: &Classifier, k: Compartment) -> usize {
    match k {
        Compartment::Attribute => c.attributes.len(),
        Compartment::Operation => c.operations.len(),
        Compartment::Literal => c.literals.len(),
    }
}

// ---- placement ----

/// Placement grid size: at most this many columns and rows of spots are tried.
pub(super) const GRID: usize = 30;

/// Whether a `size` card at (`x`, `y`) stays within the page's limits and [`cd::CARD_GAP`] clear
/// of every card of `m` (the only rules that depend on where it goes). Cheap, so placement only
/// validates spots that pass it.
pub(super) fn free(m: &Model, x: f64, y: f64, size: (f64, f64)) -> bool {
    let (w, h) = size;
    let inside = [x, y, x + w, y + h].iter().all(|v| v.is_finite() && v.abs() <= cd::COORD_LIMIT);
    inside
        && m.classifiers.iter().all(|b| {
            let dx = (b.x - (x + w)).max(x - (b.x + b.w)).max(0.0);
            let dy = (b.y - (y + h)).max(y - (b.y + b.h)).max(0.0);
            dx * dx + dy * dy >= cd::CARD_GAP * cd::CARD_GAP
        })
}

/// `put` at `at` (validated), or at the first free spot of `spots`, then of a grid from the first
/// of them. A problem that doesn't depend on the spot (a bad name, a refused relationship) is
/// reported as it is, before any search.
pub(super) fn add_at(
    cmd: &str,
    m: &Model,
    at: Option<(f64, f64)>,
    spots: &[(f64, f64)],
    size: (f64, f64),
    put: impl Fn(&mut Model, f64, f64),
) -> Result<Model> {
    let try_at = |x: f64, y: f64| {
        let mut c = m.clone();
        put(&mut c, x, y);
        cd::validate(&c).map(|()| c)
    };
    if let Some((x, y)) = at {
        return try_at(x, y).map_err(|e| bad(cmd, e.to_string()));
    }
    let Some(&first) = spots.first() else { return Err(bad(cmd, "give `x` and `y`")) };
    match try_at(first.0, first.1) {
        Ok(c) => return Ok(c),
        Err(cd::ClassError::Overlap(..) | cd::ClassError::BadGeometry(_)) => {}
        Err(e) => return Err(bad(cmd, e.to_string())),
    }
    let grid = (0..GRID).flat_map(|row| (0..GRID).map(move |col| (first.0 + col as f64 * STEP, first.1 + row as f64 * STEP)));
    spots
        .iter()
        .copied()
        .skip(1)
        .chain(grid)
        .filter(|&(x, y)| free(m, x, y, size))
        .find_map(|(x, y)| try_at(x, y).ok())
        .ok_or_else(|| bad(cmd, "there's no free room for it: give `x` and `y`, or make room first"))
}

/// A new, empty card at its minimum size (its name checked against the text cap before it is
/// measured).
pub(super) fn new_card(cmd: &str, id: u64, kind: Kind, name: &str, is_abstract: bool) -> Result<Classifier> {
    let mut c = Classifier {
        id: ClassifierId(id),
        kind,
        name: name.to_string(),
        is_abstract,
        stereotype: None,
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        min_w: 1.0,
        min_h: 1.0,
        attributes: Vec::new(),
        operations: Vec::new(),
        literals: Vec::new(),
        show_attributes: true,
        show_operations: true,
    };
    let (w, h) = measure::card_min(cmd, &c)?;
    (c.w, c.h, c.min_w, c.min_h) = (w, h, w, h);
    Ok(c)
}
