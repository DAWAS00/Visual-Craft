//! Classifier commands: add, update, move and resize.

use super::params::kind_param;
use super::{STEP, add_at, classifier_index, commit, fresh, measure, new_card, target};
use crate::Result;
use crate::Session;
use crate::diagram_params::clearable;
use crate::diagram_params::flag;
use crate::diagram_params::position;
use crate::diagram_params::req_id;
use crate::diagram_params::req_number;
use crate::diagram_params::req_text;
use crate::diagram_params::text;
use crate::error::bad;
use figcraft_doc::class_diagram as cd;
use figcraft_doc::class_diagram::Classifier;
use figcraft_doc::class_diagram::Kind;
use serde_json::Value;
use serde_json::json;

pub(super) fn add_classifier(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.addClassifier";
    let (page, m) = target(s, cmd, p)?;
    let kind = kind_param(cmd, p, "kind")?.unwrap_or_default();
    let name = req_text(cmd, p, "name")?;
    let is_abstract = flag(cmd, p, "abstract")?.unwrap_or(false);
    let new = fresh(cmd, &m)?;
    let at = position(cmd, p)?;
    let card = new_card(cmd, new, kind, &name, is_abstract)?;
    let start = (m.classifiers.iter().map(|c| c.x + c.w).fold(0.0, f64::max).min(cd::COORD_LIMIT / 2.0) + STEP, 40.0);
    let start = if m.classifiers.is_empty() { (40.0, 40.0) } else { start };
    let c = add_at(cmd, &m, at, &[start], (card.w, card.h), |c, x, y| {
        c.classifiers.push(Classifier { x, y, ..card.clone() });
    })?;
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": new}))
}

pub(super) fn update_classifier(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.updateClassifier";
    let (page, m) = target(s, cmd, p)?;
    let i = classifier_index(cmd, &m, req_id(cmd, p, "id")?)?;
    let before = m.classifiers.get(i).ok_or_else(|| bad(cmd, "no such classifier"))?;
    let mut card = before.clone();
    if let Some(n) = text(cmd, p, "name")? {
        card.name = n;
    }
    if let Some(k) = kind_param(cmd, p, "kind")? {
        // A class and an interface hold the same compartments; an enumeration holds literals.
        if k != card.kind && (k == Kind::Enumeration || card.kind == Kind::Enumeration) {
            return Err(bad(
                cmd,
                "only a class and an interface change into one another; an enumeration's literals don't carry over: add a new one instead",
            ));
        }
        card.kind = k;
    }
    if let Some(a) = flag(cmd, p, "abstract")? {
        card.is_abstract = a;
    }
    if let Some(st) = clearable(cmd, p, "stereotype")? {
        card.stereotype = st;
    }
    if let Some(v) = flag(cmd, p, "showAttributes")? {
        card.show_attributes = v;
    }
    if let Some(v) = flag(cmd, p, "showOperations")? {
        card.show_operations = v;
    }
    let id = card.id.0;
    // Nothing named changed: the stored sizes stay, so a font change since they were measured
    // never makes a no-op an undo step.
    if card == *before {
        return Ok(json!({"id": id, "changed": false}));
    }
    measure::remeasure(cmd, &mut card)?;
    let mut c = (*m).clone();
    if let Some(slot) = c.classifiers.get_mut(i) {
        *slot = card;
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn move_classifier(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.move";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let i = classifier_index(cmd, &m, id)?;
    let (x, y) = (req_number(cmd, p, "x")?, req_number(cmd, p, "y")?);
    let mut c = (*m).clone();
    if let Some(card) = c.classifiers.get_mut(i) {
        (card.x, card.y) = (x, y);
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.resize";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let i = classifier_index(cmd, &m, id)?;
    let (w, h) = (req_number(cmd, p, "width")?, req_number(cmd, p, "height")?);
    let at = position(cmd, p)?;
    let mut c = (*m).clone();
    if let Some(card) = c.classifiers.get_mut(i) {
        (card.w, card.h) = (w, h);
        if let Some((x, y)) = at {
            (card.x, card.y) = (x, y);
        }
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}
