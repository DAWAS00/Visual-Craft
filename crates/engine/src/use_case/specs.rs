//! The `diagram.useCase.*` command table.

use super::elements::{
    add_actor, add_point, add_subject, add_use_case, move_element, remove_point, resize, set_subject, update_element, update_point,
};
use super::links::{add_connected, check, connect, read, remove, update_link};
use crate::cmd::CommandSpec;
use crate::spec;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "diagram.useCase.addSubject",
            "Add system boundary",
            "{page?, name, x?, y?, width?, height?}  a subject (system boundary); placed in free room when x and y are left out → {id}",
            true,
            add_subject
        ),
        spec!("diagram.useCase.addActor", "Add actor", "{page?, name, x?, y?, presentation? (person|system)}  → {id}", true, add_actor),
        spec!(
            "diagram.useCase.addUseCase",
            "Add use case",
            "{page?, name, x?, y?, subject?}  inside the subject when one is given → {id}",
            true,
            add_use_case
        ),
        spec!(
            "diagram.useCase.updateElement",
            "Rename diagram element",
            "{page?, id, name?, presentation?}  an actor, use case or subject → {id, changed}",
            true,
            update_element
        ),
        spec!(
            "diagram.useCase.move",
            "Move diagram element",
            "{page?, id, x, y}  a subject's use cases move with it → {id, changed}",
            true,
            move_element
        ),
        spec!(
            "diagram.useCase.resize",
            "Resize diagram element",
            "{page?, id, width, height, x?, y?}  a subject or use case; a subject's use cases stay put → {id, changed}",
            true,
            resize
        ),
        spec!(
            "diagram.useCase.setSubject",
            "Set use case's system",
            "{page?, useCase, subject, x?, y?}  subject null takes it out (give a position outside) → {id, changed}",
            true,
            set_subject
        ),
        spec!("diagram.useCase.addExtensionPoint", "Add extension point", "{page?, useCase, name}  → {id}", true, add_point),
        spec!("diagram.useCase.updateExtensionPoint", "Rename extension point", "{page?, id, name}  → {id, changed}", true, update_point),
        spec!(
            "diagram.useCase.removeExtensionPoint",
            "Remove extension point",
            "{page?, id}  refused while an extend uses it → {id}",
            true,
            remove_point
        ),
        spec!(
            "diagram.useCase.connect",
            "Connect diagram elements",
            "{page?, kind (association|include|extend|generalization), from, to, points?, newPoint?, condition?, actorMultiplicity?, useCaseMultiplicity?}  an extend needs points on its base (or newPoint, made on the base) → {id, point}",
            true,
            connect
        ),
        spec!(
            "diagram.useCase.addConnected",
            "Add connected element",
            "{page?, source, relation (association|include|extension|parent|child), name, x?, y?, subject?, points?, newPoint?, condition?}  a new element and its relationship in one step; an extension points back to the source; a new use case goes in `subject`, in the source's system when it is left out, and in none when it is null → {element, link, point}",
            true,
            add_connected
        ),
        spec!(
            "diagram.useCase.updateLink",
            "Edit relationship",
            "{page?, id, points?, condition?, actorMultiplicity?, useCaseMultiplicity?}  null clears → {id, changed}",
            true,
            update_link
        ),
        spec!(
            "diagram.useCase.remove",
            "Remove diagram elements",
            "{page?, ids}  elements and relationships; an element's relationships go with it → {removed}",
            true,
            remove
        ),
        spec!(
            "diagram.useCase.check",
            "Check use case diagram",
            "{page?}  what is unfinished (never refuses) → {warnings: [{id, message}]}",
            false,
            check
        ),
        spec!("diagram.useCase.read", "Read use case diagram", "{page?}  → {page, schema, model}", false, read),
    ]
}
