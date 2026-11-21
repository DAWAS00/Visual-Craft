//! Structure guards for the typed diagram code: the files split out of the big diagram modules
//! stay small, so one file keeps one job (`docs/sequence-diagram-plan.md`, A3). A file that
//! grows past the limit is split again, not added to an exception list.

use std::path::Path;

/// The most lines a governed file may have.
const LIMIT: usize = 400;

/// The governed files, relative to `crates/`. Every Part B (Sequence) file joins this list.
const GOVERNED: &[&str] = &[
    "doc/src/class_diagram.rs",
    "doc/src/class_diagram/bounded.rs",
    "doc/src/class_diagram/check.rs",
    "doc/src/class_diagram/connect.rs",
    "doc/src/class_diagram/model.rs",
    "doc/src/class_diagram/text.rs",
    "doc/src/class_diagram/validate.rs",
    "doc/src/use_case.rs",
    "doc/src/use_case/bounded.rs",
    "doc/src/use_case/check.rs",
    "doc/src/use_case/geometry.rs",
    "doc/src/use_case/model.rs",
    "doc/src/use_case/validate.rs",
    "engine/src/class_diagram/mod.rs",
    "engine/src/class_diagram/elements.rs",
    "engine/src/class_diagram/example.rs",
    "engine/src/class_diagram/layout.rs",
    "engine/src/class_diagram/lifecycle.rs",
    "engine/src/class_diagram/links.rs",
    "engine/src/class_diagram/measure.rs",
    "engine/src/class_diagram/members.rs",
    "engine/src/class_diagram/params.rs",
    "engine/src/class_diagram/render.rs",
    "engine/src/class_diagram/specs.rs",
    "engine/src/use_case/mod.rs",
    "engine/src/use_case/elements.rs",
    "engine/src/use_case/example.rs",
    "engine/src/use_case/gate.rs",
    "engine/src/use_case/lifecycle.rs",
    "engine/src/use_case/links.rs",
    "engine/src/use_case/measure.rs",
    "engine/src/use_case/params.rs",
    "engine/src/use_case/render.rs",
    "engine/src/use_case/specs.rs",
];

#[test]
fn every_governed_diagram_file_exists_and_stays_within_the_line_limit() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let over: Vec<String> = GOVERNED
        .iter()
        .map(|f| {
            let text = std::fs::read_to_string(crates.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
            (f, text.lines().count())
        })
        .filter(|(_, n)| *n > LIMIT)
        .map(|(f, n)| format!("{f}: {n} lines"))
        .collect();
    assert!(over.is_empty(), "over {LIMIT} lines, split them: {over:?}");
}
