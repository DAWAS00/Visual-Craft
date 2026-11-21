//! Member commands: attributes, operations and literals.

use super::params::{params_param, text_error, visibility};
use super::{Compartment, classifier_index, commit, compartment, fresh, len_of, measure, member_at, target};
use crate::Result;
use crate::Session;
use crate::diagram_params::clearable;
use crate::diagram_params::flag;
use crate::diagram_params::id;
use crate::diagram_params::raw_text;
use crate::diagram_params::req_id;
use crate::diagram_params::req_text;
use crate::diagram_params::text;
use crate::error::bad;
use figcraft_doc::class_diagram::Attribute;
use figcraft_doc::class_diagram::Kind;
use figcraft_doc::class_diagram::Literal;
use figcraft_doc::class_diagram::MemberId;
use figcraft_doc::class_diagram::Operation;
use figcraft_doc::class_diagram::Visibility;
use figcraft_doc::class_diagram::text::parse_attribute;
use figcraft_doc::class_diagram::text::parse_literal;
use figcraft_doc::class_diagram::text::parse_operation;
use serde_json::Value;
use serde_json::json;

/// A member made from `text`, or from fields, for `compartment`.
pub(super) enum Member {
    Attribute(Attribute),
    Operation(Operation),
    Literal(Literal),
}

pub(super) const MEMBER_FIELDS: [&str; 10] =
    ["visibility", "name", "type", "multiplicity", "default", "static", "abstract", "params", "returnType", "text"];

pub(super) fn new_member(cmd: &str, k: Compartment, id: u64, p: &Value) -> Result<Member> {
    let id = MemberId(id);
    if let Some(line) = raw_text(cmd, p, "text")? {
        if MEMBER_FIELDS.iter().any(|f| *f != "text" && p.get(*f).is_some()) {
            return Err(bad(cmd, "give `text`, or the member's fields, not both"));
        }
        return Ok(match k {
            Compartment::Attribute => Member::Attribute(parse_attribute(id, &line).map_err(|e| text_error(cmd, e))?),
            Compartment::Operation => Member::Operation(parse_operation(id, &line).map_err(|e| text_error(cmd, e))?),
            Compartment::Literal => Member::Literal(parse_literal(id, &line).map_err(|e| text_error(cmd, e))?),
        });
    }
    let name = req_text(cmd, p, "name")?;
    Ok(match k {
        Compartment::Attribute => {
            let mut a = Attribute { id, visibility: Visibility::Public, name, type_name: None, multiplicity: None, default: None, is_static: false };
            patch_attribute(cmd, &mut a, p)?;
            Member::Attribute(a)
        }
        Compartment::Operation => {
            let mut o =
                Operation { id, visibility: Visibility::Public, name, params: Vec::new(), return_type: None, is_static: false, is_abstract: false };
            patch_operation(cmd, &mut o, p)?;
            Member::Operation(o)
        }
        Compartment::Literal => Member::Literal(Literal { id, name }),
    })
}

pub(super) fn refuse_fields(cmd: &str, p: &Value, fields: &[&str], what: &str) -> Result<()> {
    match fields.iter().find(|f| p.get(**f).is_some_and(|v| !v.is_null())) {
        Some(f) => Err(bad(cmd, format!("{what} has no `{f}`"))),
        None => Ok(()),
    }
}

pub(super) fn patch_attribute(cmd: &str, a: &mut Attribute, p: &Value) -> Result<()> {
    refuse_fields(cmd, p, &["abstract", "params", "returnType"], "an attribute")?;
    if let Some(v) = visibility(cmd, p)? {
        a.visibility = v;
    }
    if let Some(n) = text(cmd, p, "name")? {
        a.name = n;
    }
    if let Some(t) = clearable(cmd, p, "type")? {
        a.type_name = t;
    }
    if let Some(m) = clearable(cmd, p, "multiplicity")? {
        a.multiplicity = m;
    }
    if let Some(d) = clearable(cmd, p, "default")? {
        a.default = d;
    }
    if let Some(v) = flag(cmd, p, "static")? {
        a.is_static = v;
    }
    Ok(())
}

pub(super) fn patch_operation(cmd: &str, o: &mut Operation, p: &Value) -> Result<()> {
    refuse_fields(cmd, p, &["type", "multiplicity", "default"], "an operation")?;
    if let Some(v) = visibility(cmd, p)? {
        o.visibility = v;
    }
    if let Some(n) = text(cmd, p, "name")? {
        o.name = n;
    }
    if let Some(ps) = params_param(cmd, p)? {
        o.params = ps;
    }
    if let Some(r) = clearable(cmd, p, "returnType")? {
        o.return_type = r;
    }
    if let Some(v) = flag(cmd, p, "static")? {
        o.is_static = v;
    }
    if let Some(v) = flag(cmd, p, "abstract")? {
        o.is_abstract = v;
    }
    Ok(())
}

pub(super) fn add_member(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.addMember";
    let (page, m) = target(s, cmd, p)?;
    let ci = classifier_index(cmd, &m, req_id(cmd, p, "classifier")?)?;
    let k = compartment(cmd, p)?;
    let new = fresh(cmd, &m)?;
    let member = new_member(cmd, k, new, p)?;
    let mut c = (*m).clone();
    let Some(card) = c.classifiers.get_mut(ci) else { return Err(bad(cmd, "no such classifier")) };
    // Right after `after` (a member of the same compartment), or at the end.
    let at = match id(cmd, p, "after")? {
        None => len_of(card, k),
        Some(after) => match member_at(cmd, &m, after)? {
            (aci, ak, ai) if aci == ci && ak == k => ai + 1,
            _ => return Err(bad(cmd, format!("member {after} isn't in this compartment"))),
        },
    };
    match member {
        Member::Attribute(a) => card.attributes.insert(at.min(card.attributes.len()), a),
        Member::Operation(o) => card.operations.insert(at.min(card.operations.len()), o),
        Member::Literal(l) => {
            if card.kind != Kind::Enumeration {
                return Err(bad(cmd, "only an enumeration has literals"));
            }
            card.literals.insert(at.min(card.literals.len()), l)
        }
    }
    measure::remeasure(cmd, card)?;
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": new}))
}

pub(super) fn update_member(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.updateMember";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let (ci, k, i) = member_at(cmd, &m, id)?;
    let before = m.classifiers.get(ci).ok_or_else(|| bad(cmd, "no such classifier"))?;
    let mut owner = before.clone();
    let card = &mut owner;
    let by_text = raw_text(cmd, p, "text")?.is_some();
    match k {
        Compartment::Attribute => {
            if let Some(a) = card.attributes.get_mut(i) {
                if by_text {
                    let Member::Attribute(new) = new_member(cmd, k, id, p)? else { return Err(bad(cmd, "not an attribute")) };
                    *a = new;
                } else {
                    patch_attribute(cmd, a, p)?;
                }
            }
        }
        Compartment::Operation => {
            if let Some(o) = card.operations.get_mut(i) {
                if by_text {
                    let Member::Operation(new) = new_member(cmd, k, id, p)? else { return Err(bad(cmd, "not an operation")) };
                    *o = new;
                } else {
                    patch_operation(cmd, o, p)?;
                }
            }
        }
        Compartment::Literal => {
            if let Some(l) = card.literals.get_mut(i) {
                if by_text {
                    let Member::Literal(new) = new_member(cmd, k, id, p)? else { return Err(bad(cmd, "not a literal")) };
                    *l = new;
                } else {
                    refuse_fields(
                        cmd,
                        p,
                        &["visibility", "type", "multiplicity", "default", "static", "abstract", "params", "returnType"],
                        "a literal",
                    )?;
                    if let Some(n) = text(cmd, p, "name")? {
                        l.name = n;
                    }
                }
            }
        }
    }
    // Nothing changed: the stored sizes stay and no undo step is made.
    if owner == *before {
        return Ok(json!({"id": id, "changed": false}));
    }
    measure::remeasure(cmd, &mut owner)?;
    let mut c = (*m).clone();
    if let Some(slot) = c.classifiers.get_mut(ci) {
        *slot = owner;
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn move_member(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.moveMember";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let (ci, k, i) = member_at(cmd, &m, id)?;
    let (before, after) = (self::id(cmd, p, "before")?, self::id(cmd, p, "after")?);
    let (other, past) = match (before, after) {
        (Some(b), None) => (b, false),
        (None, Some(a)) => (a, true),
        _ => return Err(bad(cmd, "give `before` or `after`")),
    };
    let (oci, ok, oi) = member_at(cmd, &m, other)?;
    if oci != ci || ok != k {
        return Err(bad(cmd, "a member moves only within its own compartment"));
    }
    // The index the member lands at once it is taken out.
    let target = {
        let o = if oi > i { oi - 1 } else { oi };
        if past { o + 1 } else { o }
    };
    let mut c = (*m).clone();
    let Some(card) = c.classifiers.get_mut(ci) else { return Err(bad(cmd, "no such classifier")) };
    fn shift<T>(v: &mut Vec<T>, from: usize, to: usize) {
        if from < v.len() {
            let x = v.remove(from);
            let to = to.min(v.len());
            v.insert(to, x);
        }
    }
    if other != id {
        match k {
            Compartment::Attribute => shift(&mut card.attributes, i, target),
            Compartment::Operation => shift(&mut card.operations, i, target),
            Compartment::Literal => shift(&mut card.literals, i, target),
        }
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "changed": changed}))
}

pub(super) fn remove_member(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.class.removeMember";
    let (page, m) = target(s, cmd, p)?;
    let id = req_id(cmd, p, "id")?;
    let (ci, k, i) = member_at(cmd, &m, id)?;
    let mut c = (*m).clone();
    if let Some(card) = c.classifiers.get_mut(ci) {
        // `i` came from `member_at` on the same model; checked again so a remove never panics.
        match k {
            Compartment::Attribute if i < card.attributes.len() => drop(card.attributes.remove(i)),
            Compartment::Operation if i < card.operations.len() => drop(card.operations.remove(i)),
            Compartment::Literal if i < card.literals.len() => drop(card.literals.remove(i)),
            _ => return Err(bad(cmd, format!("there is no member {id} on this diagram"))),
        }
        measure::remeasure(cmd, card)?;
    }
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id}))
}
