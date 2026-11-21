//! How much room an element's text needs: the measured minimum each actor and use case stores
//! (see `figcraft_doc::use_case`). Measured only when a name, an actor's presentation or a use
//! case's extension points change, never on load.

use figcraft_doc::use_case::{Presentation, actor_size_range};
use figcraft_doc::{Node, NodeId, NodeKind, TextAlignH, TextAutoResize, TextData};

/// The name's text size.
pub(super) const NAME_SIZE: f64 = 14.0;
/// The keyword («actor»), the "extension points" heading and relationship labels.
pub(super) const SMALL_SIZE: f64 = 11.0;
/// An extension point's text size.
pub(super) const POINT_SIZE: f64 = 12.0;
/// A use case's text wraps at this width.
pub(super) const CASE_TEXT_WIDTH: f64 = 200.0;
/// An actor's name wraps at this width.
pub(super) const ACTOR_TEXT_WIDTH: f64 = 160.0;
/// The stick figure's height, at the top of a person actor's box.
pub(super) const FIGURE_H: f64 = 80.0;
/// The heading above a use case's extension points.
pub(super) const POINTS_HEADING: &str = "extension points";

/// A text node as the renderer makes it, for measuring.
pub(crate) fn text_data(text: &str, size: f64, auto: TextAutoResize, align: TextAlignH) -> TextData {
    let mut td = TextData { characters: text.to_string(), auto_resize: auto, align_h: align, ..TextData::default() };
    td.style.font_size = size;
    td
}

/// The size `text` takes at `size`, wrapped at `max_w`.
pub(crate) fn text_size(text: &str, size: f64, max_w: f64) -> (f64, f64) {
    let measure = |auto, w| {
        let n = Node::new(NodeId(0), NodeKind::Text(text_data(text, size, auto, TextAlignH::Center)), "m", w, size * 1.4);
        // With no font to measure with, layout reports its 1 × 1 minimum: read that as unmeasured.
        figcraft_text::auto_size(&n).filter(|(w, h)| *w > 1.0 && *h > 1.0)
    };
    let fallback = || {
        let chars = text.chars().count() as f64;
        let w = (chars * size * 0.55).min(max_w);
        let lines = (chars * size * 0.55 / max_w).ceil().max(1.0);
        (w, lines * size * 1.4)
    };
    match measure(TextAutoResize::WidthAndHeight, 1.0) {
        Some((w, h)) if w <= max_w => (w, h),
        Some(_) => measure(TextAutoResize::Height, max_w).map_or_else(fallback, |(_, h)| (max_w, h)),
        None => fallback(),
    }
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v.is_finite() { v.clamp(lo, hi) } else { lo }
}

/// The minimum size of a use case with this name and these extension point names.
pub(super) fn use_case_min<'a>(name: &str, points: impl IntoIterator<Item = &'a str>) -> (f64, f64) {
    let (mut w, mut h) = text_size(name, NAME_SIZE, CASE_TEXT_WIDTH);
    let mut any = false;
    for p in points {
        let (pw, ph) = text_size(p, POINT_SIZE, CASE_TEXT_WIDTH);
        w = w.max(pw);
        h += ph;
        any = true;
    }
    if any {
        let (hw, hh) = text_size(POINTS_HEADING, SMALL_SIZE, CASE_TEXT_WIDTH);
        w = w.max(hw);
        h += hh + 8.0;
    }
    (clamp(w * 1.42 + 24.0, 120.0, 2_000.0), clamp(h * 1.42 + 16.0, 56.0, 2_000.0))
}

/// The minimum size of an actor with this name and presentation.
pub(super) fn actor_min(name: &str, presentation: Presentation) -> (f64, f64) {
    let (lo_w, hi_w, lo_h, hi_h) = actor_size_range(presentation);
    match presentation {
        Presentation::Person => {
            let (w, h) = text_size(name, NAME_SIZE, ACTOR_TEXT_WIDTH);
            (clamp(w + 8.0, lo_w, hi_w), clamp(FIGURE_H + 6.0 + h, lo_h, hi_h))
        }
        Presentation::System => {
            let (w, h) = text_size(name, NAME_SIZE, CASE_TEXT_WIDTH);
            let (kw, kh) = text_size("«actor»", SMALL_SIZE, CASE_TEXT_WIDTH);
            (clamp(w.max(kw) + 24.0, lo_w, hi_w), clamp(h + kh + 16.0, lo_h, hi_h))
        }
    }
}
