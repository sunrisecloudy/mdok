//! MDOK QuickJS engine boundary.
//!
//! An rquickjs sandbox implementing the Postman-compatible `pm` facade
//! (profile `postman-cli-v1`): budgets and deadlines, named capability
//! diagnostics, canonical transcript output, coverage recording, a typed
//! child-request effect protocol and a CLI probe (`mdok-pm-probe`).
//!
//! The runtime shape (stack 512KB, memory 64MB, interrupt handler with an
//! injected deadline, `Context::full`, first-error capture) follows the
//! Terrane capability boundary at
//! `terrane/rust/crates/terrane-cap-js-runtime/src/sandbox.rs`.

#![forbid(unsafe_code)]

pub mod effect;
pub mod modules;
pub mod pm;
pub mod sandbox;
pub(crate) mod secrets;
pub mod transcript;

pub use effect::{ChildRequest, ChildRequestExecutor, ChildRequestResult, offline_executor};
pub use sandbox::{run_script, run_script_with_executor};
pub use transcript::{
    ChildRequestRecord, ControlFlowRecord, Diagnostic, LogEntry, ScopeWrite, TestResult,
    Transcript, VisualizerRecord,
};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The pinned compatibility profile this crate implements.
pub const PROFILE_API_VERSION: &str = "postman-cli-v1";

/// Default script timeout in milliseconds (overridable per case).
pub const DEFAULT_SCRIPT_TIMEOUT_MS: u64 = 2000;

/// Sandbox budgets. All configurable via [`Profile`].
pub const DEFAULT_MAX_STACK_BYTES: usize = 512 * 1024;
pub const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_LOG_ENTRIES: usize = 100;
pub const DEFAULT_MAX_LOG_ENTRY_BYTES: usize = 4 * 1024;
pub const DEFAULT_MAX_TRANSCRIPT_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAX_VISUALIZER_TEMPLATE_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAX_VISUALIZER_DATA_BYTES: usize = 1024 * 1024;

/// Sandbox budgets and limits (spec section 5). All fields optional in JSON;
/// defaults match the contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default = "default_api_version")]
    pub api_version: String,
    #[serde(default = "default_timeout_ms")]
    pub script_timeout_ms: u64,
    #[serde(default = "default_stack")]
    pub max_stack_bytes: usize,
    #[serde(default = "default_memory")]
    pub max_memory_bytes: usize,
    #[serde(default = "default_log_entries")]
    pub max_log_entries: usize,
    #[serde(default = "default_log_entry_bytes")]
    pub max_log_entry_bytes: usize,
    #[serde(default = "default_transcript_bytes")]
    pub max_transcript_bytes: usize,
    #[serde(default = "default_viz_template")]
    pub max_visualizer_template_bytes: usize,
    #[serde(default = "default_viz_data")]
    pub max_visualizer_data_bytes: usize,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            api_version: default_api_version(),
            script_timeout_ms: default_timeout_ms(),
            max_stack_bytes: default_stack(),
            max_memory_bytes: default_memory(),
            max_log_entries: default_log_entries(),
            max_log_entry_bytes: default_log_entry_bytes(),
            max_transcript_bytes: default_transcript_bytes(),
            max_visualizer_template_bytes: default_viz_template(),
            max_visualizer_data_bytes: default_viz_data(),
        }
    }
}

fn default_api_version() -> String {
    PROFILE_API_VERSION.to_string()
}
fn default_timeout_ms() -> u64 {
    DEFAULT_SCRIPT_TIMEOUT_MS
}
fn default_stack() -> usize {
    DEFAULT_MAX_STACK_BYTES
}
fn default_memory() -> usize {
    DEFAULT_MAX_MEMORY_BYTES
}
fn default_log_entries() -> usize {
    DEFAULT_MAX_LOG_ENTRIES
}
fn default_log_entry_bytes() -> usize {
    DEFAULT_MAX_LOG_ENTRY_BYTES
}
fn default_transcript_bytes() -> usize {
    DEFAULT_MAX_TRANSCRIPT_BYTES
}
fn default_viz_template() -> usize {
    DEFAULT_MAX_VISUALIZER_TEMPLATE_BYTES
}
fn default_viz_data() -> usize {
    DEFAULT_MAX_VISUALIZER_DATA_BYTES
}

/// A request header pair.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Header {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
}

/// Request metadata exposed to the script via `pm.request`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RequestData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    /// `{mode, raw}` body; `None` when the request has no body.
    #[serde(default)]
    pub body: Option<RequestBody>,
}

/// Response metadata exposed via `pm.response` / `pm.cookies`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResponseData {
    #[serde(default)]
    pub code: Option<u16>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub response_time_ms: Option<u64>,
    #[serde(default)]
    pub response_size_bytes: Option<u64>,
}

/// A request body descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RequestBody {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub raw: Option<String>,
}

/// The five Postman variable scopes, seeded from the probe case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct VariableSet {
    #[serde(default)]
    pub global: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub collection: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub environment: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub data: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub local: BTreeMap<String, serde_json::Value>,
}

/// A probe case: one Postman script plus its inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProbeInput {
    pub script: String,
    /// `test` | `prerequest` (any string allowed; surfaced as
    /// `pm.info.eventName`).
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub request: Option<RequestData>,
    #[serde(default)]
    pub response: Option<ResponseData>,
    #[serde(default)]
    pub variables: VariableSet,
    /// Variable/header names whose values are tainted.
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default)]
    pub profile: Profile,
    /// Record `used_api` leaf accesses (default true).
    #[serde(default = "default_true")]
    pub coverage: bool,
}

fn default_true() -> bool {
    true
}

/// Script outcome (spec section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// No failed test and no error.
    Passed,
    /// >= 1 failed test, script ran to completion.
    Failed,
    /// Exception escaped / syntax error.
    Error,
    /// Interrupt handler fired.
    Timeout,
}

/// Probe run output (spec section 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeOutput {
    /// False only on harness-level errors (bad input, runtime setup failure).
    pub ok: bool,
    pub outcome: Outcome,
    pub duration_ms: u64,
    /// Leaf `used_api` paths in first-use order (spec section 4).
    pub used_api: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub transcript: Transcript,
}

/// `--list-api` output (spec section 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListApi {
    pub profile: String,
    pub supported: Vec<String>,
    pub modules: Vec<String>,
    pub diagnostic_codes: Vec<String>,
}

pub(crate) const DIAGNOSTIC_CODES: &[&str] = &[
    "MDOK-PM-UNSUPPORTED",
    "MDOK-PM-NETWORK-OFFLINE",
    "MDOK-PM-REQUIRE",
    "MDOK-PM-SECRET-DENIED",
    "MDOK-PM-LIMIT",
    "MDOK-PM-TIMEOUT",
    "MDOK-PM-EVAL",
];

/// The static supported-API surface of the `postman-cli-v1` profile.
///
/// Mirrors the facade installed by the JS prelude: every recorded path the
/// proxy can produce, including containers (`pm.info`, `pm.response.to`, ...)
/// because the recorder keeps never-traversed containers.
pub fn list_api() -> ListApi {
    ListApi {
        profile: PROFILE_API_VERSION.to_string(),
        supported: supported_api_paths(),
        modules: modules::module_names(),
        diagnostic_codes: DIAGNOSTIC_CODES.iter().map(|s| s.to_string()).collect(),
    }
}

fn supported_api_paths() -> Vec<String> {
    let header = |p: &str| {
        vec![
            format!("{p}.get"),
            format!("{p}.has"),
            format!("{p}.toObject"),
            format!("{p}.count"),
        ]
    };
    let scope = |p: &str| {
        vec![
            format!("{p}.get"),
            format!("{p}.set"),
            format!("{p}.has"),
            format!("{p}.unset"),
            format!("{p}.replaceIn"),
            format!("{p}.toObject"),
        ]
    };
    let mut paths: Vec<String> = Vec::new();
    let mut push = |s: &[&str]| {
        for p in s {
            paths.push(p.to_string());
        }
    };
    push(&[
        "pm",
        "pm.test",
        "pm.expect",
        "pm.expect.eq",
        "pm.expect.equals",
        "pm.expect.eql",
        "pm.expect.eqls",
        "pm.expect.exist",
        "pm.expect.haveOwnProperty",
        "pm.expect.valueOf",
        "pm.info",
        "pm.info.eventName",
        "pm.info.iteration",
        "pm.info.iterationCount",
        "pm.info.requestName",
        "pm.info.requestId",
        "pm.request",
        "pm.request.method",
        "pm.request.url",
        "pm.request.headers",
        "pm.request.auth",
        "pm.request.body",
        "pm.request.body.mode",
        "pm.request.body.raw",
        "pm.request.body.toJSON",
        "pm.request.data",
        "pm.response",
        "pm.response.code",
        "pm.response.status",
        "pm.response.responseTime",
        "pm.response.responseSize",
        "pm.response.headers",
        "pm.response.text",
        "pm.response.json",
        "pm.response.toJSON",
        "pm.response.responseCode",
        "pm.response.to",
        "pm.response.to.have",
        "pm.response.to.have.status",
        "pm.response.to.have.header",
        "pm.response.to.have.body",
        "pm.response.to.have.jsonBody",
        "pm.response.to.have.jsonSchema",
        "pm.response.to.have.ok",
        "pm.response.to.have.success",
        "pm.response.to.have.redirection",
        "pm.response.to.have.clientError",
        "pm.response.to.have.serverError",
        "pm.response.to.have.error",
        "pm.response.to.be",
        "pm.response.to.be.info",
        "pm.response.to.be.ok",
        "pm.response.to.be.success",
        "pm.response.to.be.redirection",
        "pm.response.to.be.clientError",
        "pm.response.to.be.serverError",
        "pm.response.to.be.error",
        "pm.response.to.be.withBody",
        "pm.response.to.be.json",
        "pm.response.to.not",
        "pm.response.to.not.have",
        "pm.response.to.not.have.status",
        "pm.response.to.not.have.header",
        "pm.response.to.not.have.body",
        "pm.response.to.not.have.jsonBody",
        "pm.response.to.not.have.jsonSchema",
        "pm.response.to.not.have.ok",
        "pm.response.to.not.have.success",
        "pm.response.to.not.have.redirection",
        "pm.response.to.not.have.clientError",
        "pm.response.to.not.have.serverError",
        "pm.response.to.not.have.error",
        "pm.response.to.not.be",
        "pm.response.to.not.be.info",
        "pm.response.to.not.be.ok",
        "pm.response.to.not.be.success",
        "pm.response.to.not.be.redirection",
        "pm.response.to.not.be.clientError",
        "pm.response.to.not.be.serverError",
        "pm.response.to.not.be.error",
        "pm.response.to.not.be.withBody",
        "pm.response.to.not.be.json",
        "pm.payload",
        "pm.payload.code",
        "pm.payload.status",
        "pm.payload.responseTime",
        "pm.payload.responseSize",
        "pm.payload.headers",
        "pm.payload.text",
        "pm.payload.json",
        "pm.payload.toJSON",
        "pm.payload.responseCode",
        "pm.payload.to",
        "pm.payload.to.have",
        "pm.payload.to.have.status",
        "pm.payload.to.have.header",
        "pm.payload.to.have.body",
        "pm.payload.to.have.jsonBody",
        "pm.payload.to.have.jsonSchema",
        "pm.payload.to.have.ok",
        "pm.payload.to.have.success",
        "pm.payload.to.have.redirection",
        "pm.payload.to.have.clientError",
        "pm.payload.to.have.serverError",
        "pm.payload.to.have.error",
        "pm.payload.to.be",
        "pm.payload.to.be.info",
        "pm.payload.to.be.ok",
        "pm.payload.to.be.success",
        "pm.payload.to.be.redirection",
        "pm.payload.to.be.clientError",
        "pm.payload.to.be.serverError",
        "pm.payload.to.be.error",
        "pm.payload.to.be.withBody",
        "pm.payload.to.be.json",
        "pm.payload.to.not",
        "pm.variables",
        "pm.environment",
        "pm.globals",
        "pm.collectionVariables",
        "pm.iterationData",
        "pm.cookies",
        "pm.cookies.get",
        "pm.cookies.has",
        "pm.cookies.toObject",
        "pm.sendRequest",
        "pm.execution",
        "pm.execution.setNextRequest",
        "pm.execution.skipRequest",
        "pm.execution.runRequest",
        "pm.visualizer",
        "pm.visualizer.set",
        "pm.vault",
        "pm.vault.get",
        "console",
        "console.log",
        "console.info",
        "console.warn",
        "console.error",
        "console.debug",
        "require:lodash",
        "require:moment",
        "require:ajv",
        "require:uuid",
        "require:querystring",
        "require:crypto-js",
    ]);
    for p in ["pm.request.headers", "pm.response.headers"] {
        paths.extend(header(p));
    }
    for p in [
        "pm.variables",
        "pm.environment",
        "pm.globals",
        "pm.collectionVariables",
        "pm.iterationData",
    ] {
        paths.extend(scope(p));
    }
    paths
}

/// Same secret-name heuristic as `crates/mdok-postman/src/lib.rs`.
pub fn looks_secret(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '.'], "_");
    [
        "secret",
        "password",
        "passwd",
        "token",
        "api_key",
        "apikey",
        "authorization",
        "cookie",
        "set_cookie",
        "credential",
        "private_key",
        "client_secret",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}
