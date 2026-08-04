#![forbid(unsafe_code)]

use mdok_core::ValueMap;
use serde_json::Value;

#[derive(Clone)]
pub struct CompiledExpression {
    source: String,
    expression: jmespath::Expression<'static>,
}

impl std::fmt::Debug for CompiledExpression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("CompiledExpression")
            .field(&self.source)
            .finish()
    }
}

impl CompiledExpression {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn evaluate(&self, value: &Value) -> Result<Value, JmespathError> {
        let result = self
            .expression
            .search(value)
            .map_err(|error| JmespathError::Runtime(error.to_string()))?;
        serde_json::to_value(result.as_ref())
            .map_err(|error| JmespathError::Runtime(error.to_string()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JmespathError {
    #[error("MDOK-E500 invalid JMESPath syntax: {0}")]
    Syntax(String),
    #[error("MDOK-E501 JMESPath runtime or result type error: {0}")]
    Runtime(String),
    #[error("MDOK-E502 JMESPath check evaluated to false")]
    CheckFailed,
    #[error("MDOK-E503 capture did not evaluate to an object")]
    CaptureNotObject,
    #[error("MDOK-E504 invalid or colliding capture key: {0}")]
    InvalidCaptureKey(String),
}

impl JmespathError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "MDOK-E500",
            Self::Runtime(_) => "MDOK-E501",
            Self::CheckFailed => "MDOK-E502",
            Self::CaptureNotObject => "MDOK-E503",
            Self::InvalidCaptureKey(_) => "MDOK-E504",
        }
    }
}

pub fn compile(source: &str) -> Result<CompiledExpression, JmespathError> {
    if source.trim().is_empty() {
        return Err(JmespathError::Syntax("expression is empty".into()));
    }
    let expression =
        jmespath::compile(source).map_err(|error| JmespathError::Syntax(error.to_string()))?;
    Ok(CompiledExpression {
        source: source.to_owned(),
        expression,
    })
}

pub fn compile_expression(source: &str) -> Result<CompiledExpression, JmespathError> {
    compile(source)
}

pub fn evaluate_expression(source: &str, value: &Value) -> Result<Value, JmespathError> {
    evaluate(source, value)
}

pub fn evaluate(source: &str, value: &Value) -> Result<Value, JmespathError> {
    compile(source)?.evaluate(value)
}

pub fn evaluate_boolean(
    expression: &CompiledExpression,
    value: &Value,
) -> Result<(), JmespathError> {
    match expression.evaluate(value)? {
        Value::Bool(true) => Ok(()),
        Value::Bool(false) => Err(JmespathError::CheckFailed),
        other => Err(JmespathError::Runtime(format!(
            "check must return boolean, got {}",
            json_type(&other)
        ))),
    }
}

pub fn evaluate_check(source: &str, value: &Value) -> Result<(), JmespathError> {
    let expression = compile(source)?;
    evaluate_boolean(&expression, value)
}

pub fn evaluate_capture(
    expression: &CompiledExpression,
    value: &Value,
) -> Result<ValueMap, JmespathError> {
    let result = expression.evaluate(value)?;
    let Value::Object(object) = result else {
        return Err(JmespathError::CaptureNotObject);
    };
    let mut captures = ValueMap::new();
    for (key, value) in object {
        if !is_capture_key(&key) {
            return Err(JmespathError::InvalidCaptureKey(key));
        }
        if captures.insert(key.clone(), value).is_some() {
            return Err(JmespathError::InvalidCaptureKey(key));
        }
    }
    Ok(captures)
}

pub fn evaluate_capture_source(source: &str, value: &Value) -> Result<ValueMap, JmespathError> {
    evaluate_capture(&compile(source)?, value)
}

pub fn merge_captures(target: &mut ValueMap, captures: ValueMap) -> Result<(), JmespathError> {
    if captures.keys().any(|key| target.contains_key(key)) {
        return Err(JmespathError::InvalidCaptureKey(
            "capture key collision".into(),
        ));
    }
    target.extend(captures);
    Ok(())
}

pub fn is_capture_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compiles_and_evaluates_standard_jmespath() {
        let expression = compile("body.items[].id").unwrap();
        assert_eq!(
            expression
                .evaluate(&json!({"body": {"items": [{"id": 1}, {"id": 2}]}}))
                .unwrap(),
            json!([1, 2])
        );
    }

    #[test]
    fn checks_are_strictly_boolean_and_captures_are_objects() {
        let value = json!({"status": 200, "body": {"id": "u1"}});
        assert!(evaluate_check("status == `200`", &value).is_ok());
        assert_eq!(
            evaluate_check("status", &value).unwrap_err().code(),
            "MDOK-E501"
        );
        assert_eq!(
            evaluate_capture_source("{id: body.id}", &value).unwrap()["id"],
            "u1"
        );
        assert_eq!(
            evaluate_capture_source("status", &value)
                .unwrap_err()
                .code(),
            "MDOK-E503"
        );
    }
}
