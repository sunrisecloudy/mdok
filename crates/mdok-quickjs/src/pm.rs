//! The Postman `pm` facade: host state, coverage recording and the typed
//! child-request effect bridge.
//!
//! The facade object tree itself is defined in the JS prelude (`prelude.js`);
//! this module installs the `__mdok_*` host functions the prelude calls and
//! holds the run state (variable scopes, transcript accumulators, taint set,
//! pending child requests).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use rquickjs::{Ctx, Function, Object, Value};

use crate::effect::{ChildRequest, ChildRequestResult};
use crate::transcript::{
    ChildRequestRecord, ControlFlowRecord, Diagnostic, LogEntry, ScopeWrite, TestResult,
    Transcript, VisualizerRecord,
};
use crate::{ProbeInput, Profile, looks_secret};

/// The JS prelude implementing the pm facade.
pub(crate) const PRELUDE: &str = include_str!("prelude.js");

/// A pending child request waiting on the async bridge.
pub(crate) struct PendingChild<'js> {
    pub(crate) request: ChildRequest,
    pub(crate) resolve: Function<'js>,
    pub(crate) reject: Function<'js>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Global = 0,
    Collection = 1,
    Environment = 2,
    Data = 3,
    Local = 4,
}

impl Scope {
    fn parse(name: &str) -> Option<Scope> {
        Some(match name {
            "global" => Scope::Global,
            "collection" => Scope::Collection,
            "environment" => Scope::Environment,
            "data" => Scope::Data,
            "local" => Scope::Local,
            _ => return None,
        })
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Collection => "collection",
            Scope::Environment => "environment",
            Scope::Data => "data",
            Scope::Local => "local",
        }
    }
}

pub(crate) const SCOPE_ORDER: [Scope; 5] = [
    Scope::Global,
    Scope::Collection,
    Scope::Environment,
    Scope::Data,
    Scope::Local,
];

/// Run-level mutable state shared between host functions and the driver.
pub(crate) struct HostState<'js> {
    pub(crate) scopes: [BTreeMap<String, serde_json::Value>; 5],
    secrets: Vec<String>,
    taint: Vec<String>,
    /// (path, is_leaf) in first-use order; a later record may upgrade a
    /// container to a leaf (assertion getters that throw).
    used_api: Vec<(String, bool)>,
    diagnostics: Vec<Diagnostic>,
    pub(crate) tests: Vec<TestResult>,
    scope_writes: Vec<ScopeWrite>,
    logs: Vec<LogEntry>,
    pub(crate) errors: Vec<String>,
    child_requests: Vec<ChildRequestRecord>,
    control_flow: Vec<ControlFlowRecord>,
    visualizer: Option<VisualizerRecord>,
    pub(crate) pending: Vec<PendingChild<'js>>,
    pub(crate) next_op: u64,
    pub(crate) generation: u64,
    pub(crate) timed_out: bool,
    pub(crate) offline_diagnostic_emitted: bool,
    pub(crate) limit_diagnostic_emitted: bool,
    eval_diagnostic_emitted: bool,
    pub(crate) profile: Profile,
    coverage: bool,
    _marker: std::marker::PhantomData<&'js ()>,
}

impl<'js> HostState<'js> {
    pub(crate) fn new(input: &ProbeInput, taint: Vec<String>) -> HostState<'js> {
        let vars = &input.variables;
        let mut scopes = [
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        ];
        for (map, scope_idx) in [
            (&vars.global, 0usize),
            (&vars.collection, 1),
            (&vars.environment, 2),
            (&vars.data, 3),
            (&vars.local, 4),
        ] {
            for (key, value) in map {
                scopes[scope_idx].insert(key.clone(), value.clone());
            }
        }
        HostState {
            scopes,
            secrets: input.secrets.clone(),
            taint,
            used_api: Vec::new(),
            diagnostics: Vec::new(),
            tests: Vec::new(),
            scope_writes: Vec::new(),
            logs: Vec::new(),
            errors: Vec::new(),
            child_requests: Vec::new(),
            control_flow: Vec::new(),
            visualizer: None,
            pending: Vec::new(),
            next_op: 1,
            generation: 1,
            timed_out: false,
            offline_diagnostic_emitted: false,
            limit_diagnostic_emitted: false,
            eval_diagnostic_emitted: false,
            profile: input.profile.clone(),
            coverage: input.coverage,
            _marker: std::marker::PhantomData,
        }
    }

    /// Replace tainted substrings with `[redacted]`; returns whether masked.
    pub(crate) fn redact_owned(&self, s: &str) -> (String, bool) {
        crate::secrets::redact_with(&self.taint, s)
    }

    pub(crate) fn is_tainted(&self, s: &str) -> bool {
        self.taint
            .iter()
            .any(|v| !v.is_empty() && (s == v || (v.len() >= 3 && s.contains(v.as_str()))))
    }

    pub(crate) fn is_secret_name(&self, name: &str) -> bool {
        self.secrets.iter().any(|s| s == name) || looks_secret(name)
    }

    pub(crate) fn record_used(&mut self, path: String, is_leaf: bool) {
        if !self.coverage {
            return;
        }
        if let Some(entry) = self.used_api.iter_mut().find(|(p, _)| *p == path) {
            if is_leaf {
                entry.1 = true;
            }
        } else {
            self.used_api.push((path, is_leaf));
        }
    }

    pub(crate) fn record_unsupported(&mut self, path: String) {
        self.record_used(path.clone(), true);
        let api_version = self.profile.api_version.clone();
        self.push_diagnostic(
            "MDOK-PM-UNSUPPORTED",
            path.clone(),
            format!("{path} is not part of the {api_version} profile"),
        );
    }

    pub(crate) fn push_diagnostic(&mut self, code: &str, api: String, message: String) {
        if self
            .diagnostics
            .iter()
            .any(|d| d.code == code && d.api == api)
        {
            return;
        }
        let (message, _) = self.redact_owned(&message);
        self.diagnostics.push(Diagnostic {
            code: code.to_string(),
            api,
            message,
        });
    }

    pub(crate) fn record_log(&mut self, level: &str, message: &str) {
        if self.logs.len() >= self.profile.max_log_entries {
            if !self.limit_diagnostic_emitted {
                self.limit_diagnostic_emitted = true;
                self.push_diagnostic(
                    "MDOK-PM-LIMIT",
                    format!("console.{level}"),
                    format!(
                        "log limit exceeded ({} entries); further logs dropped",
                        self.profile.max_log_entries
                    ),
                );
            }
            return;
        }
        let mut msg = message.to_string();
        if msg.len() > self.profile.max_log_entry_bytes {
            msg.truncate(self.profile.max_log_entry_bytes);
        }
        let (msg, _) = self.redact_owned(&msg);
        self.logs.push(LogEntry {
            level: level.to_string(),
            message: msg,
        });
    }

    pub(crate) fn record_test(&mut self, name: &str, passed: bool, error: Option<&str>) {
        let error = error.map(|e| {
            let (e, _) = self.redact_owned(e);
            e
        });
        self.tests.push(TestResult {
            name: name.to_string(),
            passed,
            error,
        });
    }

    pub(crate) fn record_scope_write(
        &mut self,
        scope: Scope,
        key: &str,
        value: &serde_json::Value,
    ) {
        // Postman stringifies non-scalar values on set.
        let rendered = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        let secret = self.is_secret_name(key) || self.is_tainted(&rendered);
        let (value, redacted) = if secret {
            ("[redacted]".to_string(), true)
        } else {
            (rendered, false)
        };
        self.scope_writes.push(ScopeWrite {
            scope: scope.name().to_string(),
            key: key.to_string(),
            value,
            redacted,
        });
    }

    pub(crate) fn record_control(&mut self, action: &str, value: Option<String>, supported: bool) {
        if !supported {
            self.push_diagnostic(
                "MDOK-PM-UNSUPPORTED",
                format!("pm.execution.{action}"),
                format!("pm.execution.{action} is a collection-runner-only API"),
            );
        }
        let value = value.map(|v| {
            let (v, _) = self.redact_owned(&v);
            v
        });
        self.control_flow.push(ControlFlowRecord {
            action: action.to_string(),
            value,
            supported,
        });
    }

    pub(crate) fn record_child(&mut self, req: &ChildRequest, result: &ChildRequestResult) {
        let (url, url_redacted) = self.redact_owned(&req.url);
        let (error, error_redacted) = match &result.error {
            Some(e) => {
                let (e, r) = self.redact_owned(e);
                (Some(e), r)
            }
            None => (None, false),
        };
        self.child_requests.push(ChildRequestRecord {
            op: req.op,
            method: req.method.clone(),
            url,
            status: result.status,
            error,
            resolved: result.ok,
            redacted: url_redacted || error_redacted,
        });
    }

    /// Fold `used_api`: keep leaves plus never-traversed containers, in
    /// first-use order (spec section 4).
    pub(crate) fn fold_used_api(&self) -> Vec<String> {
        if !self.coverage {
            return Vec::new();
        }
        let mut seen: Vec<(String, bool)> = Vec::new();
        for (path, leaf) in &self.used_api {
            if let Some(entry) = seen.iter_mut().find(|(p, _)| p == path) {
                if *leaf {
                    entry.1 = true;
                }
            } else {
                seen.push((path.clone(), *leaf));
            }
        }
        let paths: Vec<&String> = seen.iter().map(|(p, _)| p).collect();
        let extended = |p: &str| {
            let prefix = format!("{p}.");
            paths
                .iter()
                .any(|q| q.as_str() != p && q.starts_with(&prefix))
        };
        let mut out = Vec::new();
        for (path, leaf) in &seen {
            if *leaf || !extended(path) {
                out.push(path.clone());
            }
        }
        out
    }

    pub(crate) fn fold_transcript(&self) -> Transcript {
        Transcript {
            tests: self.tests.clone(),
            scope_writes: self.scope_writes.clone(),
            logs: self.logs.clone(),
            errors: self.errors.clone(),
            child_requests: self.child_requests.clone(),
            control_flow: self.control_flow.clone(),
            visualizer: self.visualizer.clone(),
        }
    }

    pub(crate) fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.clone()
    }
}

/// Convert a serde_json value to a JS value via `JSON.parse`.
pub(crate) fn json_to_js<'js>(
    ctx: &Ctx<'js>,
    value: &serde_json::Value,
) -> rquickjs::Result<Value<'js>> {
    let json: Object = ctx.globals().get("JSON")?;
    let parse: Function = json.get("parse")?;
    parse.call((value.to_string(),))
}

/// Convert a JS value to serde_json via `JSON.stringify` (undefined → null).
pub(crate) fn js_to_json<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
) -> rquickjs::Result<serde_json::Value> {
    let json: Object = ctx.globals().get("JSON")?;
    let stringify: Function = json.get("stringify")?;
    let s: Value = stringify.call((value.clone(),))?;
    if let Some(str) = s.as_string().and_then(|s| s.to_string().ok()) {
        Ok(serde_json::from_str(&str).unwrap_or(serde_json::Value::Null))
    } else {
        Ok(serde_json::Value::Null)
    }
}

pub(crate) fn replace_in_scope(
    scope: &BTreeMap<String, serde_json::Value>,
    template: &str,
) -> String {
    let mut out = template.to_string();
    let mut search_from = 0;
    while let Some(start_rel) = out[search_from..].find("{{") {
        let start = search_from + start_rel;
        let Some(end_rel) = out[start + 2..].find("}}") else {
            break;
        };
        let end = start + 2 + end_rel;
        let key = out[start + 2..end].trim();
        if let Some(value) = scope.get(key) {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            out.replace_range(start..end + 2, &rendered);
            search_from = start + rendered.len();
        } else {
            search_from = end + 2;
        }
    }
    out
}

/// Build the JS-facing response object JSON for a resolved child request.
pub(crate) fn child_response_json(result: &ChildRequestResult) -> String {
    let headers: Vec<serde_json::Value> = result
        .headers
        .iter()
        .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
        .collect();
    serde_json::json!({
        "code": result.status,
        "status": result.status_text,
        "headers": headers,
        "body": result.body,
        "response_time_ms": result.response_time_ms,
        "response_size_bytes": result.body.as_ref().map(|b| b.len() as u64),
    })
    .to_string()
}

/// Install the `__mdok_*` host functions and input globals.
pub(crate) fn install_host<'js>(
    ctx: &Ctx<'js>,
    state: Rc<RefCell<HostState<'js>>>,
    input: &ProbeInput,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    globals.set("__mdok_phase", input.phase.as_str())?;
    let request_json = input
        .request
        .as_ref()
        .map(|r| serde_json::to_string(r).unwrap_or_else(|_| "null".into()))
        .unwrap_or_else(|| "null".into());
    globals.set("__mdok_request_json", request_json.as_str())?;
    let response_json = input
        .response
        .as_ref()
        .map(|r| serde_json::to_string(r).unwrap_or_else(|_| "null".into()))
        .unwrap_or_else(|| "null".into());
    globals.set("__mdok_response_json", response_json.as_str())?;

    let set = |name: &str, func: Function<'js>| -> rquickjs::Result<()> { globals.set(name, func) };

    // __mdok_used(path, is_leaf)
    {
        let state = state.clone();
        set(
            "__mdok_used",
            Function::new(ctx.clone(), move |path: String, is_leaf: bool| {
                state.borrow_mut().record_used(path, is_leaf);
            })?,
        )?;
    }
    // __mdok_unsupported(path)
    {
        let state = state.clone();
        set(
            "__mdok_unsupported",
            Function::new(ctx.clone(), move |path: String| {
                state.borrow_mut().record_unsupported(path);
            })?,
        )?;
    }
    // __mdok_log(level, message)
    {
        let state = state.clone();
        set(
            "__mdok_log",
            Function::new(ctx.clone(), move |level: String, message: String| {
                state.borrow_mut().record_log(&level, &message);
            })?,
        )?;
    }
    // __mdok_test(name, passed, error|null)
    {
        let state = state.clone();
        set(
            "__mdok_test",
            Function::new(
                ctx.clone(),
                move |name: String, passed: bool, error: Option<String>| {
                    state
                        .borrow_mut()
                        .record_test(&name, passed, error.as_deref());
                },
            )?,
        )?;
    }
    // __mdok_timer_error(message) — unhandled exception from a timer callback
    {
        let state = state.clone();
        set(
            "__mdok_timer_error",
            Function::new(ctx.clone(), move |message: String| {
                let mut state = state.borrow_mut();
                let (message, _) = state.redact_owned(&message);
                state.errors.push(message);
            })?,
        )?;
    }
    // __mdok_scope_get(scope, name) -> Value|null
    {
        let state = state.clone();
        set(
            "__mdok_scope_get",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, scope: String, name: String| -> rquickjs::Result<Value<'js>> {
                    let state = state.borrow();
                    let Some(scope) = Scope::parse(&scope) else {
                        return Ok(Value::new_null(ctx.clone()));
                    };
                    match state.scopes[scope as usize].get(&name) {
                        Some(value) => json_to_js(&ctx, value),
                        None => Ok(Value::new_null(ctx.clone())),
                    }
                },
            )?,
        )?;
    }
    // __mdok_scope_set(scope, name, value)
    {
        let state = state.clone();
        set(
            "__mdok_scope_set",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      scope: String,
                      name: String,
                      value: Value<'js>|
                      -> rquickjs::Result<()> {
                    let value = js_to_json(&ctx, &value)?;
                    // Postman stringifies non-scalar values on set.
                    let stored = match &value {
                        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                            serde_json::Value::String(value.to_string())
                        }
                        other => other.clone(),
                    };
                    let mut state = state.borrow_mut();
                    let Some(scope) = Scope::parse(&scope) else {
                        return Ok(());
                    };
                    state.record_scope_write(scope, &name, &value);
                    state.scopes[scope as usize].insert(name, stored);
                    Ok(())
                },
            )?,
        )?;
    }
    // __mdok_scope_has(scope, name) -> bool
    {
        let state = state.clone();
        set(
            "__mdok_scope_has",
            Function::new(ctx.clone(), move |scope: String, name: String| {
                let state = state.borrow();
                Scope::parse(&scope)
                    .map(|s| state.scopes[s as usize].contains_key(&name))
                    .unwrap_or(false)
            })?,
        )?;
    }
    // __mdok_scope_unset(scope, name)
    {
        let state = state.clone();
        set(
            "__mdok_scope_unset",
            Function::new(ctx.clone(), move |scope: String, name: String| {
                let mut state = state.borrow_mut();
                if let Some(s) = Scope::parse(&scope) {
                    state.scopes[s as usize].remove(&name);
                }
            })?,
        )?;
    }
    // __mdok_scope_replace(scope, template) -> string
    {
        let state = state.clone();
        set(
            "__mdok_scope_replace",
            Function::new(ctx.clone(), move |scope: String, template: String| {
                let state = state.borrow();
                let Some(scope) = Scope::parse(&scope) else {
                    return template;
                };
                replace_in_scope(&state.scopes[scope as usize], &template)
            })?,
        )?;
    }
    // __mdok_scope_to_object(scope) -> object
    {
        let state = state.clone();
        set(
            "__mdok_scope_to_object",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, scope: String| -> rquickjs::Result<Value<'js>> {
                    let state = state.borrow();
                    let Some(scope) = Scope::parse(&scope) else {
                        return Ok(Value::new_null(ctx.clone()));
                    };
                    let map: serde_json::Value = serde_json::Value::Object(
                        state.scopes[scope as usize]
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    );
                    json_to_js(&ctx, &map)
                },
            )?,
        )?;
    }
    // __mdok_send(opts_json, resolve, reject) -> op_id
    {
        let state = state.clone();
        set(
            "__mdok_send",
            Function::new(
                ctx.clone(),
                move |opts_json: String, resolve: Function<'js>, reject: Function<'js>| -> u64 {
                    let mut state = state.borrow_mut();
                    let op = state.next_op;
                    state.next_op += 1;
                    let opts: serde_json::Value =
                        serde_json::from_str(&opts_json).unwrap_or(serde_json::Value::Null);
                    let method = opts
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("GET")
                        .to_string();
                    let url = opts
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let headers = opts
                        .get("header")
                        .and_then(|h| h.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|h| {
                                    Some((
                                        h.get("key")?.as_str()?.to_string(),
                                        h.get("value")?.as_str()?.to_string(),
                                    ))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let body = opts
                        .get("body")
                        .and_then(|b| b.get("raw"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string());
                    let auth = opts
                        .get("auth")
                        .filter(|a| a.is_object())
                        .map(|a| a.to_string());
                    let request = ChildRequest {
                        op,
                        method,
                        url,
                        headers,
                        body,
                        auth,
                        generation: state.generation,
                    };
                    state.pending.push(PendingChild {
                        request,
                        resolve,
                        reject,
                    });
                    op
                },
            )?,
        )?;
    }
    // __mdok_vault(name, resolve, reject)
    {
        let state = state.clone();
        set(
            "__mdok_vault",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      name: String,
                      resolve: Function<'js>,
                      reject: Function<'js>|
                      -> rquickjs::Result<()> {
                    let state = state.borrow_mut();
                    let granted = state.secrets.contains(&name)
                        || state.scopes.iter().any(|scope| scope.contains_key(&name));
                    if granted {
                        let value = SCOPE_ORDER
                            .iter()
                            .find_map(|s| state.scopes[*s as usize].get(&name))
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let js_value = json_to_js(&ctx, &value)?;
                        let _ = resolve.call::<_, ()>((js_value,));
                    } else {
                        let message = format!(
                            "MDOK-PM-SECRET-DENIED: vault entry \"{name}\" is not granted in this run"
                        );
                        let _ = reject.call::<_, ()>((message,));
                    }
                    Ok(())
                },
            )?,
        )?;
    }
    // __mdok_control(action, value|null, supported)
    {
        let state = state.clone();
        set(
            "__mdok_control",
            Function::new(
                ctx.clone(),
                move |action: String, value: Option<String>, supported: bool| {
                    state.borrow_mut().record_control(&action, value, supported);
                },
            )?,
        )?;
    }
    // __mdok_visualizer(template, data) -> bool (false = over limit)
    {
        let state = state.clone();
        set(
            "__mdok_visualizer",
            Function::new(ctx.clone(), move |template: String, data: String| -> bool {
                let mut state = state.borrow_mut();
                let over = template.len() > state.profile.max_visualizer_template_bytes
                    || data.len() > state.profile.max_visualizer_data_bytes;
                if over {
                    let max_tpl = state.profile.max_visualizer_template_bytes;
                    let max_data = state.profile.max_visualizer_data_bytes;
                    state.push_diagnostic(
                        "MDOK-PM-LIMIT",
                        "pm.visualizer.set".to_string(),
                        format!(
                            "visualizer payload exceeds profile limits (template <= {max_tpl}B, data <= {max_data}B)"
                        ),
                    );
                    return false;
                }
                let (template, _) = state.redact_owned(&template);
                let (data, _) = state.redact_owned(&data);
                state.visualizer = Some(VisualizerRecord { template, data });
                true
            })?,
        )?;
    }
    // __mdok_require(name) -> source|null
    {
        let state = state.clone();
        set(
            "__mdok_require",
            Function::new(
                ctx.clone(),
                move |name: String, record: Option<bool>| -> Option<String> {
                    let mut state = state.borrow_mut();
                    let key = format!("require:{name}");
                    if record.unwrap_or(true) {
                        state.record_used(key.clone(), true);
                    }
                    match crate::modules::module_source(&name) {
                        Some(src) => Some(src.to_string()),
                        None => {
                            let api_version = state.profile.api_version.clone();
                            state.push_diagnostic(
                            "MDOK-PM-REQUIRE",
                            key,
                            format!(
                                "module \"{name}\" is not available in the {api_version} profile"
                            ),
                        );
                            None
                        }
                    }
                },
            )?,
        )?;
    }
    // __mdok_eval_module(source)
    {
        set(
            "__mdok_eval_module",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, source: String| -> rquickjs::Result<()> {
                    ctx.eval::<(), _>(source.as_bytes())?;
                    Ok(())
                },
            )?,
        )?;
    }
    // __mdok_eval_used(name)
    {
        let state = state.clone();
        set(
            "__mdok_eval_used",
            Function::new(ctx.clone(), move |name: String| {
                let mut state = state.borrow_mut();
                if !state.eval_diagnostic_emitted {
                    state.eval_diagnostic_emitted = true;
                    state.push_diagnostic(
                        "MDOK-PM-EVAL",
                        name.clone(),
                        format!("{name} is disabled in the hardened profile"),
                    );
                }
            })?,
        )?;
    }
    Ok(())
}
