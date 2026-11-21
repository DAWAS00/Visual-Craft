//! The read-only drafting check, and which relationship kinds can join two elements.

use super::{ActorId, GenEnds, Graph, Link, Model, SubjectId, UseCaseId, edges, link_key};
use std::collections::{HashMap, HashSet};

/// Something unfinished in a valid model. Never stops an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The use case belongs to no subject.
    Unassigned(UseCaseId),
    /// The subject holds no use case.
    EmptySubject(SubjectId),
    /// The actor has no association, directly or through a more general actor.
    ActorUnconnected(ActorId),
    /// No actor reaches the use case.
    UseCaseUnreached(UseCaseId),
}

impl Warning {
    /// A message for the drafting list.
    pub fn message(&self) -> String {
        match self {
            Warning::Unassigned(u) => format!("use case {u} isn't inside a subject"),
            Warning::EmptySubject(s) => format!("subject {s} has no use cases"),
            Warning::ActorUnconnected(a) => format!("actor {a} takes part in no use case"),
            Warning::UseCaseUnreached(u) => format!("no actor reaches use case {u}"),
        }
    }
}

/// The relationship kinds that can join two elements, named as the engine names them
/// (`association`, `include`, `extend`, `generalization`). An actor joins a use case by an
/// association, two use cases by an include, an extend or a generalization, and two actors by a
/// generalization. A kind is left out when that link already exists or would close a cycle,
/// as the validator refuses both. Empty for the same element twice, a system, or an unknown ID.
pub fn connection_kinds(m: &Model, from: u64, to: u64) -> Vec<&'static str> {
    connection_options(m, from, to).into_iter().filter(|(_, b)| b.is_none()).map(|(k, _)| k).collect()
}

/// Why a kind that fits the two elements still can't join them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// The same relationship is already there.
    AlreadyJoined,
    /// It would close a cycle of includes, extends or generalizations.
    Cycle,
}

/// Every kind that fits the two elements' types, each with what stops it, if anything. `from`
/// is the including, extending or specific end; an association is undirected.
pub fn connection_options(m: &Model, from: u64, to: u64) -> Vec<(&'static str, Option<Blocked>)> {
    // `Some(true)` for an actor, `Some(false)` for a use case, `None` for anything else.
    let side = |id: u64| {
        if m.actors.iter().any(|a| a.id.0 == id) {
            Some(true)
        } else if m.use_cases.iter().any(|u| u.id.0 == id) {
            Some(false)
        } else {
            None
        }
    };
    if from == to {
        return Vec::new();
    }
    let (sf, st) = (side(from), side(to));
    let kinds: &[&'static str] = match (sf, st) {
        (Some(true), Some(false)) | (Some(false), Some(true)) => &["association"],
        (Some(false), Some(false)) => &["include", "extend", "generalization"],
        (Some(true), Some(true)) => &["generalization"],
        _ => &[],
    };
    let keys: HashSet<(u8, u64, u64)> = m.links.iter().map(link_key).collect();
    kinds
        .iter()
        .map(|&kind| {
            let (key, graph) = match kind {
                "association" if sf == Some(true) => ((0, from, to), None),
                "association" => ((0, to, from), None),
                "include" => ((1, from, to), Some(Graph::Include)),
                "extend" => ((2, from, to), Some(Graph::Extend)),
                _ if sf == Some(true) => ((3, from, to), Some(Graph::ActorGeneralization)),
                _ => ((4, from, to), Some(Graph::UseCaseGeneralization)),
            };
            let blocked = if keys.contains(&key) {
                Some(Blocked::AlreadyJoined)
            } else if graph.is_some_and(|g| reaches(&edges(m, g), to, from)) {
                // A new edge from → to closes a cycle exactly when `to` already reaches `from`.
                Some(Blocked::Cycle)
            } else {
                None
            };
            (kind, blocked)
        })
        .collect()
}

/// Whether `start` reaches `target` along the directed edges (iterative, visits each node once).
fn reaches(edges: &[(u64, u64)], start: u64, target: u64) -> bool {
    let mut next: HashMap<u64, Vec<u64>> = HashMap::new();
    for &(a, b) in edges {
        next.entry(a).or_default().push(b);
    }
    let mut seen = HashSet::from([start]);
    let mut todo = vec![start];
    while let Some(n) = todo.pop() {
        if n == target {
            return true;
        }
        for &b in next.get(&n).into_iter().flatten() {
            if seen.insert(b) {
                todo.push(b);
            }
        }
    }
    false
}

/// The drafting check (`diagram.useCase.check`), in model order: unassigned use cases, empty
/// subjects, actors without an association of their own or of a more general actor, and use
/// cases no actor reaches. Reach follows association (actor → case), include (including →
/// included), extend (base → extending) and use case generalization (general → specific).
pub fn check(m: &Model) -> Vec<Warning> {
    let mut out: Vec<Warning> = m.use_cases.iter().filter(|u| u.subject.is_none()).map(|u| Warning::Unassigned(u.id)).collect();
    out.extend(m.subjects.iter().filter(|s| !m.use_cases.iter().any(|u| u.subject == Some(s.id))).map(|s| Warning::EmptySubject(s.id)));
    // An actor is connected when it, or an actor it specialises, has an association: walk from
    // the associated actors down to their specialisations.
    let associated = m.links.iter().filter_map(|l| if let Link::Association { actor, .. } = l { Some(actor.0) } else { None });
    let down: Vec<(u64, u64)> = edges(m, Graph::ActorGeneralization).into_iter().map(|(specific, general)| (general, specific)).collect();
    let connected = reach(associated, &down);
    out.extend(m.actors.iter().filter(|a| !connected.contains(&a.id.0)).map(|a| Warning::ActorUnconnected(a.id)));
    let reached = reached_use_cases(m);
    out.extend(m.use_cases.iter().filter(|u| !reached.contains(&u.id.0)).map(|u| Warning::UseCaseUnreached(u.id)));
    out
}

fn reached_use_cases(m: &Model) -> HashSet<u64> {
    let mut flow = Vec::new();
    let mut starts = Vec::new();
    for l in &m.links {
        match l {
            Link::Association { use_case, .. } => starts.push(use_case.0),
            Link::Include { including, included, .. } => flow.push((including.0, included.0)),
            Link::Extend { extending, base, .. } => flow.push((base.0, extending.0)),
            Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => flow.push((general.0, specific.0)),
            Link::Generalization { ends: GenEnds::Actors { .. }, .. } => {}
        }
    }
    reach(starts, &flow)
}

/// Everything reachable from `starts` along `edges`, the starts included.
fn reach(starts: impl IntoIterator<Item = u64>, edges: &[(u64, u64)]) -> HashSet<u64> {
    let mut next: HashMap<u64, Vec<u64>> = HashMap::new();
    for &(a, b) in edges {
        next.entry(a).or_default().push(b);
    }
    let mut seen: HashSet<u64> = HashSet::new();
    let mut todo: Vec<u64> = starts.into_iter().collect();
    while let Some(n) = todo.pop() {
        if seen.insert(n) {
            todo.extend(next.get(&n).into_iter().flatten().copied());
        }
    }
    seen
}
