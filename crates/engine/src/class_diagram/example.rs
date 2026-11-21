//! New diagrams: the blank page and the Library loans example.

use super::{EXAMPLE, render};
use crate::Result;
use crate::Session;
use figcraft_doc::Document;
use figcraft_doc::class_diagram as cd;
use figcraft_doc::class_diagram::Model;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

/// A new Class diagram's page content: empty, or the Library loans example.
pub(crate) fn start(doc: &mut Document, template: &str) -> Result<()> {
    let page = doc.first_page().ok_or_else(|| crate::EngineError::Other("the new diagram has no page".into()))?;
    let model = if template == EXAMPLE { example()? } else { Model::default() };
    cd::validate(&model).map_err(|e| crate::EngineError::Other(format!("the example diagram: {e}")))?;
    let before = doc.clone();
    doc.class_diagrams.insert(page, Arc::new(model));
    render::regenerate(doc, page)?;
    // Normalised, as every command's result is (see `use_case::start`).
    crate::normalize(doc, &before);
    Ok(())
}

/// The Library loans example, built with the same steps the commands take: all six relationship
/// kinds, and nothing for `check` to report.
pub(super) fn example() -> Result<Model> {
    let mut s = Session::new();
    let mut doc = Document::new("example");
    let page = doc.first_page().ok_or_else(|| crate::EngineError::Other("no page".into()))?;
    doc.class_diagrams.insert(page, Arc::new(Model::default()));
    s.docs.push(crate::DocState::new(doc, None));
    s.active = s.docs.len() - 1;
    let mut run = |id: &str, p: Value| -> Result<u64> {
        let r = s.execute(id, &p)?;
        Ok(r.get("id").and_then(Value::as_u64).unwrap_or(0))
    };
    let class = |name: &str, x: f64, y: f64| json!({"kind": "class", "name": name, "x": x, "y": y});
    let library = run("diagram.class.addClassifier", class("Library", 40.0, 40.0))?;
    let shelf = run("diagram.class.addClassifier", class("Shelf", 40.0, 260.0))?;
    let book = run("diagram.class.addClassifier", class("Book", 40.0, 480.0))?;
    let borrowable = run("diagram.class.addClassifier", json!({"kind": "interface", "name": "Borrowable", "x": 40.0, "y": 720.0}))?;
    let loan = run("diagram.class.addClassifier", class("Loan", 420.0, 480.0))?;
    let member = run("diagram.class.addClassifier", class("Member", 820.0, 480.0))?;
    let premium = run("diagram.class.addClassifier", class("PremiumMember", 820.0, 720.0))?;
    let clock = run("diagram.class.addClassifier", class("Clock", 420.0, 260.0))?;
    let status = run("diagram.class.addClassifier", json!({"kind": "enumeration", "name": "LoanStatus", "x": 420.0, "y": 720.0}))?;
    let attr = |c: u64, text: &str| json!({"classifier": c, "compartment": "attribute", "text": text});
    let op = |c: u64, text: &str| json!({"classifier": c, "compartment": "operation", "text": text});
    run("diagram.class.addMember", attr(library, "- name : String"))?;
    run("diagram.class.addMember", attr(shelf, "- label : String"))?;
    run("diagram.class.addMember", attr(book, "- title : String"))?;
    run("diagram.class.addMember", attr(book, "- isbn : String"))?;
    run("diagram.class.addMember", op(book, "+ isAvailable() : Bool"))?;
    run("diagram.class.addMember", op(borrowable, "+ isAvailable() : Bool"))?;
    run("diagram.class.addMember", attr(loan, "- due : Date"))?;
    run("diagram.class.addMember", attr(loan, "- status : LoanStatus"))?;
    run("diagram.class.addMember", op(loan, "+ renew(days : Int) : Bool"))?;
    run("diagram.class.addMember", attr(member, "- name : String"))?;
    run("diagram.class.addMember", attr(premium, "- discount : Percent = 10"))?;
    run("diagram.class.addMember", op(clock, "+ today() : Date"))?;
    for lit in ["Open", "Returned", "Overdue"] {
        run("diagram.class.addMember", json!({"classifier": status, "compartment": "literal", "text": lit}))?;
    }
    let link = |kind: &str, from: u64, to: u64| json!({"kind": kind, "from": from, "to": to});
    run(
        "diagram.class.connect",
        json!({"kind": "composition", "from": library, "to": shelf, "ends": [{"multiplicity": "1"}, {"multiplicity": "*"}]}),
    )?;
    run(
        "diagram.class.connect",
        json!({"kind": "aggregation", "from": shelf, "to": book, "ends": [{"multiplicity": "0..1"}, {"multiplicity": "*"}]}),
    )?;
    run(
        "diagram.class.connect",
        json!({"kind": "association", "from": member, "to": loan, "ends": [{"role": "borrower", "multiplicity": "1"}, {"role": "loans", "multiplicity": "0..*"}]}),
    )?;
    run("diagram.class.connect", json!({"kind": "association", "from": loan, "to": book, "ends": [{}, {"navigable": "yes", "multiplicity": "1"}]}))?;
    run("diagram.class.connect", link("realization", book, borrowable))?;
    run("diagram.class.connect", link("generalization", premium, member))?;
    run("diagram.class.connect", link("dependency", loan, clock))?;
    run("diagram.class.connect", link("association", loan, status))?;
    let model = s.doc()?.class_diagrams.get(&page).map(|m| (**m).clone()).unwrap_or_default();
    Ok(model)
}
