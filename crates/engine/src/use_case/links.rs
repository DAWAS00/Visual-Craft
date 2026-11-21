//! Relationship commands, and the check and read commands.

use figcraft_doc::use_case::{self as uc, Actor, ActorId, GenEnds, Link, LinkId, Model, PointId, Presentation, UseCaseId};
use serde_json::{Value, json};

use super::elements::{new_case, subject_param, with_new_point};
use super::params::{clearable, ids, req_id, req_text, text};
use super::{Item, add_at, commit, find, find_or, fresh, measure, position, target, use_case_index};
use crate::error::bad;
use crate::{Result, Session};

/// The relationship kinds, as `connect` names them.
pub(super) const KINDS: [&str; 4] = ["association", "include", "extend", "generalization"];

/// The extension points an Extend names: `points`, plus a new point `newPoint` made on the base.
pub(super) fn extend_points(cmd: &str, c: &mut Model, base: u64, p: &Value) -> Result<(Vec<PointId>, Option<u64>)> {
    let mut points: Vec<PointId> = ids(cmd, p, "points", uc::MAX_EXTEND_POINTS)?.unwrap_or_default().into_iter().map(PointId).collect();
    let made = match text(cmd, p, "newPoint")? {
        Some(name) => {
            let i = use_case_index(cmd, c, base)?;
            let id = with_new_point(cmd, c, i, &name)?;
            points.push(PointId(id));
            Some(id)
        }
        None => None,
    };
    Ok((points, made))
}

/// The relationship `kind` from `from` to `to`, checking which parameters belong to it.
pub(super) fn make_link(cmd: &str, c: &mut Model, id: u64, kind: &str, from: u64, to: u64, p: &Value) -> Result<(Link, Option<u64>)> {
    let only = |keys: &[&str]| -> Result<()> {
        for k in ["points", "newPoint", "condition", "actorMultiplicity", "useCaseMultiplicity"] {
            if !keys.contains(&k) && p.get(k).is_some_and(|v| !v.is_null()) {
                return Err(bad(cmd, format!("`{k}` doesn't belong to a {kind}")));
            }
        }
        Ok(())
    };
    let kind_of = |n: u64| find(c, n);
    let link = match kind {
        "association" => {
            only(&["actorMultiplicity", "useCaseMultiplicity"])?;
            let (actor, case) = match (kind_of(from), kind_of(to)) {
                (Some(Item::Actor(_)), Some(Item::UseCase(_))) => (from, to),
                (Some(Item::UseCase(_)), Some(Item::Actor(_))) => (to, from),
                _ => return Err(bad(cmd, "an association joins an actor and a use case")),
            };
            Link::Association {
                id: LinkId(id),
                actor: ActorId(actor),
                use_case: UseCaseId(case),
                actor_multiplicity: text(cmd, p, "actorMultiplicity")?.filter(|v| !v.is_empty()),
                use_case_multiplicity: text(cmd, p, "useCaseMultiplicity")?.filter(|v| !v.is_empty()),
            }
        }
        "include" => {
            only(&[])?;
            both_cases(cmd, c, from, to, "an include")?;
            Link::Include { id: LinkId(id), including: UseCaseId(from), included: UseCaseId(to) }
        }
        "extend" => {
            only(&["points", "newPoint", "condition"])?;
            both_cases(cmd, c, from, to, "an extend")?;
            let (points, made) = extend_points(cmd, c, to, p)?;
            let condition = text(cmd, p, "condition")?.filter(|v| !v.is_empty());
            return Ok((Link::Extend { id: LinkId(id), extending: UseCaseId(from), base: UseCaseId(to), points, condition }, made));
        }
        "generalization" => {
            only(&[])?;
            let ends = match (kind_of(from), kind_of(to)) {
                (Some(Item::Actor(_)), Some(Item::Actor(_))) => GenEnds::Actors { specific: ActorId(from), general: ActorId(to) },
                (Some(Item::UseCase(_)), Some(Item::UseCase(_))) => GenEnds::UseCases { specific: UseCaseId(from), general: UseCaseId(to) },
                _ => return Err(bad(cmd, "a generalization joins two actors or two use cases")),
            };
            Link::Generalization { id: LinkId(id), ends }
        }
        other => return Err(bad(cmd, format!("`kind` is one of {}, not `{other}`", KINDS.join(", ")))),
    };
    Ok((link, None))
}

pub(super) fn both_cases(cmd: &str, c: &Model, from: u64, to: u64, what: &str) -> Result<()> {
    match (find(c, from), find(c, to)) {
        (Some(Item::UseCase(_)), Some(Item::UseCase(_))) => Ok(()),
        _ => Err(bad(cmd, format!("{what} joins two use cases"))),
    }
}

pub(super) fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.connect";
    let (page, m) = target(s, cmd, p)?;
    let kind = req_text(cmd, p, "kind")?;
    let (from, to) = (req_id(cmd, p, "from")?, req_id(cmd, p, "to")?);
    let mut c = (*m).clone();
    // The new point (if any) takes its ID first, so the link's comes after it.
    let (link, point) = make_link(cmd, &mut c, 0, &kind, from, to, p)?;
    let id = fresh(cmd, &c)?;
    c.links.push(with_id(link, id));
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id, "point": point}))
}

/// Whether `diagram.useCase.connect` with `p` would succeed on `m`, by the same steps and the
/// same validation, without changing anything: the editor asks before it offers a join. The
/// engine's reason when it would be refused.
pub fn check_connect(m: &Model, p: &Value) -> std::result::Result<(), String> {
    let cmd = "diagram.useCase.connect";
    let attempt = || -> Result<()> {
        // As dispatch does before running the command.
        crate::cmd::refuse_unknown_params(cmd, p)?;
        let kind = req_text(cmd, p, "kind")?;
        let (from, to) = (req_id(cmd, p, "from")?, req_id(cmd, p, "to")?);
        let mut c = m.clone();
        let (link, _) = make_link(cmd, &mut c, 0, &kind, from, to, p)?;
        let id = fresh(cmd, &c)?;
        c.links.push(with_id(link, id));
        uc::validate(&c).map_err(|e| bad(cmd, e.to_string()))
    };
    attempt().map_err(|e| e.to_string())
}

pub(super) fn with_id(mut l: Link, new: u64) -> Link {
    match &mut l {
        Link::Association { id, .. } | Link::Include { id, .. } | Link::Extend { id, .. } | Link::Generalization { id, .. } => *id = LinkId(new),
    }
    l
}

/// `addConnected`'s relations, each naming which way the new link points.
pub(super) const RELATIONS: [&str; 5] = ["association", "include", "extension", "parent", "child"];

pub(super) fn add_connected(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.addConnected";
    let (page, m) = target(s, cmd, p)?;
    let source = req_id(cmd, p, "source")?;
    let relation = req_text(cmd, p, "relation")?;
    let name = req_text(cmd, p, "name")?;
    let at = position(cmd, p)?;
    let src = find_or(cmd, &m, source)?;
    let new = fresh(cmd, &m)?;
    // The new element: a use case, or an actor for an actor's parent or child.
    let mut c = match (src, relation.as_str()) {
        (Item::Actor(i), "parent" | "child") => {
            let near = m.actors.get(i).map_or((40.0, 40.0), |a| (a.x, a.y + a.h + 40.0));
            let (w, h) = measure::actor_min(&name, Presentation::Person);
            add_at(cmd, &m, at, near, 4, 20, |c, x, y| {
                c.actors.push(Actor { id: ActorId(new), name: name.clone(), presentation: Presentation::Person, x, y, w, h, min_w: w, min_h: h });
            })?
        }
        (Item::Actor(_), "association") | (Item::UseCase(_), "include" | "extension" | "parent" | "child") => {
            let subject = match subject_param(cmd, &m, p)? {
                Some(sid) => Some(sid),
                // `"subject": null` asks for no system, as `addUseCase` reads it (a case made
                // where the user dropped it, outside every system); left out, the source's.
                None if p.get("subject").is_some_and(Value::is_null) => None,
                None => match src {
                    Item::UseCase(i) => m.use_cases.get(i).and_then(|u| u.subject),
                    _ => m.subjects.first().map(|x| x.id),
                },
            };
            new_case(cmd, &m, new, &name, subject, at)?
        }
        _ if !RELATIONS.contains(&relation.as_str()) => return Err(bad(cmd, format!("`relation` is one of {}", RELATIONS.join(", ")))),
        _ => return Err(bad(cmd, format!("a {relation} can't start from {source}"))),
    };
    let (kind, from, to) = match relation.as_str() {
        "association" => ("association", source, new),
        "include" => ("include", source, new),
        // The new case extends the source: the arrow points back to the source.
        "extension" => ("extend", new, source),
        "parent" => ("generalization", source, new),
        _ => ("generalization", new, source),
    };
    let (link, point) = make_link(cmd, &mut c, 0, kind, from, to, p)?;
    let link_id = fresh(cmd, &c)?;
    c.links.push(with_id(link, link_id));
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"element": new, "link": link_id, "point": point}))
}

pub(super) fn update_link(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.updateLink";
    let (page, m) = target(s, cmd, p)?;
    let lid = req_id(cmd, p, "id")?;
    let Item::Link(i) = find_or(cmd, &m, lid)? else { return Err(bad(cmd, format!("{lid} isn't a relationship"))) };
    let mut c = (*m).clone();
    let points = ids(cmd, p, "points", uc::MAX_EXTEND_POINTS)?;
    let condition = clearable(cmd, p, "condition")?;
    let (am, um) = (clearable(cmd, p, "actorMultiplicity")?, clearable(cmd, p, "useCaseMultiplicity")?);
    match c.links.get_mut(i) {
        Some(Link::Extend { points: pts, condition: cond, .. }) => {
            if am.is_some() || um.is_some() {
                return Err(bad(cmd, "an extend has no multiplicities"));
            }
            if let Some(v) = points {
                *pts = v.into_iter().map(PointId).collect();
            }
            if let Some(v) = condition {
                *cond = v;
            }
        }
        Some(Link::Association { actor_multiplicity, use_case_multiplicity, .. }) => {
            if points.is_some() || condition.is_some() {
                return Err(bad(cmd, "an association has no extension points or condition"));
            }
            if let Some(v) = am {
                *actor_multiplicity = v;
            }
            if let Some(v) = um {
                *use_case_multiplicity = v;
            }
        }
        _ => {
            if points.is_some() || condition.is_some() || am.is_some() || um.is_some() {
                return Err(bad(cmd, "this relationship has nothing to change"));
            }
        }
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": lid, "changed": changed}))
}

pub(super) fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.remove";
    let (page, m) = target(s, cmd, p)?;
    let list = ids(cmd, p, "ids", uc::MAX_ELEMENTS + uc::MAX_LINKS)?.ok_or_else(|| bad(cmd, "`ids` is required"))?;
    for i in &list {
        find_or(cmd, &m, *i)?;
    }
    let gone: std::collections::HashSet<u64> = list.into_iter().collect();
    let mut c = (*m).clone();
    c.actors.retain(|a| !gone.contains(&a.id.0));
    c.use_cases.retain(|u| !gone.contains(&u.id.0));
    c.subjects.retain(|x| !gone.contains(&x.id.0));
    // A removed subject's use cases stay, outside any subject.
    for u in &mut c.use_cases {
        if u.subject.is_some_and(|x| gone.contains(&x.0)) {
            u.subject = None;
        }
    }
    // A removed element takes its relationships with it.
    let alive: std::collections::HashSet<u64> = c.actors.iter().map(|a| a.id.0).chain(c.use_cases.iter().map(|u| u.id.0)).collect();
    c.links.retain(|l| {
        let (a, b) = link_ends(l);
        !gone.contains(&l.id().0) && alive.contains(&a) && alive.contains(&b)
    });
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"removed": gone.len()}))
}

pub(super) fn link_ends(l: &Link) -> (u64, u64) {
    match l {
        Link::Association { actor, use_case, .. } => (actor.0, use_case.0),
        Link::Include { including, included, .. } => (including.0, included.0),
        Link::Extend { extending, base, .. } => (extending.0, base.0),
        Link::Generalization { ends: GenEnds::Actors { specific, general }, .. } => (specific.0, general.0),
        Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => (specific.0, general.0),
    }
}

pub(super) fn check(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.check";
    let (_, m) = target(s, cmd, p)?;
    let warnings: Vec<Value> = uc::check(&m)
        .iter()
        .map(|w| {
            let id = match w {
                uc::Warning::Unassigned(u) | uc::Warning::UseCaseUnreached(u) => u.0,
                uc::Warning::EmptySubject(x) => x.0,
                uc::Warning::ActorUnconnected(a) => a.0,
            };
            json!({"id": id, "message": w.message()})
        })
        .collect();
    Ok(json!({"warnings": warnings}))
}

pub(super) fn read(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.read";
    let (page, m) = target(s, cmd, p)?;
    let model = serde_json::to_value(&*m).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(json!({"page": page, "schema": uc::USE_CASE_SCHEMA, "model": model}))
}
