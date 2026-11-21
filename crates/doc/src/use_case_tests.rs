//! The Use Case model's validator and drafting check, on the Room booking example.

use proptest::prelude::*;
use serde_json::json;

use crate::use_case::{
    Actor, ActorId, Blocked, ELEMENT_GAP, ExtensionPoint, GenEnds, Graph, Link, LinkId, Model, PointId, Presentation, Subject, SubjectId,
    USE_CASE_SCHEMA, UseCase, UseCaseError, UseCaseId, Warning, check, connection_kinds, connection_options, find_cycle, multiplicity_ok, validate,
};

const ROOM: SubjectId = SubjectId(1);
const GUEST: ActorId = ActorId(2);
const MEMBER: ActorId = ActorId(3);
const RESERVE: UseCaseId = UseCaseId(4);
const AVAILABILITY: UseCaseId = UseCaseId(5);
const DISCOUNT: UseCaseId = UseCaseId(6);
const MEETING: UseCaseId = UseCaseId(7);
const BEFORE_TOTAL: PointId = PointId(8);

fn actor(id: ActorId, name: &str, x: f64, y: f64) -> Actor {
    Actor { id, name: name.into(), presentation: Presentation::Person, x, y, w: 60.0, h: 120.0, min_w: 40.0, min_h: 96.0 }
}

fn case(id: UseCaseId, name: &str, x: f64, y: f64) -> UseCase {
    UseCase { id, name: name.into(), x, y, w: 160.0, h: 70.0, min_w: 120.0, min_h: 56.0, subject: Some(ROOM), points: vec![] }
}

/// The owner's Room booking example: one subject, four use cases inside, two actors outside.
fn room_booking() -> Model {
    let mut reserve = case(RESERVE, "Reserve room", 340.0, 40.0);
    reserve.points.push(ExtensionPoint { id: BEFORE_TOTAL, name: "Before total".into() });
    Model {
        schema: USE_CASE_SCHEMA,
        actors: vec![actor(GUEST, "Guest", 100.0, 40.0), actor(MEMBER, "Member", 100.0, 300.0)],
        use_cases: vec![
            reserve,
            case(AVAILABILITY, "Check availability", 600.0, 40.0),
            case(DISCOUNT, "Apply member discount", 340.0, 200.0),
            case(MEETING, "Reserve meeting room", 600.0, 200.0),
        ],
        subjects: vec![Subject { id: ROOM, name: "Room booking".into(), x: 300.0, y: 0.0, width: 600.0, height: 700.0 }],
        links: vec![
            Link::Association { id: LinkId(10), actor: GUEST, use_case: RESERVE, actor_multiplicity: None, use_case_multiplicity: None },
            Link::Include { id: LinkId(11), including: RESERVE, included: AVAILABILITY },
            Link::Extend {
                id: LinkId(12),
                extending: DISCOUNT,
                base: RESERVE,
                points: vec![BEFORE_TOTAL],
                condition: Some("member is eligible".into()),
            },
            Link::Generalization { id: LinkId(13), ends: GenEnds::Actors { specific: MEMBER, general: GUEST } },
            Link::Generalization { id: LinkId(14), ends: GenEnds::UseCases { specific: MEETING, general: RESERVE } },
        ],
    }
}

fn with_link(link: Link) -> Result<(), UseCaseError> {
    let mut m = room_booking();
    m.links.push(link);
    validate(&m)
}

#[test]
fn the_room_booking_example_is_valid_and_complete() {
    let m = room_booking();
    assert_eq!(validate(&m), Ok(()));
    assert_eq!(check(&m), vec![]);
}

#[test]
fn an_empty_model_is_valid() {
    assert_eq!(validate(&Model::default()), Ok(()));
}

#[test]
fn the_model_round_trips_through_json_in_order() {
    let mut m = room_booking();
    m.use_cases[0].points.push(ExtensionPoint { id: PointId(9), name: "Before payment".into() });
    m.links[2] = Link::Extend { id: LinkId(12), extending: DISCOUNT, base: RESERVE, points: vec![PointId(9), BEFORE_TOTAL], condition: None };
    let text = serde_json::to_string(&m).unwrap();
    let back: Model = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["links"][2], json!({"kind": "extend", "id": 12, "extending": 6, "base": 4, "points": [9, 8]}));
    assert_eq!(v["links"][3]["ends"], json!({"of": "actors", "specific": 3, "general": 2}));
}

#[test]
fn decoding_refuses_a_missing_size_an_unknown_field_and_an_unknown_kind() {
    let v = serde_json::to_value(room_booking()).unwrap();
    let mut no_w = v.clone();
    no_w["actors"][0].as_object_mut().unwrap().remove("w");
    assert!(serde_json::from_value::<Model>(no_w).is_err());
    let mut extra = v.clone();
    extra["useCases"][0]["colour"] = json!("red");
    assert!(serde_json::from_value::<Model>(extra).is_err());
    let mut kind = v;
    kind["links"][0]["kind"] = json!("dependency");
    assert!(serde_json::from_value::<Model>(kind).is_err());
}

#[test]
fn an_association_between_two_actors_cannot_be_decoded() {
    let mut v = serde_json::to_value(room_booking()).unwrap();
    // The use case end names an actor's ID: the kinds don't match, so the model is refused.
    v["links"][0]["useCase"] = json!(MEMBER.0);
    let m: Model = serde_json::from_value(v).unwrap();
    assert_eq!(validate(&m), Err(UseCaseError::Missing { what: "use case", id: MEMBER.0 }));
}

#[test]
fn an_include_cycle_is_refused() {
    let r = with_link(Link::Include { id: LinkId(20), including: AVAILABILITY, included: RESERVE });
    assert!(matches!(r, Err(UseCaseError::Cycle { graph: Graph::Include, .. })), "{r:?}");
}

#[test]
fn an_actor_generalization_cycle_is_refused() {
    let r = with_link(Link::Generalization { id: LinkId(20), ends: GenEnds::Actors { specific: GUEST, general: MEMBER } });
    assert!(matches!(r, Err(UseCaseError::Cycle { graph: Graph::ActorGeneralization, .. })), "{r:?}");
}

#[test]
fn an_extend_cycle_is_refused() {
    let mut m = room_booking();
    m.use_cases[2].points.push(ExtensionPoint { id: PointId(30), name: "Start".into() });
    m.links.push(Link::Extend { id: LinkId(20), extending: RESERVE, base: DISCOUNT, points: vec![PointId(30)], condition: None });
    assert!(matches!(validate(&m), Err(UseCaseError::Cycle { graph: Graph::Extend, .. })));
}

#[test]
fn a_self_include_is_refused() {
    assert_eq!(with_link(Link::Include { id: LinkId(20), including: RESERVE, included: RESERVE }), Err(UseCaseError::SelfLink(LinkId(20))));
}

#[test]
fn a_repeated_include_is_refused() {
    assert_eq!(with_link(Link::Include { id: LinkId(20), including: RESERVE, included: AVAILABILITY }), Err(UseCaseError::DuplicateLink(LinkId(20))));
}

#[test]
fn a_repeated_association_is_refused() {
    let r = with_link(Link::Association {
        id: LinkId(20),
        actor: GUEST,
        use_case: RESERVE,
        actor_multiplicity: Some("1".into()),
        use_case_multiplicity: None,
    });
    assert_eq!(r, Err(UseCaseError::DuplicateLink(LinkId(20))));
}

#[test]
fn an_extend_point_from_another_use_case_is_refused() {
    let mut m = room_booking();
    m.use_cases[1].points.push(ExtensionPoint { id: PointId(30), name: "Late".into() });
    m.links[2] = Link::Extend { id: LinkId(12), extending: DISCOUNT, base: RESERVE, points: vec![PointId(30)], condition: None };
    assert_eq!(validate(&m), Err(UseCaseError::PointNotOnBase { point: PointId(30), base: RESERVE }));
}

#[test]
fn an_extend_without_points_or_with_a_repeated_point_is_refused() {
    for points in [vec![], vec![BEFORE_TOTAL, BEFORE_TOTAL]] {
        let mut m = room_booking();
        m.links[2] = Link::Extend { id: LinkId(12), extending: DISCOUNT, base: RESERVE, points, condition: None };
        assert_eq!(validate(&m), Err(UseCaseError::BadExtendPoints { link: LinkId(12) }));
    }
}

#[test]
fn removing_a_referenced_point_leaves_a_model_the_validator_refuses() {
    let mut m = room_booking();
    m.use_cases[0].points.clear();
    assert_eq!(validate(&m), Err(UseCaseError::PointNotOnBase { point: BEFORE_TOTAL, base: RESERVE }));
}

#[test]
fn a_long_condition_is_refused() {
    let mut m = room_booking();
    if let Link::Extend { condition, .. } = &mut m.links[2] {
        *condition = Some("x".repeat(201));
    }
    assert_eq!(validate(&m), Err(UseCaseError::BadCondition(LinkId(12))));
}

#[test]
fn multiplicities_follow_the_grammar() {
    for ok in ["1", "0..1", "*", "1..*", "2..5", "0", "999999999"] {
        assert!(multiplicity_ok(ok), "{ok}");
    }
    for bad in ["", "5..2", "a", "1..", "..1", "*..1", "1..2..3", "-1", "+1", "1234567890", " 1", "1.5"] {
        assert!(!multiplicity_ok(bad), "{bad}");
    }
}

#[test]
fn a_bad_multiplicity_is_refused() {
    let mut m = room_booking();
    m.links[0] =
        Link::Association { id: LinkId(10), actor: GUEST, use_case: RESERVE, actor_multiplicity: None, use_case_multiplicity: Some("many".into()) };
    assert_eq!(validate(&m), Err(UseCaseError::BadMultiplicity { link: LinkId(10), text: "many".into() }));
}

#[test]
fn a_member_outside_its_subject_is_refused() {
    let mut m = room_booking();
    m.use_cases[1].x = 750.0; // its right edge passes the subject's padding
    assert_eq!(validate(&m), Err(UseCaseError::OutsideSubject { use_case: AVAILABILITY, subject: ROOM }));
}

#[test]
fn an_actor_inside_a_subject_is_refused() {
    let mut m = room_booking();
    m.actors[0].x = 400.0;
    m.actors[0].y = 500.0;
    assert_eq!(validate(&m), Err(UseCaseError::InsideSubject { what: "actor", id: GUEST.0, subject: ROOM }));
}

#[test]
fn a_detached_use_case_still_inside_the_rectangle_is_refused() {
    let mut m = room_booking();
    m.use_cases[1].subject = None;
    assert_eq!(validate(&m), Err(UseCaseError::InsideSubject { what: "use case", id: AVAILABILITY.0, subject: ROOM }));
}

#[test]
fn a_detached_use_case_moved_outside_is_valid_and_reported_unassigned() {
    let mut m = room_booking();
    m.use_cases[1].subject = None;
    m.use_cases[1].x = 1_000.0;
    assert_eq!(validate(&m), Ok(()));
    assert_eq!(check(&m), vec![Warning::Unassigned(AVAILABILITY)]);
}

#[test]
fn actors_edge_to_edge_are_refused() {
    let mut m = room_booking();
    m.actors[1].y = 40.0;
    m.actors[1].x = 160.0; // touching Guest's right edge
    assert_eq!(validate(&m), Err(UseCaseError::TooClose { a: GUEST.0, b: MEMBER.0 }));
}

#[test]
fn actors_exactly_the_gap_apart_are_valid() {
    let mut m = room_booking();
    m.actors[0].x = 20.0;
    m.actors[1].y = 40.0;
    m.actors[1].x = 80.0 + ELEMENT_GAP;
    assert_eq!(validate(&m), Ok(()));
}

#[test]
fn overlapping_subjects_are_refused() {
    let mut m = room_booking();
    m.subjects.push(Subject { id: SubjectId(40), name: "Payments".into(), x: 850.0, y: 0.0, width: 300.0, height: 300.0 });
    assert_eq!(validate(&m), Err(UseCaseError::SubjectsOverlap(ROOM, SubjectId(40))));
}

#[test]
fn a_use_case_can_move_to_a_second_subject() {
    let mut m = room_booking();
    m.subjects.push(Subject { id: SubjectId(40), name: "Payments".into(), x: 1_000.0, y: 0.0, width: 300.0, height: 300.0 });
    m.use_cases[2].subject = Some(SubjectId(40));
    m.use_cases[2].x = 1_040.0;
    m.use_cases[2].y = 40.0;
    assert_eq!(validate(&m), Ok(()));
    assert_eq!(check(&m), vec![]);
}

#[test]
fn sizes_below_the_minimum_or_out_of_range_are_refused() {
    let mut small = room_booking();
    small.use_cases[0].w = 125.0;
    small.use_cases[0].min_w = 130.0;
    assert_eq!(validate(&small), Err(UseCaseError::BelowMinimum { what: "use case", id: RESERVE.0 }));
    for (w, min_w) in [(0.0, 120.0), (-5.0, 120.0), (2_001.0, 120.0), (160.0, 10.0)] {
        let mut m = room_booking();
        m.use_cases[0].w = w;
        m.use_cases[0].min_w = min_w;
        assert_eq!(validate(&m), Err(UseCaseError::BadSize { what: "use case", id: RESERVE.0 }), "{w} {min_w}");
    }
}

#[test]
fn a_system_actor_has_its_own_size_range() {
    let mut m = room_booking();
    m.actors[0].presentation = Presentation::System;
    // 60 wide fits a person, not a system.
    assert_eq!(validate(&m), Err(UseCaseError::BadSize { what: "actor", id: GUEST.0 }));
    m.actors[0].w = 120.0;
    m.actors[0].min_w = 80.0;
    m.actors[0].h = 60.0;
    m.actors[0].min_h = 48.0;
    assert_eq!(validate(&m), Ok(()));
}

#[test]
fn non_finite_or_far_away_numbers_are_refused() {
    for x in [f64::NAN, f64::INFINITY, 2_000_000.0] {
        let mut m = room_booking();
        m.actors[0].x = x;
        assert_eq!(validate(&m), Err(UseCaseError::BadNumber { what: "actor", id: GUEST.0 }), "{x}");
    }
}

#[test]
fn names_must_be_trimmed_and_bounded() {
    for name in ["", " Guest", "Guest ", &"g".repeat(201)] {
        let mut m = room_booking();
        m.actors[0].name = name.into();
        assert_eq!(validate(&m), Err(UseCaseError::BadName { what: "actor", id: GUEST.0, max: 200 }), "{name:?}");
    }
    let mut m = room_booking();
    m.actors[0].name = "Gäst 名前".into();
    assert_eq!(validate(&m), Ok(()));
}

#[test]
fn point_names_are_unique_ignoring_case() {
    let mut m = room_booking();
    m.use_cases[0].points.push(ExtensionPoint { id: PointId(30), name: "BEFORE TOTAL".into() });
    assert!(matches!(validate(&m), Err(UseCaseError::DuplicatePointName { use_case: RESERVE, .. })));
}

#[test]
fn an_id_used_twice_is_refused_across_kinds() {
    let mut m = room_booking();
    m.links[0] = Link::Association { id: LinkId(GUEST.0), actor: GUEST, use_case: RESERVE, actor_multiplicity: None, use_case_multiplicity: None };
    assert_eq!(validate(&m), Err(UseCaseError::DuplicateId(GUEST.0)));
}

#[test]
fn a_member_of_a_missing_subject_is_refused() {
    let mut m = room_booking();
    m.use_cases[0].subject = Some(SubjectId(99));
    assert_eq!(validate(&m), Err(UseCaseError::Missing { what: "subject", id: 99 }));
}

#[test]
fn an_inherited_association_counts_and_a_lone_actor_is_reported() {
    let mut m = room_booking();
    assert!(!check(&m).contains(&Warning::ActorUnconnected(MEMBER)));
    m.actors.push(actor(ActorId(50), "Auditor", 100.0, 500.0));
    assert_eq!(check(&m), vec![Warning::ActorUnconnected(ActorId(50))]);
}

#[test]
fn an_unreached_use_case_and_an_empty_subject_are_reported() {
    let mut m = room_booking();
    m.links.retain(|l| !matches!(l, Link::Include { .. }));
    m.subjects.push(Subject { id: SubjectId(40), name: "Payments".into(), x: 1_000.0, y: 0.0, width: 300.0, height: 300.0 });
    assert_eq!(check(&m), vec![Warning::EmptySubject(SubjectId(40)), Warning::UseCaseUnreached(AVAILABILITY)]);
}

#[test]
fn a_long_chain_has_no_cycle_and_needs_no_deep_stack() {
    let chain: Vec<(u64, u64)> = (0..200_000).map(|i| (i, i + 1)).collect();
    assert_eq!(find_cycle(&chain), None);
    let mut ring = chain;
    ring.push((200_000, 0));
    assert!(find_cycle(&ring).is_some());
}

fn number() -> impl Strategy<Value = f64> {
    prop_oneof![-50.0..2_500.0f64, Just(f64::NAN), Just(f64::INFINITY), Just(-1e9)]
}

prop_compose! {
    fn any_case(id: u64)(x in number(), y in number(), w in number(), h in number(), subject in proptest::option::of(0u64..6), points in 0usize..3)
        -> UseCase {
        let points = (0..points as u64).map(|i| ExtensionPoint { id: PointId(1_000 + id * 10 + i), name: format!("p{i}") }).collect();
        UseCase { id: UseCaseId(id), name: format!("case {id}"), x, y, w, h, min_w: 120.0, min_h: 56.0, subject: subject.map(SubjectId), points }
    }
}

fn any_link(id: u64) -> impl Strategy<Value = Link> {
    (0u8..5, 0u64..12, 0u64..12, proptest::collection::vec(1_000u64..1_200, 0..3)).prop_map(move |(k, a, b, p)| match k {
        0 => Link::Association { id: LinkId(id), actor: ActorId(a), use_case: UseCaseId(b), actor_multiplicity: None, use_case_multiplicity: None },
        1 => Link::Include { id: LinkId(id), including: UseCaseId(a), included: UseCaseId(b) },
        2 => Link::Extend {
            id: LinkId(id),
            extending: UseCaseId(a),
            base: UseCaseId(b),
            points: p.into_iter().map(PointId).collect(),
            condition: None,
        },
        3 => Link::Generalization { id: LinkId(id), ends: GenEnds::Actors { specific: ActorId(a), general: ActorId(b) } },
        _ => Link::Generalization { id: LinkId(id), ends: GenEnds::UseCases { specific: UseCaseId(a), general: UseCaseId(b) } },
    })
}

proptest! {
    /// Any model, however broken, gets an answer from the validator and the check: never a panic.
    #[test]
    fn validate_and_check_never_panic(
        cases in (6u64..12).prop_flat_map(|n| (6..n).map(any_case).collect::<Vec<_>>()),
        links in (0u64..8).prop_flat_map(|n| (0..n).map(|i| any_link(100 + i)).collect::<Vec<_>>()),
        sx in number(), sw in number(),
    ) {
        let subjects = (0u64..2).map(|i| Subject { id: SubjectId(i), name: "S".into(), x: sx + i as f64 * 900.0, y: 0.0, width: sw, height: 800.0 }).collect();
        let actors = (2u64..4).map(|i| actor(ActorId(i), "A", -400.0, i as f64 * 200.0)).collect();
        let m = Model { schema: USE_CASE_SCHEMA, actors, use_cases: cases, subjects, links };
        let _ = validate(&m);
        let _ = check(&m);
    }

    /// Arbitrary JSON never panics the decoder or the validator.
    #[test]
    fn decoding_junk_never_panics(text in "\\PC{0,200}") {
        if let Ok(m) = serde_json::from_str::<Model>(&text) {
            let _ = validate(&m);
        }
    }
}

#[test]
fn too_many_points_are_refused_before_their_ids_are_read() {
    let mut m = room_booking();
    // 33 points that all repeat one ID: the count is refused, not the repeat.
    m.use_cases[0].points = (0..33).map(|i| ExtensionPoint { id: PointId(30), name: format!("p{i}") }).collect();
    assert_eq!(validate(&m), Err(UseCaseError::TooManyPoints(RESERVE)));
}

#[test]
fn an_extend_naming_too_many_points_is_refused_before_they_are_read() {
    let mut m = room_booking();
    m.links[2] = Link::Extend { id: LinkId(12), extending: DISCOUNT, base: RESERVE, points: vec![PointId(999); 9], condition: None };
    assert_eq!(validate(&m), Err(UseCaseError::BadExtendPoints { link: LinkId(12) }));
}

#[test]
fn a_long_multiplicity_is_refused_and_reported_shortened() {
    let long = "1".repeat(10_000);
    assert!(!multiplicity_ok(&long));
    let mut m = room_booking();
    m.links[0] = Link::Association { id: LinkId(10), actor: GUEST, use_case: RESERVE, actor_multiplicity: Some(long), use_case_multiplicity: None };
    assert_eq!(validate(&m), Err(UseCaseError::BadMultiplicity { link: LinkId(10), text: "1".repeat(24) }));
}

#[test]
fn a_diagonal_neighbour_just_past_the_gap_is_valid() {
    let mut m = room_booking();
    // 18 units apart on each axis: about 25.5 units corner to corner.
    m.actors[0].x = 20.0;
    m.actors[1].x = 20.0 + 60.0 + 18.0;
    m.actors[1].y = 40.0 + 120.0 + 18.0;
    assert_eq!(validate(&m), Ok(()));
    // 16 on each axis is about 22.6: too close.
    m.actors[1].x = 20.0 + 60.0 + 16.0;
    m.actors[1].y = 40.0 + 120.0 + 16.0;
    assert_eq!(validate(&m), Err(UseCaseError::TooClose { a: GUEST.0, b: MEMBER.0 }));
}

#[test]
fn point_names_with_the_same_lower_case_are_duplicates() {
    for (a, b) in [("Σ", "σ"), ("Straße", "STRAẞE"), ("Before Total", "before total")] {
        let mut m = room_booking();
        m.use_cases[0].points.push(ExtensionPoint { id: PointId(30), name: a.into() });
        m.use_cases[0].points.push(ExtensionPoint { id: PointId(31), name: b.into() });
        assert!(matches!(validate(&m), Err(UseCaseError::DuplicatePointName { .. })), "{a} {b}");
    }
}

#[test]
fn point_names_with_different_lower_case_letters_are_distinct() {
    for (a, b) in [("I", "ı"), ("σ", "ς"), ("Straße", "STRASSE")] {
        let mut m = room_booking();
        m.use_cases[0].points.push(ExtensionPoint { id: PointId(30), name: a.into() });
        m.use_cases[0].points.push(ExtensionPoint { id: PointId(31), name: b.into() });
        assert_eq!(validate(&m), Ok(()), "{a} {b}");
    }
}

#[test]
fn connection_kinds_follow_the_engine_rules() {
    let mut m = room_booking();
    m.links.clear();
    assert_eq!(connection_kinds(&m, GUEST.0, RESERVE.0), vec!["association"]);
    assert_eq!(connection_kinds(&m, RESERVE.0, GUEST.0), vec!["association"]);
    assert_eq!(connection_kinds(&m, RESERVE.0, AVAILABILITY.0), vec!["include", "extend", "generalization"]);
    assert_eq!(connection_kinds(&m, GUEST.0, MEMBER.0), vec!["generalization"]);
    let m = room_booking();
    assert!(connection_kinds(&m, GUEST.0, GUEST.0).is_empty(), "an element doesn't join itself");
    assert!(connection_kinds(&m, GUEST.0, ROOM.0).is_empty(), "a system takes no links");
    assert!(connection_kinds(&m, GUEST.0, 999).is_empty(), "an unknown element joins nothing");
}

#[test]
fn connection_options_leave_out_a_link_that_exists_or_would_close_a_cycle() {
    let m = room_booking();
    // Guest — Reserve is there already, from either end.
    assert_eq!(connection_options(&m, RESERVE.0, GUEST.0), vec![("association", Some(Blocked::AlreadyJoined))]);
    assert!(connection_kinds(&m, GUEST.0, RESERVE.0).is_empty());
    // Reserve includes Availability: that include exists, the reverse one would be a cycle.
    assert_eq!(connection_kinds(&m, RESERVE.0, AVAILABILITY.0), vec!["extend", "generalization"]);
    assert_eq!(connection_options(&m, AVAILABILITY.0, RESERVE.0)[0], ("include", Some(Blocked::Cycle)));
    // Member specialises Guest, so Guest can't specialise Member.
    assert_eq!(connection_options(&m, GUEST.0, MEMBER.0), vec![("generalization", Some(Blocked::Cycle))]);
}

#[test]
fn the_schema_is_written_and_required_and_an_unknown_one_is_refused() {
    let v = serde_json::to_value(room_booking()).unwrap();
    assert_eq!(v["schema"], json!(USE_CASE_SCHEMA));
    let mut none = v.clone();
    none.as_object_mut().unwrap().remove("schema");
    assert!(serde_json::from_value::<Model>(none).is_err());
    let mut later = v;
    later["schema"] = json!(USE_CASE_SCHEMA + 1);
    let m: Model = serde_json::from_value(later).unwrap();
    assert_eq!(validate(&m), Err(UseCaseError::UnsupportedSchema(USE_CASE_SCHEMA + 1)));
}

#[test]
fn decoding_refuses_a_list_past_its_cap_and_a_name_past_its_byte_cap() {
    let a = serde_json::to_value(actor(GUEST, "A", 0.0, 0.0)).unwrap();
    let many = json!({"schema": USE_CASE_SCHEMA, "actors": vec![a; crate::use_case::MAX_ELEMENTS + 1]});
    let err = serde_json::from_value::<Model>(many).unwrap_err().to_string();
    assert!(err.contains("more than 500 actors"), "{err}");
    let mut v = serde_json::to_value(room_booking()).unwrap();
    v["actors"][0]["name"] = json!("x".repeat(crate::use_case::MAX_NAME * 4 + 1));
    let err = serde_json::from_value::<Model>(v).unwrap_err().to_string();
    assert!(err.contains("text longer than"), "{err}");
    let mut v = serde_json::to_value(room_booking()).unwrap();
    v["links"][2]["condition"] = json!("c".repeat(10_000));
    assert!(serde_json::from_value::<Model>(v).is_err());
    let mut v = serde_json::to_value(room_booking()).unwrap();
    v["links"][2]["points"] = json!(vec![8; 9]);
    assert!(serde_json::from_value::<Model>(v).is_err());
}

#[test]
fn decoding_stops_at_a_cap_before_the_next_item_and_a_relationship_past_its_byte_budget() {
    // At the cap the next item is only seen, not decoded: a malformed extra actor is reported as
    // one too many, not as a malformed actor.
    let a = serde_json::to_value(actor(GUEST, "A", 0.0, 0.0)).unwrap();
    let mut actors = vec![a; crate::use_case::MAX_ELEMENTS];
    actors.push(json!({"not": "an actor"}));
    let text = json!({"schema": USE_CASE_SCHEMA, "actors": actors}).to_string();
    let err = serde_json::from_str::<Model>(&text).unwrap_err().to_string();
    assert!(err.contains("more than 500 actors"), "{err}");
    // A relationship is read under a byte budget before it is decoded (Serde would buffer the
    // tagged relationship whole, its huge points list included, before the points cap applied).
    let mut v = serde_json::to_value(room_booking()).unwrap();
    v["links"][2]["points"] = json!(vec![8; 100_000]);
    let err = serde_json::from_str::<Model>(&v.to_string()).unwrap_err().to_string();
    assert!(err.contains("a relationship longer than"), "{err}");
    // Within the budget, the relationships decode as they were.
    let m = room_booking();
    assert_eq!(serde_json::from_str::<Model>(&serde_json::to_string(&m).unwrap()).unwrap(), m);
}

#[test]
fn a_model_without_its_page_is_a_problem_and_typed_records_are_reported() {
    let mut doc = crate::Document::new("d");
    assert!(!doc.has_typed_records());
    let page = doc.first_page().unwrap();
    doc.use_cases.insert(page, std::sync::Arc::new(room_booking()));
    assert!(doc.has_typed_records());
    assert!(doc.is_typed_page(page));
    assert_eq!(doc.use_case_problem(), None);
    doc.use_cases.insert(crate::NodeId(987_654), std::sync::Arc::new(Model::default()));
    assert!(doc.use_case_problem().unwrap().contains("isn't in the file"));
}
