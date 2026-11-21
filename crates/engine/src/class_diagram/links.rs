//! Relationship commands, and the check and read commands.

use super::params::{ends_param, kind_param, link_kind, patch_end, unknown_keys};
use super::{add_at, classifier_index, commit, fresh, link_index, new_card, target};
use crate::Result;
use crate::Session;
use crate::diagram_params::clearable;
use crate::diagram_params::ids;
use crate::diagram_params::position;
use crate::diagram_params::req_id;
use crate::diagram_params::req_text;
use crate::diagram_params::text;
use crate::error::bad;
use figcraft_doc::class_diagram as cd;
use figcraft_doc::class_diagram::Classifier;
use figcraft_doc::class_diagram::ClassifierId;
use figcraft_doc::class_diagram::End;
use figcraft_doc::class_diagram::Link;
use figcraft_doc::class_diagram::LinkId;
use figcraft_doc::class_diagram::LinkKind;
use figcraft_doc::class_diagram::Model;
use serde_json::Value;
use serde_json::json;
use std::collections::HashSet;

/// A relationship of `kind` from `from` to `to`, with `ends` and `label` applied.
pub(super) fn make_link(cmd: &str, m: &Model, id: u64, kind: LinkKind, from: u64, to: u64, p: &Value) -> Result<Link> {
    classifier_index(cmd, m, from)?;
    classifier_index(cmd, m, to)?;
    let mut l = Link { id: LinkId(id), kind, ends: [End::at(ClassifierId(from)), End::at(ClassifierId(to))], label: None };
    if let Some([a, b]) = ends_param(cmd, p)? {
        patch_end(cmd, &mut l.ends[0], &a)?;
        patch_end(cmd, &mut l.ends[1], &b)?;
    }
    if let Some(label) = clearable(cmd, p, "label")? {
        l.label = label;
    }
    Ok(l)
}

pub(super) fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.connect";
    let (page, m) = target(s, cmd, p)?;
    let kind = link_kind(cmd, p, "kind")?.ok_or_else(|| bad(cmd, "`kind` is required"))?;
    let new = fresh(cmd, &m)?;
    let l = make_link(cmd, &m, new, kind, req_id(cmd, p, "from")?, req_id(cmd, p, "to")?, p)?;
    let mut c = (*m).clone();
    c.links.push(l);
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": new}))
}

/// What `connect` with `p` would answer, without changing anything: the reason it would be
/// refused, if it would.
pub fn check_connect(m: &Model, p: &Value) -> std::result::Result<(), String> {
    let cmd = "diagram.class.connect";
    let run = || -> Result<()> {
        unknown_keys(cmd, p, &["page", "kind", "from", "to", "ends", "label"])?;
        let kind = link_kind(cmd, p, "kind")?.ok_or_else(|| bad(cmd, "`kind` is required"))?;
        let new = fresh(cmd, m)?;
        let l = make_link(cmd, m, new, kind, req_id(cmd, p, "from")?, req_id(cmd, p, "to")?, p)?;
        let mut c = m.clone();
        c.links.push(l);
        cd::validate(&c).map_err(|e| bad(cmd, e.to_string()))
    };
    run().map_err(|e| e.to_string())
}

pub(super) fn add_connected(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.addConnected";
    let (page, m) = target(s, cmd, p)?;
    let source = req_id(cmd, p, "source")?;
    let si = classifier_index(cmd, &m, source)?;
    let kind = link_kind(cmd, p, "relation")?.ok_or_else(|| bad(cmd, "`relation` is required"))?;
    let element = kind_param(cmd, p, "kind")?.ok_or_else(|| bad(cmd, "`kind` (the new element's: class, interface or enumeration) is required"))?;
    // `out`: the source is the relationship's first end (the specific class, the implementer, the
    // client, the whole); `in`: the new element is.
    let out = match text(cmd, p, "direction")?.as_deref() {
        Some("out") => true,
        Some("in") => false,
        None => return Err(bad(cmd, "`direction` (out or in) is required")),
        Some(other) => return Err(bad(cmd, format!("`direction` is out or in, not `{other}`"))),
    };
    // Which end is the whole of an aggregation or composition. The whole is the first end, so it
    // must agree with `direction`.
    match text(cmd, p, "wholeEnd")?.as_deref() {
        None => {}
        Some(w @ ("source" | "new")) => {
            if !matches!(kind, LinkKind::Aggregation | LinkKind::Composition) {
                return Err(bad(cmd, "`wholeEnd` belongs to an aggregation or a composition"));
            }
            if (w == "source") != out {
                let want = if w == "source" { "out" } else { "in" };
                return Err(bad(cmd, format!("`wholeEnd` {w} makes the {w} element the first end: give `direction` {want}")));
            }
        }
        Some(other) => return Err(bad(cmd, format!("`wholeEnd` is source or new, not `{other}`"))),
    }
    let src = m.classifiers.get(si).ok_or_else(|| bad(cmd, "no such classifier"))?;
    let name = req_text(cmd, p, "name")?;
    let at = position(cmd, p)?;
    let cid = fresh(cmd, &m)?;
    let lid = cid.checked_add(1).ok_or_else(|| bad(cmd, "the diagram has run out of IDs"))?;
    let card = new_card(cmd, cid, element, &name, false)?;
    let (from, to) = if out { (source, cid) } else { (cid, source) };
    let mut probe = (*m).clone();
    probe.classifiers.push(card.clone());
    let link = make_link(cmd, &probe, lid, kind, from, to, p)?;
    // Beside the source: right, below, left, above, then further out.
    let (sx, sy, sw, sh) = src.bounds();
    let gap = cd::CARD_GAP * 3.0;
    let spots = [(sx + sw + gap, sy), (sx, sy + sh + gap), (sx - card.w - gap, sy), (sx, sy - card.h - gap)];
    let put = |c: &mut Model, x: f64, y: f64| {
        c.classifiers.push(Classifier { x, y, ..card.clone() });
        c.links.push(link.clone());
    };
    let made = add_at(cmd, &m, at, &spots, (card.w, card.h), put)?;
    commit(s, cmd, page, &m, made)?;
    Ok(json!({"element": cid, "link": lid}))
}

pub(super) fn update_link(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.updateLink";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let i = link_index(cmd, &m, id)?;
    let mut c = (*m).clone();
    let Some(l) = c.links.get_mut(i) else { return Err(bad(cmd, "no such relationship")) };
    if let Some(k) = link_kind(cmd, p, "kind")? {
        // Only among the association kinds, which keep the same ends and labels.
        if k != l.kind && !(k.is_association() && l.kind.is_association()) {
            return Err(bad(cmd, "only association, aggregation and composition change into one another; remove it and connect again"));
        }
        l.kind = k;
    }
    if let Some([a, b]) = ends_param(cmd, p)? {
        patch_end(cmd, &mut l.ends[0], &a)?;
        patch_end(cmd, &mut l.ends[1], &b)?;
    }
    if let Some(label) = clearable(cmd, p, "label")? {
        l.label = label;
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn swap_ends(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.swapEnds";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let i = link_index(cmd, &m, id)?;
    let mut c = (*m).clone();
    if let Some(l) = c.links.get_mut(i) {
        l.ends.swap(0, 1);
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.remove";
    let (page, m) = target(s, cmd, p)?;
    let list = ids(cmd, p, "ids", cd::MAX_CLASSIFIERS + cd::MAX_LINKS)?.ok_or_else(|| bad(cmd, "`ids` is required"))?;
    for i in &list {
        if m.classifiers.iter().all(|c| c.id.0 != *i) && m.links.iter().all(|l| l.id.0 != *i) {
            return Err(bad(cmd, format!("there is no class, interface, enumeration or relationship {i} on this diagram")));
        }
    }
    let gone: HashSet<u64> = list.into_iter().collect();
    let mut c = (*m).clone();
    c.classifiers.retain(|x| !gone.contains(&x.id.0));
    // A removed classifier takes its relationships with it.
    let alive: HashSet<ClassifierId> = c.classifiers.iter().map(|x| x.id).collect();
    c.links.retain(|l| !gone.contains(&l.id.0) && alive.contains(&l.ends[0].classifier) && alive.contains(&l.ends[1].classifier));
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"removed": gone.len()}))
}

/// A warning's subject and message, for `check` and the panel.
pub fn warning_text(m: &Model, w: &cd::Warning) -> (u64, String) {
    let name = |c: ClassifierId| m.classifier(c).map_or_else(|| c.to_string(), |c| c.name.clone());
    match w {
        cd::Warning::AbstractInConcrete { classifier, operation } => {
            (operation.0, format!("{} isn't abstract but declares an abstract operation", name(*classifier)))
        }
        cd::Warning::MissingOperation { classifier, interface, name: op, .. } => {
            (classifier.0, format!("{} realizes {} but has no {op}(…) with the same parameters", name(*classifier), name(*interface)))
        }
        cd::Warning::Unconnected(c) => (c.0, format!("{} has no relationships", name(*c))),
        cd::Warning::EmptyEnumeration(c) => (c.0, format!("{} has no literals", name(*c))),
    }
}

pub(super) fn check(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.check";
    let (_, m) = target(s, cmd, p)?;
    let warnings: Vec<Value> = cd::check(&m)
        .iter()
        .map(|w| {
            let (id, message) = warning_text(&m, w);
            json!({"id": id, "message": message})
        })
        .collect();
    Ok(json!({"warnings": warnings}))
}

pub(super) fn read(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.read";
    let (page, m) = target(s, cmd, p)?;
    let model = serde_json::to_value(&*m).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(json!({"page": page, "schema": cd::CLASS_SCHEMA, "model": model}))
}
