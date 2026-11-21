//! UML Use Case diagram commands (`diagram.useCase.*`; `docs/diagram-type-specifications.md` § 5).
//!
//! A Use Case page keeps its model in [`Document::use_cases`]; every command builds a whole new
//! model, validates it ([`figcraft_doc::use_case::validate`]) and only then stores it and
//! regenerates the page's layers ([`render`]). A refused command changes nothing, and a command
//! that changes nothing adds no undo step.

mod elements;
mod example;
pub(crate) mod gate;
mod lifecycle;
mod links;
pub(crate) mod measure;
mod params;
pub(crate) mod render;
mod specs;

pub(crate) use example::start;
pub(crate) use lifecycle::duplicate_page;
pub use links::check_connect;
pub use specs::specs;

use params::{id, number};

use figcraft_doc::NodeId;
use figcraft_doc::use_case::{self as uc, Actor, Model, Presentation, UseCase};
use serde_json::Value;
use std::sync::Arc;

use crate::error::bad;
use crate::{Result, Session};

/// The `type` of a Use Case diagram in `file.newDiagram`.
pub(crate) const TYPE_ID: &str = "useCase";
/// Its example template.
pub(crate) const EXAMPLE: &str = "useCaseExample";
/// A new subject's size when none is given.
pub(super) const SUBJECT_W: f64 = 420.0;
pub(super) const SUBJECT_H: f64 = 520.0;
/// Placement tries spots this far apart.
pub(super) const STEP: f64 = 40.0;

// ---- the page and its model ----

/// The Use Case page a command acts on (`page`, or the page in view) and its model.
pub(super) fn target(s: &Session, cmd: &str, p: &Value) -> Result<(NodeId, Arc<Model>)> {
    let page = match id(cmd, p, "page")? {
        Some(n) => NodeId(n),
        None => s.page()?,
    };
    let model = s.doc()?.use_cases.get(&page).cloned().ok_or_else(|| bad(cmd, format!("page {page} isn't a Use Case diagram")))?;
    Ok((page, model))
}

/// Stores `after` and redraws the page, unless it equals `before` (`false`: nothing changed).
pub(super) fn commit(s: &mut Session, cmd: &str, page: NodeId, before: &Model, after: Model) -> Result<bool> {
    if *before == after {
        return Ok(false);
    }
    uc::validate(&after).map_err(|e| bad(cmd, e.to_string()))?;
    let doc = s.doc_mut()?;
    doc.use_cases.insert(page, Arc::new(after));
    render::regenerate(doc, page)?;
    Ok(true)
}

/// A new semantic ID: one past the largest in the model.
pub(super) fn next_id(m: &Model) -> Option<u64> {
    let points = m.use_cases.iter().flat_map(|u| u.points.iter().map(|p| p.id.0));
    let all = m
        .actors
        .iter()
        .map(|a| a.id.0)
        .chain(m.use_cases.iter().map(|u| u.id.0))
        .chain(m.subjects.iter().map(|s| s.id.0))
        .chain(m.links.iter().map(|l| l.id().0))
        .chain(points);
    all.max().unwrap_or(0).checked_add(1)
}

pub(super) fn fresh(cmd: &str, m: &Model) -> Result<u64> {
    next_id(m).ok_or_else(|| bad(cmd, "the diagram has run out of IDs"))
}

/// What an ID names in the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Item {
    Actor(usize),
    UseCase(usize),
    Subject(usize),
    Link(usize),
}

pub(super) fn find(m: &Model, id: u64) -> Option<Item> {
    if let Some(i) = m.actors.iter().position(|a| a.id.0 == id) {
        return Some(Item::Actor(i));
    }
    if let Some(i) = m.use_cases.iter().position(|u| u.id.0 == id) {
        return Some(Item::UseCase(i));
    }
    if let Some(i) = m.subjects.iter().position(|s| s.id.0 == id) {
        return Some(Item::Subject(i));
    }
    m.links.iter().position(|l| l.id().0 == id).map(Item::Link)
}

pub(super) fn find_or(cmd: &str, m: &Model, id: u64) -> Result<Item> {
    find(m, id).ok_or_else(|| bad(cmd, format!("there is no element or relationship {id} on this diagram")))
}

pub(super) fn use_case_index(cmd: &str, m: &Model, id: u64) -> Result<usize> {
    match find(m, id) {
        Some(Item::UseCase(i)) => Ok(i),
        _ => Err(bad(cmd, format!("{id} isn't a use case on this diagram"))),
    }
}

/// The use case owning extension point `id`, and the point's index in it.
pub(super) fn point_index(cmd: &str, m: &Model, id: u64) -> Result<(usize, usize)> {
    m.use_cases
        .iter()
        .enumerate()
        .find_map(|(u, c)| c.points.iter().position(|p| p.id.0 == id).map(|i| (u, i)))
        .ok_or_else(|| bad(cmd, format!("there is no extension point {id} on this diagram")))
}

/// Re-measures a use case after its name or points changed: the size grows to the new minimum
/// and keeps any extra room it was given.
pub(super) fn remeasure_case(u: &mut UseCase) {
    let (w, h) = measure::use_case_min(&u.name, u.points.iter().map(|p| p.name.as_str()));
    (u.min_w, u.min_h) = (w, h);
    (u.w, u.h) = (u.w.max(w), u.h.max(h));
}

/// Re-measures an actor after its name or presentation changed. A size outside the
/// presentation's range starts again from the minimum.
pub(super) fn remeasure_actor(a: &mut Actor) {
    let (w, h) = measure::actor_min(&a.name, a.presentation);
    let (lo_w, hi_w, lo_h, hi_h) = uc::actor_size_range(a.presentation);
    (a.min_w, a.min_h) = (w, h);
    a.w = if (lo_w..=hi_w).contains(&a.w) { a.w.max(w) } else { w };
    a.h = if (lo_h..=hi_h).contains(&a.h) { a.h.max(h) } else { h };
}

// ---- placement ----

/// The first spot on a grid from `start` where `put` makes a valid model, trying rows down and
/// columns across (at most `cols` × `rows` spots).
pub(super) fn place(m: &Model, start: (f64, f64), cols: usize, rows: usize, put: impl Fn(&mut Model, f64, f64)) -> Option<Model> {
    for row in 0..rows {
        for col in 0..cols {
            let mut c = m.clone();
            put(&mut c, start.0 + col as f64 * STEP, start.1 + row as f64 * STEP);
            if uc::validate(&c).is_ok() {
                return Some(c);
            }
        }
    }
    None
}

/// The right edge of everything on the diagram (0 when it is empty).
pub(super) fn right_edge(m: &Model) -> f64 {
    let a = m.actors.iter().map(|a| a.x + a.w);
    let u = m.use_cases.iter().map(|u| u.x + u.w);
    let s = m.subjects.iter().map(|s| s.x + s.width);
    a.chain(u).chain(s).fold(0.0, f64::max)
}

/// Adds `item` (made by `make` at a spot) at `at`, or at the first free spot near `start`.
pub(super) fn add_at(
    cmd: &str,
    m: &Model,
    at: Option<(f64, f64)>,
    start: (f64, f64),
    cols: usize,
    rows: usize,
    put: impl Fn(&mut Model, f64, f64),
) -> Result<Model> {
    match at {
        Some((x, y)) => {
            let mut c = m.clone();
            put(&mut c, x, y);
            uc::validate(&c).map_err(|e| bad(cmd, e.to_string()))?;
            Ok(c)
        }
        None => place(m, start, cols, rows, put).ok_or_else(|| bad(cmd, "there's no free room for it: give `x` and `y`, or make room first")),
    }
}

pub(super) fn position(cmd: &str, p: &Value) -> Result<Option<(f64, f64)>> {
    match (number(cmd, p, "x")?, number(cmd, p, "y")?) {
        (Some(x), Some(y)) => Ok(Some((x, y))),
        (None, None) => Ok(None),
        _ => Err(bad(cmd, "give both `x` and `y`, or neither")),
    }
}

/// The size `diagram.useCase.addActor` (default presentation) or `addUseCase` gives a new element
/// named `name`, so the editor can centre it on the press.
pub fn new_element_size(actor: bool, name: &str) -> (f64, f64) {
    if actor { measure::actor_min(name, Presentation::default()) } else { measure::use_case_min(name, []) }
}
