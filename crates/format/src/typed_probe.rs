//! Whether a `document.json` text holds typed diagram records (the [`TYPED_KEYS`]), read in one
//! streaming pass that stores nothing. Every other value is skipped without being decoded, and the
//! answer is "typed" as soon as a typed map holds an entry: the scan stops before that
//! entry's key is read, and no model in it is read, so no record, however large, is buffered.
//! "Not typed" comes only from a scan of the whole text that succeeded.

use std::cell::Cell;

use serde::de::{self, Deserializer, IgnoredAny, MapAccess, Visitor};

/// Why the scan stopped early: the text has a typed record (never shown; see [`typed`]).
const FOUND: &str = "\u{0}typed record found";

/// The document keys that hold typed diagram records (page id → model). A new typed diagram kind
/// adds its key here, and to [`Key::Typed`]'s aliases.
pub(crate) const TYPED_KEYS: [&str; 2] = ["use_cases", "class_diagrams"];

/// Whether `json` holds typed diagram records: a non-empty map under one of [`TYPED_KEYS`] (`null`
/// and an empty map are none; any other shape is an error). `Err`: nobody can tell.
pub(crate) fn typed(json: &[u8]) -> Result<bool, String> {
    // Skipped strings aren't checked by the scan: the text as a whole is, first.
    std::str::from_utf8(json).map_err(|e| e.to_string())?;
    // A typed key's value is seen by its first byte before serde reads it: anything but a map or
    // null is refused here, so a string there is never decoded.
    if let Some(bad) = raw::typed_value_starts(json).into_iter().find(|b| !matches!(b, b'{' | b'n')) {
        return Err(format!("a diagrams entry that isn't a map (it starts with byte {bad})"));
    }
    let found = Cell::new(false);
    let mut de = serde_json::Deserializer::from_slice(json);
    match de.deserialize_map(Top { found: &found }).and_then(|()| de.end()) {
        Ok(()) => Ok(false),
        // The scan stopped at the first sign of a record: the rest of the text isn't read.
        Err(_) if found.get() => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(serde::Deserialize)]
#[serde(field_identifier)]
enum Key {
    #[serde(rename = "use_cases", alias = "class_diagrams")]
    Typed,
    #[serde(other)]
    Other,
}

struct Top<'a> {
    found: &'a Cell<bool>,
}

impl<'de> Visitor<'de> for Top<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a document object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
        while let Some(k) = m.next_key::<Key>()? {
            match k {
                Key::Typed => m.next_value_seed(Records { found: self.found })?,
                Key::Other => {
                    m.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

/// A typed key's value: `null` and an empty object are no record; a non-empty object stops the
/// scan as typed at its first entry, before that entry's key is read (serde_json reads a key only
/// when the seed asks it to). Any other shape can't be a diagram map, and is an error. (A bare
/// string is decoded by serde_json to word that error: that scratch space is bounded by the
/// input, and serde gives no way to refuse a value without reading it.)
struct Records<'a> {
    found: &'a Cell<bool>,
}

impl<'de> de::DeserializeSeed<'de> for Records<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_option(self)
    }
}

impl<'de> Visitor<'de> for Records<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("null or a map of diagrams")
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
        m.next_key_seed(Stop { found: self.found }).map(|_| ())
    }
}

/// The first key of a non-empty diagram map: never read, the scan stops as typed.
struct Stop<'a> {
    found: &'a Cell<bool>,
}

impl<'de> de::DeserializeSeed<'de> for Stop<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        self.found.set(true);
        Err(de::Error::custom(FOUND))
    }
}

/// A light pass over the raw text, outside serde: where each top-level typed key's value starts.
mod raw {
    use super::TYPED_KEYS;

    /// The longest key text worth decoding to compare with [`TYPED_KEYS`] (escapes included).
    const MAX_KEY: usize = 128;

    fn ws(b: &[u8], mut i: usize) -> usize {
        while b.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        i
    }

    /// The index just past the string starting at `i` (its opening quote).
    fn string_end(b: &[u8], mut i: usize) -> Option<usize> {
        i += 1;
        loop {
            match b.get(i)? {
                b'\\' => i += 2,
                b'"' => return Some(i + 1),
                _ => i += 1,
            }
        }
    }

    /// The index just past the value starting at `i`.
    fn value_end(b: &[u8], i: usize) -> Option<usize> {
        match b.get(i)? {
            b'"' => string_end(b, i),
            b'{' | b'[' => {
                let (mut depth, mut j) = (0usize, i);
                loop {
                    match b.get(j)? {
                        b'"' => {
                            j = string_end(b, j)?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth = depth.checked_sub(1)?;
                            if depth == 0 {
                                return Some(j + 1);
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
            }
            _ => {
                let mut j = i;
                while b.get(j).is_some_and(|c| !matches!(c, b',' | b'}' | b']') && !c.is_ascii_whitespace()) {
                    j += 1;
                }
                Some(j)
            }
        }
    }

    fn is_typed_key(raw: &[u8]) -> bool {
        if raw.len() > MAX_KEY {
            return false;
        }
        serde_json::from_slice::<String>(raw).is_ok_and(|k| TYPED_KEYS.contains(&k.as_str()))
    }

    /// The first byte of each top-level typed key's value. Text this pass can't follow gives
    /// what it found so far: the serde scan that follows judges the text itself.
    pub(super) fn typed_value_starts(b: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = ws(b, 0);
        if b.get(i) != Some(&b'{') {
            return out;
        }
        i += 1;
        loop {
            i = ws(b, i);
            if b.get(i) != Some(&b'"') {
                return out;
            }
            let Some(key_end) = string_end(b, i) else { return out };
            let typed = b.get(i..key_end).is_some_and(is_typed_key);
            i = ws(b, key_end);
            if b.get(i) != Some(&b':') {
                return out;
            }
            i = ws(b, i + 1);
            if typed && let Some(c) = b.get(i) {
                out.push(*c);
            }
            let Some(end) = value_end(b, i) else { return out };
            i = ws(b, end);
            if b.get(i) != Some(&b',') {
                return out;
            }
            i += 1;
        }
    }
}
