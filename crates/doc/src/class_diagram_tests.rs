use crate::class_diagram::text::{format_attribute, format_literal, format_operation, parse_attribute, parse_literal, parse_operation};
use crate::class_diagram::{
    Attribute, CARD_LAYERS, ClassError, Classifier, ClassifierId, End, Kind, Link, LinkId, LinkKind, Literal, MAX_LAYERS, MAX_LINKS, MemberId, Model,
    Navigable, Operation, Param, Visibility, Warning, check, connection_kinds, layer_estimate, validate,
};

fn card(id: u64, kind: Kind, name: &str, x: f64) -> Classifier {
    let mut c = Classifier {
        id: ClassifierId(id),
        kind,
        name: name.to_string(),
        is_abstract: false,
        stereotype: None,
        x,
        y: 0.0,
        w: 160.0,
        h: 0.0,
        min_w: 80.0,
        min_h: 0.0,
        attributes: Vec::new(),
        operations: Vec::new(),
        literals: Vec::new(),
        show_attributes: true,
        show_operations: true,
    };
    fit(&mut c);
    c
}

/// Sizes a card to its floor, as the engine's measure would at the least.
fn fit(c: &mut Classifier) {
    let (_, h) = c.floor();
    c.min_h = h;
    c.h = h;
}

fn op(id: u64, name: &str) -> Operation {
    Operation {
        id: MemberId(id),
        visibility: Visibility::Public,
        name: name.into(),
        params: vec![],
        return_type: None,
        is_static: false,
        is_abstract: false,
    }
}

fn attr(id: u64, name: &str) -> Attribute {
    Attribute {
        id: MemberId(id),
        visibility: Visibility::Private,
        name: name.into(),
        type_name: None,
        multiplicity: None,
        default: None,
        is_static: false,
    }
}

fn link(id: u64, kind: LinkKind, a: u64, b: u64) -> Link {
    Link { id: LinkId(id), kind, ends: [End::at(ClassifierId(a)), End::at(ClassifierId(b))], label: None }
}

/// Book (class) realizes Borrowable (interface); Loan → Book; Shelf ◇— Book.
fn library() -> Model {
    let mut book = card(1, Kind::Class, "Book", 0.0);
    book.operations.push(op(10, "isAvailable"));
    book.attributes.push(attr(11, "title"));
    fit(&mut book);
    let mut borrowable = card(2, Kind::Interface, "Borrowable", 300.0);
    borrowable.operations.push(op(12, "isAvailable"));
    fit(&mut borrowable);
    let loan = card(3, Kind::Class, "Loan", 600.0);
    let shelf = card(4, Kind::Class, "Shelf", 900.0);
    let mut status = card(5, Kind::Enumeration, "LoanStatus", 1200.0);
    status.literals.push(Literal { id: MemberId(13), name: "Open".into() });
    fit(&mut status);
    Model {
        schema: 1,
        classifiers: vec![book, borrowable, loan, shelf, status],
        links: vec![
            link(20, LinkKind::Realization, 1, 2),
            link(21, LinkKind::Association, 3, 1),
            link(22, LinkKind::Aggregation, 4, 1),
            link(23, LinkKind::Dependency, 3, 5),
        ],
    }
}

#[test]
fn the_library_is_valid_and_has_nothing_unfinished() {
    let m = library();
    assert_eq!(validate(&m), Ok(()));
    assert_eq!(check(&m), vec![]);
}

#[test]
fn a_model_round_trips_through_json() {
    let m = library();
    let json = serde_json::to_string(&m).unwrap();
    let back: Model = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

#[test]
fn an_unknown_schema_or_field_is_refused() {
    let mut m = library();
    m.schema = 2;
    assert_eq!(validate(&m), Err(ClassError::UnsupportedSchema(2)));
    assert!(serde_json::from_str::<Model>(r#"{"schema":1,"classifiers":[],"links":[],"extra":1}"#).is_err());
    assert!(serde_json::from_str::<Model>(r#"{"classifiers":[]}"#).is_err(), "schema is required");
}

#[test]
fn ids_are_unique_across_classifiers_members_and_links() {
    let mut m = library();
    m.links[0].id = LinkId(10);
    assert_eq!(validate(&m), Err(ClassError::DuplicateId(10)));
}

#[test]
fn cards_keep_their_gap_and_their_floor() {
    let mut m = library();
    m.classifiers[1].x = 170.0; // 10 right of Book's edge, under the gap
    assert!(matches!(validate(&m), Err(ClassError::Overlap(..))));
    let mut m = library();
    m.classifiers[0].attributes.push(attr(30, "isbn"));
    assert_eq!(validate(&m), Err(ClassError::TooSmall(ClassifierId(1))), "a new row needs room");
    let mut m = library();
    m.classifiers[0].x = f64::NAN;
    assert_eq!(validate(&m), Err(ClassError::BadGeometry(ClassifierId(1))));
}

#[test]
fn member_rules() {
    let mut m = library();
    m.classifiers[0].attributes.push(attr(30, "title"));
    fit(&mut m.classifiers[0]);
    assert!(matches!(validate(&m), Err(ClassError::DuplicateAttribute { .. })));

    // Overloads are allowed; the same signature twice isn't.
    let mut m = library();
    let mut over = op(30, "isAvailable");
    over.params.push(Param { name: "on".into(), type_name: Some("Date".into()) });
    m.classifiers[0].operations.push(over);
    fit(&mut m.classifiers[0]);
    assert_eq!(validate(&m), Ok(()));
    m.classifiers[0].operations.push(op(31, "isAvailable"));
    fit(&mut m.classifiers[0]);
    assert!(matches!(validate(&m), Err(ClassError::DuplicateOperation { .. })));

    let mut m = library();
    m.classifiers[0].literals.push(Literal { id: MemberId(30), name: "X".into() });
    fit(&mut m.classifiers[0]);
    assert_eq!(validate(&m), Err(ClassError::LiteralsOutsideEnumeration(ClassifierId(1))));

    let mut m = library();
    m.classifiers[0].attributes[0].name = "has space".into();
    assert!(matches!(validate(&m), Err(ClassError::BadText { .. })));
    let mut m = library();
    m.classifiers[0].attributes[0].default = Some("\"open".into());
    assert!(matches!(validate(&m), Err(ClassError::BadText { what: "the default", .. })));
}

#[test]
fn relationship_rules() {
    let with = |l: Link| {
        let mut m = library();
        m.links.push(l);
        validate(&m)
    };
    assert_eq!(with(link(30, LinkKind::Generalization, 1, 2)), Err(ClassError::WrongEnds(LinkId(30), "a generalization joins two of the same kind")));
    assert_eq!(with(link(30, LinkKind::Generalization, 5, 5)), Err(ClassError::SelfLink(LinkId(30))));
    assert_eq!(
        with(link(30, LinkKind::Realization, 2, 2)),
        Err(ClassError::SelfLink(LinkId(30))),
        "a realization never joins a classifier to itself"
    );
    assert_eq!(
        with(link(30, LinkKind::Realization, 3, 4)),
        Err(ClassError::WrongEnds(LinkId(30), "a class or an enumeration realizes an interface"))
    );
    assert_eq!(with(link(30, LinkKind::Association, 3, 99)), Err(ClassError::MissingEnd { link: LinkId(30), missing: ClassifierId(99) }));
    assert_eq!(with(link(30, LinkKind::Association, 3, 3)), Ok(()), "a self-association is allowed");
    assert_eq!(with(link(30, LinkKind::Composition, 3, 3)), Err(ClassError::SelfLink(LinkId(30))));
    assert_eq!(with(link(30, LinkKind::Realization, 1, 2)), Err(ClassError::DuplicateLink(LinkId(30))));

    // The same pair again is another association when its roles differ.
    let mut roles = link(30, LinkKind::Association, 1, 3);
    roles.ends[0].role = Some("item".into());
    assert_eq!(with(roles), Ok(()));
    assert_eq!(with(link(30, LinkKind::Association, 1, 3)), Err(ClassError::DuplicateLink(LinkId(30))), "unordered ends");

    let mut whole = link(30, LinkKind::Composition, 4, 3);
    whole.ends[0].multiplicity = Some("2".into());
    assert_eq!(with(whole), Err(ClassError::CompositionWhole(LinkId(30))));

    let mut stray = link(30, LinkKind::Generalization, 3, 4);
    stray.ends[1].navigable = Navigable::Yes;
    assert_eq!(with(stray), Err(ClassError::StrayEndDetail(LinkId(30))));

    let mut bad = link(30, LinkKind::Association, 4, 3);
    bad.ends[1].multiplicity = Some("3..1".into());
    assert!(matches!(with(bad), Err(ClassError::BadMultiplicity { .. })));
}

#[test]
fn generalization_and_composition_stay_acyclic() {
    let mut m = library();
    m.links.push(link(30, LinkKind::Generalization, 3, 4));
    m.links.push(link(31, LinkKind::Generalization, 4, 3));
    assert!(matches!(validate(&m), Err(ClassError::Cycle("generalization", _))));
    let mut m = library();
    m.links.push(link(30, LinkKind::Composition, 3, 4));
    m.links.push(link(31, LinkKind::Composition, 4, 3));
    assert!(matches!(validate(&m), Err(ClassError::Cycle("composition", _))));
}

#[test]
fn the_check_reports_what_is_unfinished() {
    let mut m = library();
    m.classifiers[0].operations.clear();
    m.classifiers[2].operations.push(Operation { is_abstract: true, ..op(30, "due") });
    m.classifiers[4].literals.clear();
    fit(&mut m.classifiers[0]);
    fit(&mut m.classifiers[2]);
    fit(&mut m.classifiers[4]);
    let mut lone = card(40, Kind::Class, "Clock", 1500.0);
    fit(&mut lone);
    m.classifiers.push(lone);
    assert_eq!(validate(&m), Ok(()));
    assert_eq!(
        check(&m),
        vec![
            Warning::MissingOperation {
                classifier: ClassifierId(1),
                interface: ClassifierId(2),
                operation: MemberId(12),
                name: "isAvailable".into()
            },
            Warning::AbstractInConcrete { classifier: ClassifierId(3), operation: MemberId(30) },
            Warning::EmptyEnumeration(ClassifierId(5)),
            Warning::Unconnected(ClassifierId(40)),
        ]
    );
}

#[test]
fn connection_kinds_follow_the_rules() {
    let m = library();
    let kinds = connection_kinds(&m, ClassifierId(3), ClassifierId(4));
    assert_eq!(kinds, vec![LinkKind::Association, LinkKind::Aggregation, LinkKind::Composition, LinkKind::Generalization, LinkKind::Dependency]);
    assert_eq!(
        connection_kinds(&m, ClassifierId(3), ClassifierId(2)),
        vec![LinkKind::Association, LinkKind::Aggregation, LinkKind::Composition, LinkKind::Realization, LinkKind::Dependency]
    );
    assert!(!connection_kinds(&m, ClassifierId(1), ClassifierId(2)).contains(&LinkKind::Realization), "already there");
    assert!(connection_kinds(&m, ClassifierId(1), ClassifierId(99)).is_empty());
}

#[test]
fn decoding_stops_at_the_caps() {
    let many: Vec<String> = (0..201).map(|i| format!(r#"{{"id":{i},"name":"C{i}","x":0,"y":0,"w":1,"h":1,"minW":1,"minH":1}}"#)).collect();
    let json = format!(r#"{{"schema":1,"classifiers":[{}]}}"#, many.join(","));
    let e = serde_json::from_str::<Model>(&json).unwrap_err().to_string();
    assert!(e.contains("more than 200 classifiers"), "{e}");
    let long = "x".repeat(801);
    let json = format!(r#"{{"schema":1,"classifiers":[{{"id":1,"name":"{long}","x":0,"y":0,"w":1,"h":1,"minW":1,"minH":1}}]}}"#);
    assert!(serde_json::from_str::<Model>(&json).is_err());
}

// ---- text form ----

#[test]
fn attributes_parse_from_their_text() {
    let a = parse_attribute(MemberId(1), r#"- balance : Money [0..1] = "0""#).unwrap();
    assert_eq!(a.visibility, Visibility::Private);
    assert_eq!(a.name, "balance");
    assert_eq!(a.type_name.as_deref(), Some("Money"));
    assert_eq!(a.multiplicity.as_deref(), Some("0..1"));
    assert_eq!(a.default.as_deref(), Some("\"0\""));
    let s = parse_attribute(MemberId(1), "# static count:Int").unwrap();
    assert!(s.is_static);
    assert_eq!((s.visibility, s.name.as_str(), s.type_name.as_deref()), (Visibility::Protected, "count", Some("Int")));
    let bare = parse_attribute(MemberId(1), "title").unwrap();
    assert_eq!((bare.visibility, bare.type_name), (Visibility::Public, None));
    // A member may be named like a keyword.
    let k = parse_attribute(MemberId(1), "+ static : Bool").unwrap();
    assert_eq!((k.name.as_str(), k.is_static), ("static", false));
}

#[test]
fn operations_parse_from_their_text() {
    let o = parse_operation(MemberId(1), "+ static abstract submit(id: Int, note) : Bool").unwrap();
    assert!(o.is_static && o.is_abstract);
    assert_eq!(o.name, "submit");
    assert_eq!(o.params, vec![Param { name: "id".into(), type_name: Some("Int".into()) }, Param { name: "note".into(), type_name: None }]);
    assert_eq!(o.return_type.as_deref(), Some("Bool"));
    assert!(parse_operation(MemberId(1), "~ reset()").unwrap().params.is_empty());
}

#[test]
fn bad_lines_say_where() {
    let e = parse_attribute(MemberId(1), "+ two words : T").unwrap_err();
    assert_eq!(e.at, 2);
    assert!(parse_attribute(MemberId(1), "+ x : ").unwrap_err().message.contains("type"));
    assert!(parse_attribute(MemberId(1), "x [1..").is_err());
    assert!(parse_attribute(MemberId(1), "x [3..1]").is_err());
    assert!(parse_attribute(MemberId(1), "x = ").is_err());
    assert!(parse_attribute(MemberId(1), r#"x = "open"#).is_err());
    assert!(parse_attribute(MemberId(1), "abstract x").is_err());
    assert!(parse_operation(MemberId(1), "go(").is_err());
    assert!(parse_operation(MemberId(1), "go() Bool").is_err());
    assert!(parse_operation(MemberId(1), "go(a, b, c, d, e, f, g, h, i)").is_err());
    assert!(parse_literal(MemberId(1), "two words").is_err());
    assert!(parse_attribute(MemberId(1), "line\nbreak").is_err());
}

#[test]
fn formatting_parses_back_to_the_same_member() {
    let a = Attribute {
        id: MemberId(7),
        visibility: Visibility::Package,
        name: "tags".into(),
        type_name: Some("List of Tag".into()),
        multiplicity: Some("*".into()),
        default: Some(r#""a \"b\" \\""#.into()),
        is_static: true,
    };
    assert_eq!(parse_attribute(MemberId(7), &format_attribute(&a)), Ok(a));
    let o = Operation {
        id: MemberId(8),
        visibility: Visibility::Protected,
        name: "find".into(),
        params: vec![Param { name: "key".into(), type_name: Some("Key".into()) }, Param { name: "all".into(), type_name: None }],
        return_type: Some("Book".into()),
        is_static: true,
        is_abstract: true,
    };
    assert_eq!(parse_operation(MemberId(8), &format_operation(&o)), Ok(o));
    let l = Literal { id: MemberId(9), name: "Overdue".into() };
    assert_eq!(parse_literal(MemberId(9), &format_literal(&l)), Ok(l));
}

#[test]
fn cards_apart_on_both_axes_measure_the_diagonal() {
    let mut m = library();
    // 20 right of and 20 below Book: about 28.3 apart, over the 24 gap.
    let (w, h) = (m.classifiers[0].w, m.classifiers[0].h);
    m.classifiers[1].x = w + 20.0;
    m.classifiers[1].y = h + 20.0;
    assert_eq!(validate(&m), Ok(()));
    m.classifiers[1].x = w + 10.0;
    m.classifiers[1].y = h + 10.0;
    assert!(matches!(validate(&m), Err(ClassError::Overlap(..))));
}

#[test]
fn unicode_line_separators_break_a_line() {
    let e = parse_attribute(MemberId(1), "é : A\u{2028}B").unwrap_err();
    assert_eq!(e.at, 5, "a character index, after the two-byte é");
    let mut m = library();
    m.classifiers[0].attributes[0].type_name = Some("A\u{2029}B".into());
    assert!(matches!(validate(&m), Err(ClassError::BadText { .. })));
    let mut m = library();
    m.classifiers[0].attributes[0].default = Some("a\u{2028}b".into());
    assert!(matches!(validate(&m), Err(ClassError::BadText { what: "the default", .. })));
}

#[test]
fn no_connection_is_offered_past_the_link_or_id_limits() {
    let mut m = library();
    let mut id = 100;
    while m.links.len() < MAX_LINKS {
        let mut l = link(id, LinkKind::Association, 3, 4);
        l.ends[0].role = Some(format!("r{id}"));
        m.links.push(l);
        id += 1;
    }
    assert!(connection_kinds(&m, ClassifierId(1), ClassifierId(4)).is_empty());
    let mut m = library();
    m.links[0].id = LinkId(u64::MAX);
    assert!(connection_kinds(&m, ClassifierId(3), ClassifierId(4)).is_empty(), "no ID left to give");
}

#[test]
fn the_layer_estimate_is_capped() {
    let mut m = library();
    assert!(layer_estimate(&m) < MAX_LAYERS);
    // Every other cap holds: 900 relationships, then members a row at a time.
    let mut id = 100;
    while m.links.len() < 900 {
        let mut l = link(id, LinkKind::Association, 3, 4);
        l.ends[0].role = Some(format!("r{id}"));
        m.links.push(l);
        id += 1;
    }
    let mut x = 2000.0;
    'fill: loop {
        let mut c = card(id, Kind::Class, &format!("C{id}"), x);
        id += 1;
        for _ in 0..100 {
            if layer_estimate(&m) + CARD_LAYERS + c.member_count() >= MAX_LAYERS {
                fit(&mut c);
                m.classifiers.push(c);
                break 'fill;
            }
            c.attributes.push(attr(id, &format!("a{id}")));
            id += 1;
        }
        fit(&mut c);
        m.classifiers.push(c);
        x += 200.0;
    }
    assert_eq!(layer_estimate(&m), MAX_LAYERS);
    assert_eq!(validate(&m), Ok(()), "at the cap");
    assert!(connection_kinds(&m, ClassifierId(1), ClassifierId(4)).is_empty(), "a new link would pass the layer cap");
    let last = m.classifiers.len() - 1;
    m.classifiers[last].attributes.push(attr(id, "one_more"));
    fit(&mut m.classifiers[last]);
    assert_eq!(validate(&m), Err(ClassError::TooManyLayers), "one past it");
}

#[test]
fn decoded_text_keeps_the_character_cap() {
    let json = format!(r#"{{"schema":1,"classifiers":[{{"id":1,"name":"{}","x":0,"y":0,"w":1,"h":1,"minW":1,"minH":1}}]}}"#, "x".repeat(201));
    assert!(serde_json::from_str::<Model>(&json).is_err());
}

mod props {
    use proptest::prelude::*;

    use super::*;

    fn word() -> impl Strategy<Value = String> {
        // Keywords are included on purpose: a member may be named `static` or `abstract`.
        prop_oneof![Just("static".to_string()), Just("abstract".to_string()), "[A-Za-z_][A-Za-z0-9_]{0,12}"]
    }

    fn vis() -> impl Strategy<Value = Visibility> {
        prop_oneof![Just(Visibility::Public), Just(Visibility::Private), Just(Visibility::Protected), Just(Visibility::Package)]
    }

    proptest! {
        #[test]
        fn any_line_parses_without_panicking(s in "\\PC{0,300}") {
            let short = |t: &str| t.chars().count() <= crate::class_diagram::MAX_TEXT;
            if let Ok(a) = parse_attribute(MemberId(1), &s) {
                prop_assert!(short(&a.name) && a.type_name.as_deref().is_none_or(short) && a.default.as_deref().is_none_or(short));
            }
            if let Ok(o) = parse_operation(MemberId(1), &s) {
                prop_assert!(short(&o.name) && o.params.len() <= crate::class_diagram::MAX_PARAMS);
            }
            if let Ok(l) = parse_literal(MemberId(1), &s) {
                prop_assert!(short(&l.name));
            }
        }

        #[test]
        fn attributes_round_trip(v in vis(), name in word(), t in proptest::option::of(word()), st in any::<bool>(),
                                  m in proptest::option::of(prop_oneof![Just("1".to_string()), Just("0..*".to_string()), Just("*".to_string())]),
                                  d in proptest::option::of(prop_oneof![
                                      "[a-z0-9]{1,8}".boxed(),
                                      "[a-z \\\\\"]{0,8}".prop_map(|b| format!("\"{}\"", b.replace('\\', "\\\\").replace('"', "\\\""))).boxed(),
                                  ])) {
            let a = Attribute { id: MemberId(1), visibility: v, name, type_name: t, multiplicity: m, default: d, is_static: st };
            prop_assert_eq!(parse_attribute(MemberId(1), &format_attribute(&a)), Ok(a));
        }

        #[test]
        fn operations_round_trip(v in vis(), name in word(), ps in proptest::collection::vec((word(), proptest::option::of(word())), 0..=8),
                                  r in proptest::option::of(word()), st in any::<bool>(), ab in any::<bool>()) {
            let params = ps.into_iter().map(|(name, type_name)| Param { name, type_name }).collect();
            let o = Operation { id: MemberId(1), visibility: v, name, params, return_type: r, is_static: st, is_abstract: ab };
            prop_assert_eq!(parse_operation(MemberId(1), &format_operation(&o)), Ok(o));
        }

        #[test]
        fn literals_round_trip(name in word()) {
            let l = Literal { id: MemberId(1), name };
            prop_assert_eq!(parse_literal(MemberId(1), &format_literal(&l)), Ok(l));
        }
    }
}
