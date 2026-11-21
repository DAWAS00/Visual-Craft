//! The classifiers, members and relationships of a Class diagram, and their decoding rules.

use super::{CLASS_SCHEMA, COMPARTMENT_PADDING, HEADER_HEIGHT, KEYWORD_HEIGHT, MIN_CARD_WIDTH, ROW_HEIGHT, bounded};
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
    /// A classifier's semantic ID.
    ClassifierId
);
id_type!(
    /// A member's (attribute, operation or literal) semantic ID.
    MemberId
);
id_type!(
    /// A relationship's semantic ID.
    LinkId
);

/// What a classifier is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    Class,
    Interface,
    Enumeration,
}

impl Kind {
    /// The keyword drawn above the name, if any.
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            Kind::Class => None,
            Kind::Interface => Some("interface"),
            Kind::Enumeration => Some("enumeration"),
        }
    }
}

/// Who may see a member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Package,
}

impl Visibility {
    /// Its UML marker: `+ - # ~`.
    pub fn symbol(self) -> char {
        match self {
            Visibility::Public => '+',
            Visibility::Private => '-',
            Visibility::Protected => '#',
            Visibility::Package => '~',
        }
    }

    /// The visibility a marker stands for.
    pub fn from_symbol(c: char) -> Option<Self> {
        match c {
            '+' => Some(Visibility::Public),
            '-' => Some(Visibility::Private),
            '#' => Some(Visibility::Protected),
            '~' => Some(Visibility::Package),
            _ => None,
        }
    }
}

/// A property of a classifier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attribute {
    pub id: MemberId,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(deserialize_with = "bounded::text")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub multiplicity: Option<String>,
    /// As written, after the `=` (a quoted string keeps its quotes and escapes).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_static: bool,
}

/// One parameter of an operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Param {
    #[serde(deserialize_with = "bounded::text")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub type_name: Option<String>,
}

/// Behaviour of a classifier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub id: MemberId,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(deserialize_with = "bounded::text")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "bounded::params")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub return_type: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_static: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_abstract: bool,
}

impl Operation {
    /// What makes two operations of one classifier the same: the name and the parameter types.
    pub fn signature(&self) -> (String, Vec<String>) {
        (self.name.clone(), self.params.iter().map(|p| p.type_name.clone().unwrap_or_default()).collect())
    }
}

/// A value of an enumeration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Literal {
    pub id: MemberId,
    #[serde(deserialize_with = "bounded::text")]
    pub name: String,
}

/// A class, an interface or an enumeration, drawn as a card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Classifier {
    pub id: ClassifierId,
    #[serde(default)]
    pub kind: Kind,
    #[serde(deserialize_with = "bounded::text")]
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_abstract: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub stereotype: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The size its text needs, measured when it last changed.
    pub min_w: f64,
    pub min_h: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "bounded::attributes")]
    pub attributes: Vec<Attribute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "bounded::operations")]
    pub operations: Vec<Operation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "bounded::literals")]
    pub literals: Vec<Literal>,
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub show_attributes: bool,
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub show_operations: bool,
}

fn yes() -> bool {
    true
}

fn is_yes(b: &bool) -> bool {
    *b
}

impl Classifier {
    /// How many members it has, of all kinds.
    pub fn member_count(&self) -> usize {
        self.attributes.len() + self.operations.len() + self.literals.len()
    }

    /// The smallest size any font could draw it at: [`MIN_CARD_WIDTH`] wide, and the header plus
    /// one [`ROW_HEIGHT`] row per shown member (an empty shown compartment keeps one row).
    pub fn floor(&self) -> (f64, f64) {
        let keyword = self.kind.keyword().is_some() || self.stereotype.is_some();
        let mut h = HEADER_HEIGHT + if keyword { KEYWORD_HEIGHT } else { 0.0 };
        let compartment = |rows: usize| COMPARTMENT_PADDING * 2.0 + rows.max(1) as f64 * ROW_HEIGHT;
        if self.kind == Kind::Enumeration {
            h += compartment(self.literals.len());
        }
        if self.show_attributes && (self.kind != Kind::Enumeration || !self.attributes.is_empty()) {
            h += compartment(self.attributes.len());
        }
        if self.show_operations && (self.kind != Kind::Enumeration || !self.operations.is_empty()) {
            h += compartment(self.operations.len());
        }
        (MIN_CARD_WIDTH, h)
    }

    /// Its box: x, y, width, height.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.w, self.h)
    }
}

/// What a relationship is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    /// A structural relationship between instances. Undirected unless an end says navigable.
    Association,
    /// A whole and its shared parts (hollow diamond at `ends[0]`, the whole).
    Aggregation,
    /// A whole and the parts it owns (filled diamond at `ends[0]`, the whole).
    Composition,
    /// `ends[0]` (specific) specialises `ends[1]` (general).
    Generalization,
    /// `ends[0]` (a class or enumeration) implements `ends[1]` (an interface).
    Realization,
    /// `ends[0]` (client) depends on `ends[1]` (supplier).
    Dependency,
}

impl LinkKind {
    /// The three kinds that carry roles, multiplicities and navigability at their ends.
    pub fn is_association(self) -> bool {
        matches!(self, LinkKind::Association | LinkKind::Aggregation | LinkKind::Composition)
    }

    /// Its name in commands and in the interface's tool list.
    pub fn name(self) -> &'static str {
        match self {
            LinkKind::Association => "association",
            LinkKind::Aggregation => "aggregation",
            LinkKind::Composition => "composition",
            LinkKind::Generalization => "generalization",
            LinkKind::Realization => "realization",
            LinkKind::Dependency => "dependency",
        }
    }

    /// The kind a command names.
    pub fn from_name(s: &str) -> Option<Self> {
        ALL_LINK_KINDS.into_iter().find(|k| k.name() == s)
    }
}

/// Every relationship kind, in tool order.
pub const ALL_LINK_KINDS: [LinkKind; 6] =
    [LinkKind::Association, LinkKind::Aggregation, LinkKind::Composition, LinkKind::Generalization, LinkKind::Realization, LinkKind::Dependency];

/// Whether an association end may be followed from the other end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Navigable {
    #[default]
    Unspecified,
    Yes,
    No,
}

/// One end of a relationship. Only association kinds use the role, multiplicity and navigability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct End {
    pub classifier: ClassifierId,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub multiplicity: Option<String>,
    #[serde(default, skip_serializing_if = "is_unspecified")]
    pub navigable: Navigable,
}

fn is_unspecified(n: &Navigable) -> bool {
    *n == Navigable::Unspecified
}

impl End {
    /// A bare end at `classifier`.
    pub fn at(classifier: ClassifierId) -> Self {
        Self { classifier, role: None, multiplicity: None, navigable: Navigable::Unspecified }
    }
}

/// A relationship between two classifiers. What its ends mean depends on `kind` (see
/// [`LinkKind`]); `label` belongs to a dependency only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub id: LinkId,
    pub kind: LinkKind,
    pub ends: [End; 2],
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded::opt_text")]
    pub label: Option<String>,
}

impl Link {
    /// The two classifiers it joins, in end order.
    pub fn pair(&self) -> (ClassifierId, ClassifierId) {
        (self.ends[0].classifier, self.ends[1].classifier)
    }

    /// Whether it touches `c`.
    pub fn touches(&self, c: ClassifierId) -> bool {
        self.ends[0].classifier == c || self.ends[1].classifier == c
    }
}

/// One Class diagram page's model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    /// The schema the model was written with ([`CLASS_SCHEMA`]). Required.
    pub schema: u32,
    #[serde(default, deserialize_with = "bounded::classifiers")]
    pub classifiers: Vec<Classifier>,
    #[serde(default, deserialize_with = "bounded::links")]
    pub links: Vec<Link>,
}

impl Default for Model {
    fn default() -> Self {
        Self { schema: CLASS_SCHEMA, classifiers: Vec::new(), links: Vec::new() }
    }
}

impl Model {
    /// The classifier with ID `id`.
    pub fn classifier(&self, id: ClassifierId) -> Option<&Classifier> {
        self.classifiers.iter().find(|c| c.id == id)
    }

    /// The relationship with ID `id`.
    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }

    /// The classifier holding member `id`, if any.
    pub fn owner_of(&self, id: MemberId) -> Option<&Classifier> {
        self.classifiers
            .iter()
            .find(|c| c.attributes.iter().any(|a| a.id == id) || c.operations.iter().any(|o| o.id == id) || c.literals.iter().any(|l| l.id == id))
    }

    /// One past the largest ID in the model (classifiers, members and relationships share one
    /// space), or `None` at the top of the range.
    pub fn next_id(&self) -> Option<u64> {
        let members = self
            .classifiers
            .iter()
            .flat_map(|c| c.attributes.iter().map(|a| a.id.0).chain(c.operations.iter().map(|o| o.id.0)).chain(c.literals.iter().map(|l| l.id.0)));
        let all = self.classifiers.iter().map(|c| c.id.0).chain(self.links.iter().map(|l| l.id.0)).chain(members);
        all.max().unwrap_or(0).checked_add(1)
    }
}
