//! Typed child-request effect protocol.
//!
//! Postman scripts express network work through `pm.sendRequest`, which
//! becomes a typed [`ChildRequest`] effect carrying a fresh op id and the
//! current generation. The shell (probe binary or embedding host) performs
//! the request and hands back a [`ChildRequestResult`]; the sandbox resolves
//! or rejects the script's Promise with it. A completion is only honored
//! when its generation still matches the run's generation — late completions
//! from an older generation are ignored (Celld-style op/generation guard).

/// One in-flight child request emitted by the sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRequest {
    /// Monotonic op id within the run.
    pub op: u64,
    pub method: String,
    pub url: String,
    /// `(name, value)` header pairs in declaration order.
    pub headers: Vec<(String, String)>,
    /// Raw request body when present.
    pub body: Option<String>,
    /// Auth descriptor (`{type, ...}` JSON) when the script supplied one.
    pub auth: Option<String>,
    /// Generation that authorized this effect.
    pub generation: u64,
}

/// Outcome of executing a [`ChildRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRequestResult {
    pub op: u64,
    /// True when a response (any status) was produced; false on transport error.
    pub ok: bool,
    pub status: Option<u16>,
    pub status_text: Option<String>,
    /// `(name, value)` response headers.
    pub headers: Vec<(String, String)>,
    /// Raw response body.
    pub body: Option<String>,
    /// Redacted error text (transport failure, offline mode, ...).
    pub error: Option<String>,
    pub response_time_ms: Option<u64>,
}

/// Executes one child request effect. Implemented by the probe shell
/// (`reqwest` in fetch mode) or by embeddings/tests (loopback listeners).
/// Must be cheap and synchronous: the sandbox pumps it inline.
pub type ChildRequestExecutor = dyn FnMut(&ChildRequest) -> ChildRequestResult;

/// Offline-mode executor: every child request is refused with
/// `MDOK-PM-NETWORK-OFFLINE` and never resolved.
pub fn offline_executor(req: &ChildRequest) -> ChildRequestResult {
    ChildRequestResult {
        op: req.op,
        ok: false,
        status: None,
        status_text: None,
        headers: Vec::new(),
        body: None,
        error: Some(
            "MDOK-PM-NETWORK-OFFLINE: pm.sendRequest is disabled in offline mode".to_string(),
        ),
        response_time_ms: None,
    }
}
