//! The one validator every Class diagram candidate passes through, and its layer estimate.

use super::{
    CARD_GAP, CLASS_SCHEMA, COORD_LIMIT, Classifier, ClassifierId, FORBIDDEN, Kind, Link, LinkId, LinkKind, MAX_CLASSIFIERS, MAX_LINKS, MAX_MEMBERS,
    MAX_MEMBERS_PER_CLASSIFIER, MAX_PARAMS, MAX_PARAMS_TOTAL, MAX_TEXT, MemberId, Model, Navigable,
};
use std::collections::{HashMap, HashSet};

/// Why a model is refused. One variant per rule; the first problem found is returned.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ClassError {
    #[error("the diagram was written with model schema {0}; this version reads schema {CLASS_SCHEMA} only")]
    UnsupportedSchema(u32),
    #[error("ID {0} is used more than once")]
    DuplicateId(u64),
    #[error("more than {MAX_CLASSIFIERS} classes, interfaces and enumerations on one page")]
    TooManyClassifiers,
    #[error("more than {MAX_MEMBERS_PER_CLASSIFIER} members in {0}")]
    TooManyMembersIn(ClassifierId),
    #[error("more than {MAX_MEMBERS} members on one page")]
    TooManyMembers,
    #[error("more than {MAX_PARAMS} parameters in operation {0}")]
    TooManyParams(MemberId),
    #[error("more than {MAX_PARAMS_TOTAL} parameters on one page")]
    TooManyParamsTotal,
    #[error("more than {MAX_LINKS} relationships on one page")]
    TooManyLinks,
    #[error("the page would draw more than {MAX_LAYERS} layers")]
    TooManyLayers,
    #[error("{what} of {at} isn't usable: {why}")]
    BadText { at: u64, what: &'static str, why: &'static str },
    #[error("{0} has a position or size outside the page's limits")]
    BadGeometry(ClassifierId),
    #[error("{0} is smaller than its text needs")]
    TooSmall(ClassifierId),
    #[error("{0} and {1} are closer than {CARD_GAP} apart")]
    Overlap(ClassifierId, ClassifierId),
    #[error("{classifier} has two attributes named {name}")]
    DuplicateAttribute { classifier: ClassifierId, name: String },
    #[error("{classifier} has two operations {name} with the same parameter types")]
    DuplicateOperation { classifier: ClassifierId, name: String },
    #[error("{classifier} has two literals named {name}")]
    DuplicateLiteral { classifier: ClassifierId, name: String },
    #[error("only an enumeration has literals ({0} isn't one)")]
    LiteralsOutsideEnumeration(ClassifierId),
    #[error("an enumeration can't be abstract ({0})")]
    AbstractEnumeration(ClassifierId),
    #[error("relationship {link} points at {missing}, which isn't on this diagram")]
    MissingEnd { link: LinkId, missing: ClassifierId },
    #[error("relationship {0} joins a classifier to itself, which its kind doesn't allow")]
    SelfLink(LinkId),
    #[error("relationship {0}: {1}")]
    WrongEnds(LinkId, &'static str),
    #[error("relationship {0} carries a role, multiplicity, navigability or label its kind doesn't have")]
    StrayEndDetail(LinkId),
    #[error("relationship {link}: multiplicity {text} isn't valid")]
    BadMultiplicity { link: LinkId, text: String },
    #[error("relationship {0}: the whole of a composition has multiplicity 1 or 0..1")]
    CompositionWhole(LinkId),
    #[error("relationship {0} repeats another one")]
    DuplicateLink(LinkId),
    #[error("the {0} relationships would form a cycle through {1}")]
    Cycle(&'static str, ClassifierId),
}

/// Whether `c` would break a member onto another line: a control character or a Unicode line or
/// paragraph separator. Shared by the parser, the model's text checks and defaults.
pub fn breaks_line(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

/// The most generated layers one page may have (see [`layer_estimate`]). Below what the other caps
/// alone allow (about 13,600), so a page full of members and relationships is held to it.
pub const MAX_LAYERS: usize = 10_000;

/// Layers per card besides its rows: a group, box, header, name, keyword and three dividers.
pub const CARD_LAYERS: usize = 8;
/// Layers per relationship at most: a group, three shaft segments (a self loop), two markers and
/// four end labels (or one label).
pub const LINK_LAYERS: usize = 10;

/// An upper bound on the layers the renderer generates for `m`: [`CARD_LAYERS`] per card plus one
/// per member row, and [`LINK_LAYERS`] per relationship. The renderer stays within it.
pub fn layer_estimate(m: &Model) -> usize {
    let cards: usize = m.classifiers.iter().map(|c| CARD_LAYERS + c.member_count()).sum();
    cards + m.links.len() * LINK_LAYERS
}

/// Whether `s` is a usable name: not empty, at most [`MAX_TEXT`] characters, no line breaks, no
/// surrounding spaces and none of [`FORBIDDEN`]. `spaces` allows inner spaces (types, roles).
pub(crate) fn text_problem(s: &str, spaces: bool) -> Option<&'static str> {
    if s.is_empty() {
        return Some("it is empty");
    }
    if s.chars().count() > MAX_TEXT {
        return Some("it is too long");
    }
    if s.trim() != s {
        return Some("it starts or ends with a space");
    }
    if s.chars().any(breaks_line) {
        return Some("it has a line break or a control character");
    }
    if !spaces && s.chars().any(char::is_whitespace) {
        return Some("it has a space");
    }
    if s.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Some("it has one of : ( ) , = [ ] \"");
    }
    None
}

/// Whether `s` is a usable default value: a well-formed quoted string, or plain text without
/// quotes or line breaks, at most [`MAX_TEXT`] characters.
pub fn default_problem(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        return Some("it is empty");
    }
    if s.chars().count() > MAX_TEXT {
        return Some("it is too long");
    }
    if s.chars().any(breaks_line) {
        return Some("it has a line break or a control character");
    }
    if s.trim() != s {
        return Some("it starts or ends with a space");
    }
    if let Some(body) = s.strip_prefix('"') {
        let mut chars = body.chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('"' | '\\') => {}
                    _ => return Some("a quoted default only escapes \\\" and \\\\"),
                },
                '"' => {
                    closed = true;
                    break;
                }
                _ => {}
            }
        }
        if !closed || chars.next().is_some() {
            return Some("a quoted default ends with its closing quote");
        }
        return None;
    }
    if s.contains('"') {
        return Some("quotes belong around the whole default");
    }
    None
}

/// Checks every rule. The first problem found is returned; a valid model is `Ok`.
pub fn validate(m: &Model) -> Result<(), ClassError> {
    if m.schema != CLASS_SCHEMA {
        return Err(ClassError::UnsupportedSchema(m.schema));
    }
    if m.classifiers.len() > MAX_CLASSIFIERS {
        return Err(ClassError::TooManyClassifiers);
    }
    if m.links.len() > MAX_LINKS {
        return Err(ClassError::TooManyLinks);
    }
    ids(m)?;
    let (mut members, mut params) = (0usize, 0usize);
    for c in &m.classifiers {
        classifier(c)?;
        members += c.member_count();
        params += c.operations.iter().map(|o| o.params.len()).sum::<usize>();
    }
    if members > MAX_MEMBERS {
        return Err(ClassError::TooManyMembers);
    }
    if params > MAX_PARAMS_TOTAL {
        return Err(ClassError::TooManyParamsTotal);
    }
    if layer_estimate(m) > MAX_LAYERS {
        return Err(ClassError::TooManyLayers);
    }
    spacing(m)?;
    links(m)?;
    acyclic(m)
}

fn ids(m: &Model) -> Result<(), ClassError> {
    let mut seen = HashSet::new();
    let members = m
        .classifiers
        .iter()
        .flat_map(|c| c.attributes.iter().map(|a| a.id.0).chain(c.operations.iter().map(|o| o.id.0)).chain(c.literals.iter().map(|l| l.id.0)));
    for id in m.classifiers.iter().map(|c| c.id.0).chain(m.links.iter().map(|l| l.id.0)).chain(members) {
        if !seen.insert(id) {
            return Err(ClassError::DuplicateId(id));
        }
    }
    Ok(())
}

fn bad(at: u64, what: &'static str, why: &'static str) -> ClassError {
    ClassError::BadText { at, what, why }
}

fn check_text(at: u64, what: &'static str, s: &str, spaces: bool) -> Result<(), ClassError> {
    text_problem(s, spaces).map_or(Ok(()), |why| Err(bad(at, what, why)))
}

fn classifier(c: &Classifier) -> Result<(), ClassError> {
    let id = c.id;
    check_text(id.0, "the name", &c.name, true)?;
    if let Some(s) = &c.stereotype {
        check_text(id.0, "the stereotype", s, true)?;
    }
    let finite = [c.x, c.y, c.w, c.h, c.min_w, c.min_h].iter().all(|v| v.is_finite() && v.abs() <= COORD_LIMIT);
    if !finite || c.w <= 0.0 || c.h <= 0.0 || c.min_w <= 0.0 || c.min_h <= 0.0 {
        return Err(ClassError::BadGeometry(id));
    }
    let (fw, fh) = c.floor();
    if c.w < c.min_w || c.h < c.min_h || c.min_w < fw || c.min_h < fh {
        return Err(ClassError::TooSmall(id));
    }
    if c.member_count() > MAX_MEMBERS_PER_CLASSIFIER {
        return Err(ClassError::TooManyMembersIn(id));
    }
    if c.kind == Kind::Enumeration && c.is_abstract {
        return Err(ClassError::AbstractEnumeration(id));
    }
    if c.kind != Kind::Enumeration && !c.literals.is_empty() {
        return Err(ClassError::LiteralsOutsideEnumeration(id));
    }
    let mut names = HashSet::new();
    for a in &c.attributes {
        check_text(a.id.0, "the attribute name", &a.name, false)?;
        if let Some(t) = &a.type_name {
            check_text(a.id.0, "the attribute type", t, true)?;
        }
        if let Some(mult) = &a.multiplicity
            && !crate::use_case::multiplicity_ok(mult)
        {
            return Err(bad(a.id.0, "the multiplicity", "it isn't n, n..m, n..* or *"));
        }
        if let Some(d) = &a.default
            && let Some(why) = default_problem(d)
        {
            return Err(bad(a.id.0, "the default", why));
        }
        if !names.insert(a.name.as_str()) {
            return Err(ClassError::DuplicateAttribute { classifier: id, name: a.name.clone() });
        }
    }
    let mut sigs = HashSet::new();
    for o in &c.operations {
        check_text(o.id.0, "the operation name", &o.name, false)?;
        if o.params.len() > MAX_PARAMS {
            return Err(ClassError::TooManyParams(o.id));
        }
        for p in &o.params {
            check_text(o.id.0, "a parameter name", &p.name, false)?;
            if let Some(t) = &p.type_name {
                check_text(o.id.0, "a parameter type", t, true)?;
            }
        }
        if let Some(t) = &o.return_type {
            check_text(o.id.0, "the return type", t, true)?;
        }
        if !sigs.insert(o.signature()) {
            return Err(ClassError::DuplicateOperation { classifier: id, name: o.name.clone() });
        }
    }
    let mut lits = HashSet::new();
    for l in &c.literals {
        check_text(l.id.0, "the literal", &l.name, false)?;
        if !lits.insert(l.name.as_str()) {
            return Err(ClassError::DuplicateLiteral { classifier: id, name: l.name.clone() });
        }
    }
    Ok(())
}

/// Cards keep [`CARD_GAP`] apart. Sorted by left edge so most pairs are never compared.
fn spacing(m: &Model) -> Result<(), ClassError> {
    let mut boxes: Vec<&Classifier> = m.classifiers.iter().collect();
    boxes.sort_by(|a, b| a.x.total_cmp(&b.x));
    for (i, a) in boxes.iter().enumerate() {
        for b in boxes.iter().skip(i + 1) {
            if b.x >= a.x + a.w + CARD_GAP {
                break;
            }
            // The distance between the boxes' closest points, as the Use Case diagram measures it.
            let dx = (b.x - (a.x + a.w)).max(a.x - (b.x + b.w)).max(0.0);
            let dy = (b.y - (a.y + a.h)).max(a.y - (b.y + b.h)).max(0.0);
            if dx * dx + dy * dy < CARD_GAP * CARD_GAP {
                return Err(ClassError::Overlap(a.id, b.id));
            }
        }
    }
    Ok(())
}

/// A relationship's identity for duplicate detection. A plain association compares its ends
/// unordered, each with its role and multiplicity (the same pair with other roles is another
/// association); every other kind compares its kind and ordered ends.
type LinkKey = (LinkKind, Vec<(u64, Option<String>, Option<String>)>);

fn link_key(l: &Link) -> LinkKey {
    if l.kind == LinkKind::Association {
        let mut ends: Vec<_> = l.ends.iter().map(|e| (e.classifier.0, e.role.clone(), e.multiplicity.clone())).collect();
        ends.sort();
        (l.kind, ends)
    } else {
        (l.kind, l.ends.iter().map(|e| (e.classifier.0, None, None)).collect())
    }
}

pub(crate) fn links(m: &Model) -> Result<(), ClassError> {
    let kinds: HashMap<ClassifierId, Kind> = m.classifiers.iter().map(|c| (c.id, c.kind)).collect();
    let mut keys = HashSet::new();
    for l in &m.links {
        let id = l.id;
        let mut sides = [Kind::Class; 2];
        for (i, e) in l.ends.iter().enumerate() {
            match kinds.get(&e.classifier) {
                Some(k) => sides[i] = *k,
                None => return Err(ClassError::MissingEnd { link: id, missing: e.classifier }),
            }
        }
        let (a, b) = l.pair();
        if a == b && !matches!(l.kind, LinkKind::Association | LinkKind::Aggregation) {
            return Err(ClassError::SelfLink(id));
        }
        if l.kind.is_association() {
            for e in &l.ends {
                if let Some(r) = &e.role {
                    check_text(id.0, "a role", r, true)?;
                }
                if let Some(mult) = &e.multiplicity
                    && !crate::use_case::multiplicity_ok(mult)
                {
                    return Err(ClassError::BadMultiplicity { link: id, text: mult.chars().take(24).collect() });
                }
            }
            if l.label.is_some() {
                return Err(ClassError::StrayEndDetail(id));
            }
            if l.kind == LinkKind::Composition && l.ends[0].multiplicity.as_deref().is_some_and(|w| w != "1" && w != "0..1") {
                return Err(ClassError::CompositionWhole(id));
            }
        } else {
            let detail = l.ends.iter().any(|e| e.role.is_some() || e.multiplicity.is_some() || e.navigable != Navigable::Unspecified);
            if detail || (l.kind != LinkKind::Dependency && l.label.is_some()) {
                return Err(ClassError::StrayEndDetail(id));
            }
            if let Some(t) = &l.label {
                check_text(id.0, "the label", t, true)?;
            }
        }
        match l.kind {
            LinkKind::Generalization => {
                if sides[0] != sides[1] {
                    return Err(ClassError::WrongEnds(id, "a generalization joins two of the same kind"));
                }
                if sides[0] == Kind::Enumeration {
                    return Err(ClassError::WrongEnds(id, "an enumeration takes no generalization"));
                }
            }
            LinkKind::Realization if sides[0] == Kind::Interface || sides[1] != Kind::Interface => {
                return Err(ClassError::WrongEnds(id, "a class or an enumeration realizes an interface"));
            }
            _ => {}
        }
        if !keys.insert(link_key(l)) {
            return Err(ClassError::DuplicateLink(id));
        }
    }
    Ok(())
}

/// The graphs that must stay acyclic: generalization (specific → general) and composition
/// (whole → part; a FigCraft restriction, stricter than UML, which states it per object).
pub(crate) fn acyclic(m: &Model) -> Result<(), ClassError> {
    for (kind, what) in [(LinkKind::Generalization, "generalization"), (LinkKind::Composition, "composition")] {
        let edges: Vec<(u64, u64)> = m.links.iter().filter(|l| l.kind == kind).map(|l| (l.ends[0].classifier.0, l.ends[1].classifier.0)).collect();
        if let Some(at) = crate::use_case::find_cycle(&edges) {
            return Err(ClassError::Cycle(what, ClassifierId(at)));
        }
    }
    Ok(())
}
