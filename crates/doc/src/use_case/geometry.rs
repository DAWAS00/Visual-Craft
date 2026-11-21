//! The geometry rules: size ranges, boxes, subjects holding their use cases, and spacing.

use super::{ELEMENT_GAP, Model, Presentation, SUBJECT_PADDING, SubjectId, UseCase, UseCaseError};
/// A box in page units.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Box2 {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Box2 {
    fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x0: x, y0: y, x1: x + w, y1: y + h }
    }
    fn inflate(self, d: f64) -> Self {
        Self { x0: self.x0 - d, y0: self.y0 - d, x1: self.x1 + d, y1: self.y1 + d }
    }
    /// Overlap with positive area (touching edges don't count).
    fn overlaps(self, o: Self) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
    fn within(self, o: Self) -> bool {
        self.x0 >= o.x0 && self.y0 >= o.y0 && self.x1 <= o.x1 && self.y1 <= o.y1
    }
}

/// Size ranges, as (min w, max w, min h, max h).
pub(crate) const PERSON_SIZE: (f64, f64, f64, f64) = (40.0, 200.0, 96.0, 400.0);
pub(crate) const SYSTEM_SIZE: (f64, f64, f64, f64) = (80.0, 400.0, 48.0, 400.0);
pub(crate) const USE_CASE_SIZE: (f64, f64, f64, f64) = (120.0, 2_000.0, 56.0, 2_000.0);
pub(crate) const SUBJECT_SIZE: (f64, f64, f64, f64) = (80.0, 20_000.0, 80.0, 20_000.0);

/// The size range an actor with this presentation may have.
pub fn actor_size_range(p: Presentation) -> (f64, f64, f64, f64) {
    match p {
        Presentation::Person => PERSON_SIZE,
        Presentation::System => SYSTEM_SIZE,
    }
}


pub(crate) fn geometry(m: &Model) -> Result<(), UseCaseError> {
    let subjects: Vec<(SubjectId, Box2)> = m.subjects.iter().map(|s| (s.id, Box2::new(s.x, s.y, s.width, s.height))).collect();
    for (i, (a, ra)) in subjects.iter().enumerate() {
        if let Some((b, _)) = subjects.iter().skip(i + 1).find(|(_, rb)| ra.overlaps(*rb)) {
            return Err(UseCaseError::SubjectsOverlap(*a, *b));
        }
    }
    let mut boxes: Vec<(u64, Box2)> = Vec::with_capacity(m.actors.len() + m.use_cases.len());
    for a in &m.actors {
        let r = Box2::new(a.x, a.y, a.w, a.h);
        outside_subjects("actor", a.id.0, r, &subjects)?;
        boxes.push((a.id.0, r));
    }
    for u in &m.use_cases {
        let r = Box2::new(u.x, u.y, u.w, u.h);
        match u.subject {
            Some(s) => inside_subject(u, r, &subjects, s)?,
            None => outside_subjects("use case", u.id.0, r, &subjects)?,
        }
        boxes.push((u.id.0, r));
    }
    separated(&boxes)
}

fn inside_subject(u: &UseCase, r: Box2, subjects: &[(SubjectId, Box2)], s: SubjectId) -> Result<(), UseCaseError> {
    let home = subjects.iter().find(|(id, _)| *id == s).map(|(_, b)| *b).ok_or(UseCaseError::Missing { what: "subject", id: s.0 })?;
    // Subjects don't overlap (checked first), so a member padded inside its own is clear of the
    // others.
    if r.inflate(SUBJECT_PADDING).within(home) { Ok(()) } else { Err(UseCaseError::OutsideSubject { use_case: u.id, subject: s }) }
}

fn outside_subjects(what: &'static str, id: u64, r: Box2, subjects: &[(SubjectId, Box2)]) -> Result<(), UseCaseError> {
    let r = r.inflate(SUBJECT_PADDING);
    match subjects.iter().find(|(_, s)| r.overlaps(*s)) {
        Some((subject, _)) => Err(UseCaseError::InsideSubject { what, id, subject: *subject }),
        None => Ok(()),
    }
}

/// Every pair of actor and use case boxes is at least [`ELEMENT_GAP`] apart (the shortest distance
/// between the boxes, so a diagonal neighbour may be closer on each axis). Sorted by left edge,
/// so each box is compared only with boxes that start less than the gap past its right edge.
fn separated(boxes: &[(u64, Box2)]) -> Result<(), UseCaseError> {
    let mut sorted: Vec<(u64, Box2)> = boxes.to_vec();
    sorted.sort_by(|a, b| a.1.x0.total_cmp(&b.1.x0).then(a.0.cmp(&b.0)));
    for (i, (a, ra)) in sorted.iter().enumerate() {
        for (b, rb) in sorted.iter().skip(i + 1) {
            if rb.x0 - ra.x1 >= ELEMENT_GAP {
                break;
            }
            if distance_squared(*ra, *rb) < ELEMENT_GAP * ELEMENT_GAP {
                return Err(UseCaseError::TooClose { a: (*a).min(*b), b: (*a).max(*b) });
            }
        }
    }
    Ok(())
}

/// The squared shortest distance between two boxes (0 when they touch or overlap).
fn distance_squared(a: Box2, b: Box2) -> f64 {
    let dx = (b.x0 - a.x1).max(a.x0 - b.x1).max(0.0);
    let dy = (b.y0 - a.y1).max(a.y0 - b.y1).max(0.0);
    dx * dx + dy * dy
}
