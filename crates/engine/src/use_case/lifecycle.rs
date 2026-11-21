//! Page lifecycle: duplicating a page's model.

use std::sync::Arc;

use figcraft_doc::use_case::{self as uc, ActorId, GenEnds, Link, LinkId, Model, PointId, SubjectId, UseCaseId};
use figcraft_doc::{Document, NodeId};

use super::{next_id, render};
use crate::Result;
use crate::error::bad;

/// The largest semantic ID in the model (0 when it is empty).
pub(super) fn max_id(m: &Model) -> u64 {
    next_id(m).map_or(u64::MAX, |n| n - 1)
}

/// `m` with every semantic ID moved up by `by` (references follow), so none is shared with
/// another model's.
pub(super) fn shifted(m: &Model, by: u64) -> Option<Model> {
    let f = |id: u64| id.checked_add(by);
    let mut c = m.clone();
    for a in &mut c.actors {
        a.id = ActorId(f(a.id.0)?);
    }
    for s in &mut c.subjects {
        s.id = SubjectId(f(s.id.0)?);
    }
    for u in &mut c.use_cases {
        u.id = UseCaseId(f(u.id.0)?);
        u.subject = match u.subject {
            Some(s) => Some(SubjectId(f(s.0)?)),
            None => None,
        };
        for p in &mut u.points {
            p.id = PointId(f(p.id.0)?);
        }
    }
    for l in &mut c.links {
        match l {
            Link::Association { id, actor, use_case, .. } => {
                *id = LinkId(f(id.0)?);
                *actor = ActorId(f(actor.0)?);
                *use_case = UseCaseId(f(use_case.0)?);
            }
            Link::Include { id, including, included } => {
                *id = LinkId(f(id.0)?);
                *including = UseCaseId(f(including.0)?);
                *included = UseCaseId(f(included.0)?);
            }
            Link::Extend { id, extending, base, points, .. } => {
                *id = LinkId(f(id.0)?);
                *extending = UseCaseId(f(extending.0)?);
                *base = UseCaseId(f(base.0)?);
                for p in points.iter_mut() {
                    *p = PointId(f(p.0)?);
                }
            }
            Link::Generalization { id, ends } => {
                *id = LinkId(f(id.0)?);
                *ends = match *ends {
                    GenEnds::Actors { specific, general } => GenEnds::Actors { specific: ActorId(f(specific.0)?), general: ActorId(f(general.0)?) },
                    GenEnds::UseCases { specific, general } => {
                        GenEnds::UseCases { specific: UseCaseId(f(specific.0)?), general: UseCaseId(f(general.0)?) }
                    }
                };
            }
        }
    }
    Some(c)
}

/// `page.duplicate` of a Use Case page: the copy `dst` gets the model of `src` with fresh IDs
/// (past every ID in the file's models) and layers generated from it, in place of the copied
/// generated layers. Nothing when `src` has no model.
pub(crate) fn duplicate_page(doc: &mut Document, src: NodeId, dst: NodeId) -> Result<()> {
    let cmd = "page.duplicate";
    let Some(model) = doc.use_cases.get(&src).cloned() else { return Ok(()) };
    let by = doc.use_cases.values().map(|m| max_id(m)).max().unwrap_or(0);
    let copy = shifted(&model, by).ok_or_else(|| bad(cmd, "the diagram has run out of IDs"))?;
    uc::validate(&copy).map_err(|e| bad(cmd, format!("the copied diagram: {e}")))?;
    doc.use_cases.insert(dst, Arc::new(copy));
    render::regenerate(doc, dst)
}
