//! Reading and checking command parameters.

use crate::Result;
use crate::diagram_params::clearable;
use crate::diagram_params::req_text;
use crate::diagram_params::text;
use crate::error::bad;
use figcraft_doc::class_diagram as cd;
use figcraft_doc::class_diagram::End;
use figcraft_doc::class_diagram::Kind;
use figcraft_doc::class_diagram::LinkKind;
use figcraft_doc::class_diagram::Navigable;
use figcraft_doc::class_diagram::Param;
use figcraft_doc::class_diagram::Visibility;
use figcraft_doc::class_diagram::text::TextError;
use serde_json::Value;

pub(super) fn kind_param(cmd: &str, p: &Value, key: &str) -> Result<Option<Kind>> {
    match text(cmd, p, key)?.as_deref() {
        None => Ok(None),
        Some("class") => Ok(Some(Kind::Class)),
        Some("interface") => Ok(Some(Kind::Interface)),
        Some("enumeration") => Ok(Some(Kind::Enumeration)),
        Some(other) => Err(bad(cmd, format!("`{key}` is class, interface or enumeration, not `{other}`"))),
    }
}

pub(super) fn link_kind(cmd: &str, p: &Value, key: &str) -> Result<Option<LinkKind>> {
    match text(cmd, p, key)? {
        None => Ok(None),
        Some(k) => LinkKind::from_name(&k).map(Some).ok_or_else(|| {
            bad(cmd, format!("`{key}` is association, aggregation, composition, generalization, realization or dependency, not `{k}`"))
        }),
    }
}

pub(super) fn visibility(cmd: &str, p: &Value) -> Result<Option<Visibility>> {
    match text(cmd, p, "visibility")?.as_deref() {
        None => Ok(None),
        Some("public" | "+") => Ok(Some(Visibility::Public)),
        Some("private" | "-") => Ok(Some(Visibility::Private)),
        Some("protected" | "#") => Ok(Some(Visibility::Protected)),
        Some("package" | "~") => Ok(Some(Visibility::Package)),
        Some(other) => Err(bad(cmd, format!("`visibility` is public, private, protected or package, not `{other}`"))),
    }
}

/// `navigable`: absent leaves it, `null` clears it to unspecified.
pub(super) fn navigable(cmd: &str, v: &Value) -> Result<Option<Navigable>> {
    match v.get("navigable") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(Navigable::Unspecified)),
        Some(Value::String(s)) => match s.as_str() {
            "unspecified" => Ok(Some(Navigable::Unspecified)),
            "yes" => Ok(Some(Navigable::Yes)),
            "no" => Ok(Some(Navigable::No)),
            other => Err(bad(cmd, format!("`navigable` is unspecified, yes or no, not `{other}`"))),
        },
        Some(Value::Bool(b)) => Ok(Some(if *b { Navigable::Yes } else { Navigable::No })),
        Some(_) => Err(bad(cmd, "`navigable` is unspecified, yes or no")),
    }
}

/// Applies `{role?, multiplicity?, navigable?}` to an end (an omitted key stays, null clears).
pub(super) fn patch_end(cmd: &str, e: &mut End, v: &Value) -> Result<()> {
    if !v.is_object() {
        return Err(bad(cmd, "each of `ends` is {role?, multiplicity?, navigable?}"));
    }
    unknown_keys(cmd, v, &["role", "multiplicity", "navigable"])?;
    if let Some(r) = clearable(cmd, v, "role")? {
        e.role = r;
    }
    if let Some(m) = clearable(cmd, v, "multiplicity")? {
        e.multiplicity = m;
    }
    if let Some(n) = navigable(cmd, v)? {
        e.navigable = n;
    }
    Ok(())
}

/// `ends`: two end patches, or none.
pub(super) fn ends_param(cmd: &str, p: &Value) -> Result<Option<[Value; 2]>> {
    match p.get("ends") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() == 2 => Ok(Some([a.first().cloned().unwrap_or(Value::Null), a.get(1).cloned().unwrap_or(Value::Null)])),
        Some(_) => Err(bad(cmd, "`ends` is a list of two ends")),
    }
}

pub(super) fn unknown_keys(cmd: &str, v: &Value, allowed: &[&str]) -> Result<()> {
    if let Some(o) = v.as_object()
        && let Some(k) = o.keys().find(|k| !allowed.contains(&k.as_str()))
    {
        let k: String = k.chars().take(40).collect();
        return Err(bad(cmd, format!("`{k}` isn't a parameter here")));
    }
    Ok(())
}

/// `params`: a list of `{name, type?}`; absent leaves them, `null` clears them.
pub(super) fn params_param(cmd: &str, p: &Value) -> Result<Option<Vec<Param>>> {
    match p.get("params") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(Vec::new())),
        Some(Value::Array(a)) if a.len() <= cd::MAX_PARAMS => a
            .iter()
            .map(|v| {
                unknown_keys(cmd, v, &["name", "type"])?;
                Ok(Param { name: req_text(cmd, v, "name")?, type_name: text(cmd, v, "type")?.filter(|t| !t.is_empty()) })
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Some(_) => Err(bad(cmd, format!("`params` is a list of at most {} {{name, type?}}", cd::MAX_PARAMS))),
    }
}

pub(super) fn text_error(cmd: &str, e: TextError) -> crate::EngineError {
    bad(cmd, e.to_string())
}
