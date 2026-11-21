//! New diagrams: the blank page and the example.

use std::sync::Arc;

use figcraft_doc::Document;
use figcraft_doc::use_case::{self as uc, Model};
use serde_json::{Value, json};

use super::{EXAMPLE, render};
use crate::{Result, Session};

/// A new Use Case diagram's page content: empty, or the Room booking example.
pub(crate) fn start(doc: &mut Document, template: &str) -> Result<()> {
    let page = doc.first_page().ok_or_else(|| crate::EngineError::Other("the new diagram has no page".into()))?;
    let model = if template == EXAMPLE { example()? } else { Model::default() };
    uc::validate(&model).map_err(|e| crate::EngineError::Other(format!("the example diagram: {e}")))?;
    let before = doc.clone();
    doc.use_cases.insert(page, Arc::new(model));
    render::regenerate(doc, page)?;
    // The new file starts normalised (groups fitted, nothing left dirty), as every command's result
    // is: otherwise the first later command would refit these generated layers without a model
    // change, which the typed-page gate (rightly) refuses on undo.
    crate::normalize(doc, &before);
    Ok(())
}

/// The Room booking example, built with the same steps the commands take.
pub(super) fn example() -> Result<Model> {
    let mut s = Session::new();
    let mut doc = Document::new("example");
    let page = doc.first_page().ok_or_else(|| crate::EngineError::Other("no page".into()))?;
    doc.use_cases.insert(page, Arc::new(Model::default()));
    s.docs.push(crate::DocState::new(doc, None));
    s.active = s.docs.len() - 1;
    let mut run = |id: &str, p: Value| -> Result<u64> {
        let r = s.execute(id, &p)?;
        Ok(r.get("id").or_else(|| r.get("element")).and_then(Value::as_u64).unwrap_or(0))
    };
    let room = run("diagram.useCase.addSubject", json!({"name": "Room booking", "x": 300, "y": 40, "width": 700, "height": 460}))?;
    let reserve = run("diagram.useCase.addUseCase", json!({"name": "Reserve room", "subject": room, "x": 340, "y": 100}))?;
    let check = run("diagram.useCase.addUseCase", json!({"name": "Check availability", "subject": room, "x": 660, "y": 100}))?;
    let discount = run("diagram.useCase.addUseCase", json!({"name": "Apply member discount", "subject": room, "x": 340, "y": 300}))?;
    let meeting = run("diagram.useCase.addUseCase", json!({"name": "Reserve meeting room", "subject": room, "x": 660, "y": 300}))?;
    let guest = run("diagram.useCase.addActor", json!({"name": "Guest", "x": 100, "y": 90}))?;
    let member = run("diagram.useCase.addActor", json!({"name": "Member", "x": 100, "y": 330}))?;
    run("diagram.useCase.connect", json!({"kind": "association", "from": reserve, "to": guest}))?;
    run("diagram.useCase.connect", json!({"kind": "include", "from": reserve, "to": check}))?;
    run(
        "diagram.useCase.connect",
        json!({"kind": "extend", "from": discount, "to": reserve, "newPoint": "Before total", "condition": "member is eligible"}),
    )?;
    run("diagram.useCase.connect", json!({"kind": "generalization", "from": member, "to": guest}))?;
    run("diagram.useCase.connect", json!({"kind": "generalization", "from": meeting, "to": reserve}))?;
    let model = s.doc()?.use_cases.get(&page).map(|m| (**m).clone()).unwrap_or_default();
    Ok(model)
}
