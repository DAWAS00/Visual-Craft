//! The `diagram.class.*` command table.

use super::elements::{add_classifier, move_classifier, resize, update_classifier};
use super::links::{add_connected, check, connect, read, remove, swap_ends, update_link};
use super::members::{add_member, move_member, remove_member, update_member};
use crate::cmd::CommandSpec;
use crate::spec;
pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "diagram.class.addClassifier",
            "Add class",
            "{page?, kind? (class|interface|enumeration), name, x?, y?, abstract?}  placed in free room when x and y are left out → {id}",
            true,
            add_classifier
        ),
        spec!(
            "diagram.class.updateClassifier",
            "Edit class",
            "{page?, id, name?, kind? (class and interface change into one another; an enumeration stays one), abstract?, stereotype? (null clears), showAttributes?, showOperations?}  → {id, changed}",
            true,
            update_classifier
        ),
        spec!("diagram.class.move", "Move class", "{page?, id, x, y}  → {id, changed}", true, move_classifier),
        spec!(
            "diagram.class.resize",
            "Resize class",
            "{page?, id, width, height, x?, y?}  never below what its text needs → {id, changed}",
            true,
            resize
        ),
        spec!(
            "diagram.class.addMember",
            "Add member",
            "{page?, classifier, compartment (attribute|operation|literal), text?, visibility?, name?, type?, multiplicity?, default?, static?, abstract?, params? [{name, type?}], returnType?, after?}  `text` as `- name : Type [m] = default` or `+ name(p: T) : R`, or the fields (name required) → {id}",
            true,
            add_member
        ),
        spec!(
            "diagram.class.updateMember",
            "Edit member",
            "{page?, id, text?, visibility?, name?, type?, multiplicity?, default?, static?, abstract?, params?, returnType?}  `text` replaces the member; fields change only what they name (null clears) → {id, changed}",
            true,
            update_member
        ),
        spec!(
            "diagram.class.moveMember",
            "Reorder member",
            "{page?, id, before?, after?}  one of them, within its own compartment → {id, changed}",
            true,
            move_member
        ),
        spec!("diagram.class.removeMember", "Remove member", "{page?, id}  → {id}", true, remove_member),
        spec!(
            "diagram.class.connect",
            "Connect classes",
            "{page?, kind (association|aggregation|composition|generalization|realization|dependency), from, to, ends? [{role?, multiplicity?, navigable? (unspecified|yes|no)}, …], label?}  `from` is the whole, the specific class, the implementer or the client → {id}",
            true,
            connect
        ),
        spec!(
            "diagram.class.addConnected",
            "Add connected class",
            "{page?, source, relation (association|aggregation|composition|generalization|realization|dependency), kind (class|interface|enumeration: the new element's), direction (out: the source is the first end, the specific class, implementer, client or whole; in: the new element is), wholeEnd? (source|new: the whole of an aggregation or composition, agreeing with direction), name, x?, y?, ends?, label?}  a new classifier and its relationship in one step → {element, link}",
            true,
            add_connected
        ),
        spec!(
            "diagram.class.updateLink",
            "Edit relationship",
            "{page?, id, kind? (among association, aggregation, composition), ends? [{role?, multiplicity?, navigable?}, …] (null clears each), label? (null clears)}  → {id, changed}",
            true,
            update_link
        ),
        spec!(
            "diagram.class.swapEnds",
            "Swap relationship ends",
            "{page?, id}  for an aggregation or composition this changes which side is the whole → {id, changed}",
            true,
            swap_ends
        ),
        spec!(
            "diagram.class.remove",
            "Remove classes",
            "{page?, ids}  classifiers and relationships; a classifier's relationships go with it → {removed}",
            true,
            remove
        ),
        spec!(
            "diagram.class.check",
            "Check class diagram",
            "{page?}  what is unfinished (never refuses) → {warnings: [{id, message}]}",
            false,
            check
        ),
        spec!("diagram.class.read", "Read class diagram", "{page?}  → {page, schema, model}", false, read),
    ]
}
