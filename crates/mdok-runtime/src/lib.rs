#![forbid(unsafe_code)]

//! Sequential document execution over the owned curl and JMESPath APIs.

use mdok_curl::{
    CurlError, CurlPlan, CurlPolicy, E_BODY_LIMIT, ExecutionSession, TransferResponse,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use thiserror::Error;

pub const E_JMES_PARSE: &str = "MDOK-E500";
pub const E_JMES_TYPE: &str = "MDOK-E501";
pub const E_ASSERTION: &str = "MDOK-E502";
pub const E_CAPTURE_TYPE: &str = "MDOK-E503";
pub const E_CAPTURE_COLLISION: &str = "MDOK-E504";

#[derive(Clone, Debug)]
pub struct RuntimePolicy {
    pub curl: CurlPolicy,
    pub max_json_body_bytes: usize,
    pub fail_fast: bool,
    pub allow_capture_override: bool,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            curl: CurlPolicy::default(),
            max_json_body_bytes: 8 * 1024 * 1024,
            fail_fast: false,
            allow_capture_override: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DocumentPlan {
    pub path: String,
    pub variables: BTreeMap<String, Value>,
    pub steps: Vec<StepPlan>,
}

#[derive(Clone, Debug)]
pub struct StepPlan {
    pub name: String,
    pub argv: Vec<String>,
    pub checks: Vec<CheckPlan>,
    pub captures: Vec<CapturePlan>,
}

impl StepPlan {
    pub fn new(name: impl Into<String>, argv: Vec<String>) -> Self {
        Self {
            name: name.into(),
            argv,
            checks: Vec::new(),
            captures: Vec::new(),
        }
    }
    pub fn check(mut self, expression: impl Into<String>) -> Result<Self, RuntimeError> {
        self.checks.push(CheckPlan::new(expression)?);
        Ok(self)
    }
    pub fn capture(mut self, expression: impl Into<String>) -> Result<Self, RuntimeError> {
        self.captures.push(CapturePlan::new(expression)?);
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct CheckPlan {
    pub expression: String,
    compiled: mdok_jmespath::CompiledExpression,
}
impl CheckPlan {
    pub fn new(expression: impl Into<String>) -> Result<Self, RuntimeError> {
        let expression = expression.into();
        let compiled = mdok_jmespath::compile(&expression)
            .map_err(|e| RuntimeError::jmes(E_JMES_PARSE, e.to_string()))?;
        Ok(Self {
            expression,
            compiled,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CapturePlan {
    pub expression: String,
    compiled: mdok_jmespath::CompiledExpression,
}
impl CapturePlan {
    pub fn new(expression: impl Into<String>) -> Result<Self, RuntimeError> {
        let expression = expression.into();
        let compiled = mdok_jmespath::compile(&expression)
            .map_err(|e| RuntimeError::jmes(E_JMES_PARSE, e.to_string()))?;
        Ok(Self {
            expression,
            compiled,
        })
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("{code}: {message}")]
    Diagnostic {
        code: &'static str,
        message: String,
        step: Option<String>,
        expression: Option<String>,
        observed: Option<Value>,
    },
    #[error("{code}: {message}")]
    Curl {
        code: &'static str,
        message: String,
        step: Option<String>,
    },
    #[error("{code}: {message}")]
    Body {
        code: &'static str,
        message: String,
        step: Option<String>,
    },
}

impl RuntimeError {
    fn jmes(code: &'static str, message: String) -> Self {
        Self::Diagnostic {
            code,
            message,
            step: None,
            expression: None,
            observed: None,
        }
    }
    fn diagnostic(
        code: &'static str,
        message: impl Into<String>,
        step: &str,
        expression: &str,
        observed: Option<Value>,
    ) -> Self {
        Self::Diagnostic {
            code,
            message: message.into(),
            step: Some(step.to_owned()),
            expression: Some(expression.to_owned()),
            observed,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Diagnostic { code, .. } | Self::Curl { code, .. } | Self::Body { code, .. } => {
                code
            }
        }
    }
}

#[derive(Debug)]
pub struct DocumentResult {
    pub path: String,
    pub passed: bool,
    pub variables: BTreeMap<String, Value>,
    pub steps: Vec<StepResult>,
}

#[derive(Debug)]
pub struct StepResult {
    pub name: String,
    pub response: Option<TransferResponse>,
    pub checks: Vec<CheckResult>,
    pub captures: BTreeMap<String, Value>,
    pub error: Option<RuntimeError>,
}

/// Mutable state for one document execution.
///
/// The template engine consumes the BTreeMap while the curl response context
/// consumes JSON values. Cache the JSON variable snapshot lazily across steps
/// and invalidate it only when captures change variables. The snapshots
/// contain only current variables and summaries; they do not retain response
/// bodies or prior contexts.
struct ExecutionState {
    variables: BTreeMap<String, Value>,
    variables_json: Option<Value>,
    step_summaries: Value,
}

impl ExecutionState {
    fn new(variables: &BTreeMap<String, Value>) -> Self {
        Self {
            variables: variables.clone(),
            variables_json: None,
            step_summaries: Value::Object(Map::new()),
        }
    }

    fn context_values(&mut self) -> (&Value, &Value) {
        let Self {
            variables,
            variables_json,
            step_summaries,
        } = self;
        let variables_json = variables_json.get_or_insert_with(|| {
            Value::Object(
                variables
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            )
        });
        (variables_json, step_summaries)
    }

    fn commit_captures(&mut self, pending: &BTreeMap<String, Value>) {
        for (key, value) in pending {
            self.variables.insert(key.clone(), value.clone());
        }
        self.variables_json = None;
    }

    fn record_step(&mut self, name: String, status: Option<u16>, passed: bool) {
        self.step_summaries
            .as_object_mut()
            .expect("step summaries JSON must remain an object")
            .insert(name, json!({ "status": status, "passed": passed }));
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckResult {
    pub expression: String,
    pub passed: bool,
    pub result: Option<Value>,
    pub error_code: Option<String>,
}

impl DocumentPlan {
    pub fn new(
        path: impl Into<String>,
        variables: BTreeMap<String, Value>,
        steps: Vec<StepPlan>,
    ) -> Self {
        Self {
            path: path.into(),
            variables,
            steps,
        }
    }

    /// Execute requests in source order. Captures are committed only after all
    /// checks and all capture expressions for a step have succeeded.
    pub fn execute(&self, policy: &RuntimePolicy) -> Result<DocumentResult, RuntimeError> {
        self.execute_with_cancel(policy, None)
    }

    /// Execute requests in source order with a document-scoped native session.
    /// The cancellation callback is forwarded into libcurl's progress callback
    /// for native transfers and checked between fallback attempts.
    pub fn execute_with_cancel(
        &self,
        policy: &RuntimePolicy,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<DocumentResult, RuntimeError> {
        let mut state = ExecutionState::new(&self.variables);
        let mut results = Vec::with_capacity(self.steps.len());
        let mut session = ExecutionSession::new();
        for step in &self.steps {
            let argv = resolve_argv(&step.argv, &state.variables)
                .map_err(|e| RuntimeError::diagnostic("MDOK-E505", e, &step.name, "", None))?;
            let plan =
                CurlPlan::parse(&argv, &policy.curl).map_err(|e| curl_error(e, &step.name))?;
            let response = plan
                .execute_in_session_with_cancel(&policy.curl, &mut session, cancelled)
                .map_err(|e| curl_error(e, &step.name))?;
            let (variables_json, step_summaries) = state.context_values();
            let context = response
                .evaluation_json_limited(variables_json, step_summaries, policy.max_json_body_bytes)
                .map_err(|e| body_error(e, &step.name))?;
            let mut checks = Vec::with_capacity(step.checks.len());
            let mut check_failure = None;
            for check in &step.checks {
                let result = evaluate(&check.compiled, &context);
                match result {
                    Ok(value) => {
                        if value.as_bool() == Some(true) {
                            checks.push(CheckResult {
                                expression: check.expression.clone(),
                                passed: true,
                                result: Some(value),
                                error_code: None,
                            });
                        } else if value.as_bool() == Some(false) {
                            checks.push(CheckResult {
                                expression: check.expression.clone(),
                                passed: false,
                                result: Some(value.clone()),
                                error_code: Some(E_ASSERTION.to_owned()),
                            });
                            check_failure.get_or_insert(RuntimeError::diagnostic(
                                E_ASSERTION,
                                "assertion evaluated to false",
                                &step.name,
                                &check.expression,
                                Some(value),
                            ));
                        } else {
                            checks.push(CheckResult {
                                expression: check.expression.clone(),
                                passed: false,
                                result: Some(value.clone()),
                                error_code: Some(E_JMES_TYPE.to_owned()),
                            });
                            check_failure.get_or_insert(RuntimeError::diagnostic(
                                E_JMES_TYPE,
                                "check result must be boolean",
                                &step.name,
                                &check.expression,
                                Some(value),
                            ));
                        }
                    }
                    Err(error) => {
                        let runtime_error = RuntimeError::diagnostic(
                            E_JMES_TYPE,
                            error,
                            &step.name,
                            &check.expression,
                            None,
                        );
                        checks.push(CheckResult {
                            expression: check.expression.clone(),
                            passed: false,
                            result: None,
                            error_code: Some(E_JMES_TYPE.to_owned()),
                        });
                        check_failure.get_or_insert(runtime_error);
                    }
                }
                if policy.fail_fast && check_failure.is_some() {
                    break;
                }
            }
            if let Some(error) = check_failure {
                state.record_step(step.name.clone(), response.status, false);
                let result = StepResult {
                    name: step.name.clone(),
                    response: Some(response),
                    checks,
                    captures: BTreeMap::new(),
                    error: Some(error),
                };
                results.push(result);
                return Ok(DocumentResult {
                    path: self.path.clone(),
                    passed: false,
                    variables: state.variables,
                    steps: results,
                });
            }
            let mut evaluated_captures = Vec::with_capacity(step.captures.len());
            for capture in &step.captures {
                let value = evaluate(&capture.compiled, &context).map_err(|e| {
                    RuntimeError::diagnostic(E_JMES_TYPE, e, &step.name, &capture.expression, None)
                })?;
                evaluated_captures.push((capture.expression.as_str(), value));
            }
            let pending = collect_captures(
                &evaluated_captures,
                &state.variables,
                policy.allow_capture_override,
                &step.name,
            )?;
            state.commit_captures(&pending);
            state.record_step(step.name.clone(), response.status, true);
            results.push(StepResult {
                name: step.name.clone(),
                response: Some(response),
                checks,
                captures: pending,
                error: None,
            });
        }
        Ok(DocumentResult {
            path: self.path.clone(),
            passed: true,
            variables: state.variables,
            steps: results,
        })
    }
}

fn evaluate(
    expression: &mdok_jmespath::CompiledExpression,
    context: &Value,
) -> Result<Value, String> {
    expression.evaluate(context).map_err(|e| e.to_string())
}

fn resolve_argv(
    argv: &[String],
    variables: &BTreeMap<String, Value>,
) -> Result<Vec<String>, String> {
    argv.iter().map(|arg| resolve_arg(arg, variables)).collect()
}
fn resolve_arg(arg: &str, variables: &BTreeMap<String, Value>) -> Result<String, String> {
    mdok_template::render(arg, variables).map_err(|error| error.to_string())
}
fn collect_captures(
    evaluated_captures: &[(&str, Value)],
    variables: &BTreeMap<String, Value>,
    allow_capture_override: bool,
    step: &str,
) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let mut pending = BTreeMap::new();
    for (expression, value) in evaluated_captures {
        let object = value.as_object().ok_or_else(|| {
            RuntimeError::diagnostic(
                E_CAPTURE_TYPE,
                "capture expression must return an object",
                step,
                expression,
                Some(value.clone()),
            )
        })?;
        for (key, value) in object {
            if !valid_variable_name(key) {
                return Err(RuntimeError::diagnostic(
                    E_CAPTURE_COLLISION,
                    format!("invalid capture key `{key}`"),
                    step,
                    expression,
                    None,
                ));
            }
            if !allow_capture_override && (pending.contains_key(key) || variables.contains_key(key))
            {
                return Err(RuntimeError::diagnostic(
                    E_CAPTURE_COLLISION,
                    format!("capture key `{key}` collides with an existing variable"),
                    step,
                    expression,
                    None,
                ));
            }
            pending.insert(key.clone(), value.clone());
        }
    }
    Ok(pending)
}
fn valid_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
fn curl_error(error: CurlError, step: &str) -> RuntimeError {
    RuntimeError::Curl {
        code: error.code,
        message: error.message,
        step: Some(step.to_owned()),
    }
}
fn body_error(error: CurlError, step: &str) -> RuntimeError {
    let code = if error.code == E_BODY_LIMIT {
        E_BODY_LIMIT
    } else {
        error.code
    };
    RuntimeError::Body {
        code,
        message: error.message,
        step: Some(step.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;
    #[test]
    fn check_and_capture_compile() {
        let step = StepPlan::new("one", vec!["curl".into(), "http://example.test".into()])
            .check("status == `200`")
            .unwrap()
            .capture("{token: body.token}")
            .unwrap();
        assert_eq!(step.checks.len(), 1);
    }
    #[test]
    fn interpolation_stays_one_argument() {
        let mut vars = BTreeMap::new();
        vars.insert("x".into(), Value::String("a;b $(echo no)".into()));
        let args = resolve_argv(&["curl".into(), "{{x|raw}}".into()], &vars).unwrap();
        assert_eq!(args[1], "a;b $(echo no)");
    }
    #[test]
    fn interpolation_resolves_nested_array_indexes() {
        let vars = [("payload".into(), json!({"items": [{"id": "item-7"}]}))]
            .into_iter()
            .collect();
        let args = resolve_argv(&["{{payload.items[0].id}}".into()], &vars).unwrap();
        assert_eq!(args, vec!["item-7"]);
    }

    #[test]
    fn url_filter_uses_rfc3986_component_encoding() {
        let vars = [("component".into(), json!("a+b c/d?x=y&z"))]
            .into_iter()
            .collect();
        let args = resolve_argv(&["{{component|url}}".into()], &vars).unwrap();
        assert_eq!(args, vec!["a%2Bb%20c%2Fd%3Fx%3Dy%26z"]);
    }

    #[test]
    fn invalid_capture_keys_use_collision_error() {
        let captures = [("capture", json!({"bad-key": true}))];
        let error = collect_captures(&captures, &BTreeMap::new(), false, "one").unwrap_err();
        assert_eq!(error.code(), E_CAPTURE_COLLISION);
    }

    #[test]
    fn capture_override_applies_to_existing_and_pending_keys() {
        let variables = [("token".to_owned(), json!("original"))]
            .into_iter()
            .collect();
        let existing_collision = [("capture", json!({"token": "new"}))];
        let error = collect_captures(&existing_collision, &variables, false, "one").unwrap_err();
        assert_eq!(error.code(), E_CAPTURE_COLLISION);

        let duplicate_pending = vec![
            ("first", json!({"token": "first"})),
            ("second", json!({"token": "second"})),
        ];
        let error =
            collect_captures(&duplicate_pending, &BTreeMap::new(), false, "one").unwrap_err();
        assert_eq!(error.code(), E_CAPTURE_COLLISION);

        let captured = collect_captures(&duplicate_pending, &variables, true, "one").unwrap();
        assert_eq!(captured.get("token"), Some(&json!("second")));
    }

    #[test]
    fn capture_names_are_validated() {
        assert!(valid_variable_name("token_1"));
        assert!(!valid_variable_name("1token"));
    }

    #[test]
    fn execution_state_updates_json_snapshots_incrementally() {
        let initial = [("request_id".to_owned(), json!("req-1"))]
            .into_iter()
            .collect();
        let mut state = ExecutionState::new(&initial);
        assert!(state.variables_json.is_none());
        let (variables_json, step_summaries) = state.context_values();
        assert_eq!(variables_json, &json!({"request_id": "req-1"}));
        assert_eq!(step_summaries, &json!({}));

        let pending = [("token".to_owned(), json!("abc"))].into_iter().collect();
        state.commit_captures(&pending);
        assert!(state.variables_json.is_none());
        let (variables_json, _) = state.context_values();
        assert_eq!(
            variables_json,
            &json!({"request_id": "req-1", "token": "abc"})
        );
        state.record_step("login".into(), Some(200), true);

        assert_eq!(state.variables.get("token"), Some(&json!("abc")));
        assert_eq!(
            state.step_summaries,
            json!({"login": {"status": 200, "passed": true}})
        );
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set request timeout");
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let count = stream.read(&mut chunk).expect("read request");
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[test]
    fn document_reuses_native_cookie_session_and_propagates_cancellation() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind runtime fixture");
        let address = listener.local_addr().expect("runtime fixture address");
        let server = thread::spawn(move || {
            let mut saw_cookie = false;
            for request_number in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept runtime request");
                let request = read_request(&mut stream);
                if request_number == 1 {
                    saw_cookie = request.to_ascii_lowercase().contains("cookie: session=abc");
                }
                let response = if request_number == 0 {
                    b"HTTP/1.1 200 OK\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
                } else if saw_cookie {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                        .as_slice()
                } else {
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno"
                        .as_slice()
                };
                stream.write_all(response).expect("write runtime response");
            }
            saw_cookie
        });

        let base = format!("http://127.0.0.1:{}", address.port());
        let document = DocumentPlan::new(
            "session.md",
            BTreeMap::new(),
            vec![
                StepPlan::new("set", vec!["curl".into(), format!("{base}/set")]),
                StepPlan::new("check", vec!["curl".into(), format!("{base}/check")]),
            ],
        );
        let policy = RuntimePolicy {
            curl: CurlPolicy::local_test(),
            ..RuntimePolicy::default()
        };
        let result = document.execute(&policy).expect("execute session document");
        assert!(result.passed);
        assert!(server.join().expect("join runtime fixture"));

        let cancel_listener =
            TcpListener::bind(("127.0.0.1", 0)).expect("bind cancellation fixture");
        let cancel_address = cancel_listener
            .local_addr()
            .expect("cancellation fixture address");
        let cancellation_server = thread::spawn(move || {
            let (mut stream, _) = cancel_listener
                .accept()
                .expect("accept cancellation request");
            let _ = read_request(&mut stream);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1048576\r\nConnection: close\r\n\r\n",
            );
            let _ = stream.write_all(&vec![b'x'; 1024]);
            thread::sleep(Duration::from_millis(100));
            let _ = stream.write_all(&vec![b'x'; 1024 * 1024 - 1024]);
        });
        let cancelled = DocumentPlan::new(
            "cancel.md",
            BTreeMap::new(),
            vec![StepPlan::new(
                "cancelled",
                vec![
                    "curl".into(),
                    format!("http://127.0.0.1:{}/cancel", cancel_address.port()),
                ],
            )],
        );
        let callback_calls = Arc::new(AtomicUsize::new(0));
        let callback_calls_for_check = Arc::clone(&callback_calls);
        let cancel_after_start =
            move || callback_calls_for_check.fetch_add(1, Ordering::SeqCst) >= 1;
        let error = cancelled
            .execute_with_cancel(&policy, Some(&cancel_after_start))
            .expect_err("cancelled document must return an error");
        assert_eq!(error.code(), mdok_curl::E_CANCELLED);
        assert!(callback_calls.load(Ordering::SeqCst) >= 2);
        cancellation_server
            .join()
            .expect("join cancellation fixture");
    }
}
