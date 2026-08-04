//! Stable report and output primitives for the MDOK command line interface.
//!
//! The runtime owns execution state; this crate owns the wire format and all
//! user-facing renderers. Keeping the redactor here makes it difficult for a
//! new output path to accidentally print a secret.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "1";
pub const MDOK_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CURL_COMPAT_VERSION: &str = "8.21";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Passed,
    Failed,
    Skipped,
    Planned,
    Error,
}

impl Status {
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failed | Self::Error)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Planned => "planned",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Summary {
    pub documents: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub planned: usize,
    pub errors: usize,
    pub checks: usize,
    pub checks_failed: usize,
}

impl Summary {
    pub fn absorb(&mut self, status: Status) {
        match status {
            Status::Passed => self.passed += 1,
            Status::Failed => self.failed += 1,
            Status::Skipped => self.skipped += 1,
            Status::Planned => self.planned += 1,
            Status::Error => self.errors += 1,
        }
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.errors > 0 || self.checks_failed > 0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub kind: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub file: Option<String>,
    pub step: Option<String>,
    pub span: Option<Span>,
    pub observed: Option<Value>,
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redactions: Vec<String>,
    /// Optional machine-readable execution context. These fields are
    /// additive so reports produced without context retain their existing
    /// wire representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cause_chain: Vec<DiagnosticCause>,
}

/// A redacted, machine-readable cause in a diagnostic's causal chain.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticCause {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<Value>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            kind: "diagnostic".to_string(),
            severity: Severity::Error,
            title: title.into(),
            message: message.into(),
            file: None,
            step: None,
            span: None,
            observed: None,
            hint: None,
            redactions: Vec::new(),
            run_id: None,
            document_ordinal: None,
            step_ordinal: None,
            check_ordinal: None,
            capture_ordinal: None,
            expression: None,
            result: None,
            cause_chain: Vec::new(),
        }
    }

    pub fn at_file(mut self, path: &Path) -> Self {
        self.file = Some(path.display().to_string());
        self
    }

    pub fn at_step(mut self, step: impl Into<String>) -> Self {
        self.step = Some(step.into());
        self
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_ordinals(
        mut self,
        document: Option<usize>,
        step: Option<usize>,
        check: Option<usize>,
        capture: Option<usize>,
    ) -> Self {
        self.document_ordinal = document;
        self.step_ordinal = step;
        self.check_ordinal = check;
        self.capture_ordinal = capture;
        self
    }

    pub fn with_expression(mut self, expression: impl Into<String>) -> Self {
        self.expression = Some(expression.into());
        self
    }

    pub fn with_result(mut self, result: Value) -> Self {
        self.result = Some(result);
        self
    }

    pub fn caused_by(mut self, cause: DiagnosticCause) -> Self {
        self.cause_chain.push(cause);
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CheckReport {
    pub expression: String,
    pub status: Status,
    pub result: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StepReport {
    pub name: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<CheckReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captures: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DocumentReport {
    pub path: String,
    pub status: Status,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Event {
    pub sequence: u64,
    pub kind: String,
    pub document: Option<String>,
    pub step: Option<String>,
    pub status: Option<Status>,
    pub message: Option<String>,
}

/// Optional structured context for an event.
///
/// `Event` remains source-compatible with the existing CLI's struct literals;
/// callers that have richer execution context attach it through `Report`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct EventMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_ordinal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl EventMetadata {
    pub fn is_empty(&self) -> bool {
        self.run_id.is_none()
            && self.document_ordinal.is_none()
            && self.step_ordinal.is_none()
            && self.check_ordinal.is_none()
            && self.capture_ordinal.is_none()
            && self.timestamp.is_none()
            && self.duration_ms.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EventMetadataRecord {
    pub sequence: u64,
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

/// The JSON-lines representation of an event with optional PRD metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EventRecord {
    #[serde(flatten)]
    pub event: Event,
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Report {
    pub schema_version: String,
    pub mdok_version: String,
    pub curl_version: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub summary: Summary,
    pub documents: Vec<DocumentReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_metadata: Vec<EventMetadataRecord>,
}

impl Report {
    pub fn new(started_at: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            mdok_version: MDOK_VERSION.to_string(),
            curl_version: CURL_COMPAT_VERSION.to_string(),
            started_at: started_at.into(),
            duration_ms: 0,
            summary: Summary::default(),
            documents: Vec::new(),
            diagnostics: Vec::new(),
            events: Vec::new(),
            event_metadata: Vec::new(),
        }
    }

    pub fn now() -> Self {
        Self::new(timestamp())
    }

    pub fn add_document(&mut self, document: DocumentReport) {
        self.summary.documents += 1;
        self.summary.absorb(document.status);
        for step in &document.steps {
            self.summary.checks += step.checks.len();
            self.summary.checks_failed += step
                .checks
                .iter()
                .filter(|check| check.status.is_failure())
                .count();
        }
        self.documents.push(document);
    }

    /// Append an event and, when supplied, its optional structured context.
    /// Existing callers may continue to push directly to `events`.
    pub fn push_event(&mut self, event: Event, metadata: Option<EventMetadata>) {
        if let Some(metadata) = metadata {
            self.set_event_metadata(event.sequence, metadata);
        }
        self.events.push(event);
    }

    pub fn set_event_metadata(&mut self, sequence: u64, metadata: EventMetadata) {
        if metadata.is_empty() {
            self.event_metadata
                .retain(|record| record.sequence != sequence);
            return;
        }
        if let Some(record) = self
            .event_metadata
            .iter_mut()
            .find(|record| record.sequence == sequence)
        {
            record.metadata = metadata;
        } else {
            self.event_metadata
                .push(EventMetadataRecord { sequence, metadata });
        }
    }

    /// Return events in wire order with optional metadata flattened into each
    /// record. With no metadata this serializes identically to `Event`.
    pub fn event_records(&self) -> Vec<EventRecord> {
        self.events
            .iter()
            .map(|event| EventRecord {
                event: event.clone(),
                metadata: self
                    .event_metadata
                    .iter()
                    .find(|record| record.sequence == event.sequence)
                    .map(|record| record.metadata.clone())
                    .unwrap_or_default(),
            })
            .collect()
    }

    pub fn json(&self) -> Result<String, ReportError> {
        serde_json::to_string_pretty(self).map_err(ReportError::Serialize)
    }

    pub fn human(&self, color: bool, verbose: bool) -> String {
        let mut output = String::new();
        for document in &self.documents {
            let label = if color {
                match document.status {
                    Status::Passed => format!("\x1b[32m{}\x1b[0m", document.status.as_str()),
                    Status::Failed | Status::Error => {
                        format!("\x1b[31m{}\x1b[0m", document.status.as_str())
                    }
                    _ => document.status.as_str().to_string(),
                }
            } else {
                document.status.as_str().to_string()
            };
            let _ = writeln!(output, "{}  {}", label, document.path);
            for step in &document.steps {
                let _ = writeln!(output, "  {}  {}", step.status.as_str(), step.name);
                if verbose && !step.command.is_empty() {
                    let _ = writeln!(output, "    $ {}", step.command.join(" "));
                }
                for check in &step.checks {
                    let _ = writeln!(
                        output,
                        "    {}  {}",
                        check.status.as_str(),
                        check.expression
                    );
                }
            }
            for diagnostic in &document.diagnostics {
                write_diagnostic(&mut output, diagnostic);
            }
        }
        for diagnostic in &self.diagnostics {
            write_diagnostic(&mut output, diagnostic);
        }
        let _ = writeln!(
            output,
            "\n{} document(s): {} passed, {} failed, {} skipped, {} check(s) ({} failed)",
            self.summary.documents,
            self.summary.passed,
            self.summary.failed + self.summary.errors,
            self.summary.skipped + self.summary.planned,
            self.summary.checks,
            self.summary.checks_failed
        );
        output
    }

    pub fn junit(&self) -> String {
        let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>");
        for document in &self.documents {
            // Build the cases first, then derive both attributes from the
            // cases that are actually emitted. This keeps the JUnit counts
            // correct when a step has several checks or a document has more
            // than one diagnostic.
            let cases = junit_cases(document);
            let tests = cases.len();
            let failures = cases.iter().filter(|case| case.failure.is_some()).count();
            let _ = write!(
                output,
                "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{}\">",
                xml_escape(&document.path),
                tests,
                failures,
                document.duration_ms as f64 / 1000.0
            );
            for case in cases {
                let _ = write!(
                    output,
                    "<testcase name=\"{}\" time=\"{}\">",
                    xml_escape(&case.name),
                    case.duration_ms as f64 / 1000.0
                );
                if let Some(message) = case.failure {
                    let _ = write!(output, "<failure message=\"{}\"/>", xml_escape(&message));
                } else if case.skipped {
                    output.push_str("<skipped/>");
                }
                output.push_str("</testcase>");
            }
            output.push_str("</testsuite>");
        }
        output.push_str("</testsuites>\n");
        output
    }

    pub fn json_lines(&self) -> Result<String, ReportError> {
        let mut output = String::new();
        for event in self.event_records() {
            output.push_str(&serde_json::to_string(&event).map_err(ReportError::Serialize)?);
            output.push('\n');
        }
        Ok(output)
    }
}

pub fn timestamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("unix-ms:{millis}")
}

#[derive(Clone, Debug, Default)]
pub struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    pub fn new<I, S>(secrets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut values: Vec<String> = secrets
            .into_iter()
            .map(Into::into)
            .filter(|value| !value.is_empty())
            .collect();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values.dedup();
        Self { secrets: values }
    }

    pub fn redact_text(&self, input: &str) -> String {
        self.secrets.iter().fold(input.to_string(), |text, secret| {
            text.replace(secret, "[REDACTED]")
        })
    }

    pub fn redact_value(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.redact_text(text)),
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| self.redact_value(value))
                    .collect(),
            ),
            Value::Object(object) => {
                let mut redacted = Map::new();
                for (key, value) in object {
                    if is_sensitive_key(key) {
                        redacted.insert(key.clone(), Value::String("[REDACTED]".to_string()));
                    } else {
                        redacted.insert(key.clone(), self.redact_value(value));
                    }
                }
                Value::Object(redacted)
            }
            _ => value.clone(),
        }
    }

    pub fn redact_report(&self, report: &Report) -> Result<Report, ReportError> {
        let value = serde_json::to_value(report).map_err(ReportError::Serialize)?;
        serde_json::from_value(self.redact_value(&value)).map_err(ReportError::Serialize)
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    normalized.contains("token")
        || normalized.contains("password")
        || normalized.contains("secret")
        || normalized == "authorization"
        || normalized == "cookie"
        || normalized == "setcookie"
}

fn write_diagnostic(output: &mut String, diagnostic: &Diagnostic) {
    let location = diagnostic
        .file
        .as_deref()
        .map(|file| match &diagnostic.span {
            Some(span) => format!("{}:{}:{}", file, span.line, span.column),
            None => file.to_string(),
        })
        .unwrap_or_else(|| "<mdok>".to_string());
    let _ = writeln!(
        output,
        "  {}[{}]: {}\n    {}\n    {}",
        match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        },
        diagnostic.code,
        diagnostic.title,
        location,
        diagnostic.message
    );
    if let Some(hint) = &diagnostic.hint {
        let _ = writeln!(output, "    hint: {}", hint);
    }
}

#[derive(Debug)]
struct JunitCase {
    name: String,
    duration_ms: u64,
    failure: Option<String>,
    skipped: bool,
}

fn junit_cases(document: &DocumentReport) -> Vec<JunitCase> {
    let mut cases = Vec::new();
    for step in &document.steps {
        let failure = step_has_execution_failure(step).then(|| step_failure_message(step));
        cases.push(JunitCase {
            name: step.name.clone(),
            duration_ms: step.duration_ms,
            skipped: failure.is_none() && matches!(step.status, Status::Skipped | Status::Planned),
            failure,
        });

        let mut failed_check_number = 0;
        for (index, check) in step.checks.iter().enumerate() {
            let failure = if check.status.is_failure() {
                let message = check_failure_message(step, index, failed_check_number, check);
                failed_check_number += 1;
                Some(message)
            } else {
                None
            };
            cases.push(JunitCase {
                name: format!("{} :: check {}", step.name, index + 1),
                duration_ms: 0,
                skipped: failure.is_none()
                    && matches!(check.status, Status::Skipped | Status::Planned),
                failure,
            });
        }
    }

    for (index, diagnostic) in document
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .enumerate()
    {
        cases.push(JunitCase {
            name: format!("document diagnostic {}", index + 1),
            duration_ms: 0,
            failure: Some(diagnostic.message.clone()),
            skipped: false,
        });
    }

    if document.status.is_failure() && !cases.iter().any(|case| case.failure.is_some()) {
        cases.push(JunitCase {
            name: "document".to_string(),
            duration_ms: 0,
            failure: Some("document failed".to_string()),
            skipped: false,
        });
    }
    cases
}

fn step_failure_message(step: &StepReport) -> String {
    step.diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "step failed".to_string())
}

fn check_failure_message(
    step: &StepReport,
    check_index: usize,
    failed_check_number: usize,
    check: &CheckReport,
) -> String {
    let diagnostic = step.diagnostics.iter().find(|diagnostic| {
        diagnostic.severity == Severity::Error
            && (diagnostic.expression.as_deref() == Some(check.expression.as_str())
                || diagnostic.check_ordinal == Some(check_index))
    });
    let diagnostic = diagnostic.or_else(|| {
        step.diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == Severity::Error && diagnostic.code == "MDOK-E502"
            })
            .nth(failed_check_number)
    });
    diagnostic
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| format!("check failed: {}", check.expression))
}

fn step_has_execution_failure(step: &StepReport) -> bool {
    let has_failed_check = step.checks.iter().any(|check| check.status.is_failure());
    let has_non_assertion_error = step
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error && diagnostic.code != "MDOK-E502");
    has_non_assertion_error || (step.status.is_failure() && !has_failed_check)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("could not serialize report: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not write report {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

/// Serialize JSON and replace the destination with a fully-written file.
/// The temporary file lives beside the destination, so rename is atomic on
/// the filesystems supported by the CLI and a failed write leaves old output.
pub fn write_atomic_json(path: &Path, report: &Report) -> Result<(), ReportError> {
    let payload = report.json()?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| ReportError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("report.json");
    let temp = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        unique_suffix()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|source| ReportError::Io {
                path: temp.clone(),
                source,
            })?;
        file.write_all(payload.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|source| ReportError::Io {
                path: temp.clone(),
                source,
            })?;
        fs::rename(&temp, path).map_err(|source| ReportError::Io {
            path: path.to_path_buf(),
            source,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_values_and_sensitive_keys() {
        let redactor = Redactor::new(["super-secret"]);
        let value = serde_json::json!({
            "message": "super-secret appeared",
            "Authorization": "Bearer visible",
            "nested": ["super-secret"]
        });
        assert_eq!(
            redactor.redact_value(&value),
            serde_json::json!({
                "message": "[REDACTED] appeared",
                "Authorization": "[REDACTED]",
                "nested": ["[REDACTED]"]
            })
        );
    }

    #[test]
    fn atomic_write_replaces_complete_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("report.json");
        let report = Report::now();
        write_atomic_json(&path, &report).expect("write report");
        let parsed: Report = serde_json::from_str(&fs::read_to_string(path).expect("read report"))
            .expect("parse report");
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn junit_counts_each_failed_check_and_execution_separately() {
        let assertion_diagnostic =
            || Diagnostic::error("MDOK-E502", "Check failed", "assertion was false");
        let report = Report {
            schema_version: SCHEMA_VERSION.to_string(),
            mdok_version: MDOK_VERSION.to_string(),
            curl_version: CURL_COMPAT_VERSION.to_string(),
            started_at: "unix-ms:1".to_string(),
            duration_ms: 12,
            summary: Summary::default(),
            documents: vec![DocumentReport {
                path: "workflow.md".to_string(),
                status: Status::Failed,
                duration_ms: 12,
                steps: vec![
                    StepReport {
                        name: "login".to_string(),
                        status: Status::Failed,
                        command: Vec::new(),
                        checks: vec![
                            CheckReport {
                                expression: "status == `200`".to_string(),
                                status: Status::Failed,
                                result: Some(Value::Bool(false)),
                            },
                            CheckReport {
                                expression: "body.ok".to_string(),
                                status: Status::Failed,
                                result: Some(Value::Bool(false)),
                            },
                            CheckReport {
                                expression: "status == `500`".to_string(),
                                status: Status::Passed,
                                result: Some(Value::Bool(true)),
                            },
                        ],
                        captures: Vec::new(),
                        diagnostics: vec![assertion_diagnostic(), assertion_diagnostic()],
                        duration_ms: 4,
                    },
                    StepReport {
                        name: "request".to_string(),
                        status: Status::Error,
                        command: Vec::new(),
                        checks: Vec::new(),
                        captures: Vec::new(),
                        diagnostics: vec![Diagnostic::error(
                            "MDOK-E600",
                            "Transfer failed",
                            "connection failed",
                        )],
                        duration_ms: 8,
                    },
                ],
                diagnostics: Vec::new(),
            }],
            diagnostics: Vec::new(),
            events: Vec::new(),
            event_metadata: Vec::new(),
        };

        let junit = report.junit();
        assert!(junit.contains("tests=\"5\" failures=\"3\""));
        assert_eq!(junit.matches("<testcase ").count(), 5);
        assert_eq!(junit.matches("<failure ").count(), 3);
        assert!(junit.contains("login :: check 1"));
        assert!(junit.contains("login :: check 2"));
        assert!(junit.contains("connection failed"));
    }

    #[test]
    fn junit_represents_document_errors_without_steps() {
        let report = Report {
            schema_version: SCHEMA_VERSION.to_string(),
            mdok_version: MDOK_VERSION.to_string(),
            curl_version: CURL_COMPAT_VERSION.to_string(),
            started_at: "unix-ms:1".to_string(),
            duration_ms: 0,
            summary: Summary::default(),
            documents: vec![DocumentReport {
                path: "invalid.md".to_string(),
                status: Status::Error,
                duration_ms: 0,
                steps: Vec::new(),
                diagnostics: vec![Diagnostic::error(
                    "MDOK-E100",
                    "Invalid document",
                    "metadata is malformed",
                )],
            }],
            diagnostics: Vec::new(),
            events: Vec::new(),
            event_metadata: Vec::new(),
        };

        let junit = report.junit();
        assert!(junit.contains("tests=\"1\" failures=\"1\""));
        assert!(junit.contains("document diagnostic 1"));
        assert!(junit.contains("metadata is malformed"));
    }

    #[test]
    fn metadata_is_additive_and_flattened_for_event_lines() {
        let base = Diagnostic::error("MDOK-E502", "Check failed", "assertion was false");
        let base_json = serde_json::to_value(&base).expect("serialize base diagnostic");
        assert!(base_json.get("run_id").is_none());
        assert!(base_json.get("expression").is_none());
        assert!(base_json.get("cause_chain").is_none());

        let empty_report_json =
            serde_json::to_value(Report::new("unix-ms:1")).expect("serialize empty report");
        assert!(empty_report_json.get("event_metadata").is_none());

        let enriched = base
            .with_run_id("run-1")
            .with_ordinals(Some(2), Some(1), Some(0), None)
            .with_expression("status == `200`")
            .with_result(serde_json::json!({"status": 401}))
            .caused_by(DiagnosticCause {
                code: Some("MDOK-E600".to_string()),
                kind: Some("transport".to_string()),
                message: "request failed".to_string(),
                observed: None,
            });
        let enriched_json = serde_json::to_value(&enriched).expect("serialize enriched diagnostic");
        assert_eq!(enriched_json["run_id"], "run-1");
        assert_eq!(enriched_json["document_ordinal"], 2);
        assert_eq!(enriched_json["check_ordinal"], 0);
        assert_eq!(enriched_json["expression"], "status == `200`");
        assert_eq!(enriched_json["cause_chain"][0]["code"], "MDOK-E600");

        let mut report = Report::new("unix-ms:1");
        report.push_event(
            Event {
                sequence: 7,
                kind: "check.finished".to_string(),
                document: Some("workflow.md".to_string()),
                step: Some("login".to_string()),
                status: Some(Status::Failed),
                message: None,
            },
            Some(EventMetadata {
                run_id: Some("run-1".to_string()),
                document_ordinal: Some(2),
                step_ordinal: Some(1),
                check_ordinal: Some(0),
                capture_ordinal: None,
                timestamp: Some("unix-ms:2".to_string()),
                duration_ms: Some(3),
            }),
        );

        let event_lines = report.json_lines().expect("serialize event line");
        let line = event_lines.lines().next().expect("event line");
        let event_json: Value = serde_json::from_str(line).expect("parse event line");
        assert_eq!(event_json["sequence"], 7);
        assert_eq!(event_json["run_id"], "run-1");
        assert_eq!(event_json["document_ordinal"], 2);
        assert_eq!(event_json["duration_ms"], 3);

        let report_json: Value =
            serde_json::from_str(&report.json().expect("serialize report")).expect("parse report");
        assert_eq!(report_json["event_metadata"][0]["sequence"], 7);
        assert_eq!(report_json["event_metadata"][0]["run_id"], "run-1");
    }

    #[test]
    fn empty_event_metadata_keeps_the_legacy_wire_shape() {
        let event = Event {
            sequence: 1,
            kind: "document.finished".to_string(),
            document: Some("workflow.md".to_string()),
            step: None,
            status: Some(Status::Passed),
            message: None,
        };
        let mut report = Report::new("unix-ms:1");
        report.push_event(event.clone(), Some(EventMetadata::default()));

        assert!(report.event_metadata.is_empty());
        let legacy_line = serde_json::to_string(&event).expect("serialize legacy event");
        assert_eq!(
            report.json_lines().expect("serialize event lines").trim(),
            legacy_line
        );
        let report_json: Value =
            serde_json::from_str(&report.json().expect("serialize report")).expect("parse report");
        assert!(report_json.get("event_metadata").is_none());
    }

    #[test]
    fn redactor_covers_structured_diagnostic_context() {
        let report = Report {
            schema_version: SCHEMA_VERSION.to_string(),
            mdok_version: MDOK_VERSION.to_string(),
            curl_version: CURL_COMPAT_VERSION.to_string(),
            started_at: "unix-ms:1".to_string(),
            duration_ms: 0,
            summary: Summary::default(),
            documents: Vec::new(),
            diagnostics: vec![
                Diagnostic::error("MDOK-E502", "Check failed", "assertion failed")
                    .with_expression("body.secret == `super-secret`")
                    .with_result(serde_json::json!({"token": "super-secret"}))
                    .caused_by(DiagnosticCause {
                        code: Some("MDOK-E600".to_string()),
                        kind: Some("transport".to_string()),
                        message: "secret was observed".to_string(),
                        observed: Some(Value::String("super-secret".to_string())),
                    }),
            ],
            events: Vec::new(),
            event_metadata: Vec::new(),
        };

        let redacted = Redactor::new(["super-secret"])
            .redact_report(&report)
            .expect("redact report");
        let diagnostic = &redacted.diagnostics[0];
        assert_eq!(
            diagnostic.expression.as_deref(),
            Some("body.secret == `[REDACTED]`")
        );
        assert_eq!(
            diagnostic.result,
            Some(serde_json::json!({"token": "[REDACTED]"}))
        );
        assert_eq!(
            diagnostic.cause_chain[0].observed,
            Some(Value::String("[REDACTED]".to_string()))
        );
    }
}
