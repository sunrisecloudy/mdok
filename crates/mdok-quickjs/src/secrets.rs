//! Taint collection: values that must never appear in transcripts, logs,
//! diagnostics or exception text (spec section 6).
//!
//! A value is tainted when it comes from a variable/header whose name is
//! listed in the case `secrets` or matches the secret-looking heuristic from
//! `crates/mdok-postman/src/lib.rs` (`looks_secret`, `sensitive_header`).

use crate::{ProbeInput, looks_secret};

/// Whether a header name is sensitive by convention.
pub(crate) fn sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "x-auth-token"
    ) || looks_secret(name)
}

/// Collect every tainted value string reachable from the probe case.
pub(crate) fn taint_from_input(input: &ProbeInput) -> Vec<String> {
    let mut taint = Vec::new();
    let mut add = |v: &serde_json::Value| match v {
        serde_json::Value::String(s) if !s.is_empty() => taint.push(s.clone()),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            let s = v.to_string();
            if !s.is_empty() {
                taint.push(s);
            }
        }
        _ => {}
    };
    for map in [
        &input.variables.global,
        &input.variables.collection,
        &input.variables.environment,
        &input.variables.data,
        &input.variables.local,
    ] {
        for (key, value) in map {
            if input.secrets.iter().any(|s| s == key) || looks_secret(key) {
                add(value);
            }
        }
    }
    for header in input
        .request
        .iter()
        .flat_map(|r| r.headers.iter())
        .chain(input.response.iter().flat_map(|r| r.headers.iter()))
    {
        if (input.secrets.iter().any(|s| s == &header.key) || sensitive_header(&header.key))
            && !header.value.is_empty()
        {
            taint.push(header.value.clone());
        }
    }
    taint.sort();
    taint.dedup();
    taint
}

/// Redact a string against the taint set. Full-string matches are always
/// masked; substring matches only for values of length >= 3 (short tokens
/// would corrupt ordinary text). Returns whether anything was masked.
pub(crate) fn redact_with(taint: &[String], s: &str) -> (String, bool) {
    let mut out = s.to_string();
    let mut redacted = false;
    for v in taint {
        if v.is_empty() {
            continue;
        }
        if out == *v {
            return ("[redacted]".to_string(), true);
        }
        if v.len() >= 3 && out.contains(v.as_str()) {
            out = out.replace(v.as_str(), "[redacted]");
            redacted = true;
        }
    }
    (out, redacted)
}
