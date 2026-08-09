//! Stable report and output primitives for the MDOK command line interface.
//!
//! The runtime owns execution state; this crate owns the wire format and all
//! user-facing renderers. Keeping the redactor here makes it difficult for a
//! new output path to accidentally print a secret.

#![forbid(unsafe_code)]

use serde::ser::{Error as SerError, SerializeSeq, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "1";
pub const MDOK_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CURL_SOURCE_VERSION: &str = env!("MDOK_CURL_SOURCE_VERSION");
pub const CURL_COMPAT_VERSION: &str = env!("MDOK_CURL_COMPAT_VERSION");
pub const LIBCURL_VERSION: &str = concat!(env!("MDOK_CURL_SOURCE_VERSION"), "-vendored-static");

#[cfg(target_os = "windows")]
pub const TLS_BACKEND: &str = "Schannel";
#[cfg(not(target_os = "windows"))]
pub const TLS_BACKEND: &str = "OpenSSL";

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

/// The kind of source associated with a reported step.
///
/// This is kept outside `StepReport` so existing callers can continue to use
/// their current struct literals. New metadata is attached to a `Report` and
/// rendered into the corresponding step object on the wire.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Curl,
    Exec,
}

/// Structured result metadata for an external process execution.
///
/// Output contents are intentionally not part of this report type. Callers
/// can report bounded accounting and lifecycle state without making command
/// output part of the persisted report or weakening the existing redaction
/// boundary.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ExternalExecutionResult {
    /// The canonical program identity or configured command profile.
    pub program: String,
    /// The argv used by the direct process invocation, when reporting it is
    /// permitted by the caller's policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
    #[serde(default)]
    pub timed_out: bool,
    #[serde(default)]
    pub output_limit_exceeded: bool,
    #[serde(default)]
    pub output_truncated: bool,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub duration_ms: u64,
}

/// Report-side attachment for optional metadata on one document step.
///
/// Ordinals are zero-based and refer to the positions in `Report.documents`
/// and `DocumentReport.steps`. The attachment is serialized into that nested
/// step, so consumers do not need to understand this implementation detail.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StepExecutionMetadata {
    pub document_ordinal: usize,
    pub step_ordinal: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<StepKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExternalExecutionResult>,
}

impl StepExecutionMetadata {
    pub fn new(
        document_ordinal: usize,
        step_ordinal: usize,
        kind: Option<StepKind>,
        execution: Option<ExternalExecutionResult>,
    ) -> Self {
        Self {
            document_ordinal,
            step_ordinal,
            kind,
            execution,
        }
    }
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

fn event_metadata_is_sorted(metadata: &[EventMetadataRecord]) -> bool {
    metadata
        .windows(2)
        .all(|records| records[0].sequence <= records[1].sequence)
}

fn event_metadata_position(metadata: &[EventMetadataRecord], sequence: u64) -> usize {
    let mut left = 0;
    let mut right = metadata.len();
    while left < right {
        let middle = left + (right - left) / 2;
        if metadata[middle].sequence < sequence {
            left = middle + 1;
        } else {
            right = middle;
        }
    }
    left
}

/// Lookup view for the public metadata vector.
///
/// Report-owned mutations keep `event_metadata` sorted. The fallback index
/// keeps direct construction or legacy callers that mutate the public vector
/// correct without turning every event lookup into a linear scan.
struct EventMetadataLookup<'a> {
    records: &'a [EventMetadataRecord],
    sorted_indices: Option<Vec<usize>>,
}

impl<'a> EventMetadataLookup<'a> {
    fn new(records: &'a [EventMetadataRecord]) -> Self {
        let sorted_indices = if event_metadata_is_sorted(records) {
            None
        } else {
            let mut indices: Vec<usize> = (0..records.len()).collect();
            indices.sort_by_key(|&index| records[index].sequence);
            Some(indices)
        };
        Self {
            records,
            sorted_indices,
        }
    }

    fn get(&self, sequence: u64) -> Option<&'a EventMetadata> {
        let record_index = match &self.sorted_indices {
            None => event_metadata_position(self.records, sequence),
            Some(indices) => {
                let mut left = 0;
                let mut right = indices.len();
                while left < right {
                    let middle = left + (right - left) / 2;
                    if self.records[indices[middle]].sequence < sequence {
                        left = middle + 1;
                    } else {
                        right = middle;
                    }
                }
                if indices
                    .get(left)
                    .is_some_and(|&index| self.records[index].sequence == sequence)
                {
                    indices[left]
                } else {
                    return None;
                }
            }
        };
        self.records
            .get(record_index)
            .filter(|record| record.sequence == sequence)
            .map(|record| &record.metadata)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub schema_version: String,
    pub mdok_version: String,
    pub curl_version: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub summary: Summary,
    pub documents: Vec<DocumentReport>,
    pub diagnostics: Vec<Diagnostic>,
    pub events: Vec<Event>,
    pub event_metadata: Vec<EventMetadataRecord>,
    /// Optional report-side attachments rendered into individual step
    /// objects. Keeping these out of `StepReport` preserves existing curl
    /// struct literals in callers that have not adopted execution metadata.
    pub step_execution_metadata: Vec<StepExecutionMetadata>,
}

#[derive(Deserialize)]
struct ReportWire {
    schema_version: String,
    mdok_version: String,
    curl_version: String,
    started_at: String,
    duration_ms: u64,
    summary: Summary,
    documents: Vec<DocumentWire>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default)]
    event_metadata: Vec<EventMetadataRecord>,
}

#[derive(Deserialize)]
struct DocumentWire {
    path: String,
    status: Status,
    duration_ms: u64,
    #[serde(default)]
    steps: Vec<StepWire>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
}

#[derive(Deserialize)]
struct StepWire {
    name: String,
    status: Status,
    #[serde(default)]
    command: Vec<String>,
    #[serde(default)]
    checks: Vec<CheckReport>,
    #[serde(default)]
    captures: Vec<String>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
    duration_ms: u64,
    #[serde(default)]
    kind: Option<StepKind>,
    #[serde(default)]
    execution: Option<ExternalExecutionResult>,
}

impl<'de> Deserialize<'de> for Report {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ReportWire::deserialize(deserializer)?;
        let mut step_execution_metadata = Vec::new();
        let mut event_metadata = wire.event_metadata;
        event_metadata.sort_by_key(|record| record.sequence);
        let documents = wire
            .documents
            .into_iter()
            .enumerate()
            .map(|(document_ordinal, document)| {
                let steps = document
                    .steps
                    .into_iter()
                    .enumerate()
                    .map(|(step_ordinal, step)| {
                        let StepWire {
                            name,
                            status,
                            command,
                            checks,
                            captures,
                            diagnostics,
                            duration_ms,
                            kind,
                            execution,
                        } = step;
                        if kind.is_some() || execution.is_some() {
                            step_execution_metadata.push(StepExecutionMetadata::new(
                                document_ordinal,
                                step_ordinal,
                                kind,
                                execution,
                            ));
                        }
                        StepReport {
                            name,
                            status,
                            command,
                            checks,
                            captures,
                            diagnostics,
                            duration_ms,
                        }
                    })
                    .collect();
                DocumentReport {
                    path: document.path,
                    status: document.status,
                    duration_ms: document.duration_ms,
                    steps,
                    diagnostics: document.diagnostics,
                }
            })
            .collect();

        Ok(Self {
            schema_version: wire.schema_version,
            mdok_version: wire.mdok_version,
            curl_version: wire.curl_version,
            started_at: wire.started_at,
            duration_ms: wire.duration_ms,
            summary: wire.summary,
            documents,
            diagnostics: wire.diagnostics,
            events: wire.events,
            event_metadata,
            step_execution_metadata,
        })
    }
}

struct SerializedDocuments<'a> {
    documents: &'a [DocumentReport],
    metadata: &'a [StepExecutionMetadata],
}

impl Serialize for SerializedDocuments<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut documents = serializer.serialize_seq(Some(self.documents.len()))?;
        for (document_ordinal, document) in self.documents.iter().enumerate() {
            documents.serialize_element(&SerializedDocument {
                document,
                document_ordinal,
                metadata: self.metadata,
            })?;
        }
        documents.end()
    }
}

struct SerializedDocument<'a> {
    document: &'a DocumentReport,
    document_ordinal: usize,
    metadata: &'a [StepExecutionMetadata],
}

impl Serialize for SerializedDocument<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serde_json::to_value(self.document).map_err(S::Error::custom)?;
        if let Some(steps) = value.get_mut("steps").and_then(Value::as_array_mut) {
            for metadata in self
                .metadata
                .iter()
                .filter(|metadata| metadata.document_ordinal == self.document_ordinal)
            {
                if let Some(step) = steps
                    .get_mut(metadata.step_ordinal)
                    .and_then(Value::as_object_mut)
                {
                    if let Some(kind) = metadata.kind {
                        step.insert(
                            "kind".to_string(),
                            serde_json::to_value(kind).map_err(S::Error::custom)?,
                        );
                    }
                    if let Some(execution) = &metadata.execution {
                        step.insert(
                            "execution".to_string(),
                            serde_json::to_value(execution).map_err(S::Error::custom)?,
                        );
                    }
                }
            }
        }
        value.serialize(serializer)
    }
}

impl Serialize for Report {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut fields = 7;
        if !self.diagnostics.is_empty() {
            fields += 1;
        }
        if !self.events.is_empty() {
            fields += 1;
        }
        if !self.event_metadata.is_empty() {
            fields += 1;
        }
        let mut report = serializer.serialize_struct("Report", fields)?;
        report.serialize_field("schema_version", &self.schema_version)?;
        report.serialize_field("mdok_version", &self.mdok_version)?;
        report.serialize_field("curl_version", &self.curl_version)?;
        report.serialize_field("started_at", &self.started_at)?;
        report.serialize_field("duration_ms", &self.duration_ms)?;
        report.serialize_field("summary", &self.summary)?;
        report.serialize_field(
            "documents",
            &SerializedDocuments {
                documents: &self.documents,
                metadata: &self.step_execution_metadata,
            },
        )?;
        if !self.diagnostics.is_empty() {
            report.serialize_field("diagnostics", &self.diagnostics)?;
        }
        if !self.events.is_empty() {
            report.serialize_field(
                "events",
                &SerializedEventRecords {
                    events: &self.events,
                    metadata: &self.event_metadata,
                },
            )?;
        }
        // Keep the additive metadata table for consumers that already use it;
        // events themselves now carry the same context inline.
        if !self.event_metadata.is_empty() {
            report.serialize_field("event_metadata", &self.event_metadata)?;
        }
        report.end()
    }
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
            step_execution_metadata: Vec::new(),
        }
    }

    /// Attach optional kind and external execution metadata to one step.
    ///
    /// The document and step ordinals are zero-based. Replacing an existing
    /// attachment keeps repeated report enrichment deterministic.
    pub fn set_step_execution_metadata(&mut self, metadata: StepExecutionMetadata) {
        if let Some(existing) = self.step_execution_metadata.iter_mut().find(|existing| {
            existing.document_ordinal == metadata.document_ordinal
                && existing.step_ordinal == metadata.step_ordinal
        }) {
            *existing = metadata;
        } else {
            self.step_execution_metadata.push(metadata);
        }
    }

    pub fn step_execution_metadata(
        &self,
        document_ordinal: usize,
        step_ordinal: usize,
    ) -> Option<&StepExecutionMetadata> {
        self.step_execution_metadata.iter().find(|metadata| {
            metadata.document_ordinal == document_ordinal && metadata.step_ordinal == step_ordinal
        })
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
        if !event_metadata_is_sorted(&self.event_metadata) {
            self.event_metadata.sort_by_key(|record| record.sequence);
        }
        let index = event_metadata_position(&self.event_metadata, sequence);
        if metadata.is_empty() {
            if self
                .event_metadata
                .get(index)
                .is_some_and(|record| record.sequence == sequence)
            {
                let mut end = index + 1;
                while self
                    .event_metadata
                    .get(end)
                    .is_some_and(|record| record.sequence == sequence)
                {
                    end += 1;
                }
                self.event_metadata.drain(index..end);
            }
            return;
        }
        if let Some(record) = self.event_metadata.get_mut(index) {
            if record.sequence != sequence {
                self.event_metadata
                    .insert(index, EventMetadataRecord { sequence, metadata });
                return;
            }
            record.metadata = metadata;
        } else {
            self.event_metadata
                .insert(index, EventMetadataRecord { sequence, metadata });
        }
    }

    /// Return the metadata attached to an event sequence, if present.
    pub fn event_metadata_for(&self, sequence: u64) -> Option<&EventMetadata> {
        EventMetadataLookup::new(&self.event_metadata).get(sequence)
    }

    fn write_json_lines_range_impl<W: Write + ?Sized>(
        &self,
        range: Range<usize>,
        writer: &mut W,
    ) -> Result<(), ReportError> {
        let start = range.start.min(self.events.len());
        let end = range.end.min(self.events.len());
        if start >= end {
            return Ok(());
        }
        let metadata = EventMetadataLookup::new(&self.event_metadata);
        let empty = EventMetadata::default();
        for event in &self.events[start..end] {
            let event_metadata = metadata.get(event.sequence).unwrap_or(&empty);
            serde_json::to_writer(
                &mut *writer,
                &BorrowedEventRecord {
                    event,
                    metadata: event_metadata,
                },
            )
            .map_err(ReportError::Serialize)?;
            writer
                .write_all(b"\n")
                .map_err(|source| ReportError::Output { source })?;
        }
        Ok(())
    }

    /// Return events in wire order with optional metadata flattened into each
    /// record. With no metadata this serializes identically to `Event`.
    pub fn event_records(&self) -> Vec<EventRecord> {
        let metadata = EventMetadataLookup::new(&self.event_metadata);
        self.events
            .iter()
            .map(|event| EventRecord {
                event: event.clone(),
                metadata: metadata.get(event.sequence).cloned().unwrap_or_default(),
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
            let _ = writeln!(output, "{}  {}", label, escape_human_text(&document.path));
            for step in &document.steps {
                let _ = writeln!(
                    output,
                    "  {}  {}",
                    step.status.as_str(),
                    escape_human_text(&step.name)
                );
                if verbose && !step.command.is_empty() {
                    let command = step
                        .command
                        .iter()
                        .map(|argument| escape_human_text(argument))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(output, "    $ {}", command);
                }
                for check in &step.checks {
                    let _ = writeln!(
                        output,
                        "    {}  {}",
                        check.status.as_str(),
                        escape_human_text(&check.expression)
                    );
                }
                for capture in &step.captures {
                    let _ = writeln!(output, "    capture  {}", escape_human_text(capture));
                }
                for diagnostic in &step.diagnostics {
                    write_diagnostic(&mut output, diagnostic);
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
            write_junit_suite(&mut output, &document.path, cases, document.duration_ms);
        }
        if !self.diagnostics.is_empty() {
            let cases = self
                .diagnostics
                .iter()
                .enumerate()
                .map(|(index, diagnostic)| JunitCase {
                    name: format!("report diagnostic {}", index + 1),
                    duration_ms: 0,
                    failure: (diagnostic.severity == Severity::Error)
                        .then(|| diagnostic.message.clone()),
                    skipped: diagnostic.severity != Severity::Error,
                })
                .collect();
            write_junit_suite(&mut output, "mdok report", cases, 0);
        }
        output.push_str("</testsuites>\n");
        output
    }

    pub fn json_lines(&self) -> Result<String, ReportError> {
        let mut output = Vec::with_capacity(self.events.len().saturating_mul(128));
        self.write_json_lines_range_impl(0..self.events.len(), &mut output)?;
        Ok(String::from_utf8(output).expect("serde_json always emits UTF-8"))
    }

    /// Stream all event records as newline-delimited JSON into `writer`.
    ///
    /// The writer receives each record directly; no accumulated `EventRecord`
    /// vector is built.
    pub fn write_json_lines<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), ReportError> {
        self.write_json_lines_range_impl(0..self.events.len(), writer)
    }

    /// Stream the selected half-open event range as newline-delimited JSON.
    ///
    /// Bounds beyond the current event count are clamped, and an empty or
    /// reversed range emits nothing. This keeps incremental callers bounded
    /// when a report grows between selecting and writing a range.
    pub fn write_json_lines_range<W: Write + ?Sized>(
        &self,
        range: Range<usize>,
        writer: &mut W,
    ) -> Result<(), ReportError> {
        self.write_json_lines_range_impl(range, writer)
    }

    /// Return only the selected event range as newline-delimited JSON.
    pub fn json_lines_range(&self, range: Range<usize>) -> Result<String, ReportError> {
        let mut output = Vec::new();
        self.write_json_lines_range_impl(range, &mut output)?;
        Ok(String::from_utf8(output).expect("serde_json always emits UTF-8"))
    }
}

struct BorrowedEventRecord<'a> {
    event: &'a Event,
    metadata: &'a EventMetadata,
}

impl Serialize for BorrowedEventRecord<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("EventRecord", 2)?;
        record.serialize_field("sequence", &self.event.sequence)?;
        record.serialize_field("kind", &self.event.kind)?;
        record.serialize_field("document", &self.event.document)?;
        record.serialize_field("step", &self.event.step)?;
        record.serialize_field("status", &self.event.status)?;
        record.serialize_field("message", &self.event.message)?;
        if let Some(run_id) = &self.metadata.run_id {
            record.serialize_field("run_id", run_id)?;
        }
        if let Some(document_ordinal) = self.metadata.document_ordinal {
            record.serialize_field("document_ordinal", &document_ordinal)?;
        }
        if let Some(step_ordinal) = self.metadata.step_ordinal {
            record.serialize_field("step_ordinal", &step_ordinal)?;
        }
        if let Some(check_ordinal) = self.metadata.check_ordinal {
            record.serialize_field("check_ordinal", &check_ordinal)?;
        }
        if let Some(capture_ordinal) = self.metadata.capture_ordinal {
            record.serialize_field("capture_ordinal", &capture_ordinal)?;
        }
        if let Some(timestamp) = &self.metadata.timestamp {
            record.serialize_field("timestamp", timestamp)?;
        }
        if let Some(duration_ms) = self.metadata.duration_ms {
            record.serialize_field("duration_ms", &duration_ms)?;
        }
        record.end()
    }
}

struct SerializedEventRecords<'a> {
    events: &'a [Event],
    metadata: &'a [EventMetadataRecord],
}

impl Serialize for SerializedEventRecords<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let empty = EventMetadata::default();
        let metadata = EventMetadataLookup::new(self.metadata);
        let mut sequence = serializer.serialize_seq(Some(self.events.len()))?;
        for event in self.events {
            let event_metadata = metadata.get(event.sequence).unwrap_or(&empty);
            sequence.serialize_element(&BorrowedEventRecord {
                event,
                metadata: event_metadata,
            })?;
        }
        sequence.end()
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

/// Canonical encodings of a secret value.
///
/// Redaction matches by exact substring. A captured or logged value that is a
/// reversible transform of a secret (base64, hex, percent-encoded, reversed)
/// is not a substring of the original and would otherwise evade redaction.
/// Adding these encoded forms to the taint set closes that gap. See security
/// finding F2 (secret exfiltration via transformed captures).
fn encoded_forms(secret: &str) -> Vec<String> {
    use base64::Engine as _;
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let bytes = secret.as_bytes();
    let mut forms = Vec::with_capacity(5);
    // Base64 (standard and URL-safe).
    forms.push(base64::engine::general_purpose::STANDARD.encode(bytes));
    forms.push(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes));
    // Hex (lowercase).
    forms.push(bytes.iter().map(|b| format!("{b:02x}")).collect());
    // Percent-encoding (as produced by the `url` template filter).
    forms.push(utf8_percent_encode(secret, NON_ALPHANUMERIC).to_string());
    // Reversed.
    forms.push(secret.chars().rev().collect());
    forms
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
            .flat_map(|value| {
                let mut forms = vec![value.clone()];
                forms.extend(encoded_forms(&value));
                forms
            })
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

fn escape_human_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\u{2028}' => escaped.push_str("\\u{2028}"),
            '\u{2029}' => escaped.push_str("\\u{2029}"),
            character if character.is_control() => {
                let codepoint = character as u32;
                if codepoint <= u8::MAX as u32 {
                    let _ = write!(escaped, "\\x{codepoint:02X}");
                } else {
                    let _ = write!(escaped, "\\u{{{codepoint:04X}}}");
                }
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn write_diagnostic(output: &mut String, diagnostic: &Diagnostic) {
    let location = diagnostic
        .file
        .as_deref()
        .map(|file| match &diagnostic.span {
            Some(span) => format!("{}:{}:{}", escape_human_text(file), span.line, span.column),
            None => escape_human_text(file),
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
        escape_human_text(&diagnostic.code),
        escape_human_text(&diagnostic.title),
        location,
        escape_human_text(&diagnostic.message)
    );
    if let Some(hint) = &diagnostic.hint {
        let _ = writeln!(output, "    hint: {}", escape_human_text(hint));
    }
}

#[derive(Debug)]
struct JunitCase {
    name: String,
    duration_ms: u64,
    failure: Option<String>,
    skipped: bool,
}

fn write_junit_suite(output: &mut String, name: &str, cases: Vec<JunitCase>, duration_ms: u64) {
    let tests = cases.len();
    let failures = cases.iter().filter(|case| case.failure.is_some()).count();
    let _ = write!(
        output,
        "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{}\">",
        xml_escape(name),
        tests,
        failures,
        duration_ms as f64 / 1000.0
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
    #[error("could not write JSON Lines output: {source}")]
    Output { source: io::Error },
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
            step_execution_metadata: Vec::new(),
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
            step_execution_metadata: Vec::new(),
        };

        let junit = report.junit();
        assert!(junit.contains("tests=\"1\" failures=\"1\""));
        assert!(junit.contains("document diagnostic 1"));
        assert!(junit.contains("metadata is malformed"));
    }

    #[test]
    fn human_and_junit_include_step_and_report_diagnostics() {
        let mut report = Report::now();
        report.documents.push(DocumentReport {
            path: "workflow.md".to_string(),
            status: Status::Failed,
            duration_ms: 0,
            steps: vec![StepReport {
                name: "request".to_string(),
                status: Status::Failed,
                diagnostics: vec![Diagnostic::error(
                    "MDOK-E600",
                    "Transfer failed",
                    "connection refused",
                )],
                ..StepReport::default()
            }],
            diagnostics: Vec::new(),
        });
        report.diagnostics.push(Diagnostic::error(
            "MDOK-E800",
            "Report failed",
            "serialization failed",
        ));

        let human = report.human(false, false);
        assert!(human.contains("connection refused"));
        assert!(report.junit().contains("report diagnostic 1"));
        assert!(report.junit().contains("serialization failed"));
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
        assert_eq!(report_json["events"][0]["run_id"], "run-1");
        assert_eq!(report_json["events"][0]["step_ordinal"], 1);
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
    fn event_metadata_stays_sorted_and_lookup_preserves_sequence_mapping() {
        let mut report = Report::new("unix-ms:1");
        for sequence in [10, 2, 7] {
            report.set_event_metadata(
                sequence,
                EventMetadata {
                    run_id: Some(format!("run-{sequence}")),
                    ..EventMetadata::default()
                },
            );
            report.events.push(Event {
                sequence,
                kind: "step.finished".to_string(),
                document: None,
                step: None,
                status: Some(Status::Passed),
                message: None,
            });
        }

        assert_eq!(
            report
                .event_metadata
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![2, 7, 10]
        );
        assert_eq!(
            report
                .event_metadata_for(7)
                .and_then(|metadata| metadata.run_id.as_deref()),
            Some("run-7")
        );
        assert_eq!(
            report
                .event_records()
                .into_iter()
                .map(|record| record.metadata.run_id)
                .collect::<Vec<_>>(),
            vec![
                Some("run-10".to_string()),
                Some("run-2".to_string()),
                Some("run-7".to_string())
            ]
        );

        report.set_event_metadata(7, EventMetadata::default());
        assert_eq!(
            report
                .event_metadata
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![2, 10]
        );
    }

    #[test]
    fn direct_and_legacy_metadata_construction_keep_lookup_correct() {
        let event = |sequence| Event {
            sequence,
            kind: "step.finished".to_string(),
            document: None,
            step: None,
            status: Some(Status::Passed),
            message: None,
        };
        let mut report = Report::new("unix-ms:1");
        report.events = vec![event(9), event(1)];
        report.event_metadata = vec![
            EventMetadataRecord {
                sequence: 9,
                metadata: EventMetadata {
                    run_id: Some("run-9".to_string()),
                    ..EventMetadata::default()
                },
            },
            EventMetadataRecord {
                sequence: 1,
                metadata: EventMetadata {
                    run_id: Some("run-1".to_string()),
                    ..EventMetadata::default()
                },
            },
        ];

        assert_eq!(
            report
                .event_records()
                .into_iter()
                .map(|record| record.metadata.run_id)
                .collect::<Vec<_>>(),
            vec![Some("run-9".to_string()), Some("run-1".to_string())]
        );

        let wire = serde_json::to_value(&report).expect("serialize direct report");
        let parsed: Report = serde_json::from_value(wire).expect("deserialize legacy report");
        assert_eq!(
            parsed
                .event_metadata
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![1, 9]
        );
        assert_eq!(
            parsed
                .event_metadata_for(9)
                .and_then(|metadata| metadata.run_id.as_deref()),
            Some("run-9")
        );
    }

    #[test]
    fn json_lines_ranges_stream_without_accumulating_event_records() {
        let mut report = Report::new("unix-ms:1");
        for sequence in 0..5 {
            report.push_event(
                Event {
                    sequence,
                    kind: "step.finished".to_string(),
                    document: Some(format!("document-{sequence}")),
                    step: Some(format!("step-{sequence}")),
                    status: Some(Status::Passed),
                    message: None,
                },
                Some(EventMetadata {
                    run_id: Some(format!("run-{sequence}")),
                    ..EventMetadata::default()
                }),
            );
        }

        let expected = report.json_lines_range(1..4).expect("serialize range");
        let mut streamed = Vec::new();
        report
            .write_json_lines_range(1..4, &mut streamed)
            .expect("stream range");
        assert_eq!(String::from_utf8(streamed).expect("UTF-8 JSONL"), expected);
        assert_eq!(
            expected
                .lines()
                .map(|line| {
                    serde_json::from_str::<Value>(line).expect("event JSON")["sequence"].clone()
                })
                .collect::<Vec<_>>(),
            vec![Value::from(1), Value::from(2), Value::from(3)]
        );

        let mut all_streamed = Vec::new();
        report
            .write_json_lines(&mut all_streamed)
            .expect("stream all events");
        assert_eq!(
            String::from_utf8(all_streamed).expect("UTF-8 JSONL"),
            report.json_lines().expect("serialize all events")
        );
        assert!(
            report
                .json_lines_range(99..100)
                .expect("empty range")
                .is_empty()
        );
    }

    #[test]
    fn human_escapes_control_and_ansi_content_in_verbose_output() {
        let mut diagnostic = Diagnostic::error(
            "MDOK-E\x1b[99m",
            "diagnostic\n title",
            "message\rwith\0control",
        )
        .at_file(Path::new("path\x1b[99m\n.md"));
        diagnostic.hint = Some("hint\u{009b}31m".to_string());

        let mut report = Report::new("unix-ms:1");
        report.documents.push(DocumentReport {
            path: "workflow\x1b[99m\n.md".to_string(),
            status: Status::Failed,
            duration_ms: 0,
            steps: vec![StepReport {
                name: "step\rname".to_string(),
                status: Status::Failed,
                command: vec!["echo".to_string(), "arg\x1b[99m\nvalue".to_string()],
                checks: vec![CheckReport {
                    expression: "body\tvalue".to_string(),
                    status: Status::Passed,
                    result: None,
                }],
                captures: vec!["capture\0value".to_string()],
                diagnostics: vec![diagnostic],
                duration_ms: 0,
            }],
            diagnostics: Vec::new(),
        });

        let human = report.human(false, true);
        assert!(!human.as_bytes().contains(&0x00));
        assert!(!human.as_bytes().contains(&0x1b));
        assert!(!human.contains('\r'));
        assert!(!human.contains('\t'));
        assert!(!human.contains('\u{009b}'));
        assert!(human.contains(r"workflow\x1B[99m\n.md"));
        assert!(human.contains(r"step\rname"));
        assert!(human.contains(r"arg\x1B[99m\nvalue"));
        assert!(human.contains(r"body\tvalue"));
        assert!(human.contains(r"capture\x00value"));
        assert!(human.contains(r"path\x1B[99m\n.md"));
        assert!(human.contains(r"diagnostic\n title"));
        assert!(human.contains(r"message\rwith\x00control"));
        assert!(human.contains(r"hint: hint\x9B31m"));

        let colored = report.human(true, true);
        assert!(!colored.contains("\x1b[99m"));
    }

    #[test]
    fn execution_metadata_is_nested_and_round_trips() {
        let mut report = Report::now();
        report.documents.push(DocumentReport {
            path: "commands.md".to_string(),
            status: Status::Passed,
            duration_ms: 8,
            steps: vec![StepReport {
                name: "validate".to_string(),
                status: Status::Passed,
                duration_ms: 5,
                ..StepReport::default()
            }],
            diagnostics: Vec::new(),
        });
        let execution = ExternalExecutionResult {
            program: "mdok-fixture".to_string(),
            argv: vec!["mdok-fixture".to_string(), "--mode=validate".to_string()],
            exit_code: Some(0),
            signal: None,
            timed_out: false,
            output_limit_exceeded: false,
            output_truncated: false,
            stdout_bytes: 12,
            stderr_bytes: 0,
            duration_ms: 5,
        };
        let metadata =
            StepExecutionMetadata::new(0, 0, Some(StepKind::Exec), Some(execution.clone()));
        report.set_step_execution_metadata(metadata.clone());

        let value = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(value["documents"][0]["steps"][0]["kind"], "exec");
        assert_eq!(
            value["documents"][0]["steps"][0]["execution"]["program"],
            "mdok-fixture"
        );
        assert_eq!(
            value["documents"][0]["steps"][0]["execution"]["stdout_bytes"],
            12
        );
        assert!(value.get("step_execution_metadata").is_none());

        let parsed: Report = serde_json::from_value(value).expect("deserialize report");
        assert_eq!(parsed.documents, report.documents);
        assert_eq!(parsed.step_execution_metadata, vec![metadata]);
        assert_eq!(
            parsed
                .step_execution_metadata(0, 0)
                .and_then(|metadata| metadata.execution.as_ref()),
            Some(&execution)
        );
    }

    #[test]
    fn curl_steps_keep_the_legacy_shape_without_execution_metadata() {
        let mut report = Report::now();
        report.documents.push(DocumentReport {
            path: "curl.md".to_string(),
            status: Status::Passed,
            duration_ms: 3,
            steps: vec![StepReport {
                name: "request".to_string(),
                status: Status::Passed,
                command: vec!["curl".to_string(), "https://example.test".to_string()],
                duration_ms: 3,
                ..StepReport::default()
            }],
            diagnostics: Vec::new(),
        });

        let value = serde_json::to_value(&report).expect("serialize report");
        let step = &value["documents"][0]["steps"][0];
        assert!(step.get("kind").is_none());
        assert!(step.get("execution").is_none());
        let parsed: Report = serde_json::from_value(value).expect("deserialize report");
        assert!(parsed.step_execution_metadata.is_empty());
    }

    #[test]
    fn redaction_preserves_execution_metadata_and_redacts_argv() {
        let mut report = Report::now();
        report.documents.push(DocumentReport {
            path: "commands.md".to_string(),
            status: Status::Failed,
            duration_ms: 11,
            steps: vec![StepReport {
                name: "inspect".to_string(),
                status: Status::Failed,
                duration_ms: 11,
                ..StepReport::default()
            }],
            diagnostics: Vec::new(),
        });
        report.set_step_execution_metadata(StepExecutionMetadata::new(
            0,
            0,
            Some(StepKind::Exec),
            Some(ExternalExecutionResult {
                program: "mdok-fixture".to_string(),
                argv: vec!["mdok-fixture".to_string(), "super-secret".to_string()],
                exit_code: Some(7),
                signal: Some(9),
                timed_out: true,
                output_limit_exceeded: true,
                output_truncated: true,
                stdout_bytes: 1024,
                stderr_bytes: 2048,
                duration_ms: 11,
            }),
        ));

        let redacted = Redactor::new(["super-secret"])
            .redact_report(&report)
            .expect("redact report");
        let metadata = redacted
            .step_execution_metadata(0, 0)
            .expect("execution metadata");
        let execution = metadata.execution.as_ref().expect("execution result");
        assert_eq!(metadata.kind, Some(StepKind::Exec));
        assert_eq!(execution.program, "mdok-fixture");
        assert_eq!(execution.argv[1], "[REDACTED]");
        assert_eq!(execution.exit_code, Some(7));
        assert_eq!(execution.signal, Some(9));
        assert!(execution.timed_out);
        assert!(execution.output_limit_exceeded);
        assert!(execution.output_truncated);
        assert_eq!(execution.stdout_bytes, 1024);
        assert_eq!(execution.stderr_bytes, 2048);
        assert_eq!(execution.duration_ms, 11);
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
            step_execution_metadata: Vec::new(),
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

    /// F2 regression: a reversibly-transformed copy of a secret (base64, hex,
    /// url-encoded, reversed) must also be redacted, not just the raw value.
    #[test]
    fn redactor_matches_encoded_forms_of_secrets() {
        use base64::Engine as _;
        let secret = "SUPERSECRET-LEAK-ME-12345";
        let redactor = Redactor::new([secret]);
        let b64 = base64::engine::general_purpose::STANDARD.encode(secret);
        let hex = secret
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let reversed: String = secret.chars().rev().collect();
        for encoded in [b64.as_str(), hex.as_str(), reversed.as_str()] {
            assert!(
                redactor.redact_text(encoded).contains("[REDACTED]"),
                "encoded form should be redacted: {encoded}"
            );
        }
    }
}
