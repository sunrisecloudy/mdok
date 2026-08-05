#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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
    is_valid_identifier(value, Some(64)) && !RESERVED_NAMES.contains(&value)
}

/// Returns whether `value` can be used as a user-defined variable or capture
/// key in language version 1.
///
/// Capture keys share the identifier grammar with step names.  Keeping the
/// rule in the core crate means callers that construct plans directly cannot
/// bypass the Markdown planner's name checks.
pub fn is_valid_variable_name(value: &str) -> bool {
    is_valid_identifier(value, None) && !RESERVED_NAMES.contains(&value)
}

pub fn is_valid_capture_key(value: &str) -> bool {
    is_valid_variable_name(value)
}

fn is_valid_identifier(value: &str, max_len: Option<usize>) -> bool {
    !value.is_empty()
        && max_len.is_none_or(|limit| value.len() <= limit)
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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
    #[error("forward step reference: {0}")]
    ForwardReference(String),
    #[error("invalid capture key: {0}")]
    InvalidCaptureKey(String),
    #[error("duplicate capture key: {0}")]
    DuplicateCaptureKey(String),
    #[error("invalid capture expression: {0}")]
    InvalidCaptureExpression(String),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidStepName(_) | Self::DuplicateStep(_) => "MDOK-E101",
            Self::UnknownStep(_) | Self::ForwardReference(_) => "MDOK-E102",
            Self::InvalidCaptureKey(_) | Self::DuplicateCaptureKey(_) => "MDOK-E504",
            Self::InvalidCaptureExpression(_) => "MDOK-E500",
        }
    }
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
pub struct ExecSourcePlan {
    /// Raw command text from the Markdown fence.  Tokenization and execution
    /// policy belong to the command adapter, while the shared plan preserves
    /// the exact source and its location.
    pub source: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepSource {
    Curl(CurlSourcePlan),
    Exec(ExecSourcePlan),
}

impl StepSource {
    pub fn source(&self) -> &str {
        match self {
            Self::Curl(source) => &source.source,
            Self::Exec(source) => &source.source,
        }
    }

    pub fn span(&self) -> &SourceSpan {
        match self {
            Self::Curl(source) => &source.span,
            Self::Exec(source) => &source.span,
        }
    }
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

impl CapturePlan {
    /// Return keys that are statically visible in a top-level JMESPath
    /// object expression.  Expressions whose result type is only knowable at
    /// runtime return an empty list and are checked by the JMESPath runtime.
    pub fn capture_keys(&self) -> Result<Vec<String>, CoreError> {
        capture_keys_from_expression(&self.expression)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPlan {
    pub name: StepName,
    pub heading_path: Vec<String>,
    pub source: StepSource,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationOptions {
    /// Allow a later capture to replace an earlier key.  The default follows
    /// the PRD's immutable capture behavior.
    pub allow_capture_override: bool,
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
        self.validate_with_options(ValidationOptions::default())
    }

    /// Validate all invariants that can be established from the immutable
    /// public plan representation.
    pub fn validate_with_options(&self, options: ValidationOptions) -> Result<(), CoreError> {
        // Validate every name before resolving references.  `StepName` keeps
        // its tuple field public for backwards compatibility, so a caller
        // can construct an invalid value without going through `StepName::new`.
        let mut names = BTreeMap::new();
        for (index, step) in self.steps.iter().enumerate() {
            if !is_valid_step_name(step.name.as_str()) {
                return Err(CoreError::InvalidStepName(step.name.0.clone()));
            }
            if names.insert(step.name.clone(), index).is_some() {
                return Err(CoreError::DuplicateStep(step.name.0.clone()));
            }
        }

        let mut capture_keys = BTreeSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            for check in &step.checks {
                validate_reference_span(&step.name, &step.span, &check.span, "check")?;
                validate_step_references(&check.expression, index, &names)?;
            }
            for capture in &step.captures {
                validate_reference_span(&step.name, &step.span, &capture.span, "capture")?;
                let keys = capture.capture_keys()?;
                for key in keys {
                    if !options.allow_capture_override && !capture_keys.insert(key.clone()) {
                        return Err(CoreError::DuplicateCaptureKey(key));
                    }
                }
                validate_step_references(&capture.expression, index, &names)?;
            }
        }
        Ok(())
    }

    /// Validate a named reference from a request at `source_step`.
    ///
    /// Version 1 exposes only completed earlier request summaries to checks
    /// and captures, so a target at the same or a later index is a forward
    /// reference.  This helper gives callers that construct plans or metadata
    /// outside the Markdown planner the same unknown/forward distinction.
    pub fn validate_reference(&self, target: &str, source_step: usize) -> Result<(), CoreError> {
        if source_step >= self.steps.len() {
            return Err(CoreError::UnknownStep(format!(
                "source step index {source_step}"
            )));
        }
        if !is_valid_step_name(target) {
            return Err(CoreError::InvalidStepName(target.to_owned()));
        }
        let Some(target_index) = self
            .steps
            .iter()
            .position(|step| step.name.as_str() == target)
        else {
            return Err(CoreError::UnknownStep(target.to_owned()));
        };
        if target_index >= source_step {
            return Err(CoreError::ForwardReference(target.to_owned()));
        }
        Ok(())
    }
}

fn validate_reference_span(
    step: &StepName,
    request_span: &SourceSpan,
    reference_span: &SourceSpan,
    kind: &str,
) -> Result<(), CoreError> {
    if request_span.path == reference_span.path
        && reference_span.byte_start < request_span.byte_start
    {
        return Err(CoreError::ForwardReference(format!(
            "{kind} for step `{step}` appears before its request"
        )));
    }
    Ok(())
}

fn capture_keys_from_expression(expression: &str) -> Result<Vec<String>, CoreError> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Err(CoreError::InvalidCaptureExpression(
            "capture expression is empty".into(),
        ));
    }

    // A capture may be any JMESPath expression at this layer; the runtime
    // enforces that its result is an object.  When the expression is a
    // multi-select hash, however, its keys are statically available and can
    // be checked before execution.
    let Some(inner) = expression.strip_prefix('{') else {
        return Ok(Vec::new());
    };
    let Some(inner) = inner.strip_suffix('}') else {
        // The expression may be a larger JMESPath expression beginning with
        // an object literal (for example, `{id: body.id} | values(@)`).
        // Its result shape is runtime-defined, so leave key validation to the
        // JMESPath compiler/evaluator rather than misclassifying it here.
        return Ok(Vec::new());
    };

    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut keys = Vec::new();
    for field in split_top_level(inner, ',')? {
        let field = field.trim();
        if field.is_empty() {
            return Err(CoreError::InvalidCaptureExpression(
                "capture object contains an empty field".into(),
            ));
        }
        let Some(colon) = find_top_level(field, ':')? else {
            return Err(CoreError::InvalidCaptureExpression(format!(
                "capture object field `{field}` has no key separator"
            )));
        };
        let key = parse_capture_key(field[..colon].trim())
            .ok_or_else(|| CoreError::InvalidCaptureKey(field[..colon].trim().to_owned()))?;
        if !is_valid_capture_key(&key) {
            return Err(CoreError::InvalidCaptureKey(key));
        }
        if keys.iter().any(|existing| existing == &key) {
            return Err(CoreError::DuplicateCaptureKey(key));
        }
        keys.push(key);
    }
    Ok(keys)
}

fn parse_capture_key(source: &str) -> Option<String> {
    if source.is_empty() {
        return None;
    }
    let mut chars = source.chars();
    match (chars.next(), chars.next_back()) {
        (Some(quote @ ('"' | '\'')), Some(last)) if quote == last => {
            let contents = &source[quote.len_utf8()..source.len() - quote.len_utf8()];
            Some(unescape_capture_key(contents))
        }
        _ if source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) =>
        {
            Some(source.to_owned())
        }
        _ => None,
    }
}

fn unescape_capture_key(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut escaped = false;
    for character in source.chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        output.push('\\');
    }
    output
}

fn split_top_level(source: &str, delimiter: char) -> Result<Vec<&str>, CoreError> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut levels = [0usize; 3]; // square, paren, curly
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in source.char_indices() {
        if let Some(current) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' && current != '`' {
                escaped = true;
            } else if character == current {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '[' => levels[0] += 1,
            ']' => {
                if levels[0] == 0 {
                    return Err(CoreError::InvalidCaptureExpression(
                        "unbalanced `]` in capture expression".into(),
                    ));
                }
                levels[0] -= 1;
            }
            '(' => levels[1] += 1,
            ')' => {
                if levels[1] == 0 {
                    return Err(CoreError::InvalidCaptureExpression(
                        "unbalanced `)` in capture expression".into(),
                    ));
                }
                levels[1] -= 1;
            }
            '{' => levels[2] += 1,
            '}' => {
                if levels[2] == 0 {
                    return Err(CoreError::InvalidCaptureExpression(
                        "unbalanced `}` in capture expression".into(),
                    ));
                }
                levels[2] -= 1;
            }
            value if value == delimiter && levels == [0, 0, 0] => {
                fields.push(&source[start..index]);
                start = index + value.len_utf8();
            }
            _ => {}
        }
    }

    if quote.is_some() || levels != [0, 0, 0] {
        return Err(CoreError::InvalidCaptureExpression(
            "unbalanced delimiter in capture expression".into(),
        ));
    }
    fields.push(&source[start..]);
    Ok(fields)
}

fn find_top_level(source: &str, delimiter: char) -> Result<Option<usize>, CoreError> {
    let mut levels = [0usize; 3];
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in source.char_indices() {
        if let Some(current) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' && current != '`' {
                escaped = true;
            } else if character == current {
                quote = None;
            }
            continue;
        }
        if character == delimiter && levels == [0, 0, 0] {
            return Ok(Some(index));
        }
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '[' => levels[0] += 1,
            ']' if levels[0] > 0 => levels[0] -= 1,
            '(' => levels[1] += 1,
            ')' if levels[1] > 0 => levels[1] -= 1,
            '{' => levels[2] += 1,
            '}' if levels[2] > 0 => levels[2] -= 1,
            _ => {}
        }
    }

    if quote.is_some() || levels != [0, 0, 0] {
        return Err(CoreError::InvalidCaptureExpression(
            "unbalanced delimiter in capture expression".into(),
        ));
    }
    Ok(None)
}

fn validate_step_references(
    expression: &str,
    current_index: usize,
    names: &BTreeMap<StepName, usize>,
) -> Result<(), CoreError> {
    for reference in referenced_steps(expression) {
        let Some(reference_index) = names.get(&StepName(reference.clone())) else {
            return Err(CoreError::UnknownStep(reference));
        };
        if *reference_index >= current_index {
            return Err(CoreError::ForwardReference(reference));
        }
    }
    Ok(())
}

fn referenced_steps(expression: &str) -> BTreeSet<String> {
    let characters: Vec<char> = expression.chars().collect();
    let mut references = BTreeSet::new();
    let mut index = 0;
    while index < characters.len() {
        if matches!(characters[index], '\'' | '"' | '`') {
            skip_quoted(&characters, &mut index);
            continue;
        }
        if !is_identifier_start(characters[index]) {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < characters.len() && is_identifier_continue(characters[index]) {
            index += 1;
        }
        if characters[start..index].iter().collect::<String>() != "steps" {
            continue;
        }
        let mut previous = start;
        while previous > 0 && characters[previous - 1].is_ascii_whitespace() {
            previous -= 1;
        }
        let is_root_reference = previous == 0
            || (!is_identifier_continue(characters[previous - 1])
                && characters[previous - 1] != '.');
        if !is_root_reference {
            continue;
        }

        while index < characters.len() && characters[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= characters.len() {
            continue;
        }
        if matches!(characters[index], '.' | '[') {
            index += 1;
            while index < characters.len() && characters[index].is_ascii_whitespace() {
                index += 1;
            }
            if let Some(reference) = read_reference_segment(&characters, &mut index) {
                references.insert(reference);
            }
        }
    }
    references
}

fn read_reference_segment(characters: &[char], index: &mut usize) -> Option<String> {
    if *index >= characters.len() {
        return None;
    }
    if matches!(characters[*index], '\'' | '"') {
        let quote = characters[*index];
        *index += 1;
        let mut value = String::new();
        let mut escaped = false;
        while *index < characters.len() {
            let character = characters[*index];
            *index += 1;
            if escaped {
                value.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                return Some(value);
            } else {
                value.push(character);
            }
        }
        return None;
    }
    if !is_identifier_start(characters[*index]) {
        return None;
    }
    let start = *index;
    *index += 1;
    while *index < characters.len() && is_identifier_continue(characters[*index]) {
        *index += 1;
    }
    Some(characters[start..*index].iter().collect())
}

fn skip_quoted(characters: &[char], index: &mut usize) {
    let quote = characters[*index];
    *index += 1;
    let mut escaped = false;
    while *index < characters.len() {
        let character = characters[*index];
        *index += 1;
        if escaped {
            escaped = false;
        } else if character == '\\' && quote != '`' {
            escaped = true;
        } else if character == quote {
            break;
        }
    }
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> SourceSpan {
        SourceSpan::point("test.md", 0, 1, 1)
    }

    fn span_at(byte: usize) -> SourceSpan {
        SourceSpan::point("test.md", byte, 1, byte as u32 + 1)
    }

    fn step(name: &str) -> StepPlan {
        StepPlan {
            name: StepName(name.into()),
            heading_path: Vec::new(),
            source: StepSource::Curl(CurlSourcePlan {
                source: "curl example.test".into(),
                span: span(),
            }),
            checks: Vec::new(),
            captures: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn validates_step_name_contract() {
        assert!(StepName::new("login_1").is_ok());
        assert!(StepName::new("1-login").is_err());
        assert!(StepName::new("variables").is_err());
        assert!(StepName::new("a".repeat(65)).is_err());
    }

    #[test]
    fn step_source_keeps_curl_and_exec_typed() {
        let curl = StepSource::Curl(CurlSourcePlan {
            source: "curl https://example.test".into(),
            span: span(),
        });
        let exec = StepSource::Exec(ExecSourcePlan {
            source: "printf ok".into(),
            span: span_at(10),
        });

        assert!(matches!(&curl, StepSource::Curl(_)));
        assert!(matches!(&exec, StepSource::Exec(_)));
        assert_eq!(curl.source(), "curl https://example.test");
        assert_eq!(exec.source(), "printf ok");
        assert_eq!(exec.span().byte_start, 10);
    }

    #[test]
    fn detects_duplicate_plan_steps() {
        let step = step("one");
        let mut plan = DocumentPlan::new("test.md");
        plan.steps = vec![step.clone(), step];
        assert!(matches!(plan.validate(), Err(CoreError::DuplicateStep(value)) if value == "one"));
    }

    #[test]
    fn validates_names_on_deserialized_or_directly_constructed_plans() {
        let mut plan = DocumentPlan::new("test.md");
        plan.steps.push(step("steps"));
        assert!(matches!(
            plan.validate(),
            Err(CoreError::InvalidStepName(value)) if value == "steps"
        ));

        plan.steps[0].name = StepName("has space".into());
        assert!(matches!(
            plan.validate(),
            Err(CoreError::InvalidStepName(value)) if value == "has space"
        ));
    }

    #[test]
    fn rejects_invalid_and_reserved_capture_keys() {
        let mut plan = DocumentPlan::new("test.md");
        let mut source = step("source");
        source.captures.push(CapturePlan {
            expression: "{variables: body.id}".into(),
            span: span(),
        });
        plan.steps.push(source);
        assert!(matches!(
            plan.validate(),
            Err(CoreError::InvalidCaptureKey(value)) if value == "variables"
        ));

        plan.steps[0].captures[0].expression = "{1id: body.id}".into();
        assert!(matches!(
            plan.validate(),
            Err(CoreError::InvalidCaptureKey(value)) if value == "1id"
        ));
    }

    #[test]
    fn rejects_duplicate_capture_keys_across_captures() {
        let mut plan = DocumentPlan::new("test.md");
        let mut source = step("source");
        source.captures = vec![
            CapturePlan {
                expression: "{id: body.id}".into(),
                span: span(),
            },
            CapturePlan {
                expression: "{id: body.other}".into(),
                span: span(),
            },
        ];
        plan.steps.push(source);
        assert!(matches!(
            plan.validate(),
            Err(CoreError::DuplicateCaptureKey(value)) if value == "id"
        ));

        assert_eq!(
            plan.validate_with_options(ValidationOptions {
                allow_capture_override: true,
            }),
            Ok(())
        );
    }

    #[test]
    fn rejects_unknown_and_forward_step_references() {
        let mut plan = DocumentPlan::new("test.md");
        let mut first = step("first");
        first.checks.push(CheckPlan {
            expression: "steps.second.status == `200`".into(),
            span: span(),
        });
        let second = step("second");
        plan.steps = vec![first, second];
        assert!(matches!(
            plan.validate(),
            Err(CoreError::ForwardReference(value)) if value == "second"
        ));

        plan.steps[0].checks[0].expression = "steps.missing.status == `200`".into();
        assert!(matches!(
            plan.validate(),
            Err(CoreError::UnknownStep(value)) if value == "missing"
        ));
    }

    #[test]
    fn accepts_references_to_completed_steps_and_nested_capture_values() {
        let mut first = step("first");
        first.captures.push(CapturePlan {
            expression: "{id: body.id, nested: {value: body.value}}".into(),
            span: span(),
        });
        let mut second = step("second");
        second.checks.push(CheckPlan {
            expression: "steps.first.status == `200`".into(),
            span: span(),
        });
        second.checks.push(CheckPlan {
            expression: "body.steps.first == `200`".into(),
            span: span(),
        });
        second.captures.push(CapturePlan {
            expression: "{seen: steps.first.status}".into(),
            span: span(),
        });
        let mut plan = DocumentPlan::new("test.md");
        plan.steps = vec![first, second];
        assert_eq!(plan.validate(), Ok(()));
    }

    #[test]
    fn validates_named_reference_and_source_order() {
        let mut first = step("first");
        first.span = span_at(10);
        let mut second = step("second");
        second.span = span_at(30);
        let mut plan = DocumentPlan::new("test.md");
        plan.steps = vec![first, second];

        assert_eq!(plan.validate_reference("first", 1), Ok(()));
        assert!(matches!(
            plan.validate_reference("second", 1),
            Err(CoreError::ForwardReference(value)) if value == "second"
        ));
        assert!(matches!(
            plan.validate_reference("missing", 1),
            Err(CoreError::UnknownStep(value)) if value == "missing"
        ));
        assert!(matches!(
            plan.validate_reference("1bad", 1),
            Err(CoreError::InvalidStepName(value)) if value == "1bad"
        ));

        let mut out_of_order = step("ordered");
        out_of_order.span = span_at(20);
        out_of_order.checks.push(CheckPlan {
            expression: "status == `200`".into(),
            span: span_at(10),
        });
        let mut out_of_order_plan = DocumentPlan::new("test.md");
        out_of_order_plan.steps.push(out_of_order);
        assert!(matches!(
            out_of_order_plan.validate(),
            Err(CoreError::ForwardReference(message))
                if message.contains("check") && message.contains("ordered")
        ));
    }

    #[test]
    fn capture_key_validation_uses_identifier_rules() {
        assert!(is_valid_capture_key("a".repeat(65).as_str()));
        assert!(!is_valid_capture_key("1id"));
        assert!(!is_valid_capture_key("response"));

        let capture = CapturePlan {
            expression: "{id: body.id, nested: {value: body.value}}".into(),
            span: span(),
        };
        assert_eq!(capture.capture_keys().unwrap(), ["id", "nested"]);
        assert!(
            CapturePlan {
                expression: "{}".into(),
                span: span(),
            }
            .capture_keys()
            .unwrap()
            .is_empty()
        );

        assert!(matches!(
            CapturePlan {
                expression: "{id: body.id, id: body.other}".into(),
                span: span(),
            }
            .capture_keys(),
            Err(CoreError::DuplicateCaptureKey(value)) if value == "id"
        ));
    }
}
