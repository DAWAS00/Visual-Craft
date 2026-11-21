//! What keeps a typed diagram page (a page with a model, [`Document::is_typed_page`]) in step
//! with its model: its layers are generated from the model, so only the diagram's own commands
//! change them, ordinary layers can't be added to the page, and a diagram command doesn't run in
//! the middle of an ordinary edit.

/// The plugin-data keys generated diagram layers carry, one per typed diagram kind.
pub(crate) const TYPED_LAYER_KEYS: [&str; 2] = [figcraft_doc::use_case::LAYER_KEY, figcraft_doc::class_diagram::LAYER_KEY];
use figcraft_doc::{Document, NodeId};
use serde_json::Value;

/// The commands that edit a typed diagram's model.
/// The commands that edit a typed diagram's model, one prefix per diagram kind.
const TYPED_PREFIXES: [&str; 2] = ["diagram.useCase.", "diagram.class."];
/// How deep nested batches are searched for a typed command (a deeper batch counts as one).
const MAX_BATCH_DEPTH: usize = 16;

fn typed_id(id: &str) -> bool {
    TYPED_PREFIXES.iter().any(|p| id.starts_with(p))
}

/// The command `id` with `params` runs a typed diagram command: itself, the command an
/// `edit.repeat` repeats (`last`), or a step of a `command.batch`, nested ones included.
pub(crate) fn runs_typed(id: &str, params: &Value, last: Option<&str>) -> bool {
    fn walk(id: &str, params: &Value, last: Option<&str>, depth: usize) -> bool {
        if typed_id(id) {
            return true;
        }
        match id {
            "edit.repeat" => last.is_some_and(|l| l != "edit.repeat" && typed_id(l)),
            "command.batch" if depth >= MAX_BATCH_DEPTH => true,
            "command.batch" => params.get("commands").and_then(Value::as_array).is_some_and(|steps| {
                steps.iter().any(|step| {
                    let cmd = step.get("command").and_then(Value::as_str).unwrap_or("");
                    let p = step.get("params").unwrap_or(&Value::Null);
                    walk(cmd, p, last, depth + 1)
                })
            }),
            _ => false,
        }
    }
    walk(id, params, last, 0)
}

/// `id` is inside one of `doc`'s generated diagram layers: it, or a layer holding it below its
/// page, carries one of [`TYPED_LAYER_KEYS`].
fn generated(doc: &Document, id: NodeId) -> bool {
    let tagged = |n: NodeId| doc.node(n).is_some_and(|n| TYPED_LAYER_KEYS.iter().any(|k| n.plugin_data.contains_key(*k)));
    tagged(id) || doc.ancestors(id).into_iter().any(tagged)
}

/// The typed pages holding `id` in `before` or `after`, with the document that has it there.
fn typed_homes<'a>(before: &'a Document, after: &'a Document, id: NodeId) -> impl Iterator<Item = (NodeId, &'a Document)> {
    [after, before].into_iter().filter_map(move |d| {
        let page = d.page_of(id)?;
        (page != id && (before.is_typed_page(page) || after.is_typed_page(page))).then_some((page, d))
    })
}

/// The model of `page` is the same object in both versions (no diagram command changed it).
fn same_model(before: &Document, after: &Document, page: NodeId) -> bool {
    before.typed_model_id(page) == after.typed_model_id(page)
}

/// Refuses an edit (`before` → `after`) that changed a typed page's layers without changing its
/// model: an ordinary command on a generated layer, or a layer added to (or moved onto) a typed
/// page. A diagram command changes the model and regenerates the layers; deleting, duplicating or
/// making a typed page changes its model too. A typed page's own properties (its name, colour and
/// so on) stay editable, but not the order of its layers (`arrange.front` on a generated root).
pub(crate) fn check_edit(before: &Document, after: &Document) -> Result<(), String> {
    if !before.has_typed_records() && !after.has_typed_records() {
        return Ok(());
    }
    for page in before.typed_pages().into_iter().chain(after.typed_pages()) {
        let (was, now) = (before.node(page).map(|n| n.children.as_slice()), after.node(page).map(|n| n.children.as_slice()));
        if was != now && same_model(before, after, page) {
            let (was, now) = (was.unwrap_or_default(), now.unwrap_or_default());
            // Name the layer that came onto the page, or else the first one that moved.
            let added = now.iter().find(|c| !was.contains(c));
            let moved = now.iter().zip(was).find(|(a, b)| a != b).map(|(a, _)| a);
            let id = added.or(moved).copied().unwrap_or(page);
            return Err(refusal(after, page, id));
        }
    }
    for id in after.changed_ids(before, usize::MAX).unwrap_or_default() {
        for (page, doc) in typed_homes(before, after, id) {
            if same_model(before, after, page) {
                return Err(refusal(doc, page, id));
            }
        }
    }
    Ok(())
}

/// Refuses an edit that starts at `id` (a layer, or a page) on a typed page, before it changes
/// anything: the text tool's new text and text editing, which change the document outside a
/// command.
pub(crate) fn refuse_at(doc: &Document, id: NodeId) -> Result<(), String> {
    let page = if doc.is_typed_page(id) { Some(id) } else { doc.page_of(id).filter(|p| doc.is_typed_page(*p)) };
    match page {
        Some(page) => Err(refusal(doc, page, id)),
        None => Ok(()),
    }
}

fn refusal(doc: &Document, page: NodeId, id: NodeId) -> String {
    let name = |n: NodeId| doc.node(n).map(|n| n.name.chars().take(60).collect::<String>()).unwrap_or_default();
    if generated(doc, id) {
        format!("\"{}\" is part of a diagram: use the diagram tools to change it", name(id))
    } else {
        format!("page \"{}\" shows a diagram, and its layers come from the diagram: add other layers on another page", name(page))
    }
}

/// The commands that send a file to others or merge another version of it, refused while the file
/// holds a typed diagram record anywhere (live sessions and branches don't carry the models yet).
const SHARING: [&str; 6] = ["collab.host", "collab.share", "collab.publish", "collab.reconnect", "branch.merge", "branch.updateFromMain"];

/// Why typed diagrams can't be shared or merged yet.
pub(crate) const SHARING_REFUSAL: &str = "diagrams can't be shared, published or merged yet in this preview";

/// `ds` holds a typed diagram record: in the document shown, the document itself under a version
/// preview or a live text preview ([`crate::DocState::document_itself`]), its undo or redo history
/// (or the gesture in progress), a version, or a branch. A stored version that can't be read to
/// tell counts as holding one.
pub(crate) fn holds_typed(ds: &crate::DocState) -> bool {
    let unknown_or_typed = |r: Result<bool, figcraft_format::FormatError>| r.unwrap_or(true);
    ds.doc.has_typed_records()
        || ds.document_itself().has_typed_records()
        || ds.preview.as_ref().is_some_and(|p| p.doc.has_typed_records())
        || ds.text_preview.as_ref().is_some_and(|d| d.has_typed_records())
        || ds.history.undo.iter().chain(&ds.history.redo).any(|e| e.doc.has_typed_records())
        || ds.interaction.as_ref().is_some_and(|i| i.doc.has_typed_records())
        || ds.versions.iter().any(|v| unknown_or_typed(v.has_typed_records()))
        || ds.branches.iter().any(|b| unknown_or_typed(b.has_typed_records()))
}

/// Refuses sharing, hosting, publishing and branch merges of a file holding a typed record,
/// before any transport or merge planner starts. A branch tab is checked with its main file.
pub(crate) fn refuse_sharing(s: &crate::Session, id: &str) -> crate::Result<()> {
    if !SHARING.contains(&id) {
        return Ok(());
    }
    let Ok(ds) = s.ds() else { return Ok(()) };
    let main = ds.branch_of.as_ref().and_then(|(uid, _)| s.docs.iter().find(|d| d.uid == *uid));
    if holds_typed(ds) || main.is_some_and(holds_typed) {
        return Err(crate::EngineError::Disabled(id.into(), SHARING_REFUSAL.into()));
    }
    Ok(())
}

/// A typed command waits for an ordinary edit in progress (a gesture, a text edit or a path
/// edit) to finish: it regenerates the page under the edit.
pub(crate) fn ordinary_edit_open(ds: &crate::DocState) -> bool {
    ds.interaction.is_some() || ds.text_edit.is_some() || ds.text_preview.is_some() || ds.vector_edit.is_some()
}

/// Refuses a tool that would put a layer on the active page while it is a typed diagram page,
/// before the gesture stores anything (the pen's first anchor, the pencil's first sample).
pub(crate) fn refuse_page(s: &crate::Session, tool: &str) -> crate::Result<()> {
    let ds = s.ds()?;
    refuse_at(&ds.doc, ds.page).map_err(|why| crate::EngineError::Disabled(tool.into(), why))
}
