//! The read-only drafting check: what is worth finishing but still allowed.

use super::{ClassifierId, Kind, LinkKind, MemberId, Model, Operation};
use std::collections::HashSet;

/// Something worth finishing that the validator allows.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// A concrete class declares an abstract operation.
    AbstractInConcrete { classifier: ClassifierId, operation: MemberId },
    /// A class realizes an interface but lacks one of its operations (same name and parameter types).
    MissingOperation { classifier: ClassifierId, interface: ClassifierId, operation: MemberId, name: String },
    /// A classifier with no relationships.
    Unconnected(ClassifierId),
    /// An enumeration without literals.
    EmptyEnumeration(ClassifierId),
}

/// The drafting check (`diagram.class.check`), in model order. Never refuses.
pub fn check(m: &Model) -> Vec<Warning> {
    let mut out = Vec::new();
    for c in &m.classifiers {
        if c.kind == Kind::Class && !c.is_abstract {
            for o in c.operations.iter().filter(|o| o.is_abstract) {
                out.push(Warning::AbstractInConcrete { classifier: c.id, operation: o.id });
            }
        }
        for l in m.links.iter().filter(|l| l.kind == LinkKind::Realization && l.ends[0].classifier == c.id) {
            let Some(contract) = m.classifier(l.ends[1].classifier) else { continue };
            let have: HashSet<(String, Vec<String>)> = c.operations.iter().map(Operation::signature).collect();
            for o in &contract.operations {
                if !have.contains(&o.signature()) {
                    out.push(Warning::MissingOperation { classifier: c.id, interface: contract.id, operation: o.id, name: o.name.clone() });
                }
            }
        }
        if !m.links.iter().any(|l| l.touches(c.id)) {
            out.push(Warning::Unconnected(c.id));
        }
        if c.kind == Kind::Enumeration && c.literals.is_empty() {
            out.push(Warning::EmptyEnumeration(c.id));
        }
    }
    out
}
