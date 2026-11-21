//! Reading typed diagram commands' parameters: each helper names the key in its error, treats an
//! absent key and `null` alike unless it says otherwise, and never panics on junk.

use serde_json::Value;

use crate::Result;
use crate::error::bad;

pub(crate) fn text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(v)) => Ok(Some(v.trim().to_string())),
        Some(_) => Err(bad(cmd, format!("`{key}` is text"))),
    }
}

pub(crate) fn req_text(cmd: &str, p: &Value, key: &str) -> Result<String> {
    text(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

/// Text as typed (not trimmed): for a member line, whose parser reports character positions.
pub(crate) fn raw_text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(v)) => Ok(Some(v.clone())),
        Some(_) => Err(bad(cmd, format!("`{key}` is text"))),
    }
}

pub(crate) fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` is a number"))),
    }
}

pub(crate) fn req_number(cmd: &str, p: &Value, key: &str) -> Result<f64> {
    number(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

pub(crate) fn flag(cmd: &str, p: &Value, key: &str) -> Result<Option<bool>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(bad(cmd, format!("`{key}` is true or false"))),
    }
}

pub(crate) fn id(cmd: &str, p: &Value, key: &str) -> Result<Option<u64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| bad(cmd, format!("`{key}` is an ID"))),
    }
}

pub(crate) fn req_id(cmd: &str, p: &Value, key: &str) -> Result<u64> {
    id(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

/// A list of IDs (at most `max`).
pub(crate) fn ids(cmd: &str, p: &Value, key: &str, max: usize) -> Result<Option<Vec<u64>>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() <= max => {
            a.iter().map(|v| v.as_u64().ok_or_else(|| bad(cmd, format!("`{key}` is a list of IDs")))).collect::<Result<Vec<_>>>().map(Some)
        }
        Some(_) => Err(bad(cmd, format!("`{key}` is a list of at most {max} IDs"))),
    }
}

/// A key that holds text or `null` (clearing it): `None` when absent.
pub(crate) fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<String>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(v)) => Ok(Some(Some(v.trim().to_string()).filter(|v| !v.is_empty()))),
        Some(_) => Err(bad(cmd, format!("`{key}` is text or null"))),
    }
}

/// `x` and `y` together, or neither.
pub(crate) fn position(cmd: &str, p: &Value) -> Result<Option<(f64, f64)>> {
    match (number(cmd, p, "x")?, number(cmd, p, "y")?) {
        (Some(x), Some(y)) => Ok(Some((x, y))),
        (None, None) => Ok(None),
        _ => Err(bad(cmd, "give both `x` and `y`, or neither")),
    }
}
