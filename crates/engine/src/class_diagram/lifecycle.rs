//! Page lifecycle: duplicating a page's model.

use super::render;
use crate::Result;
use crate::error::bad;
use figcraft_doc::Document;
use figcraft_doc::NodeId;
use figcraft_doc::class_diagram as cd;
use figcraft_doc::class_diagram::ClassifierId;
use figcraft_doc::class_diagram::LinkId;
use figcraft_doc::class_diagram::MemberId;
use figcraft_doc::class_diagram::Model;
use std::sync::Arc;

/// The largest semantic ID in the model (0 when it is empty).
pub(super) fn max_id(m: &Model) -> u64 {
    m.next_id().map_or(u64::MAX, |n| n - 1)
}

/// `m` with every semantic ID moved up by `by` (references follow).
pub(super) fn shifted(m: &Model, by: u64) -> Option<Model> {
    let f = |id: u64| id.checked_add(by);
    let mut c = m.clone();
    for k in &mut c.classifiers {
        k.id = ClassifierId(f(k.id.0)?);
        for a in &mut k.attributes {
            a.id = MemberId(f(a.id.0)?);
        }
        for o in &mut k.operations {
            o.id = MemberId(f(o.id.0)?);
        }
        for l in &mut k.literals {
            l.id = MemberId(f(l.id.0)?);
        }
    }
    for l in &mut c.links {
        l.id = LinkId(f(l.id.0)?);
        for e in &mut l.ends {
            e.classifier = ClassifierId(f(e.classifier.0)?);
        }
    }
    Some(c)
}

/// `page.duplicate` of a Class diagram page: the copy `dst` gets the model of `src` with fresh
/// IDs and layers generated from it. Nothing when `src` has no model.
pub(crate) fn duplicate_page(doc: &mut Document, src: NodeId, dst: NodeId) -> Result<()> {
    let cmd = "page.duplicate";
    let Some(model) = doc.class_diagrams.get(&src).cloned() else { return Ok(()) };
    let by = doc.class_diagrams.values().map(|m| max_id(m)).max().unwrap_or(0);
    let copy = shifted(&model, by).ok_or_else(|| bad(cmd, "the diagram has run out of IDs"))?;
    cd::validate(&copy).map_err(|e| bad(cmd, format!("the copied diagram: {e}")))?;
    doc.class_diagrams.insert(dst, Arc::new(copy));
    render::regenerate(doc, dst)
}
