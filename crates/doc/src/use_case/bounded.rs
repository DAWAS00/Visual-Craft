//! Decoding under limits: every list is refused as soon as it passes its cap and every string as
//! soon as it passes its byte cap, before Serde stores the rest, so a hostile file costs no more
//! than the limits allow. The validator then applies the exact rules.

use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};

use super::{Actor, ExtensionPoint, Link, PointId, Subject, UseCase};
use super::{MAX_CONDITION, MAX_ELEMENTS, MAX_EXTEND_POINTS, MAX_LINKS, MAX_NAME, MAX_POINTS};

/// A UTF-8 character is at most four bytes.
const NAME_BYTES: usize = MAX_NAME * 4;
/// Conditions and multiplicities.
const SHORT_BYTES: usize = MAX_CONDITION * 4;

struct Capped<T> {
    max: usize,
    what: &'static str,
    item: PhantomData<T>,
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for Capped<T> {
    type Value = Vec<T>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a list of at most {} {}", self.max, self.what)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(self.max).min(64));
        loop {
            // At the cap, one more item is only skipped to see that it's there, never decoded.
            if out.len() >= self.max {
                return match seq.next_element::<de::IgnoredAny>()? {
                    Some(_) => Err(de::Error::custom(format_args!("more than {} {}", self.max, self.what))),
                    None => Ok(out),
                };
            }
            match seq.next_element()? {
                Some(v) => out.push(v),
                None => return Ok(out),
            }
        }
    }
}

fn capped<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D, max: usize, what: &'static str) -> Result<Vec<T>, D::Error> {
    d.deserialize_seq(Capped { max, what, item: PhantomData })
}

pub(super) fn actors<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Actor>, D::Error> {
    capped(d, MAX_ELEMENTS, "actors")
}
pub(super) fn use_cases<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<UseCase>, D::Error> {
    capped(d, MAX_ELEMENTS, "use cases")
}
pub(super) fn subjects<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Subject>, D::Error> {
    capped(d, MAX_ELEMENTS, "subjects")
}
pub(super) fn links<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Link>, D::Error> {
    let links: Vec<Budgeted<Link>> = capped(d, MAX_LINKS, "relationships")?;
    Ok(links.into_iter().map(|b| b.0).collect())
}

/// The most one relationship may take before it is decoded, counted as its text (see
/// [`Charge`]). A relationship is an internally tagged enum, which Serde buffers whole before
/// its fields' limits apply: this keeps that buffer small.
const LINK_BYTES: usize = 16 * 1024;

/// A value read under a byte budget into a JSON value first, then decoded from that.
struct Budgeted<T>(T);

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Budgeted<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let left = std::cell::Cell::new(LINK_BYTES);
        let v = d.deserialize_any(Charge { left: &left })?;
        T::deserialize(v).map(Budgeted).map_err(de::Error::custom)
    }
}

/// Builds a JSON value, charging `left` as it goes: strings by their length, other scalars 8
/// bytes, lists and objects 2 more; past the budget it fails before reading further.
#[derive(Clone, Copy)]
struct Charge<'a> {
    left: &'a std::cell::Cell<usize>,
}

impl Charge<'_> {
    fn take<E: de::Error>(self, n: usize) -> Result<(), E> {
        match self.left.get().checked_sub(n) {
            Some(rest) => {
                self.left.set(rest);
                Ok(())
            }
            None => Err(E::custom(format_args!("a relationship longer than {LINK_BYTES} bytes"))),
        }
    }
}

impl<'de> de::DeserializeSeed<'de> for Charge<'_> {
    type Value = serde_json::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<serde_json::Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Charge<'_> {
    type Value = serde_json::Value;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a relationship of at most {LINK_BYTES} bytes")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| v.into())
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| v.into())
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<serde_json::Value, E> {
        self.take(v.len().saturating_add(2)).map(|()| v.into())
    }
    fn visit_unit<E: de::Error>(self) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| serde_json::Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<serde_json::Value, E> {
        self.take(8).map(|()| serde_json::Value::Null)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<serde_json::Value, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<serde_json::Value, A::Error> {
        self.take(2)?;
        let mut out = Vec::new();
        while let Some(v) = s.next_element_seed(self)? {
            out.push(v);
        }
        Ok(out.into())
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut m: A) -> Result<serde_json::Value, A::Error> {
        self.take(2)?;
        let mut out = serde_json::Map::new();
        while let Some(k) = m.next_key::<String>()? {
            self.take(k.len().saturating_add(2))?;
            let v = m.next_value_seed(self)?;
            out.insert(k, v);
        }
        Ok(out.into())
    }
}
pub(super) fn points<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ExtensionPoint>, D::Error> {
    capped(d, MAX_POINTS, "extension points")
}
pub(super) fn extend_points<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PointId>, D::Error> {
    capped(d, MAX_EXTEND_POINTS, "extension points")
}

struct Text {
    max: usize,
}

impl Visitor<'_> for Text {
    type Value = String;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text of at most {} bytes", self.max)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
        if v.len() > self.max { Err(E::custom(format_args!("text longer than {} bytes", self.max))) } else { Ok(v.to_owned()) }
    }
}

pub(super) fn name<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    d.deserialize_str(Text { max: NAME_BYTES })
}

struct ShortText;

impl<'de> Visitor<'de> for ShortText {
    type Value = Option<String>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "null or text of at most {SHORT_BYTES} bytes")
    }
    fn visit_none<E: de::Error>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Option<String>, D::Error> {
        d.deserialize_str(Text { max: SHORT_BYTES }).map(Some)
    }
}

pub(super) fn short<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    d.deserialize_option(ShortText)
}

struct Pages;

impl<'de> Visitor<'de> for Pages {
    type Value = super::Pages;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at most {} Use Case pages", super::MAX_PAGES)
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<super::Pages, A::Error> {
        let mut out = super::Pages::new();
        loop {
            // At the cap, one more key is only skipped to see that it's there.
            if out.len() >= super::MAX_PAGES {
                return match map.next_key::<de::IgnoredAny>()? {
                    Some(_) => Err(de::Error::custom(format_args!("more than {} Use Case pages", super::MAX_PAGES))),
                    None => Ok(out),
                };
            }
            match map.next_entry()? {
                Some((page, model)) => {
                    out.insert(page, model);
                }
                None => return Ok(out),
            }
        }
    }
}

/// [`crate::Document::use_cases`], at most [`super::MAX_PAGES`] models.
pub(crate) fn pages<'de, D: Deserializer<'de>>(d: D) -> Result<super::Pages, D::Error> {
    d.deserialize_map(Pages)
}
