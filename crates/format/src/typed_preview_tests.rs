//! Typed diagrams are saved as format version 2: a file whose state or history holds a typed
//! record has a version 2 `document.json`, each typed snapshot has the version 2 header, and text
//! with a typed record under any other header (document, version or branch) refuses the file whole.

use std::collections::BTreeMap;
use std::sync::Arc;

use figcraft_doc::Document;
use figcraft_doc::use_case::Model;

use super::*;

fn typed() -> Document {
    let mut d = Document::new("Typed");
    let page = d.first_page().unwrap();
    d.use_cases.insert(page, Arc::new(Model::default()));
    d
}

/// A stored snapshot (as read from a file) of `doc` under the plain header.
fn stored(id: &str, doc: &Document) -> Version {
    let mut json = Vec::new();
    craft_figcraft_format::write_document_json(&FiniteValue(doc), &[], &mut json).unwrap();
    Version {
        snapshot: Snapshot::Stored { json: Arc::new(json), images: BTreeMap::new() },
        ..Version::new(id, id, "", 1, Arc::new(Document::new("x")))
    }
}

fn branch(head: Document, versions: Vec<Version>) -> Branch {
    Branch {
        id: "b1".into(),
        name: "B".into(),
        time: 1,
        client: 7,
        state: BranchState::Active,
        base: Arc::new(Document::new("base")),
        head: Arc::new(head),
        versions,
    }
}

/// The header version of an archive member.
fn member_version(bytes: &[u8], name: &str) -> u64 {
    let mut z =
        craft_figcraft_format::Archive::open(bytes, craft_figcraft_format::ArchiveLimits { max_entries: 64, max_total_bytes: 1 << 24 }).unwrap();
    let json = z.read(name, 1 << 24).unwrap().unwrap();
    serde_json::from_slice::<Value>(&json).unwrap()["version"].as_u64().unwrap()
}

#[test]
fn every_writer_writes_a_typed_document_as_version_two_and_it_reads_back() {
    let d = typed();
    let bytes = save_bytes(&d, None).unwrap();
    assert_eq!(member_version(&bytes, "document.json"), u64::from(TYPED_VERSION));
    let (back, issues) = load_bytes(&bytes).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(back.use_cases, d.use_cases);
    assert_eq!(to_json(&d).unwrap()["version"], TYPED_VERSION);
    let mut out = Vec::new();
    write_json(&d, &mut out).unwrap();
    assert_eq!(from_json(std::str::from_utf8(&out).unwrap()).unwrap().0.use_cases, d.use_cases);
    // A plain document stays version 1, so older readers still open it.
    let plain = Document::new("Plain");
    assert_eq!(member_version(&save_bytes(&plain, None).unwrap(), "document.json"), u64::from(VERSION));
    assert_eq!(to_json(&plain).unwrap()["version"], VERSION);
}

#[test]
fn a_typed_record_only_in_a_version_or_a_branch_makes_the_file_version_two() {
    let plain = Document::new("Plain");
    let bytes = save_bytes_with_versions(&plain, None, &[Version::new("v1", "v1", "", 1, Arc::new(typed()))]).unwrap();
    // The whole file says version 2 (an older reader refuses it rather than drop the diagram from
    // the history), and so does the typed snapshot itself.
    assert_eq!(member_version(&bytes, "document.json"), u64::from(TYPED_VERSION));
    assert_eq!(member_version(&bytes, "versions/v1.json"), u64::from(TYPED_VERSION));
    let l = load_bytes_with_versions(&bytes).unwrap();
    assert!(l.versions[0].has_typed_records().unwrap());
    assert_eq!(l.versions[0].document().unwrap().use_cases, typed().use_cases);

    let mut base_typed = branch(Document::new("h"), vec![]);
    base_typed.base = Arc::new(typed());
    let bytes = save_bytes_with_history(&plain, None, &[], &[base_typed]).unwrap();
    assert_eq!(member_version(&bytes, "document.json"), u64::from(TYPED_VERSION));
    assert_eq!(member_version(&bytes, "branches/b1/base.json"), u64::from(TYPED_VERSION));
    assert_eq!(member_version(&bytes, "branches/b1/head.json"), u64::from(VERSION));
    let l = load_bytes_with_versions(&bytes).unwrap();
    assert!(l.branches[0].base.has_typed_records() && !l.branches[0].head.has_typed_records());
}

#[test]
fn a_typed_stored_snapshot_read_from_a_file_is_written_back_as_it_was() {
    let plain = Document::new("Plain");
    let bytes = save_bytes_with_versions(&plain, None, &[Version::new("v1", "v1", "", 1, Arc::new(typed()))]).unwrap();
    let l = load_bytes_with_versions(&bytes).unwrap();
    assert!(matches!(l.versions[0].snapshot, Snapshot::Stored { .. }));
    let again = save_bytes_with_history(&l.doc, None, &l.versions, &l.branches).unwrap();
    assert_eq!(member_version(&again, "document.json"), u64::from(TYPED_VERSION));
    assert_eq!(load_bytes_with_versions(&again).unwrap().versions[0].document().unwrap().use_cases, typed().use_cases);
}

#[test]
fn a_typed_snapshot_with_a_bad_model_fails_the_whole_load() {
    let mut bad = typed();
    bad.use_cases.insert(figcraft_doc::NodeId(987_654), Arc::new(Model::default()));
    let bad = typed_text(&bad);
    let plain = text_of(&Document::new("Plain"));
    let index = br#"{"versions":[{"id":"v1","name":"V"}]}"#;
    // In the document, in a saved version (checked as it's read, though kept unparsed) and in a
    // branch part: never opened without its diagram, nor skipped as damaged.
    assert!(from_json(std::str::from_utf8(&bad).unwrap()).unwrap_err().to_string().contains("isn't in the file"));
    let e = load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", index), ("versions/v1.json", &bad)])).unwrap_err();
    assert!(e.to_string().contains("version `v1` can't be read") && e.to_string().contains("isn't in the file"), "{e}");
    let branches = br#"{"branches":[{"id":"b1","name":"B"}]}"#;
    let e = load_bytes_with_versions(&zip_of(&[
        ("document.json", &plain),
        (BRANCH_INDEX, branches),
        ("branches/b1/base.json", &plain),
        ("branches/b1/head.json", &bad),
    ]))
    .unwrap_err();
    assert!(e.to_string().contains("isn't in the file"), "{e}");
    // A good typed snapshot under the version 2 header opens in each place.
    let good = typed_text(&typed());
    let l = load_bytes_with_versions(&zip_of(&[
        ("document.json", &good),
        ("versions/index.json", index),
        ("versions/v1.json", &good),
        (BRANCH_INDEX, branches),
        ("branches/b1/base.json", &plain),
        ("branches/b1/head.json", &good),
    ]))
    .unwrap();
    assert!(l.doc.has_typed_records() && l.branches[0].head.has_typed_records() && l.versions.len() == 1, "{:?}", l.issues);
}

#[test]
fn a_newer_typed_file_is_reported_as_newer_not_as_a_bad_diagram() {
    let text = String::from_utf8(typed_text(&typed())).unwrap().replacen(r#""version":2"#, r#""version":3"#, 1);
    assert!(matches!(from_json(&text), Err(FormatError::Newer(3))));
}

/// `doc`'s `document.json` text under the version 2 header.
fn typed_text(doc: &Document) -> Vec<u8> {
    let mut json = Vec::new();
    craft_figcraft_format::write_document_json_version(&FiniteValue(doc), &[], TYPED_VERSION, &mut json).unwrap();
    json
}

/// `doc`'s `document.json` text under the plain (version 1) header, whatever it holds: no
/// FigCraft writes a typed document this way, so a file like it is refused.
fn text_of(doc: &Document) -> Vec<u8> {
    let mut json = Vec::new();
    craft_figcraft_format::write_document_json(&FiniteValue(doc), &[], &mut json).unwrap();
    json
}

fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut buf);
    for (name, bytes) in members {
        z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap();
    buf.into_inner()
}

fn load_refused<T: std::fmt::Debug>(r: Result<T, FormatError>) {
    assert_eq!(r.unwrap_err().to_string(), TYPED_LOAD_REFUSAL);
}

#[test]
fn a_typed_record_under_the_plain_header_anywhere_in_a_file_fails_the_whole_load() {
    let (plain, typed) = (text_of(&Document::new("Plain")), text_of(&typed()));
    // In the document itself: valid or not, it isn't migrated.
    load_refused(from_json(std::str::from_utf8(&typed).unwrap()));
    load_refused(load_bytes_with_versions(&typed));
    load_refused(load_bytes_with_versions(&zip_of(&[("document.json", &typed)])));
    // Spelt with an escape, it is still the same key.
    let escaped = String::from_utf8(typed.clone()).unwrap().replace(r#""use_cases""#, &format!(r#""use{}u005fcases""#, char::from(92u8)));
    load_refused(from_json(&escaped));
    // In a saved version (kept unparsed, so probed as it's read).
    let index = br#"{"versions":[{"id":"v1","name":"V"}]}"#;
    load_refused(load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", index), ("versions/v1.json", &typed)])));
    // In a branch's base or head, even beside a damaged part, and in a branch version.
    let branches = br#"{"branches":[{"id":"b1","name":"B"}]}"#;
    for (base, head) in [(&typed[..], &plain[..]), (&plain[..], &typed[..]), (&b"{nope"[..], &typed[..]), (&typed[..], &b"{nope"[..])] {
        let bytes = zip_of(&[("document.json", &plain), (BRANCH_INDEX, branches), ("branches/b1/base.json", base), ("branches/b1/head.json", head)]);
        load_refused(load_bytes_with_versions(&bytes));
    }
    let bytes = zip_of(&[
        ("document.json", &plain),
        (BRANCH_INDEX, branches),
        ("branches/b1/base.json", &plain),
        ("branches/b1/head.json", &plain),
        ("branches/b1/versions/index.json", index),
        ("branches/b1/versions/v1.json", &typed),
    ]);
    load_refused(load_bytes_with_versions(&bytes));
    // The same files without the typed record open, history and all.
    let bytes = zip_of(&[
        ("document.json", &plain),
        ("versions/index.json", index),
        ("versions/v1.json", &plain),
        (BRANCH_INDEX, branches),
        ("branches/b1/base.json", &plain),
        ("branches/b1/head.json", &plain),
    ]);
    let l = load_bytes_with_versions(&bytes).unwrap();
    assert_eq!((l.versions.len(), l.branches.len()), (1, 1), "{:?}", l.issues);
}

fn stored_text(id: &str, json: &[u8]) -> Version {
    Version { snapshot: Snapshot::Stored { json: Arc::new(json.to_vec()), images: BTreeMap::new() }, ..stored(id, &Document::new("x")) }
}

#[test]
fn an_unreadable_stored_snapshot_has_an_unknown_typed_status_and_is_not_copied() {
    let plain = Document::new("Plain");
    let broken = stored_text("v1", br#"{"name": "x", "pages": [1, "#);
    assert!(broken.has_typed_records().is_err());
    assert!(branch(Document::new("h"), vec![broken.clone()]).has_typed_records().is_err());
    assert!(save_bytes_with_versions(&plain, None, std::slice::from_ref(&broken)).is_err());
    assert!(file_version(&plain, &[], &[branch(Document::new("h"), vec![broken])]).is_err());
    // "Not typed" needs a whole scan that succeeded: junk and bad UTF-8 without the key's letters
    // are unknown too.
    assert!(stored_text("v2", b"not json at all").has_typed_records().is_err());
    assert!(stored_text("v3", b"{\"name\": \"\xff\xfe\"}").has_typed_records().is_err());
    assert!(stored_text("v4", br#"{"name": "x"} trailing"#).has_typed_records().is_err());
    // Null and an empty map are no record; a non-empty map is one; any other shape is unknown.
    assert!(!stored_text("v5", br#"{"use_cases": {}}"#).has_typed_records().unwrap());
    assert!(!stored_text("v6", br#"{"use_cases": null, "name": "x"}"#).has_typed_records().unwrap());
    assert!(stored_text("v7", br#"{"use_cases": [1]}"#).has_typed_records().is_err());
    assert!(stored_text("v7b", br#"{"use_cases": "a diagram"}"#).has_typed_records().is_err());
    assert!(stored_text("v7c", br#"{"use_cases": {"1": 0}}"#).has_typed_records().unwrap());
    assert!(!stored("v8", &plain).has_typed_records().unwrap());
    assert!(stored("v9", &typed()).has_typed_records().unwrap());
}

#[test]
fn a_typed_record_is_seen_from_its_first_key_without_reading_the_record() {
    // A record far past any budget, with a huge escaped string inside: the probe answers "typed"
    // at the map's first key, and reads (and stops on) nothing of the record itself. Even the
    // text after it may be junk: the scan never gets there.
    let huge = format!("{}u0041", char::from(92u8)).repeat(4 << 20);
    let text = format!(r#"{{"use_cases": {{"1": {{"links": [{{"kind": "extend", "name": "{huge}", "points": [1, 2"#);
    assert_eq!(typed_probe::typed(text.as_bytes()), Ok(true));
    // Its header can't be read to see a version that holds diagrams (the text is cut off), so it
    // is refused as damaged: never decoded into a model.
    assert!(matches!(from_json(&text), Err(FormatError::Corrupt(_))));
    // Not even the first entry's key is read: a huge, unterminated one still answers "typed".
    let key_only = format!(r#"{{"use_cases": {{"{huge}"#);
    assert_eq!(typed_probe::typed(key_only.as_bytes()), Ok(true));
    // In a version, it fails the whole load (never "skipped as damaged" and saveable without it).
    let index = br#"{"versions":[{"id":"v1","name":"V"}]}"#;
    let plain = text_of(&Document::new("Plain"));
    assert!(matches!(
        load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", index), ("versions/v1.json", text.as_bytes()),])),
        Err(FormatError::Corrupt(_))
    ));
}

#[test]
fn nothing_whose_typed_status_is_unknown_is_dropped_on_load() {
    let plain = text_of(&Document::new("Plain"));
    let index = br#"{"versions":[{"id":"v1","name":"V"}]}"#;
    // A damaged version is kept as it is, and the file can't be saved until it's deleted.
    let l = load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", index), ("versions/v1.json", b"{nope")])).unwrap();
    assert_eq!(l.versions.len(), 1, "{:?}", l.issues);
    assert!(l.versions[0].has_typed_records().is_err());
    assert!(save_bytes_with_history(&l.doc, None, &l.versions, &l.branches).is_err());
    // A damaged branch's versions are checked before the branch is dropped.
    let branches = br#"{"branches":[{"id":"b1","name":"B"}]}"#;
    let typed = text_of(&typed());
    let damaged_branch = |version: &[u8]| {
        zip_of(&[
            ("document.json", &plain),
            (BRANCH_INDEX, branches),
            ("branches/b1/base.json", &plain),
            ("branches/b1/versions/index.json", index),
            ("branches/b1/versions/v1.json", version),
        ])
    };
    // (Its head is missing.) A typed version fails the load; so does an unreadable one, which
    // would go with the branch.
    load_refused(load_bytes_with_versions(&damaged_branch(&typed)));
    assert!(load_bytes_with_versions(&damaged_branch(b"{nope")).is_err());
    // A plain one goes with it, as before, reported.
    let l = load_bytes_with_versions(&damaged_branch(&plain)).unwrap();
    assert!(l.branches.is_empty() && !l.issues.is_empty());
    // A branch part that can't be read to tell (it can't be kept unparsed) fails the load.
    let bytes =
        zip_of(&[("document.json", &plain), (BRANCH_INDEX, branches), ("branches/b1/base.json", b"{nope"), ("branches/b1/head.json", &plain)]);
    let e = load_bytes_with_versions(&bytes).unwrap_err().to_string();
    assert!(e.contains("can't be read to tell"), "{e}");
}

#[test]
fn a_native_file_is_not_decoded_as_rest_json_even_when_its_title_says_document() {
    let mut d = typed();
    d.name = "document".into();
    let text = text_of(&d);
    assert!(String::from_utf8_lossy(&text).contains(r#""document""#));
    // The REST sniff leaves it alone after a top-level key scan, and the native reader refuses it.
    assert!(rest::try_import(&text, &|_: &str| None).is_none());
    load_refused(load_bytes_with_versions(&text));
    // Without its header, with its `nodes` map and a field named `document`: still left to the
    // native reader (the diagram probe runs before the REST attempt).
    let mut v: Value = serde_json::from_slice(&text).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("format");
    o.remove("version");
    o.insert("document".into(), json!({"type": "DOCUMENT"}));
    assert!(o.contains_key("nodes"));
    let headerless = serde_json::to_vec(&v).unwrap();
    assert!(rest::try_import(&headerless, &|_: &str| None).is_none());
}

/// A zip whose `damaged` member fails its checksum (one stored byte changed after writing).
fn zip_with_damaged(members: &[(&str, &[u8])], damaged: &str, bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut buf);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in members.iter().copied().chain([(damaged, bytes)]) {
        z.start_file(name, stored).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
    let mut out = buf.into_inner();
    let at = out.windows(9).position(|w| w == b"MARKERXYZ").unwrap();
    out[at] = b'N';
    out
}

#[test]
fn a_snapshot_present_but_unreadable_is_never_dropped() {
    let plain = text_of(&Document::new("Plain"));
    let marked = text_of(&Document::new("MARKERXYZ"));
    let index = br#"{"versions":[{"id":"v1","name":"V"}]}"#;
    // A version: kept, unreadable, and the file can't be saved until it's deleted.
    let bytes = zip_with_damaged(&[("document.json", &plain), ("versions/index.json", index)], "versions/v1.json", &marked);
    let l = load_bytes_with_versions(&bytes).unwrap();
    assert_eq!(l.versions.len(), 1, "{:?}", l.issues);
    assert!(l.versions[0].has_typed_records().is_err());
    assert!(l.versions[0].document().is_err());
    assert!(save_bytes_with_history(&l.doc, None, &l.versions, &l.branches).is_err());
    // A branch part: the load fails (a branch can't be kept unparsed).
    let branches = br#"{"branches":[{"id":"b1","name":"B"}]}"#;
    let bytes =
        zip_with_damaged(&[("document.json", &plain), (BRANCH_INDEX, branches), ("branches/b1/base.json", &plain)], "branches/b1/head.json", &marked);
    let e = load_bytes_with_versions(&bytes).unwrap_err().to_string();
    assert!(e.contains("can't be read to tell"), "{e}");
    // Missing parts are still only skipped.
    let l = load_bytes_with_versions(&zip_of(&[("document.json", &plain), (BRANCH_INDEX, branches), ("branches/b1/base.json", &plain)])).unwrap();
    assert!(l.branches.is_empty());
}

#[test]
fn an_unreadable_index_beside_its_snapshots_fails_the_load() {
    let plain = text_of(&Document::new("Plain"));
    // A version index that can't be read, with a snapshot it would list: refused.
    let e = load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", b"{nope"), ("versions/v1.json", &plain)]))
        .unwrap_err()
        .to_string();
    assert!(e.contains("version history can't be read"), "{e}");
    // A branch index likewise.
    let e = load_bytes_with_versions(&zip_of(&[("document.json", &plain), (BRANCH_INDEX, b"{nope"), ("branches/b1/head.json", &plain)]))
        .unwrap_err()
        .to_string();
    assert!(e.contains("branches can't be read"), "{e}");
    // Without snapshots beside it, the damaged index is only skipped, as before.
    let l = load_bytes_with_versions(&zip_of(&[("document.json", &plain), ("versions/index.json", b"{nope"), (BRANCH_INDEX, b"{nope")])).unwrap();
    assert!(l.versions.is_empty() && l.branches.is_empty() && l.issues.len() == 2, "{:?}", l.issues);
}

#[test]
fn every_typed_key_counts_and_a_non_map_is_refused_by_its_first_byte() {
    assert_eq!(typed_probe::typed(br#"{"class_diagrams": {"1": {}}}"#), Ok(true));
    assert_eq!(typed_probe::typed(br#"{"class_diagrams": {}, "use_cases": null}"#), Ok(false));
    // A string (or list, number...) under a typed key is refused from its first byte, before serde
    // reads it: the error names the byte, not the text.
    let huge = "x".repeat(1 << 20);
    for text in [format!(r#"{{"use_cases": "{huge}"}}"#), format!(r#"{{"name": "a", "class_diagrams": ["{huge}"]}}"#)] {
        let e = typed_probe::typed(text.as_bytes()).unwrap_err();
        assert!(e.contains("isn't a map") && e.len() < 200, "{}", &e[..e.len().min(200)]);
    }
    // Spelt with an escape, still the same key.
    let escaped = format!(r#"{{"use{}u005fcases": "a"}}"#, char::from(92u8));
    assert!(typed_probe::typed(escaped.as_bytes()).unwrap_err().contains("isn't a map"));
    // A typed key nested inside another value isn't the document's.
    assert_eq!(typed_probe::typed(br#"{"meta": {"use_cases": "a"}, "x": "}\"{"}"#), Ok(false));
}

#[test]
fn snapshots_an_index_doesnt_list_stay_in_the_file_and_block_saving_over_it() {
    let plain = text_of(&Document::new("Plain"));
    let typed = text_of(&typed());
    // An index that reads as empty, with a (typed) snapshot beside it: nothing is dropped
    // silently; the history stays in the file and saving over the file is refused.
    for (index, member) in [("versions/index.json", "versions/v1.json"), (BRANCH_INDEX, "branches/b1/head.json")] {
        let empty: &[u8] = if index == BRANCH_INDEX { br#"{"branches": null}"# } else { br#"{"versions": null}"# };
        let l = load_bytes_with_versions(&zip_of(&[("document.json", &plain), (index, empty), (member, &typed)])).unwrap();
        assert_eq!(l.history_past_budget, 1, "{index}: {:?}", l.issues);
        assert!(l.issues.iter().any(|i| i.contains("index doesn't list")), "{:?}", l.issues);
    }
}
