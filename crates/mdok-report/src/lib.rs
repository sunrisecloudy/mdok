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
            let failures = usize::from(document.status.is_failure());
            let _ = write!(
                output,
                "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{}\">",
                xml_escape(&document.path),
                document.steps.len(),
                failures,
                document.duration_ms as f64 / 1000.0
            );
            for step in &document.steps {
                let _ = write!(
                    output,
                    "<testcase name=\"{}\" time=\"{}\">",
                    xml_escape(&step.name),
                    step.duration_ms as f64 / 1000.0
                );
                if step.status.is_failure() || !step.diagnostics.is_empty() {
                    let message = step
                        .diagnostics
                        .first()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .unwrap_or("step failed");
                    let _ = write!(output, "<failure message=\"{}\"/>", xml_escape(message));
                } else if step.status == Status::Skipped {
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
        for event in &self.events {
            output.push_str(&serde_json::to_string(event).map_err(ReportError::Serialize)?);
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
}
