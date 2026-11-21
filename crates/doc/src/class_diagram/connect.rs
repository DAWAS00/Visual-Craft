//! Which relationship kinds can join two classifiers.

use super::{
    ALL_LINK_KINDS, Classifier, ClassifierId, End, LINK_LAYERS, Link, LinkId, LinkKind, MAX_LAYERS, MAX_LINKS, Model, acyclic, layer_estimate, links,
};

/// The relationship kinds that can join `from` to `to` as the validator would accept them, the
/// new one added with bare ends: the type rules, no duplicate and no cycle. Empty for an unknown
/// classifier.
pub fn connection_kinds(m: &Model, from: ClassifierId, to: ClassifierId) -> Vec<LinkKind> {
    let full = m.links.len() >= MAX_LINKS || layer_estimate(m) + LINK_LAYERS > MAX_LAYERS;
    if full || m.classifier(from).is_none() || m.classifier(to).is_none() {
        return Vec::new();
    }
    let Some(id) = m.next_id() else { return Vec::new() };
    // Only the relationship rules run: they read the classifiers' kinds, so the trial holds bare
    // copies of the cards and the links, not the members.
    let bare = |c: &Classifier| Classifier { attributes: Vec::new(), operations: Vec::new(), literals: Vec::new(), ..c.clone() };
    let mut trial = Model { schema: m.schema, classifiers: m.classifiers.iter().map(bare).collect(), links: m.links.clone() };
    trial.links.push(Link { id: LinkId(id), kind: LinkKind::Association, ends: [End::at(from), End::at(to)], label: None });
    ALL_LINK_KINDS
        .into_iter()
        .filter(|&kind| {
            if let Some(last) = trial.links.last_mut() {
                last.kind = kind;
            }
            links(&trial).is_ok() && acyclic(&trial).is_ok()
        })
        .collect()
}
