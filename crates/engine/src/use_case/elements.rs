//! Element commands: actors, use cases, subjects and extension points.

use figcraft_doc::use_case::{self as uc, Actor, ActorId, ExtensionPoint, Link, Model, PointId, Subject, SubjectId, UseCase, UseCaseId};
use serde_json::{Value, json};

use super::params::{id, number, presentation, req_id, req_number, req_text, text};
use super::{
    Item, STEP, SUBJECT_H, SUBJECT_W, add_at, commit, find_or, fresh, measure, place, point_index, position, remeasure_actor, remeasure_case,
    right_edge, target, use_case_index,
};
use crate::error::bad;
use crate::{Result, Session};

pub(super) fn add_subject(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.addSubject";
    let (page, m) = target(s, cmd, p)?;
    let name = req_text(cmd, p, "name")?;
    let (w, h) = (number(cmd, p, "width")?.unwrap_or(SUBJECT_W), number(cmd, p, "height")?.unwrap_or(SUBJECT_H));
    let id = fresh(cmd, &m)?;
    let start = (right_edge(&m) + if m.subjects.is_empty() && m.actors.is_empty() { 200.0 } else { 80.0 }, 40.0);
    let after = add_at(cmd, &m, position(cmd, p)?, start, 20, 10, |c, x, y| {
        c.subjects.push(Subject { id: SubjectId(id), name: name.clone(), x, y, width: w, height: h });
    })?;
    commit(s, cmd, page, &m, after)?;
    Ok(json!({"id": id}))
}

pub(super) fn add_actor(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.addActor";
    let (page, m) = target(s, cmd, p)?;
    let name = req_text(cmd, p, "name")?;
    let presentation = presentation(cmd, p)?.unwrap_or_default();
    let id = fresh(cmd, &m)?;
    let (w, h) = measure::actor_min(&name, presentation);
    // Left of the first subject, or at the left of the page.
    let start = m.subjects.first().map_or((40.0, 40.0), |s| (s.x - w - 80.0, s.y + 40.0));
    let after = add_at(cmd, &m, position(cmd, p)?, start, 4, 20, |c, x, y| {
        c.actors.push(Actor { id: ActorId(id), name: name.clone(), presentation, x, y, w, h, min_w: w, min_h: h });
    })?;
    commit(s, cmd, page, &m, after)?;
    Ok(json!({"id": id}))
}

/// Where to start looking for room for a new use case: inside `subject`, or right of everything.
pub(super) fn case_start(m: &Model, subject: Option<SubjectId>) -> ((f64, f64), usize, usize) {
    match subject.and_then(|s| m.subjects.iter().find(|x| x.id == s)) {
        Some(s) => ((s.x + 24.0, s.y + 48.0), ((s.width / STEP) as usize).max(1), ((s.height / STEP) as usize).max(1)),
        None => ((right_edge(m) + 80.0, 40.0), 10, 20),
    }
}

pub(super) fn subject_param(cmd: &str, m: &Model, p: &Value) -> Result<Option<SubjectId>> {
    match id(cmd, p, "subject")? {
        None => Ok(None),
        Some(sid) if m.subjects.iter().any(|s| s.id.0 == sid) => Ok(Some(SubjectId(sid))),
        Some(sid) => Err(bad(cmd, format!("{sid} isn't a subject on this diagram"))),
    }
}

pub(super) fn add_use_case(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.addUseCase";
    let (page, m) = target(s, cmd, p)?;
    let name = req_text(cmd, p, "name")?;
    let subject = subject_param(cmd, &m, p)?;
    let id = fresh(cmd, &m)?;
    let after = new_case(cmd, &m, id, &name, subject, position(cmd, p)?)?;
    commit(s, cmd, page, &m, after)?;
    Ok(json!({"id": id}))
}

pub(super) fn new_case(cmd: &str, m: &Model, id: u64, name: &str, subject: Option<SubjectId>, at: Option<(f64, f64)>) -> Result<Model> {
    let (w, h) = measure::use_case_min(name, []);
    let (start, cols, rows) = case_start(m, subject);
    add_at(cmd, m, at, start, cols, rows, |c, x, y| {
        c.use_cases.push(UseCase { id: UseCaseId(id), name: name.to_string(), x, y, w, h, min_w: w, min_h: h, subject, points: Vec::new() });
    })
}

pub(super) fn update_element(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.updateElement";
    let (page, m) = target(s, cmd, p)?;
    let target_id = req_id(cmd, p, "id")?;
    let name = text(cmd, p, "name")?;
    let pres = presentation(cmd, p)?;
    let mut c = (*m).clone();
    match find_or(cmd, &m, target_id)? {
        Item::Actor(i) => {
            if let Some(a) = c.actors.get_mut(i) {
                let changed = name.as_ref().is_some_and(|n| *n != a.name) || pres.is_some_and(|p| p != a.presentation);
                if changed {
                    a.name = name.unwrap_or_else(|| a.name.clone());
                    a.presentation = pres.unwrap_or(a.presentation);
                    remeasure_actor(a);
                }
            }
        }
        Item::UseCase(i) => {
            if pres.is_some() {
                return Err(bad(cmd, "only an actor has a `presentation`"));
            }
            if let Some(u) = c.use_cases.get_mut(i)
                && let Some(n) = name.filter(|n| *n != u.name)
            {
                u.name = n;
                remeasure_case(u);
            }
        }
        Item::Subject(i) => {
            if pres.is_some() {
                return Err(bad(cmd, "only an actor has a `presentation`"));
            }
            if let (Some(x), Some(n)) = (c.subjects.get_mut(i), name) {
                x.name = n;
            }
        }
        Item::Link(_) => return Err(bad(cmd, "use diagram.useCase.updateLink for a relationship")),
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": target_id, "changed": changed}))
}

pub(super) fn move_element(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.move";
    let (page, m) = target(s, cmd, p)?;
    let target_id = req_id(cmd, p, "id")?;
    let (x, y) = (req_number(cmd, p, "x")?, req_number(cmd, p, "y")?);
    let mut c = (*m).clone();
    match find_or(cmd, &m, target_id)? {
        Item::Actor(i) => {
            if let Some(a) = c.actors.get_mut(i) {
                (a.x, a.y) = (x, y);
            }
        }
        Item::UseCase(i) => {
            if let Some(u) = c.use_cases.get_mut(i) {
                (u.x, u.y) = (x, y);
            }
        }
        Item::Subject(i) => {
            let Some(sub) = c.subjects.get_mut(i) else { return Err(bad(cmd, "no such subject")) };
            let (dx, dy) = (x - sub.x, y - sub.y);
            (sub.x, sub.y) = (x, y);
            let sid = sub.id;
            // Its use cases move with it.
            for u in c.use_cases.iter_mut().filter(|u| u.subject == Some(sid)) {
                (u.x, u.y) = (u.x + dx, u.y + dy);
            }
        }
        Item::Link(_) => return Err(bad(cmd, "a relationship follows its elements; move them instead")),
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": target_id, "changed": changed}))
}

pub(super) fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.resize";
    let (page, m) = target(s, cmd, p)?;
    let target_id = req_id(cmd, p, "id")?;
    let (w, h) = (req_number(cmd, p, "width")?, req_number(cmd, p, "height")?);
    let at = position(cmd, p)?;
    let mut c = (*m).clone();
    match find_or(cmd, &m, target_id)? {
        // A subject's members stay where they are.
        Item::Subject(i) => {
            if let Some(x) = c.subjects.get_mut(i) {
                (x.width, x.height) = (w, h);
                if let Some((nx, ny)) = at {
                    (x.x, x.y) = (nx, ny);
                }
            }
        }
        Item::UseCase(i) => {
            if let Some(u) = c.use_cases.get_mut(i) {
                (u.w, u.h) = (w, h);
                if let Some((nx, ny)) = at {
                    (u.x, u.y) = (nx, ny);
                }
            }
        }
        _ => return Err(bad(cmd, "only subjects and use cases are resized")),
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": target_id, "changed": changed}))
}

pub(super) fn set_subject(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.setSubject";
    let (page, m) = target(s, cmd, p)?;
    let case = req_id(cmd, p, "useCase")?;
    if p.get("subject").is_none() {
        return Err(bad(cmd, "`subject` is required (a subject's ID, or null to take the use case out)"));
    }
    let subject = subject_param(cmd, &m, p)?;
    let i = use_case_index(cmd, &m, case)?;
    let at = position(cmd, p)?;
    let mut c = (*m).clone();
    if let Some(u) = c.use_cases.get_mut(i) {
        u.subject = subject;
        if let Some((x, y)) = at {
            (u.x, u.y) = (x, y);
        }
    }
    // Without a position, it stays where it is if that's allowed, else it moves to free room:
    // inside its new subject, or right of everything when taken out of one.
    if at.is_none() && uc::validate(&c).is_err() {
        let (start, cols, rows) = case_start(&c, subject);
        let moved = place(&c, start, cols, rows, |c, x, y| {
            if let Some(u) = c.use_cases.get_mut(i) {
                (u.x, u.y) = (x, y);
            }
        });
        c = moved.ok_or_else(|| bad(cmd, "there's no free room for it: give `x` and `y`, or make room first"))?;
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": case, "changed": changed}))
}

pub(super) fn add_point(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.addExtensionPoint";
    let (page, m) = target(s, cmd, p)?;
    let i = use_case_index(cmd, &m, req_id(cmd, p, "useCase")?)?;
    let name = req_text(cmd, p, "name")?;
    let mut c = (*m).clone();
    let id = with_new_point(cmd, &mut c, i, &name)?;
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": id}))
}

/// Adds extension point `name` at the end of use case `i`'s list.
pub(super) fn with_new_point(cmd: &str, c: &mut Model, i: usize, name: &str) -> Result<u64> {
    let id = fresh(cmd, c)?;
    let u = c.use_cases.get_mut(i).ok_or_else(|| bad(cmd, "no such use case"))?;
    u.points.push(ExtensionPoint { id: PointId(id), name: name.to_string() });
    remeasure_case(u);
    Ok(id)
}

pub(super) fn update_point(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.updateExtensionPoint";
    let (page, m) = target(s, cmd, p)?;
    let pid = req_id(cmd, p, "id")?;
    let name = req_text(cmd, p, "name")?;
    let (u, i) = point_index(cmd, &m, pid)?;
    let mut c = (*m).clone();
    if let Some(case) = c.use_cases.get_mut(u)
        && case.points.get(i).is_some_and(|x| x.name != name)
    {
        if let Some(point) = case.points.get_mut(i) {
            point.name = name;
        }
        remeasure_case(case);
    }
    let changed = commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": pid, "changed": changed}))
}

pub(super) fn remove_point(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "diagram.useCase.removeExtensionPoint";
    let (page, m) = target(s, cmd, p)?;
    let pid = req_id(cmd, p, "id")?;
    if m.links.iter().any(|l| matches!(l, Link::Extend { points, .. } if points.contains(&PointId(pid)))) {
        return Err(bad(cmd, "an extend relationship uses this extension point: change or remove it first"));
    }
    let (u, i) = point_index(cmd, &m, pid)?;
    let mut c = (*m).clone();
    if let Some(case) = c.use_cases.get_mut(u) {
        case.points.remove(i);
        remeasure_case(case);
    }
    commit(s, cmd, page, &m, c)?;
    Ok(json!({"id": pid}))
}
