//! JQ-style path machinery shared by JQ@, JQ!, JQ-DEL (record module)
//!
//! Ported from forthic-ts record_module.ts (the injection-safe replacement
//! for the removed |REC@ — paths are data, never interpolated source).
//!
//! Grammar (string paths): `.` separates fields; `[n]` indexes (strict
//! integers — ts's parseInt("1x") leniency is deliberately not ported);
//! `["key"]` / `['key']` quotes a field; bare `[]` is the ITERATE segment
//! (JQ@ only). Array paths supply dynamic keys: numbers become indexes,
//! everything else stringifies to a field; array paths can never produce
//! Iterate.
//!
//! Divergence from ts, documented: ts iterates/indexes RECORDS in
//! sorted-key order (a workaround for JS object-order quirks); rs uses
//! insertion order, consistent with the #33 IndexMap philosophy.

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PathSegment {
    Field(String),
    Index(i64),
    Iterate,
}

fn path_error(message: impl Into<String>) -> ForthicError {
    ForthicError::InvalidOperation {
        forthic: String::new(),
        message: message.into(),
        location: None,
        cause: None,
    }
}

/// Parse a path value (string or array) into segments
pub(crate) fn parse_jq_path(path: &ForthicValue) -> Result<Vec<PathSegment>, ForthicError> {
    match path {
        ForthicValue::Array(parts) => parts
            .iter()
            .map(|p| match p {
                ForthicValue::Int(i) => Ok(PathSegment::Index(*i)),
                ForthicValue::Float(f) if f.fract() == 0.0 => Ok(PathSegment::Index(*f as i64)),
                ForthicValue::String(s) => Ok(PathSegment::Field(s.clone())),
                ForthicValue::Bool(b) => Ok(PathSegment::Field(b.to_string())),
                ForthicValue::Null => Ok(PathSegment::Field("null".to_string())),
                other => Err(path_error(format!(
                    "JQ path: invalid path element {other:?}"
                ))),
            })
            .collect(),
        ForthicValue::String(s) => parse_string_path(s),
        other => Err(path_error(format!(
            "JQ path must be a string or array, got {other:?}"
        ))),
    }
}

fn parse_string_path(path: &str) -> Result<Vec<PathSegment>, ForthicError> {
    let mut segments = Vec::new();
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    // A single leading '.' is stripped ("" and "." are the empty path)
    if chars.first() == Some(&'.') {
        i = 1;
    }
    while i < chars.len() {
        match chars[i] {
            '.' => i += 1, // separator (doubled dots collapse)
            '[' => {
                i += 1;
                if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                    // ["key"] / ['key'] — scan to the matching quote
                    let quote = chars[i];
                    i += 1;
                    let start = i;
                    while i < chars.len() && chars[i] != quote {
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err(path_error("JQ path: unclosed quote"));
                    }
                    let name: String = chars[start..i].iter().collect();
                    i += 1; // past quote
                    if i >= chars.len() || chars[i] != ']' {
                        return Err(path_error("JQ path: missing ] after quoted key"));
                    }
                    i += 1;
                    segments.push(PathSegment::Field(name));
                } else {
                    let start = i;
                    while i < chars.len() && chars[i] != ']' {
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err(path_error("JQ path: unclosed ["));
                    }
                    let inner: String = chars[start..i].iter().collect();
                    i += 1;
                    if inner.is_empty() {
                        segments.push(PathSegment::Iterate);
                    } else {
                        // Strict integer parse — ts's parseInt("1x") == 1
                        // leniency is deliberately not ported
                        let n = inner
                            .parse::<i64>()
                            .map_err(|_| path_error(format!("JQ path: invalid index '{inner}'")))?;
                        segments.push(PathSegment::Index(n));
                    }
                }
            }
            _ => {
                let start = i;
                while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                if !name.is_empty() {
                    segments.push(PathSegment::Field(name));
                }
            }
        }
    }
    Ok(segments)
}

/// Get the value at a path. Any miss is NULL. Iterate (`[]`) maps over the
/// container's items/values; when a LATER segment is also Iterate and a
/// sub-result is an array, it splats (flattens one level) — so
/// `.users[].name` is flat while `.[].tags` is an array of arrays.
pub(crate) fn jq_get(container: &ForthicValue, segments: &[PathSegment]) -> ForthicValue {
    let Some((head, rest)) = segments.split_first() else {
        return container.clone();
    };
    match head {
        PathSegment::Field(name) => match container {
            ForthicValue::Record(rec) => match rec.get(name) {
                Some(v) => jq_get(v, rest),
                None => ForthicValue::Null,
            },
            _ => ForthicValue::Null,
        },
        PathSegment::Index(n) => match container {
            ForthicValue::Array(arr) => {
                let idx = normalize_index(*n, arr.len());
                match idx {
                    Some(i) => jq_get(&arr[i], rest),
                    None => ForthicValue::Null,
                }
            }
            // Records index their entries in INSERTION order (ts sorts —
            // documented divergence)
            ForthicValue::Record(rec) => {
                let idx = normalize_index(*n, rec.len());
                match idx.and_then(|i| rec.get_index(i)) {
                    Some((_, v)) => jq_get(v, rest),
                    None => ForthicValue::Null,
                }
            }
            _ => ForthicValue::Null,
        },
        PathSegment::Iterate => {
            let items: Vec<ForthicValue> = match container {
                ForthicValue::Array(arr) => arr.clone(),
                ForthicValue::Record(rec) => rec.values().cloned().collect(),
                _ => Vec::new(),
            };
            let later_iterate = rest.contains(&PathSegment::Iterate);
            let mut out = Vec::new();
            for item in items {
                let sub = if rest.is_empty() {
                    item
                } else {
                    jq_get(&item, rest)
                };
                match sub {
                    ForthicValue::Array(inner) if later_iterate => out.extend(inner),
                    other => out.push(other),
                }
            }
            ForthicValue::Array(out)
        }
    }
}

fn normalize_index(n: i64, len: usize) -> Option<usize> {
    let idx = if n < 0 { n + len as i64 } else { n };
    (idx >= 0 && (idx as usize) < len).then_some(idx as usize)
}

/// Set the value at a path, auto-creating missing intermediates: the
/// created container's kind is decided by the NEXT segment (Index -> array,
/// Field -> record). Scalar intermediates are clobbered. Array sets pad
/// out-of-range indexes with NULL (ts leaves JS holes); negative set
/// indexes error; Field-into-array errors (ts sets a stray JS property).
/// Index-into-record coerces to a string key (ts behavior).
pub(crate) fn jq_set(
    container: &mut ForthicValue,
    segments: &[PathSegment],
    value: ForthicValue,
) -> Result<(), ForthicError> {
    let Some((head, rest)) = segments.split_first() else {
        *container = value;
        return Ok(());
    };

    match head {
        PathSegment::Iterate => Err(path_error("JQ!: [] iteration not supported in set paths")),
        PathSegment::Field(name) => {
            let ForthicValue::Record(_) = container else {
                if let ForthicValue::Array(_) = container {
                    return Err(path_error(format!(
                        "JQ!: cannot set field '{name}' on an array"
                    )));
                }
                // Scalar/NULL intermediate: clobber with a record
                *container = ForthicValue::Record(IndexMap::new());
                return jq_set(container, segments, value);
            };
            let ForthicValue::Record(rec) = container else {
                unreachable!()
            };
            if rest.is_empty() {
                rec.insert(name.clone(), value);
                return Ok(());
            }
            let entry = rec
                .entry(name.clone())
                .or_insert_with(|| new_container_for(rest.first()));
            ensure_container(entry, rest.first());
            jq_set(entry, rest, value)
        }
        PathSegment::Index(n) => match container {
            ForthicValue::Array(arr) => {
                if *n < 0 {
                    return Err(path_error(format!("JQ!: negative set index {n}")));
                }
                let idx = *n as usize;
                if idx >= arr.len() {
                    arr.resize(idx + 1, ForthicValue::Null);
                }
                if rest.is_empty() {
                    arr[idx] = value;
                    return Ok(());
                }
                ensure_container(&mut arr[idx], rest.first());
                jq_set(&mut arr[idx], rest, value)
            }
            ForthicValue::Record(rec) => {
                // Index into a record sets the string key (ts behavior)
                let key = n.to_string();
                if rest.is_empty() {
                    rec.insert(key, value);
                    return Ok(());
                }
                let entry = rec
                    .entry(key)
                    .or_insert_with(|| new_container_for(rest.first()));
                ensure_container(entry, rest.first());
                jq_set(entry, rest, value)
            }
            _ => {
                // Scalar/NULL: clobber with an array and retry
                *container = ForthicValue::Array(Vec::new());
                jq_set(container, segments, value)
            }
        },
    }
}

/// The container kind auto-creation should produce, given the NEXT segment
pub(crate) fn new_container_for(next: Option<&PathSegment>) -> ForthicValue {
    match next {
        Some(PathSegment::Index(_)) => ForthicValue::Array(Vec::new()),
        _ => ForthicValue::Record(IndexMap::new()),
    }
}

/// Clobber a scalar/NULL intermediate with the container kind the next
/// segment needs (existing containers are left alone)
fn ensure_container(value: &mut ForthicValue, next: Option<&PathSegment>) {
    if !matches!(value, ForthicValue::Array(_) | ForthicValue::Record(_)) {
        *value = new_container_for(next);
    }
}

/// Delete the value at a path. NO auto-creation: any missing or scalar
/// intermediate is a silent no-op, as is a missing leaf. Array deletes
/// shift left (order preserved); negative indexes normalize.
pub(crate) fn jq_del(
    container: &mut ForthicValue,
    segments: &[PathSegment],
) -> Result<(), ForthicError> {
    let Some((head, rest)) = segments.split_first() else {
        return Ok(());
    };
    if matches!(head, PathSegment::Iterate)
        || rest.iter().any(|s| matches!(s, PathSegment::Iterate))
    {
        return Err(path_error(
            "JQ-DEL: [] iteration not supported in delete paths",
        ));
    }

    if rest.is_empty() {
        match (head, container) {
            (PathSegment::Field(name), ForthicValue::Record(rec)) => {
                // shift_remove: preserve the order of remaining entries
                rec.shift_remove(name);
            }
            (PathSegment::Index(n), ForthicValue::Array(arr)) => {
                if let Some(idx) = normalize_index(*n, arr.len()) {
                    arr.remove(idx);
                }
            }
            (PathSegment::Index(n), ForthicValue::Record(rec)) => {
                rec.shift_remove(&n.to_string());
            }
            _ => {} // scalar or shape mismatch: no-op
        }
        return Ok(());
    }

    let child = match (head, container) {
        (PathSegment::Field(name), ForthicValue::Record(rec)) => rec.get_mut(name),
        (PathSegment::Index(n), ForthicValue::Array(arr)) => {
            let len = arr.len();
            normalize_index(*n, len).and_then(move |i| arr.get_mut(i))
        }
        (PathSegment::Index(n), ForthicValue::Record(rec)) => rec.get_mut(&n.to_string()),
        _ => None,
    };
    match child {
        Some(child) => jq_del(child, rest),
        None => Ok(()), // missing intermediate: silent no-op
    }
}
