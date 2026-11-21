//! The UML Class diagram's model (page kind `class`, model schema version 1): classes, interfaces
//! and enumerations with their attributes, operations and literals, and six relationship kinds
//! (association, aggregation, composition, generalization, realization, dependency), with the one
//! validator every command candidate passes through and the read-only drafting check.
//!
//! Pure data, like [`crate::use_case`]: nothing here measures text or draws. A card keeps its size
//! and its measured minimum, and the validator also holds every card to a font-independent floor
//! (header plus one fixed-height row per member), so validity never depends on the machine.
//!
//! A relationship is a plain struct (`kind` plus two ends) rather than a tagged enum, so decoding
//! it never buffers a whole value before the limits apply. What each end means depends on the
//! kind; see [`Link`].

mod check;
mod connect;
mod model;
pub mod text;
mod validate;

pub(crate) mod bounded;

pub use check::*;
pub use connect::*;
pub use model::*;
pub use validate::*;

/// The plugin data key on a Class diagram's generated layers (`"<id>:<role>"`, e.g. `"4:card"`).
pub const LAYER_KEY: &str = "figcraft.class";

/// The model schema this module reads and writes.
pub const CLASS_SCHEMA: u32 = 1;
/// At most this many classifiers on one page.
pub const MAX_CLASSIFIERS: usize = 200;
/// At most this many members (attributes, operations, literals) on one classifier.
pub const MAX_MEMBERS_PER_CLASSIFIER: usize = 100;
/// At most this many members on one page.
pub const MAX_MEMBERS: usize = 2_000;
/// An operation has at most this many parameters.
pub const MAX_PARAMS: usize = 8;
/// At most this many parameters on one page.
pub const MAX_PARAMS_TOTAL: usize = 1_000;
/// At most this many relationships on one page.
pub const MAX_LINKS: usize = 1_000;
/// Longest name, type, role, label, stereotype or default, in characters.
pub const MAX_TEXT: usize = 200;
/// Coordinates and sizes stay within ± this.
pub const COORD_LIMIT: f64 = 1_000_000.0;
/// Cards keep at least this gap, so every relationship has a visible length.
pub const CARD_GAP: f64 = 24.0;
/// The narrowest a card may be, whatever its text.
pub const MIN_CARD_WIDTH: f64 = 80.0;
/// The font-independent header height (name only).
pub const HEADER_HEIGHT: f64 = 28.0;
/// The extra header height a keyword line («interface», «enumeration», a stereotype) takes.
pub const KEYWORD_HEIGHT: f64 = 14.0;
/// One member row.
pub const ROW_HEIGHT: f64 = 18.0;
/// Padding above and below the rows of one compartment.
pub const COMPARTMENT_PADDING: f64 = 4.0;
/// Characters that would break the text form of a member, refused in every name and type.
pub const FORBIDDEN: [char; 8] = [':', '(', ')', ',', '=', '[', ']', '"'];

/// The Class diagram models of a document, by page.
pub type Pages = std::collections::BTreeMap<crate::NodeId, std::sync::Arc<Model>>;
/// At most this many Class diagram pages in one document.
pub const MAX_PAGES: usize = 1_000;
