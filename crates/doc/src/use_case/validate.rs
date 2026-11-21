//! The one validator every Use Case candidate and loaded file passes through.

use super::{
    ActorId, COORD_LIMIT, ELEMENT_GAP, GenEnds, Link, LinkId, MAX_BOUND_DIGITS, MAX_CONDITION, MAX_ELEMENTS, MAX_EXTEND_POINTS, MAX_LINKS, MAX_NAME,
    MAX_POINT_NAME, MAX_POINTS, Model, PointId, SUBJECT_SIZE, SubjectId, USE_CASE_SCHEMA, USE_CASE_SIZE, UseCase, UseCaseId, actor_size_range,
    geometry,
};
use std::collections::{HashMap, HashSet};

/// The relationship kinds that must stay acyclic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graph {
    Include,
    Extend,
    ActorGeneralization,
    UseCaseGeneralization,
}

impl std::fmt::Display for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Graph::Include => "include",
            Graph::Extend => "extend",
            Graph::ActorGeneralization => "actor generalization",
            Graph::UseCaseGeneralization => "use case generalization",
        })
    }
}

/// Why a model is not valid. Each message says what to change.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UseCaseError {
    #[error("the diagram was written with model schema {0}; this version reads schema {USE_CASE_SCHEMA} only")]
    UnsupportedSchema(u32),
    #[error("a page holds at most {MAX_ELEMENTS} actors, use cases and subjects")]
    TooManyElements,
    #[error("a page holds at most {MAX_LINKS} relationships")]
    TooManyLinks,
    #[error("ID {0} is used twice")]
    DuplicateId(u64),
    #[error("{what} {id}: a name is 1 to {max} characters, without spaces at either end")]
    BadName { what: &'static str, id: u64, max: usize },
    #[error("use case {use_case} has two extension points called \"{name}\"")]
    DuplicatePointName { use_case: UseCaseId, name: String },
    #[error("use case {0} has more than {MAX_POINTS} extension points")]
    TooManyPoints(UseCaseId),
    #[error("{what} {id}: a coordinate or size is not a finite number within ±{COORD_LIMIT}")]
    BadNumber { what: &'static str, id: u64 },
    #[error("{what} {id}: its size is outside the allowed range")]
    BadSize { what: &'static str, id: u64 },
    #[error("{what} {id} is smaller than its text needs")]
    BelowMinimum { what: &'static str, id: u64 },
    #[error("there is no {what} {id}")]
    Missing { what: &'static str, id: u64 },
    #[error("relationship {0} connects an element to itself")]
    SelfLink(LinkId),
    #[error("relationship {0} repeats another one between the same elements")]
    DuplicateLink(LinkId),
    #[error("the {graph} relationships would form a cycle through {at}")]
    Cycle { graph: Graph, at: u64 },
    #[error("extend {link} must name 1 to {MAX_EXTEND_POINTS} different extension points")]
    BadExtendPoints { link: LinkId },
    #[error("extension point {point} isn't on the base use case {base}")]
    PointNotOnBase { point: PointId, base: UseCaseId },
    #[error("relationship {link}: {text:?} isn't a multiplicity (use 1, 0..1, *, 1..* or m..n)")]
    BadMultiplicity {
        link: LinkId,
        /// The start of the rejected text (at most 24 characters).
        text: String,
    },
    #[error("extend {0}: a condition is at most {MAX_CONDITION} characters")]
    BadCondition(LinkId),
    #[error("use case {use_case} must stay inside its subject {subject}")]
    OutsideSubject { use_case: UseCaseId, subject: SubjectId },
    #[error("{what} {id} overlaps subject {subject}: only member use cases go inside a subject")]
    InsideSubject { what: &'static str, id: u64, subject: SubjectId },
    #[error("{a} and {b} are closer than {ELEMENT_GAP} units")]
    TooClose { a: u64, b: u64 },
    #[error("subjects {0} and {1} overlap")]
    SubjectsOverlap(SubjectId, SubjectId),
}

/// Checks a whole model: limits, IDs, names, numbers and sizes, references, relationship rules,
/// acyclicity and the geometry rules. The first problem found is returned.
pub fn validate(m: &Model) -> Result<(), UseCaseError> {
    if m.schema != USE_CASE_SCHEMA {
        return Err(UseCaseError::UnsupportedSchema(m.schema));
    }
    if m.actors.len() + m.use_cases.len() + m.subjects.len() > MAX_ELEMENTS {
        return Err(UseCaseError::TooManyElements);
    }
    if m.links.len() > MAX_LINKS {
        return Err(UseCaseError::TooManyLinks);
    }
    // The nested lists too, before anything walks them, so hostile input costs no more than the
    // limits allow.
    if let Some(u) = m.use_cases.iter().find(|u| u.points.len() > MAX_POINTS) {
        return Err(UseCaseError::TooManyPoints(u.id));
    }
    if let Some(l) = m.links.iter().find(|l| matches!(l, Link::Extend { points, .. } if points.len() > MAX_EXTEND_POINTS)) {
        return Err(UseCaseError::BadExtendPoints { link: l.id() });
    }
    unique_ids(m)?;
    elements(m)?;
    links(m)?;
    acyclic(m)?;
    geometry(m)
}

fn unique_ids(m: &Model) -> Result<(), UseCaseError> {
    let mut seen = HashSet::new();
    let points = m.use_cases.iter().flat_map(|u| u.points.iter().map(|p| p.id.0));
    let all = m
        .actors
        .iter()
        .map(|a| a.id.0)
        .chain(m.use_cases.iter().map(|u| u.id.0))
        .chain(m.subjects.iter().map(|s| s.id.0))
        .chain(m.links.iter().map(|l| l.id().0))
        .chain(points);
    for id in all {
        if !seen.insert(id) {
            return Err(UseCaseError::DuplicateId(id));
        }
    }
    Ok(())
}

/// `text` has at most `max` characters (counting stops one past the limit).
fn at_most_chars(text: &str, max: usize) -> bool {
    text.chars().take(max.saturating_add(1)).count() <= max
}

fn name_ok(name: &str, max: usize) -> bool {
    !name.is_empty() && at_most_chars(name, max) && name.trim() == name
}

/// The key two extension point names share when they differ only in case: the name in lower case
/// (`str::to_lowercase`). This is FigCraft's rule, not full Unicode case folding: it never joins
/// two letters that have different lower case forms (`I` and `ı`, `σ` and `ς`, `ß` and `ss` stay
/// distinct): extension point names on one use case must have distinct `str::to_lowercase()`
/// results, and nothing more.
fn case_key(name: &str) -> String {
    name.to_lowercase()
}

fn check_name(name: &str, what: &'static str, id: u64, max: usize) -> Result<(), UseCaseError> {
    if name_ok(name, max) { Ok(()) } else { Err(UseCaseError::BadName { what, id, max }) }
}

fn number_ok(v: f64) -> bool {
    v.is_finite() && v.abs() <= COORD_LIMIT
}

/// Position, size and minimum are finite, the box stays within the coordinate limits, both sizes
/// are in `range`, and the size is at least the minimum.
fn check_box(
    what: &'static str,
    id: u64,
    (x, y, w, h): (f64, f64, f64, f64),
    min: Option<(f64, f64)>,
    range: (f64, f64, f64, f64),
) -> Result<(), UseCaseError> {
    let (min_w, min_h) = min.unwrap_or((range.0, range.2));
    if ![x, y, w, h, min_w, min_h, x + w, y + h].into_iter().all(number_ok) {
        return Err(UseCaseError::BadNumber { what, id });
    }
    let fits = |v: f64, lo: f64, hi: f64| v >= lo && v <= hi;
    let (lo_w, hi_w, lo_h, hi_h) = range;
    if !(fits(w, lo_w, hi_w) && fits(h, lo_h, hi_h) && fits(min_w, lo_w, hi_w) && fits(min_h, lo_h, hi_h)) {
        return Err(UseCaseError::BadSize { what, id });
    }
    if w < min_w || h < min_h {
        return Err(UseCaseError::BelowMinimum { what, id });
    }
    Ok(())
}

fn elements(m: &Model) -> Result<(), UseCaseError> {
    for a in &m.actors {
        check_name(&a.name, "actor", a.id.0, MAX_NAME)?;
        check_box("actor", a.id.0, (a.x, a.y, a.w, a.h), Some((a.min_w, a.min_h)), actor_size_range(a.presentation))?;
    }
    let subjects: HashSet<SubjectId> = m.subjects.iter().map(|s| s.id).collect();
    for s in &m.subjects {
        check_name(&s.name, "subject", s.id.0, MAX_NAME)?;
        check_box("subject", s.id.0, (s.x, s.y, s.width, s.height), None, SUBJECT_SIZE)?;
    }
    for u in &m.use_cases {
        check_name(&u.name, "use case", u.id.0, MAX_NAME)?;
        check_box("use case", u.id.0, (u.x, u.y, u.w, u.h), Some((u.min_w, u.min_h)), USE_CASE_SIZE)?;
        if let Some(s) = u.subject.filter(|s| !subjects.contains(s)) {
            return Err(UseCaseError::Missing { what: "subject", id: s.0 });
        }
        points(u)?;
    }
    Ok(())
}

fn points(u: &UseCase) -> Result<(), UseCaseError> {
    let mut names = HashSet::new();
    for p in &u.points {
        check_name(&p.name, "extension point", p.id.0, MAX_POINT_NAME)?;
        if !names.insert(case_key(&p.name)) {
            return Err(UseCaseError::DuplicatePointName { use_case: u.id, name: p.name.clone() });
        }
    }
    Ok(())
}

/// `1`, `*`, `n`, `m..n` or `m..*` with 0 ≤ m ≤ n and at most nine digits per bound.
pub fn multiplicity_ok(text: &str) -> bool {
    // The longest valid form is two nine-digit bounds and "..".
    if text.len() > 2 * MAX_BOUND_DIGITS + 2 {
        return false;
    }
    let bound = |s: &str| -> Option<u64> {
        if s.is_empty() || s.len() > MAX_BOUND_DIGITS || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    match text.split_once("..") {
        None => text == "*" || bound(text).is_some(),
        Some((lo, "*")) => bound(lo).is_some(),
        Some((lo, hi)) => matches!((bound(lo), bound(hi)), (Some(a), Some(b)) if a <= b),
    }
}

/// The IDs relationships may point at, by kind.
struct Index<'a> {
    actors: HashSet<ActorId>,
    use_cases: HashMap<UseCaseId, &'a UseCase>,
}

impl<'a> Index<'a> {
    fn new(m: &'a Model) -> Self {
        Self { actors: m.actors.iter().map(|a| a.id).collect(), use_cases: m.use_cases.iter().map(|u| (u.id, u)).collect() }
    }
    fn actor(&self, id: ActorId) -> Result<(), UseCaseError> {
        if self.actors.contains(&id) { Ok(()) } else { Err(UseCaseError::Missing { what: "actor", id: id.0 }) }
    }
    fn use_case(&self, id: UseCaseId) -> Result<&'a UseCase, UseCaseError> {
        self.use_cases.get(&id).copied().ok_or(UseCaseError::Missing { what: "use case", id: id.0 })
    }
}

/// A relationship's identity for duplicate detection: its kind and ends (ordered where it is
/// directed, canonical actor-then-case for an association).
pub(crate) fn link_key(l: &Link) -> (u8, u64, u64) {
    match l {
        Link::Association { actor, use_case, .. } => (0, actor.0, use_case.0),
        Link::Include { including, included, .. } => (1, including.0, included.0),
        Link::Extend { extending, base, .. } => (2, extending.0, base.0),
        Link::Generalization { ends: GenEnds::Actors { specific, general }, .. } => (3, specific.0, general.0),
        Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => (4, specific.0, general.0),
    }
}

fn links(m: &Model) -> Result<(), UseCaseError> {
    let ix = Index::new(m);
    let mut keys = HashSet::new();
    for l in &m.links {
        link(&ix, l)?;
        if !keys.insert(link_key(l)) {
            return Err(UseCaseError::DuplicateLink(l.id()));
        }
    }
    Ok(())
}

fn link(ix: &Index<'_>, l: &Link) -> Result<(), UseCaseError> {
    let id = l.id();
    let (_, a, b) = link_key(l);
    if a == b && !matches!(l, Link::Association { .. }) {
        return Err(UseCaseError::SelfLink(id));
    }
    match l {
        Link::Association { actor, use_case, actor_multiplicity, use_case_multiplicity, .. } => {
            ix.actor(*actor)?;
            ix.use_case(*use_case)?;
            for text in [actor_multiplicity, use_case_multiplicity].into_iter().flatten() {
                if !multiplicity_ok(text) {
                    return Err(UseCaseError::BadMultiplicity { link: id, text: text.chars().take(24).collect() });
                }
            }
        }
        Link::Include { including, included, .. } => {
            ix.use_case(*including)?;
            ix.use_case(*included)?;
        }
        Link::Extend { extending, base, points, condition, .. } => {
            ix.use_case(*extending)?;
            extend_points(id, ix.use_case(*base)?, points)?;
            if condition.as_ref().is_some_and(|c| !at_most_chars(c, MAX_CONDITION)) {
                return Err(UseCaseError::BadCondition(id));
            }
        }
        Link::Generalization { ends: GenEnds::Actors { specific, general }, .. } => {
            ix.actor(*specific)?;
            ix.actor(*general)?;
        }
        Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. } => {
            ix.use_case(*specific)?;
            ix.use_case(*general)?;
        }
    }
    Ok(())
}

fn extend_points(link: LinkId, base: &UseCase, points: &[PointId]) -> Result<(), UseCaseError> {
    let distinct: HashSet<PointId> = points.iter().copied().collect();
    if points.is_empty() || points.len() > MAX_EXTEND_POINTS || distinct.len() != points.len() {
        return Err(UseCaseError::BadExtendPoints { link });
    }
    match points.iter().find(|p| !base.points.iter().any(|q| q.id == **p)) {
        Some(p) => Err(UseCaseError::PointNotOnBase { point: *p, base: base.id }),
        None => Ok(()),
    }
}

/// The directed edges of one acyclic graph.
pub(crate) fn edges(m: &Model, graph: Graph) -> Vec<(u64, u64)> {
    m.links
        .iter()
        .filter_map(|l| match (graph, l) {
            (Graph::Include, Link::Include { including, included, .. }) => Some((including.0, included.0)),
            (Graph::Extend, Link::Extend { extending, base, .. }) => Some((extending.0, base.0)),
            (Graph::ActorGeneralization, Link::Generalization { ends: GenEnds::Actors { specific, general }, .. }) => Some((specific.0, general.0)),
            (Graph::UseCaseGeneralization, Link::Generalization { ends: GenEnds::UseCases { specific, general }, .. }) => {
                Some((specific.0, general.0))
            }
            _ => None,
        })
        .collect()
}

fn acyclic(m: &Model) -> Result<(), UseCaseError> {
    for graph in [Graph::Include, Graph::Extend, Graph::ActorGeneralization, Graph::UseCaseGeneralization] {
        if let Some(at) = find_cycle(&edges(m, graph)) {
            return Err(UseCaseError::Cycle { graph, at });
        }
    }
    Ok(())
}

/// A node on a cycle, if the edges have one. Iterative depth-first search, so a long chain
/// can't exhaust the stack.
pub(crate) fn find_cycle(edges: &[(u64, u64)]) -> Option<u64> {
    let mut next: HashMap<u64, Vec<u64>> = HashMap::new();
    for &(a, b) in edges {
        next.entry(a).or_default().push(b);
    }
    // Absent: not visited; false: on the current path; true: finished.
    let mut done: HashMap<u64, bool> = HashMap::new();
    let mut starts: Vec<u64> = next.keys().copied().collect();
    starts.sort_unstable();
    for start in starts {
        if done.contains_key(&start) {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        done.insert(start, false);
        while let Some((node, i)) = stack.pop() {
            let child = next.get(&node).and_then(|c| c.get(i)).copied();
            let Some(child) = child else {
                done.insert(node, true);
                continue;
            };
            stack.push((node, i + 1));
            match done.get(&child) {
                Some(false) => return Some(child),
                Some(true) => {}
                None => {
                    done.insert(child, false);
                    stack.push((child, 0));
                }
            }
        }
    }
    None
}
