//! Typed child-request effect protocol.
//!
//! Postman scripts express network work through `pm.sendRequest`, which
//! becomes a typed [`ChildRequest`] effect carrying a fresh op id and the
//! current generation. The shell (probe binary or embedding host) performs
//! the request and hands back a [`ChildRequestResult`]; the sandbox resolves
//! or rejects the script's Promise with it. A completion is only honored
//! when its generation still matches the run's generation — late completions
//! from an older generation are ignored (Celld-style op/generation guard).

use std::time::{Duration, Instant};

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

/// Fetch-mode executor used by the standalone probe and embedding hosts.
///
/// The callback is deliberately synchronous because the sandbox pumps one
/// typed effect at a time. Response bodies are capped so a script cannot make
/// the probe retain an unbounded amount of network data. The sandbox controls
/// transcript redaction for secret-bearing request metadata before output.
pub fn fetch_executor(
    request_timeout: Duration,
    policy: mdok_curl::CurlPolicy,
) -> impl FnMut(&ChildRequest) -> ChildRequestResult {
    const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
    let client = policy.build_gated_blocking_client(Some(request_timeout), 10, true);
    move |req: &ChildRequest| {
        let client = match &client {
            Ok(client) => client,
            Err(error) => {
                return request_error(req, format!("gated http client setup failed: {error}"));
            }
        };
        // Pre-flight the URL through the same egress policy the curl path
        // enforces (scheme/host allow+deny, SSRF/private-network guard,
        // post-DNS resolved-address check). See security finding F4.
        let parsed = match reqwest::Url::parse(&req.url) {
            Ok(url) => url,
            Err(error) => {
                return request_error(req, format!("invalid request url: {error}"));
            }
        };
        if let Err(error) = policy.enforce_url(&parsed) {
            return request_error(req, format!("request blocked by policy: {error}"));
        }
        let method = req
            .method
            .parse::<reqwest::Method>()
            .unwrap_or(reqwest::Method::GET);
        let mut builder = client.request(method, &req.url);
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &req.body {
            builder = builder.body(body.clone());
        }
        let started = Instant::now();
        match builder.send() {
            Ok(response) => {
                let status = response.status().as_u16();
                let status_text = response.status().canonical_reason().map(str::to_string);
                let headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.as_str().to_string(),
                            value.to_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect();
                let body = match response.bytes() {
                    Ok(bytes) => {
                        let truncated: Vec<u8> =
                            bytes.iter().take(MAX_BODY_BYTES).cloned().collect();
                        String::from_utf8_lossy(&truncated).into_owned()
                    }
                    Err(error) => {
                        return ChildRequestResult {
                            op: req.op,
                            ok: false,
                            status: None,
                            status_text: None,
                            headers,
                            body: None,
                            error: Some(format!("response read failed: {error}")),
                            response_time_ms: Some(started.elapsed().as_millis() as u64),
                        };
                    }
                };
                ChildRequestResult {
                    op: req.op,
                    ok: true,
                    status: Some(status),
                    status_text,
                    headers,
                    body: Some(body),
                    error: None,
                    response_time_ms: Some(started.elapsed().as_millis() as u64),
                }
            }
            Err(error) => request_error(req, format!("request failed: {error}")),
        }
    }
}

fn request_error(req: &ChildRequest, error: String) -> ChildRequestResult {
    ChildRequestResult {
        op: req.op,
        ok: false,
        status: None,
        status_text: None,
        headers: Vec::new(),
        body: None,
        error: Some(error),
        response_time_ms: None,
    }
}

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
