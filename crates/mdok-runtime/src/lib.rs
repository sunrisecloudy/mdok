#![forbid(unsafe_code)]

//! Sequential document execution over the owned curl and JMESPath APIs.

use mdok_curl::{CurlError, CurlPlan, CurlPolicy, E_BODY_LIMIT, TransferResponse};
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
        let mut variables = self.variables.clone();
        let mut step_summaries = Map::new();
        let mut results = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let argv = resolve_argv(&step.argv, &variables)
                .map_err(|e| RuntimeError::diagnostic("MDOK-E505", e, &step.name, "", None))?;
            let plan =
                CurlPlan::parse(&argv, &policy.curl).map_err(|e| curl_error(e, &step.name))?;
            let response = plan
                .execute(&policy.curl)
                .map_err(|e| curl_error(e, &step.name))?;
            let context = response
                .evaluation_json_limited(
                    &json!(variables),
                    &Value::Object(step_summaries.clone()),
                    policy.max_json_body_bytes,
                )
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
                let summary = json!({ "status": response.status, "passed": false });
                step_summaries.insert(step.name.clone(), summary);
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
                    variables,
                    steps: results,
                });
            }
            let mut evaluated_captures = Vec::with_capacity(step.captures.len());
            for capture in &step.captures {
                let value = evaluate(&capture.compiled, &context).map_err(|e| {
                    RuntimeError::diagnostic(E_JMES_TYPE, e, &step.name, &capture.expression, None)
                })?;
                evaluated_captures.push((capture.expression.clone(), value));
            }
            let pending = collect_captures(
                &evaluated_captures,
                &variables,
                policy.allow_capture_override,
                &step.name,
            )?;
            for (key, value) in &pending {
                variables.insert(key.clone(), value.clone());
            }
            step_summaries.insert(
                step.name.clone(),
                json!({ "status": response.status, "passed": true }),
            );
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
            variables,
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
    evaluated_captures: &[(String, Value)],
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
        let captures = [("capture".to_owned(), json!({"bad-key": true}))];
        let error = collect_captures(&captures, &BTreeMap::new(), false, "one").unwrap_err();
        assert_eq!(error.code(), E_CAPTURE_COLLISION);
    }

    #[test]
    fn capture_override_applies_to_existing_and_pending_keys() {
        let variables = [("token".to_owned(), json!("original"))]
            .into_iter()
            .collect();
        let existing_collision = [("capture".to_owned(), json!({"token": "new"}))];
        let error = collect_captures(&existing_collision, &variables, false, "one").unwrap_err();
        assert_eq!(error.code(), E_CAPTURE_COLLISION);

        let duplicate_pending = vec![
            ("first".to_owned(), json!({"token": "first"})),
            ("second".to_owned(), json!({"token": "second"})),
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
}
