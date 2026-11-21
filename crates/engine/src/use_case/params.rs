//! Reading and checking command parameters.

use figcraft_doc::use_case::Presentation;
use serde_json::Value;

use crate::Result;
use crate::error::bad;

pub(super) fn text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(v)) => Ok(Some(v.trim().to_string())),
        Some(_) => Err(bad(cmd, format!("`{key}` is text"))),
    }
}

pub(super) fn req_text(cmd: &str, p: &Value, key: &str) -> Result<String> {
    text(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

pub(super) fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` is a number"))),
    }
}

pub(super) fn req_number(cmd: &str, p: &Value, key: &str) -> Result<f64> {
    number(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

pub(super) fn id(cmd: &str, p: &Value, key: &str) -> Result<Option<u64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| bad(cmd, format!("`{key}` is an ID"))),
    }
}

pub(super) fn req_id(cmd: &str, p: &Value, key: &str) -> Result<u64> {
    id(cmd, p, key)?.ok_or_else(|| bad(cmd, format!("`{key}` is required")))
}

/// A list of IDs (at most `max`).
pub(super) fn ids(cmd: &str, p: &Value, key: &str, max: usize) -> Result<Option<Vec<u64>>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() <= max => {
            a.iter().map(|v| v.as_u64().ok_or_else(|| bad(cmd, format!("`{key}` is a list of IDs")))).collect::<Result<Vec<_>>>().map(Some)
        }
        Some(_) => Err(bad(cmd, format!("`{key}` is a list of at most {max} IDs"))),
    }
}

/// A key that holds text or `null` (clearing it): `None` when absent.
pub(super) fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<String>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(v)) => Ok(Some(Some(v.trim().to_string()).filter(|v| !v.is_empty()))),
        Some(_) => Err(bad(cmd, format!("`{key}` is text or null"))),
    }
}

pub(super) fn presentation(cmd: &str, p: &Value) -> Result<Option<Presentation>> {
    match text(cmd, p, "presentation")?.as_deref() {
        None => Ok(None),
        Some("person") => Ok(Some(Presentation::Person)),
        Some("system") => Ok(Some(Presentation::System)),
        Some(other) => Err(bad(cmd, format!("`presentation` is person or system, not `{other}`"))),
    }
}
