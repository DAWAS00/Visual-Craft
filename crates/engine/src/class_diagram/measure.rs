//! How wide a card's text needs it to be: the measured minimum each classifier stores (see
//! `figcraft_doc::class_diagram`). Heights come from the font-independent row layout
//! ([`Classifier::floor`], plus the «abstract» line, see [`super::layout::abstract_line`]), so only
//! the width is measured. Measured when a card's text changes, never on load.

use figcraft_doc::class_diagram::{Classifier, KEYWORD_HEIGHT, MAX_TEXT, MIN_CARD_WIDTH};

use super::layout::{keyword_text, sections};
use crate::Result;
use crate::error::bad;
use crate::use_case::measure::text_size;

/// The class name's text size.
pub(super) const NAME_SIZE: f64 = 14.0;
/// The keyword line and relationship labels.
pub(super) const SMALL_SIZE: f64 = 11.0;
/// A member row's text size.
pub(super) const ROW_SIZE: f64 = 12.0;
/// Room left and right of a card's text.
pub(super) const PAD: f64 = 8.0;
/// Text is measured unwrapped up to this width; longer rows are cut short when drawn.
const MAX_TEXT_WIDTH: f64 = 600.0;

/// Whether `s` is longer than [`MAX_TEXT`] characters (counting no further than that).
fn too_long(s: &str) -> bool {
    s.chars().nth(MAX_TEXT).is_some()
}

/// Whether every text of `c` is within [`MAX_TEXT`] characters, so measuring it stays cheap. The
/// validator holds the same caps; this runs first, before any text is measured.
fn texts_capped(c: &Classifier) -> bool {
    let opt = |s: &Option<String>| s.as_deref().is_some_and(too_long);
    !(too_long(&c.name)
        || opt(&c.stereotype)
        || c.attributes.iter().any(|a| too_long(&a.name) || opt(&a.type_name) || opt(&a.multiplicity) || opt(&a.default))
        || c.operations.iter().any(|o| too_long(&o.name) || opt(&o.return_type) || o.params.iter().any(|p| too_long(&p.name) || opt(&p.type_name)))
        || c.literals.iter().any(|l| too_long(&l.name)))
}

/// The minimum size of `c`: its text's width (at least [`MIN_CARD_WIDTH`]) and its floor height,
/// plus a line for «abstract» when no other keyword line carries it. Refused, before measuring,
/// when a text is over the cap.
pub(super) fn card_min(cmd: &str, c: &Classifier) -> Result<(f64, f64)> {
    if !texts_capped(c) {
        return Err(bad(cmd, format!("a name, type or value is longer than {MAX_TEXT} characters")));
    }
    let mut w = text_size(&c.name, NAME_SIZE, MAX_TEXT_WIDTH).0;
    if let Some(k) = keyword_text(c, c.is_abstract) {
        w = w.max(text_size(&format!("«{k}»"), SMALL_SIZE, MAX_TEXT_WIDTH).0);
    }
    for (_, rows) in sections(c) {
        for (_, text, _) in rows {
            w = w.max(text_size(&text, ROW_SIZE, MAX_TEXT_WIDTH).0);
        }
    }
    let w = if w.is_finite() { (w + PAD * 2.0).ceil() } else { MIN_CARD_WIDTH };
    let own_line = c.is_abstract && keyword_text(c, false).is_none();
    let h = c.floor().1 + if own_line { KEYWORD_HEIGHT } else { 0.0 };
    Ok((w.clamp(MIN_CARD_WIDTH, MAX_TEXT_WIDTH + PAD * 2.0), h))
}

/// Re-measures `c` after its text changed. Each side grows to its new minimum; one already at
/// least that keeps its size (extra room a resize gave is used before the card grows), and a side
/// that was fitted to its old minimum follows the new one, so a removed row shrinks it.
pub(super) fn remeasure(cmd: &str, c: &mut Classifier) -> Result<()> {
    let (old_w, old_h) = (c.min_w, c.min_h);
    let (w, h) = card_min(cmd, c)?;
    (c.min_w, c.min_h) = (w, h);
    c.w = if c.w <= old_w { w } else { c.w.max(w) };
    c.h = if c.h <= old_h { h } else { c.h.max(h) };
    Ok(())
}
