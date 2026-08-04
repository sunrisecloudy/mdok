#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub type ValueMap = BTreeMap<String, Value>;

pub const RESERVED_NAMES: [&str; 6] = [
    "variables",
    "steps",
    "environment",
    "request",
    "response",
    "mdok",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub path: PathBuf,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: u32,
    pub column: u32,
}

impl SourceSpan {
    pub fn new(
        path: impl Into<PathBuf>,
        byte_start: usize,
        byte_end: usize,
        line: u32,
        column: u32,
    ) -> Self {
        Self {
            path: path.into(),
            byte_start,
            byte_end,
            line,
            column,
        }
    }

    pub fn point(path: impl Into<PathBuf>, byte: usize, line: u32, column: u32) -> Self {
        Self::new(path, byte, byte, line, column)
    }

    pub fn is_empty(&self) -> bool {
        self.byte_start == self.byte_end
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepName(pub String);

impl StepName {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if !is_valid_step_name(&value) {
            return Err(CoreError::InvalidStepName(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for StepName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

pub fn is_valid_step_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        && !RESERVED_NAMES.contains(&value)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub step: Option<StepName>,
    pub hint: Option<String>,
    pub cause: Option<String>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            title: title.into(),
            message: message.into(),
            span: None,
            step: None,
            hint: None,
            cause: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }
    pub fn with_step(mut self, step: StepName) -> Self {
        self.step = Some(step);
        self
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("invalid step name: {0}")]
    InvalidStepName(String),
    #[error("duplicate step name: {0}")]
    DuplicateStep(String),
    #[error("unknown step reference: {0}")]
    UnknownStep(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageVersion(pub String);

impl Default for LanguageVersion {
    fn default() -> Self {
        Self("1".to_owned())
    }
}

impl LanguageVersion {
    pub fn v1() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurlSourcePlan {
    pub source: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckPlan {
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturePlan {
    pub expression: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPlan {
    pub name: StepName,
    pub heading_path: Vec<String>,
    pub curl: CurlSourcePlan,
    pub checks: Vec<CheckPlan>,
    pub captures: Vec<CapturePlan>,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentPlan {
    pub path: PathBuf,
    pub language_version: LanguageVersion,
    pub variables: ValueMap,
    pub steps: Vec<StepPlan>,
}

impl DocumentPlan {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            language_version: LanguageVersion::default(),
            variables: ValueMap::new(),
            steps: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        let mut names = std::collections::HashSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            if !names.insert(step.name.clone()) {
                return Err(CoreError::DuplicateStep(step.name.0.clone()));
            }
            for capture in &step.captures {
                if capture.expression.trim().is_empty() {
                    return Err(CoreError::UnknownStep(format!(
                        "empty capture at step {index}"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_step_name_contract() {
        assert!(StepName::new("login_1").is_ok());
        assert!(StepName::new("1-login").is_err());
        assert!(StepName::new("variables").is_err());
        assert!(StepName::new("a".repeat(65)).is_err());
    }

    #[test]
    fn detects_duplicate_plan_steps() {
        let name = StepName::new("one").unwrap();
        let span = SourceSpan::point("test.md", 0, 1, 1);
        let step = StepPlan {
            name: name.clone(),
            heading_path: Vec::new(),
            curl: CurlSourcePlan {
                source: "curl example.test".into(),
                span: span.clone(),
            },
            checks: Vec::new(),
            captures: Vec::new(),
            span: span.clone(),
        };
        let mut plan = DocumentPlan::new("test.md");
        plan.steps = vec![step.clone(), step];
        assert!(matches!(plan.validate(), Err(CoreError::DuplicateStep(value)) if value == "one"));
    }
}
