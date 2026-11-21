//! The actors, use cases, subjects and relationships of a Use Case diagram.

use super::{USE_CASE_SCHEMA, bounded};
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(
    /// An actor's semantic ID (from the file's one allocator).
    ActorId
);
id_type!(
    /// A use case's semantic ID.
    UseCaseId
);
id_type!(
    /// A subject's semantic ID.
    SubjectId
);
id_type!(
    /// A relationship's semantic ID.
    LinkId
);
id_type!(
    /// An extension point's semantic ID.
    PointId
);

/// How an actor is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Presentation {
    /// A stick figure with the name below.
    #[default]
    Person,
    /// A rectangle with the keyword «actor», for another system or a device.
    System,
}

/// A role outside the system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Actor {
    pub id: ActorId,
    #[serde(deserialize_with = "bounded::name")]
    pub name: String,
    #[serde(default)]
    pub presentation: Presentation,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The size its name needs, measured when the name or presentation last changed.
    pub min_w: f64,
    pub min_h: f64,
}

/// A named place in a use case where an Extend adds behaviour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionPoint {
    pub id: PointId,
    #[serde(deserialize_with = "bounded::name")]
    pub name: String,
}

/// A goal the system provides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseCase {
    pub id: UseCaseId,
    #[serde(deserialize_with = "bounded::name")]
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The size its name and extension points need, measured when they last changed.
    pub min_w: f64,
    pub min_h: f64,
    /// The subject it belongs to; none while drafting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectId>,
    /// In display order.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "bounded::points")]
    pub points: Vec<ExtensionPoint>,
}

/// A system boundary: the use cases inside belong to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subject {
    pub id: SubjectId,
    #[serde(deserialize_with = "bounded::name")]
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A Generalization's ends: actors and use cases keep separate hierarchies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "of", rename_all = "camelCase", deny_unknown_fields)]
pub enum GenEnds {
    Actors { specific: ActorId, general: ActorId },
    UseCases { specific: UseCaseId, general: UseCaseId },
}

/// A relationship. Each kind holds only the endpoint kinds it allows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Link {
    /// The actor takes part in the use case. Undirected; multiplicities are kept by role.
    Association {
        id: LinkId,
        actor: ActorId,
        use_case: UseCaseId,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::short")]
        actor_multiplicity: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::short")]
        use_case_multiplicity: Option<String>,
    },
    /// The including case always uses the included one.
    Include { id: LinkId, including: UseCaseId, included: UseCaseId },
    /// The extending case adds behaviour to the base at the named points (in order).
    Extend {
        id: LinkId,
        extending: UseCaseId,
        base: UseCaseId,
        #[serde(deserialize_with = "bounded::extend_points")]
        points: Vec<PointId>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::short")]
        condition: Option<String>,
    },
    /// The specific element specialises the general one.
    Generalization { id: LinkId, ends: GenEnds },
}

impl Link {
    pub fn id(&self) -> LinkId {
        match self {
            Link::Association { id, .. } | Link::Include { id, .. } | Link::Extend { id, .. } | Link::Generalization { id, .. } => *id,
        }
    }
}

/// One Use Case page's model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    /// The schema the model was written with ([`USE_CASE_SCHEMA`]). Required: a model without
    /// one (written before the field existed, never released) is refused, not guessed.
    pub schema: u32,
    #[serde(default, deserialize_with = "bounded::actors")]
    pub actors: Vec<Actor>,
    #[serde(default, deserialize_with = "bounded::use_cases")]
    pub use_cases: Vec<UseCase>,
    #[serde(default, deserialize_with = "bounded::subjects")]
    pub subjects: Vec<Subject>,
    #[serde(default, deserialize_with = "bounded::links")]
    pub links: Vec<Link>,
}

impl Default for Model {
    fn default() -> Self {
        Self { schema: USE_CASE_SCHEMA, actors: Vec::new(), use_cases: Vec::new(), subjects: Vec::new(), links: Vec::new() }
    }
}
