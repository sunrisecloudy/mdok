//! Canonical serializable transcript for a probe run.
//!
//! The transcript is the machine-readable record of everything a Postman
//! script did: tests, scope writes, logs, script errors, child requests,
//! control-flow decisions and the visualizer payload. Tainted values are
//! redacted to `[redacted]` before they are ever stored here (see
//! [`crate::pm::Taint`]).

use serde::{Deserialize, Serialize};

/// A single `pm.test(name, fn)` result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    /// Redacted assertion/exception text; `None` when the test passed.
    pub error: Option<String>,
}

/// A variable write made by the script (`pm.environment.set` & friends).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopeWrite {
    /// One of `global`, `collection`, `environment`, `data`, `local`.
    pub scope: String,
    pub key: String,
    /// Stored value (JSON-stringified for non-scalars), redacted when tainted.
    pub value: String,
    /// True when the recorded value was masked.
    pub redacted: bool,
}

/// One `console.*` call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
}

/// A child request (`pm.sendRequest`) emitted by the script.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildRequestRecord {
    /// Effect op id (monotonic within the run).
    pub op: u64,
    pub method: String,
    /// Redacted URL.
    pub url: String,
    pub status: Option<u16>,
    /// Redacted transport/HTTP error text, if any.
    pub error: Option<String>,
    /// False when the effect was not executed (e.g. network offline).
    pub resolved: bool,
    /// True when the URL or error text was masked.
    pub redacted: bool,
}

/// A runner control-flow decision (`pm.execution.*`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlFlowRecord {
    /// `skip_request`, `set_next_request` or `run_request`.
    pub action: String,
    pub value: Option<String>,
    pub supported: bool,
}

/// `pm.visualizer.set(template, data)` payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualizerRecord {
    pub template: String,
    pub data: String,
}

/// A named compatibility diagnostic.
///
/// `code` is one of `MDOK-PM-UNSUPPORTED`, `MDOK-PM-NETWORK-OFFLINE`,
/// `MDOK-PM-REQUIRE`, `MDOK-PM-SECRET-DENIED`, `MDOK-PM-TIMEOUT`,
/// `MDOK-PM-LIMIT`, `MDOK-PM-EVAL`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    /// API path the diagnostic refers to (empty for whole-run diagnostics).
    pub api: String,
    /// Redacted human-readable message.
    pub message: String,
}

/// The full canonical transcript of a script run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Transcript {
    pub tests: Vec<TestResult>,
    pub scope_writes: Vec<ScopeWrite>,
    pub logs: Vec<LogEntry>,
    /// Redacted script-level error text (uncaught exceptions, syntax errors).
    pub errors: Vec<String>,
    pub child_requests: Vec<ChildRequestRecord>,
    pub control_flow: Vec<ControlFlowRecord>,
    pub visualizer: Option<VisualizerRecord>,
}

impl Transcript {
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
            && self.scope_writes.is_empty()
            && self.logs.is_empty()
            && self.errors.is_empty()
            && self.child_requests.is_empty()
            && self.control_flow.is_empty()
            && self.visualizer.is_none()
    }
}
