//! The UML Use Case diagram's model (page kind `useCase`, model schema version 1): actors, use
//! cases with their extension points, subjects (system boundaries) and the four relationship kinds,
//! with the one validator every command candidate and every loaded file passes through, and the
//! read-only drafting check.
//!
//! Pure data: nothing here measures text or draws. Sizes are stored (each element keeps its
//! measured minimum next to its size), so whether a model is valid never depends on the fonts of
//! the machine that opens it. See `docs/diagram-type-specifications.md` § 5.

mod check;
mod geometry;
mod model;
mod validate;

pub(crate) mod bounded;

pub use check::*;
pub use geometry::*;
pub use model::*;
pub use validate::*;

/// The plugin data key on a diagram's generated layers: the semantic ID and role they draw
/// (`"<id>:<role>"`, e.g. `"4:card"`).
pub const LAYER_KEY: &str = "figcraft.useCase";

/// The model schema this module reads and writes.
pub const USE_CASE_SCHEMA: u32 = 1;
/// At most this many actors, use cases and subjects on one page.
pub const MAX_ELEMENTS: usize = 500;
/// At most this many relationships on one page.
pub const MAX_LINKS: usize = 2_000;
/// At most this many extension points on one use case.
pub const MAX_POINTS: usize = 32;
/// An Extend names between one and this many extension points of its base.
pub const MAX_EXTEND_POINTS: usize = 8;
/// Coordinates stay within ± this.
pub const COORD_LIMIT: f64 = 1_000_000.0;
/// A member use case keeps this far inside its subject; anything else keeps this far outside one.
pub const SUBJECT_PADDING: f64 = 16.0;
/// Actor and use case boxes keep at least this gap, so every link has a visible length.
pub const ELEMENT_GAP: f64 = 24.0;
/// Longest element and subject name, in characters.
pub const MAX_NAME: usize = 200;
/// Longest extension point name, in characters.
pub const MAX_POINT_NAME: usize = 120;
/// Longest Extend condition, in characters.
pub const MAX_CONDITION: usize = 200;
/// A multiplicity bound has at most this many digits.
const MAX_BOUND_DIGITS: usize = 9;

/// The Use Case models of a document, by page.
pub type Pages = std::collections::BTreeMap<crate::NodeId, std::sync::Arc<Model>>;
/// At most this many Use Case pages in one document.
pub const MAX_PAGES: usize = 1_000;
