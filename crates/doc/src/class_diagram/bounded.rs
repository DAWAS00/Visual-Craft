//! Decoding under limits, as in [`crate::use_case`]: lists stop at their cap before the next item
//! is decoded, and text past its cap is refused when it is read. The per-list caps bound a decode
//! to 200 classifiers of at most 100 attributes, 100 operations and 100 literals each (a bounded
//! 60,000 members in all); the combined 100 per classifier and the page-wide member, parameter and layer totals are
//! checked by [`validate`], which every load runs straight after decoding. serde_json unescapes a
//! string with escapes before the visitor sees it; that scratch space is bounded by the input,
//! which the format layer caps before decoding.

use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};

use super::{Attribute, Classifier, Link, Literal, Operation, Param};
use super::{MAX_CLASSIFIERS, MAX_LINKS, MAX_MEMBERS_PER_CLASSIFIER, MAX_PARAMS, MAX_TEXT};

/// A UTF-8 character is at most four bytes.
const TEXT_BYTES: usize = MAX_TEXT * 4;

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

pub(super) fn classifiers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Classifier>, D::Error> {
    capped(d, MAX_CLASSIFIERS, "classifiers")
}
pub(super) fn links<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Link>, D::Error> {
    capped(d, MAX_LINKS, "relationships")
}
pub(super) fn attributes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Attribute>, D::Error> {
    capped(d, MAX_MEMBERS_PER_CLASSIFIER, "attributes")
}
pub(super) fn operations<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Operation>, D::Error> {
    capped(d, MAX_MEMBERS_PER_CLASSIFIER, "operations")
}
pub(super) fn literals<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Literal>, D::Error> {
    capped(d, MAX_MEMBERS_PER_CLASSIFIER, "literals")
}
pub(super) fn params<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Param>, D::Error> {
    capped(d, MAX_PARAMS, "parameters")
}

struct Text;

impl Visitor<'_> for Text {
    type Value = String;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text of at most {TEXT_BYTES} bytes")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
        if v.len() > TEXT_BYTES || v.chars().count() > MAX_TEXT {
            return Err(E::custom(format_args!("text longer than {MAX_TEXT} characters")));
        }
        Ok(v.to_string())
    }
}

pub(super) fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    d.deserialize_str(Text)
}

struct OptText;

impl<'de> Visitor<'de> for OptText {
    type Value = Option<String>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "null or text of at most {TEXT_BYTES} bytes")
    }
    fn visit_none<E: de::Error>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Option<String>, D::Error> {
        text(d).map(Some)
    }
}

pub(super) fn opt_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    d.deserialize_option(OptText)
}

struct Pages;

impl<'de> Visitor<'de> for Pages {
    type Value = super::Pages;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at most {} Class diagram pages", super::MAX_PAGES)
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<super::Pages, A::Error> {
        let mut out = super::Pages::new();
        loop {
            // At the cap, one more key is only skipped to see that it's there.
            if out.len() >= super::MAX_PAGES {
                return match map.next_key::<de::IgnoredAny>()? {
                    Some(_) => Err(de::Error::custom(format_args!("more than {} Class diagram pages", super::MAX_PAGES))),
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

/// [`crate::Document::class_diagrams`], at most [`super::MAX_PAGES`] models.
pub(crate) fn pages<'de, D: Deserializer<'de>>(d: D) -> Result<super::Pages, D::Error> {
    d.deserialize_map(Pages)
}
