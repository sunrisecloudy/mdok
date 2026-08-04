//! `mdok` command line adapter.
//!
//! Rust owns Markdown planning, typed templates, checks, captures, policy, and
//! reporting. Native vendored libcurl is used for the conservative plain-GET
//! fast path; the compatibility adapter remains the fallback for the broader
//! supported option surface.

#![forbid(unsafe_code)]

use clap::{Args, Parser, Subcommand};
use mdok_curl::{CurlError, CurlPlan, CurlPolicy};
use mdok_markdown::{MarkdownError, parse, plan_document};
use mdok_report::{
    CheckReport, Diagnostic, DocumentReport, Event, EventMetadata, Redactor, Report, Severity,
    Status, StepReport, write_atomic_json,
};
use mdok_template::{
    Filter, PathPart, Template, TemplateError, TemplateExpression, TemplatePart,
    lookup as lookup_template, render_expression,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use url::Url;
use walkdir::WalkDir;

const EXIT_OK: u8 = 0;
const EXIT_CHECK_FAILED: u8 = 1;
const EXIT_INPUT: u8 = 2;
const EXIT_POLICY: u8 = 3;
const EXIT_INTERNAL: u8 = 4;
#[allow(dead_code)]
const EXIT_INTERRUPTED: u8 = 130;

#[derive(Parser, Debug)]
#[command(
    name = "mdok",
    about = "Test and validate executable Markdown API documents",
    version,
    arg_required_else_help = false
)]
struct Cli {
    #[command(flatten)]
    options: CommonOptions,
    #[command(subcommand)]
    command: Option<Command>,
    /// A path without a command is an alias for `mdok test PATH`.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Args, Clone, Debug, Default)]
struct CommonOptions {
    /// Project configuration; otherwise search for mdok.toml upward.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Select an environment profile from mdok.toml.
    #[arg(long, global = true, value_name = "NAME")]
    env: Option<String>,
    /// Set a non-secret variable. Repeatable.
    #[arg(long, global = true, value_name = "KEY=VALUE")]
    var: Vec<String>,
    /// Set a secret. Supports literal values, @env:NAME, and @file:PATH.
    #[arg(long, global = true, value_name = "KEY=VALUE")]
    secret: Vec<String>,
    /// Add an allowed destination host pattern.
    #[arg(long, global = true, value_name = "PATTERN")]
    allow_host: Vec<String>,
    /// Deny a destination host pattern.
    #[arg(long, global = true, value_name = "PATTERN")]
    deny_host: Vec<String>,
    /// Maximum number of documents processed concurrently.
    #[arg(long, global = true, default_value_t = 1, value_name = "N")]
    jobs: usize,
    /// Stop scheduling after the first failed document or check.
    #[arg(long, global = true)]
    fail_fast: bool,
    /// Per-transfer timeout, such as 30s or 500ms.
    #[arg(long, global = true, value_name = "DURATION")]
    timeout: Option<String>,
    /// Maximum response body captured in memory.
    #[arg(long, global = true, value_name = "BYTES")]
    max_body: Option<usize>,
    /// Emit one JSON report to stdout.
    #[arg(long, global = true)]
    json: bool,
    /// Emit one JSON event per line to stdout.
    #[arg(long, global = true)]
    json_lines: bool,
    /// Write JUnit XML to this path.
    #[arg(long, global = true, value_name = "PATH")]
    junit: Option<PathBuf>,
    /// Atomically write the JSON report to this path.
    #[arg(long, global = true, value_name = "PATH")]
    report: Option<PathBuf>,
    /// Do not emit ANSI color.
    #[arg(long, global = true)]
    no_color: bool,
    /// Deny network execution.
    #[arg(long, global = true)]
    offline: bool,
    /// Seed reserved for deterministic generators.
    #[arg(long, global = true)]
    seed: Option<u64>,
    /// Include redacted request metadata in human output.
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse, plan, execute, and check documents.
    Test { paths: Vec<PathBuf> },
    /// Parse and statically validate without network access.
    Lint { paths: Vec<PathBuf> },
    /// Print the normalized redacted execution plan.
    Plan { paths: Vec<PathBuf> },
    /// List documents, steps, checks, and captures.
    List { paths: Vec<PathBuf> },
    /// Print MDOK and compatibility versions.
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Test,
    Lint,
    Plan,
    List,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct FileConfig {
    language: Option<String>,
    curl_compat: Option<String>,
    #[serde(default)]
    execution: ExecutionConfig,
    #[serde(default)]
    policy: PolicyConfig,
    #[serde(default)]
    vars: BTreeMap<String, toml::Value>,
    #[serde(default)]
    env: BTreeMap<String, EnvironmentConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct ExecutionConfig {
    #[serde(default = "default_jobs")]
    jobs: usize,
    #[serde(default)]
    fail_fast: bool,
    max_body_bytes: Option<usize>,
    memory_body_threshold_bytes: Option<usize>,
    connect_timeout: Option<String>,
    total_timeout: Option<String>,
    #[serde(default)]
    allowed_schemes: Vec<String>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            jobs: default_jobs(),
            fail_fast: false,
            max_body_bytes: None,
            memory_body_threshold_bytes: None,
            connect_timeout: None,
            total_timeout: None,
            allowed_schemes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct PolicyConfig {
    #[serde(default)]
    allowed_hosts: Vec<String>,
    #[serde(default)]
    allowed_schemes: Vec<String>,
    #[serde(default)]
    allow_proxy: bool,
    #[serde(default)]
    allow_insecure_tls: bool,
    #[serde(default)]
    allow_resolve: bool,
    #[serde(default)]
    allow_connect_to: bool,
    #[serde(default)]
    allow_private_network: bool,
    #[serde(default)]
    allowed_read_paths: Vec<String>,
    #[serde(default)]
    allowed_write_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct EnvironmentConfig {
    #[serde(default)]
    vars: BTreeMap<String, toml::Value>,
    #[serde(default)]
    secrets: BTreeMap<String, SecretSpec>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum SecretSpec {
    Source { from_env: String },
    Value(String),
}

fn default_jobs() -> usize {
    1
}

#[derive(Clone, Debug)]
struct EffectiveConfig {
    vars: BTreeMap<String, Variable>,
    allowed_hosts: Vec<String>,
    denied_hosts: Vec<String>,
    allowed_schemes: Vec<String>,
    allow_proxy: bool,
    allow_insecure_tls: bool,
    allow_resolve: bool,
    allow_connect_to: bool,
    allow_private_network: bool,
    allow_file_reads: bool,
    allowed_read_roots: Vec<PathBuf>,
    allow_artifact_writes: bool,
    allowed_artifact_roots: Vec<PathBuf>,
    memory_body_threshold: usize,
    connect_timeout: Duration,
    jobs: usize,
    fail_fast: bool,
    timeout: Duration,
    max_body: usize,
}

#[derive(Clone, Debug)]
struct Variable {
    value: Value,
    secret: bool,
}

impl EffectiveConfig {
    fn secret_values(&self) -> impl Iterator<Item = &str> {
        self.vars
            .values()
            .filter(|var| var.secret)
            .filter_map(|var| match &var.value {
                Value::String(value) => Some(value.as_str()),
                _ => None,
            })
    }
}

#[derive(Clone, Debug)]
struct DocumentPlan {
    path: PathBuf,
    steps: Vec<StepPlan>,
    variables: BTreeMap<String, Variable>,
}

#[derive(Clone, Debug)]
struct StepPlan {
    name: String,
    command: Vec<String>,
    raw_command: String,
    checks: Vec<String>,
    captures: Vec<String>,
}

#[derive(Clone, Debug)]
struct PlanOutcome {
    plan: Option<DocumentPlan>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct Fence {
    language: String,
    attrs: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug)]
struct CliError {
    code: u8,
    diagnostic: Diagnostic,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.options.json;
    let json_lines = cli.options.json_lines;
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let CliError { code, diagnostic } = *error;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": mdok_report::SCHEMA_VERSION,
                        "diagnostics": [&diagnostic]
                    })
                );
            } else if json_lines {
                println!(
                    "{}",
                    serde_json::json!({
                        "kind": "diagnostic",
                        "status": "error",
                        "diagnostic": &diagnostic
                    })
                );
            } else {
                eprintln!("error[{}]: {}", diagnostic.code, diagnostic.message);
            }
            ExitCode::from(code)
        }
    }
}

fn run(cli: Cli) -> Result<u8, Box<CliError>> {
    let (mode, paths) = match cli.command {
        Some(Command::Version) => return print_version(&cli.options),
        Some(Command::Test { paths }) => (Mode::Test, paths),
        Some(Command::Lint { paths }) => (Mode::Lint, paths),
        Some(Command::Plan { paths }) => (Mode::Plan, paths),
        Some(Command::List { paths }) => (Mode::List, paths),
        None => (Mode::Test, cli.paths),
    };
    if paths.is_empty() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "No input paths",
                "provide at least one Markdown path",
            ),
        ));
    }
    if cli.options.json && cli.options.json_lines {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Conflicting output modes",
                "--json and --json-lines cannot be selected together",
            ),
        ));
    }
    if cli.options.jobs == 0 {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Invalid jobs value",
                "--jobs must be at least 1",
            ),
        ));
    }
    let paths = discover_paths(&paths)?;
    let config = load_config(&paths, &cli.options)?;
    let started = Instant::now();
    let mut report = Report::now();
    let parallel = cli.options.jobs.max(config.jobs).min(paths.len().max(1));
    let sequential = cli.options.fail_fast || config.fail_fast || parallel == 1 || paths.len() == 1;
    let stream_jsonl = cli.options.json_lines && sequential;
    if stream_jsonl {
        for (document_ordinal, path) in paths.into_iter().enumerate() {
            let document_path = path.display().to_string();
            let mut stream_error = None;
            let result = process_document_with_hook(
                &path,
                mode,
                &config,
                &cli.options,
                |step_ordinal, step| {
                    if stream_error.is_some() {
                        return;
                    }
                    let event_start = report.events.len();
                    append_step_event(
                        &mut report,
                        &document_path,
                        document_ordinal,
                        step_ordinal,
                        step,
                    );
                    if let Err(error) = stream_event_range(&report, event_start) {
                        stream_error = Some(error);
                    }
                },
            );
            if let Some(error) = stream_error {
                return Err(error);
            }
            let failed = result.status.is_failure();
            let event_start = report.events.len();
            append_document_event(&mut report, &result, document_ordinal);
            stream_event_range(&report, event_start)?;
            report.add_document(result);
            if failed && (cli.options.fail_fast || config.fail_fast) {
                break;
            }
        }
    } else {
        let results = if sequential {
            let mut results = Vec::with_capacity(paths.len());
            for path in paths {
                let result = process_document(&path, mode, &config, &cli.options);
                let failed = result.status.is_failure();
                results.push(result);
                if failed && (cli.options.fail_fast || config.fail_fast) {
                    break;
                }
            }
            results
        } else {
            process_documents_parallel(paths, mode, config.clone(), cli.options.clone(), parallel)
        };
        for (document_ordinal, document) in results.into_iter().enumerate() {
            append_report_document(&mut report, document, document_ordinal);
        }
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    let redactor = Redactor::new(config.secret_values().map(str::to_string));
    let report = redactor.redact_report(&report).map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Report error", error.to_string()),
        )
    })?;
    emit_report(&report, &cli.options, mode, stream_jsonl)?;
    Ok(report_exit_code(&report))
}

fn report_exit_code(report: &Report) -> u8 {
    let diagnostics = all_report_diagnostics(report);
    let policy_failure = diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "MDOK-E302" | "MDOK-E303" | "MDOK-E304" | "MDOK-E602" | "MDOK-E603" | "MDOK-E604"
        )
    });
    if policy_failure {
        EXIT_POLICY
    } else if diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            && (diagnostic.code == "MDOK-E800"
                || diagnostic.code == "MDOK-E500"
                || diagnostic.code.starts_with("MDOK-E0")
                || diagnostic.code.starts_with("MDOK-E1")
                || diagnostic.code.starts_with("MDOK-E2")
                || diagnostic.code.starts_with("MDOK-E3")
                || diagnostic.code.starts_with("MDOK-E4"))
    }) {
        EXIT_INPUT
    } else if report.summary.has_failures() {
        EXIT_CHECK_FAILED
    } else {
        EXIT_OK
    }
}

fn all_report_diagnostics(report: &Report) -> Vec<&Diagnostic> {
    let mut diagnostics = report.diagnostics.iter().collect::<Vec<_>>();
    for document in &report.documents {
        diagnostics.extend(document.diagnostics.iter());
        for step in &document.steps {
            diagnostics.extend(step.diagnostics.iter());
        }
    }
    diagnostics
}

fn print_version(options: &CommonOptions) -> Result<u8, Box<CliError>> {
    if options.json {
        println!(
            "{}",
            serde_json::json!({
                "mdok_version": mdok_report::MDOK_VERSION,
                "curl_version": mdok_report::CURL_COMPAT_VERSION,
                "libcurl": mdok_report::LIBCURL_VERSION,
                "tls": mdok_report::TLS_BACKEND,
                "features": {"runtime_adapter": true, "network_execution": true, "native_curl_bridge": true, "native_curl_fast_path": true, "vendored_curl": true}
            })
        );
    } else {
        println!("mdok {}", mdok_report::MDOK_VERSION);
        println!("curl compatibility {}", mdok_report::CURL_COMPAT_VERSION);
        println!("libcurl: {}", mdok_report::LIBCURL_VERSION);
        println!("TLS backend: {}", mdok_report::TLS_BACKEND);
        println!("native bridge: plain-GET fast path; compatibility adapter fallback");
    }
    Ok(EXIT_OK)
}

fn append_report_document(report: &mut Report, document: DocumentReport, document_ordinal: usize) {
    for (step_ordinal, step) in document.steps.iter().enumerate() {
        append_step_event(report, &document.path, document_ordinal, step_ordinal, step);
    }
    append_document_event(report, &document, document_ordinal);
    report.add_document(document);
}

fn append_step_event(
    report: &mut Report,
    path: &str,
    document_ordinal: usize,
    step_ordinal: usize,
    step: &StepReport,
) {
    let sequence = report.events.len() as u64;
    report.push_event(
        Event {
            sequence,
            kind: "step.finished".to_string(),
            document: Some(path.to_string()),
            step: Some(step.name.clone()),
            status: Some(step.status),
            message: None,
        },
        Some(EventMetadata {
            run_id: Some(report.started_at.clone()),
            document_ordinal: Some(document_ordinal),
            step_ordinal: Some(step_ordinal),
            duration_ms: Some(step.duration_ms),
            ..EventMetadata::default()
        }),
    );
}

fn append_document_event(report: &mut Report, document: &DocumentReport, document_ordinal: usize) {
    let sequence = report.events.len() as u64;
    report.push_event(
        Event {
            sequence,
            kind: "document.finished".to_string(),
            document: Some(document.path.clone()),
            step: None,
            status: Some(document.status),
            message: None,
        },
        Some(EventMetadata {
            run_id: Some(report.started_at.clone()),
            document_ordinal: Some(document_ordinal),
            duration_ms: Some(document.duration_ms),
            ..EventMetadata::default()
        }),
    );
}

fn stream_event_range(report: &Report, start: usize) -> Result<(), Box<CliError>> {
    let records = report.event_records();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    for record in records.into_iter().skip(start) {
        serde_json::to_writer(&mut output, &record).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Event serialization failed", error.to_string()),
            )
        })?;
        output.write_all(b"\n").map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Event output failed", error.to_string()),
            )
        })?;
    }
    output.flush().map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Event output failed", error.to_string()),
        )
    })?;
    Ok(())
}

fn emit_report(
    report: &Report,
    options: &CommonOptions,
    mode: Mode,
    suppress_stdout: bool,
) -> Result<(), Box<CliError>> {
    if let Some(path) = &options.report {
        write_atomic_json(path, report).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Report write failed", error.to_string()),
            )
        })?;
    }
    if let Some(path) = &options.junit {
        write_text(path, &report.junit()).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "JUnit write failed", error.to_string()),
            )
        })?;
    }
    if suppress_stdout {
        return Ok(());
    }
    if options.json {
        println!(
            "{}",
            report.json().map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error("MDOK-E800", "JSON serialization failed", error.to_string()),
                )
            })?
        );
    } else if options.json_lines {
        print!(
            "{}",
            report.json_lines().map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error("MDOK-E800", "Event serialization failed", error.to_string()),
                )
            })?
        );
    } else {
        print!(
            "{}",
            report.human(!options.no_color, options.verbose || mode == Mode::Plan)
        );
    }
    Ok(())
}

fn process_documents_parallel(
    paths: Vec<PathBuf>,
    mode: Mode,
    config: EffectiveConfig,
    options: CommonOptions,
    jobs: usize,
) -> Vec<DocumentReport> {
    let paths = Arc::new(paths);
    let slots = Arc::new(Mutex::new((0..paths.len()).collect::<Vec<usize>>()));
    let results = Arc::new(Mutex::new(vec![None; paths.len()]));
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            let slots = Arc::clone(&slots);
            let results = Arc::clone(&results);
            let paths = Arc::clone(&paths);
            let config = config.clone();
            let options = options.clone();
            scope.spawn(move || {
                loop {
                    let index = slots.lock().ok().and_then(|mut slots| slots.pop());
                    let Some(index) = index else { break };
                    let result = process_document(&paths[index], mode, &config, &options);
                    if let Ok(mut results) = results.lock() {
                        results[index] = Some(result);
                    }
                }
            });
        }
    });
    Arc::try_unwrap(results)
        .ok()
        .and_then(|results| results.into_inner().ok())
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

fn process_document(
    path: &Path,
    mode: Mode,
    config: &EffectiveConfig,
    options: &CommonOptions,
) -> DocumentReport {
    process_document_with_hook(path, mode, config, options, |_, _| {})
}

fn process_document_with_hook<F>(
    path: &Path,
    mode: Mode,
    config: &EffectiveConfig,
    options: &CommonOptions,
    mut on_step: F,
) -> DocumentReport
where
    F: FnMut(usize, &StepReport),
{
    let started = Instant::now();
    let outcome = build_plan(path, config);
    let Some(plan) = outcome.plan else {
        return DocumentReport {
            path: path.display().to_string(),
            status: if outcome.diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code.as_str(),
                    "MDOK-E302"
                        | "MDOK-E303"
                        | "MDOK-E304"
                        | "MDOK-E602"
                        | "MDOK-E603"
                        | "MDOK-E604"
                )
            }) {
                Status::Error
            } else {
                Status::Failed
            },
            duration_ms: started.elapsed().as_millis() as u64,
            steps: Vec::new(),
            diagnostics: outcome.diagnostics,
        };
    };
    let mut document = match mode {
        Mode::Test => execute_plan_with_hook(&plan, config, options, &mut on_step),
        Mode::Lint => plan_report(&plan, Status::Passed, true),
        Mode::Plan => plan_report(&plan, Status::Planned, true),
        Mode::List => plan_report(&plan, Status::Planned, false),
    };
    if mode != Mode::Test {
        for (step_ordinal, step) in document.steps.iter().enumerate() {
            on_step(step_ordinal, step);
        }
    }
    document.diagnostics.extend(outcome.diagnostics);
    document.path = path.display().to_string();
    document.duration_ms = started.elapsed().as_millis() as u64;
    document
}

fn plan_report(plan: &DocumentPlan, status: Status, include_commands: bool) -> DocumentReport {
    DocumentReport {
        path: plan.path.display().to_string(),
        status,
        duration_ms: 0,
        steps: plan
            .steps
            .iter()
            .map(|step| StepReport {
                name: step.name.clone(),
                status,
                command: if include_commands {
                    step.command.clone()
                } else {
                    Vec::new()
                },
                checks: step
                    .checks
                    .iter()
                    .map(|expression| CheckReport {
                        expression: expression.clone(),
                        status,
                        result: None,
                    })
                    .collect(),
                captures: step.captures.clone(),
                diagnostics: Vec::new(),
                duration_ms: 0,
            })
            .collect(),
        diagnostics: Vec::new(),
    }
}

fn execute_plan_with_hook<F>(
    plan: &DocumentPlan,
    config: &EffectiveConfig,
    options: &CommonOptions,
    on_step: &mut F,
) -> DocumentReport
where
    F: FnMut(usize, &StepReport),
{
    let mut variables = plan.variables.clone();
    let mut step_summaries = Map::new();
    if options.offline {
        let document = DocumentReport {
            path: plan.path.display().to_string(),
            status: Status::Error,
            duration_ms: 0,
            steps: plan
                .steps
                .iter()
                .map(|step| StepReport {
                    name: step.name.clone(),
                    status: Status::Skipped,
                    command: step.command.clone(),
                    checks: Vec::new(),
                    captures: step.captures.clone(),
                    diagnostics: vec![
                        Diagnostic::error(
                            "MDOK-E302",
                            "Offline execution denied",
                            "--offline prevents network transfers in test mode",
                        )
                        .at_file(&plan.path)
                        .at_step(step.name.clone()),
                    ],
                    duration_ms: 0,
                })
                .collect(),
            diagnostics: Vec::new(),
        };
        for (step_ordinal, step) in document.steps.iter().enumerate() {
            on_step(step_ordinal, step);
        }
        return document;
    }
    let mut steps = Vec::new();
    for step in &plan.steps {
        let started = Instant::now();
        let mut report = StepReport {
            name: step.name.clone(),
            status: Status::Passed,
            command: step.command.clone(),
            checks: Vec::new(),
            captures: step.captures.clone(),
            diagnostics: Vec::new(),
            duration_ms: 0,
        };
        let tokens = match tokenize_command(&step.raw_command) {
            Ok(tokens) => tokens,
            Err(message) => {
                report.status = Status::Failed;
                report.diagnostics.push(
                    Diagnostic::error(
                        tokenize_error_code(&message),
                        "Invalid curl syntax",
                        message,
                    )
                    .at_file(&plan.path)
                    .at_step(step.name.clone()),
                );
                report.duration_ms = started.elapsed().as_millis() as u64;
                step_summaries.insert(
                    step.name.clone(),
                    serde_json::json!({"status": report.status.as_str()}),
                );
                let step_ordinal = steps.len();
                steps.push(report);
                on_step(step_ordinal, steps.last().expect("step was just pushed"));
                continue;
            }
        };
        let rendered_command = normalize_command(
            &tokens,
            &variables,
            &plan.path,
            &mut report.diagnostics,
            false,
            false,
        );
        let mut display_diagnostics = Vec::new();
        report.command = normalize_command(
            &tokens,
            &variables,
            &plan.path,
            &mut display_diagnostics,
            true,
            true,
        );
        if !report.diagnostics.is_empty() {
            report.status = Status::Failed;
            report.duration_ms = started.elapsed().as_millis() as u64;
            step_summaries.insert(
                step.name.clone(),
                serde_json::json!({"status": report.status.as_str()}),
            );
            let step_ordinal = steps.len();
            steps.push(report);
            on_step(step_ordinal, steps.last().expect("step was just pushed"));
            if options.fail_fast || config.fail_fast {
                break;
            }
            continue;
        }
        match transfer(
            &rendered_command,
            config,
            &variables_to_value(&variables),
            &Value::Object(step_summaries.clone()),
        ) {
            Ok(context) => {
                for expression in &step.checks {
                    match evaluate_check(expression, &context) {
                        Ok(result) if result => report.checks.push(CheckReport {
                            expression: expression.clone(),
                            status: Status::Passed,
                            result: Some(Value::Bool(true)),
                        }),
                        Ok(_) => {
                            report.status = Status::Failed;
                            report.checks.push(CheckReport {
                                expression: expression.clone(),
                                status: Status::Failed,
                                result: Some(Value::Bool(false)),
                            });
                            report.diagnostics.push(
                                Diagnostic::error(
                                    "MDOK-E502",
                                    "Check failed",
                                    format!("JMESPath check evaluated to false: {expression}"),
                                )
                                .at_file(&plan.path)
                                .at_step(step.name.clone()),
                            );
                            if options.fail_fast || config.fail_fast {
                                break;
                            }
                        }
                        Err(message) => {
                            report.status = Status::Failed;
                            report.diagnostics.push(
                                Diagnostic::error("MDOK-E501", "Check evaluation failed", message)
                                    .at_file(&plan.path)
                                    .at_step(step.name.clone()),
                            );
                            if options.fail_fast || config.fail_fast {
                                break;
                            }
                        }
                    }
                }
                if report.status == Status::Passed {
                    publish_captures(
                        &step.captures,
                        &context,
                        &mut variables,
                        &plan.path,
                        &step.name,
                        &mut report.diagnostics,
                    );
                    if !report.diagnostics.is_empty() {
                        report.status = Status::Failed;
                    }
                }
            }
            Err(diagnostic) => {
                report.status = Status::Failed;
                report
                    .diagnostics
                    .push(diagnostic.at_file(&plan.path).at_step(step.name.clone()));
            }
        }
        report.duration_ms = started.elapsed().as_millis() as u64;
        let failed = report.status.is_failure();
        step_summaries.insert(
            step.name.clone(),
            serde_json::json!({
                "status": report.status.as_str(),
                "checks": report.checks.iter().map(|check| {
                    serde_json::json!({"expression": check.expression, "status": check.status.as_str()})
                }).collect::<Vec<_>>()
            }),
        );
        let step_ordinal = steps.len();
        steps.push(report);
        on_step(step_ordinal, steps.last().expect("step was just pushed"));
        if failed && (options.fail_fast || config.fail_fast) {
            break;
        }
    }
    DocumentReport {
        path: plan.path.display().to_string(),
        status: if steps.iter().any(|step| step.status.is_failure()) {
            Status::Failed
        } else {
            Status::Passed
        },
        duration_ms: 0,
        steps,
        diagnostics: Vec::new(),
    }
}

fn variables_to_value(variables: &BTreeMap<String, Variable>) -> Value {
    Value::Object(
        variables
            .iter()
            .map(|(key, variable)| (key.clone(), variable.value.clone()))
            .collect(),
    )
}

fn curl_policy(config: &EffectiveConfig) -> CurlPolicy {
    CurlPolicy {
        allowed_schemes: config.allowed_schemes.iter().cloned().collect(),
        allowed_hosts: None,
        allowed_host_patterns: config.allowed_hosts.clone(),
        denied_host_patterns: config.denied_hosts.clone(),
        allow_private_network: config.allow_private_network,
        allow_insecure_tls: config.allow_insecure_tls,
        allow_proxy: config.allow_proxy,
        allow_resolve: config.allow_resolve,
        allow_connect_to: config.allow_connect_to,
        allow_file_reads: config.allow_file_reads,
        allowed_read_roots: config.allowed_read_roots.clone(),
        allow_artifact_writes: config.allow_artifact_writes,
        allowed_artifact_roots: config.allowed_artifact_roots.clone(),
        max_body_bytes: config.max_body as u64,
        memory_body_threshold_bytes: config.memory_body_threshold,
        ..CurlPolicy::default()
    }
}

#[allow(clippy::result_large_err)]
fn transfer(
    argv: &[String],
    config: &EffectiveConfig,
    variables: &Value,
    steps: &Value,
) -> Result<Value, Diagnostic> {
    let policy = curl_policy(config);
    let mut plan = CurlPlan::parse(argv, &policy).map_err(curl_diagnostic)?;
    enforce_policy(&plan.url, config)?;
    if plan.timeout.is_none() {
        plan.timeout = Some(config.timeout);
    }
    if plan.connect_timeout.is_none() {
        plan.connect_timeout = Some(config.connect_timeout);
    }
    let response = plan.execute(&policy).map_err(curl_diagnostic)?;
    response
        .evaluation_json_limited(variables, steps, config.max_body)
        .map_err(curl_diagnostic)
}

fn curl_diagnostic(error: CurlError) -> Diagnostic {
    Diagnostic::error(error.code, "Curl transfer error", error.message)
}

fn publish_captures(
    captures: &[String],
    context: &Value,
    variables: &mut BTreeMap<String, Variable>,
    path: &Path,
    step_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic_start = diagnostics.len();
    let mut published = BTreeMap::new();
    for expression in captures {
        let normalized_expression = normalize_jmespath(expression);
        let compiled = match jmespath::compile(&normalized_expression) {
            Ok(compiled) => compiled,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("MDOK-E500", "Invalid capture", error.to_string())
                        .at_file(path)
                        .at_step(step_name.to_string()),
                );
                continue;
            }
        };
        let input = match jmespath::Variable::try_from(context.clone()) {
            Ok(input) => input,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("MDOK-E501", "Capture context error", error.to_string())
                        .at_file(path)
                        .at_step(step_name.to_string()),
                );
                continue;
            }
        };
        let result = match compiled.search(input) {
            Ok(result) => result,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("MDOK-E501", "Capture evaluation failed", error.to_string())
                        .at_file(path)
                        .at_step(step_name.to_string()),
                );
                continue;
            }
        };
        let json: Value = match serde_json::from_str(&result.to_string()) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error(
                        "MDOK-E501",
                        "Capture serialization failed",
                        error.to_string(),
                    )
                    .at_file(path)
                    .at_step(step_name.to_string()),
                );
                continue;
            }
        };
        let Some(object) = json.as_object() else {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E503",
                    "Invalid capture",
                    "capture expression must return an object",
                )
                .at_file(path)
                .at_step(step_name.to_string()),
            );
            continue;
        };
        for (key, value) in object {
            if !valid_name(key) {
                diagnostics.push(
                    Diagnostic::error(
                        "MDOK-E504",
                        "Invalid capture name",
                        format!("capture key `{key}` is not a valid variable name"),
                    )
                    .at_file(path)
                    .at_step(step_name.to_string()),
                );
                continue;
            }
            if variables.contains_key(key) || published.contains_key(key) {
                diagnostics.push(
                    Diagnostic::error(
                        "MDOK-E504",
                        "Capture collision",
                        format!("capture key `{key}` is already defined"),
                    )
                    .at_file(path)
                    .at_step(step_name.to_string()),
                );
                continue;
            }
            published.insert(
                key.clone(),
                Variable {
                    value: value.clone(),
                    secret: is_secret_name(key),
                },
            );
        }
    }
    if diagnostics.len() == diagnostic_start {
        variables.extend(published);
    }
}

fn evaluate_check(expression: &str, context: &Value) -> Result<bool, String> {
    let normalized_expression = normalize_jmespath(expression);
    let expression =
        jmespath::compile(&normalized_expression).map_err(|error| error.to_string())?;
    let variable =
        jmespath::Variable::try_from(context.clone()).map_err(|error| error.to_string())?;
    let result = expression
        .search(variable)
        .map_err(|error| error.to_string())?;
    result
        .as_boolean()
        .ok_or_else(|| "check result must be boolean".to_string())
}

fn normalize_jmespath(expression: &str) -> String {
    let mut normalized = String::with_capacity(expression.len());
    let mut chars = expression.chars().peekable();
    let mut quote = None;
    let mut escaped = false;

    while let Some(character) = chars.next() {
        if let Some(current_quote) = quote {
            normalized.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }

        if matches!(character, '\'' | '"') {
            quote = Some(character);
            normalized.push(character);
            continue;
        }

        if character != '`' {
            normalized.push(character);
            continue;
        }

        let mut literal = String::new();
        let mut closed = false;
        for value in chars.by_ref() {
            if value == '`' {
                closed = true;
                break;
            }
            literal.push(value);
        }

        if closed && is_bare_jmespath_literal(&literal) {
            normalized.push('\'');
            normalized.push_str(&literal);
            normalized.push('\'');
        } else {
            normalized.push('`');
            normalized.push_str(&literal);
            if closed {
                normalized.push('`');
            }
        }
    }

    normalized
}

fn is_bare_jmespath_literal(literal: &str) -> bool {
    let mut chars = literal.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return false;
    }

    serde_json::from_str::<Value>(literal).is_err()
}

#[allow(clippy::result_large_err)]
fn enforce_policy(url: &Url, config: &EffectiveConfig) -> Result<(), Diagnostic> {
    let scheme = url.scheme();
    if !config.allowed_schemes.is_empty()
        && !config.allowed_schemes.iter().any(|item| item == scheme)
    {
        return Err(Diagnostic::error(
            "MDOK-E302",
            "Scheme denied",
            format!("scheme `{scheme}` is not allowed by policy"),
        ));
    }
    let host = url.host_str().unwrap_or_default();
    if config
        .denied_hosts
        .iter()
        .any(|pattern| host_matches(host, pattern))
    {
        return Err(Diagnostic::error(
            "MDOK-E302",
            "Host denied",
            format!("host `{host}` is denied"),
        ));
    }
    if !config.allowed_hosts.is_empty()
        && !config
            .allowed_hosts
            .iter()
            .any(|pattern| host_matches(host, pattern))
    {
        return Err(Diagnostic::error(
            "MDOK-E302",
            "Host not allowed",
            format!("host `{host}` is not in the allowed host policy"),
        ));
    }
    Ok(())
}

fn host_matches(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    pattern == "*"
        || pattern == host
        || pattern
            .strip_prefix("*.")
            .is_some_and(|suffix| host.ends_with(&format!(".{suffix}")) || host == suffix)
}

fn build_plan(path: &Path, config: &EffectiveConfig) -> PlanOutcome {
    let source = match read_document_source(path) {
        Ok(source) => source,
        Err(error) => {
            return PlanOutcome {
                plan: None,
                diagnostics: vec![
                    Diagnostic::error("MDOK-E001", "Cannot read document", error.to_string())
                        .at_file(path),
                ],
            };
        }
    };
    // Keep the Comrak/core plan as the authoritative structural view for
    // documents that pass the adapter's compatibility checks.  The adapter
    // still performs CLI-specific template, curl-policy, and secret-taint
    // validation below, but it no longer has to be the only parser in the
    // product path.
    let authoritative = match parse(&source, path.to_path_buf()) {
        Ok(document) => match plan_document(&document) {
            Ok(plan) => Ok(plan),
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    let fences = parse_fences(&source);
    let mut diagnostics = Vec::new();
    let mut variables = config.vars.clone();
    if let Ok(core_plan) = &authoritative {
        for (key, value) in &core_plan.variables {
            variables.entry(key.clone()).or_insert_with(|| Variable {
                value: value.clone(),
                secret: is_secret_name(key),
            });
        }
    }
    let mut steps = Vec::new();
    let mut names = BTreeSet::new();
    let mut inline_names = BTreeSet::new();
    for fence in fences {
        if let Some(error) = fence.attrs.get("__metadata_error") {
            diagnostics.push(
                Diagnostic::error("MDOK-E100", "Invalid fence metadata", error).at_file(path),
            );
            continue;
        }
        match fence.language.as_str() {
            "toml" if fence.attrs.contains_key("vars") => {
                match toml::from_str::<toml::Value>(&fence.body) {
                    Ok(toml::Value::Table(table)) => {
                        for (key, value) in table {
                            if !valid_name(&key)
                                || matches!(
                                    key.as_str(),
                                    "variables"
                                        | "steps"
                                        | "environment"
                                        | "request"
                                        | "response"
                                        | "mdok"
                                )
                                || !inline_names.insert(key.clone())
                            {
                                diagnostics.push(
                                    Diagnostic::error(
                                        "MDOK-E110",
                                        "Invalid variables block",
                                        format!("invalid or duplicate variable `{key}`"),
                                    )
                                    .at_file(path),
                                );
                                continue;
                            }
                            variables.entry(key.clone()).or_insert_with(|| Variable {
                                value: toml_to_json(value),
                                secret: is_secret_name(&key),
                            });
                        }
                    }
                    Ok(_) => diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E110",
                            "Invalid variables block",
                            "the TOML root must be a table",
                        )
                        .at_file(path),
                    ),
                    Err(error) => diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E110",
                            "Invalid variables block",
                            error.to_string(),
                        )
                        .at_file(path),
                    ),
                }
            }
            "curl" => {
                let Some(name) = fence.attrs.get("name").cloned() else {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E100",
                            "Missing step name",
                            "curl fences require name=...",
                        )
                        .at_file(path),
                    );
                    continue;
                };
                if name.is_empty() {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E100",
                            "Invalid fence metadata",
                            "curl fence name cannot be empty",
                        )
                        .at_file(path),
                    );
                    continue;
                }
                if !valid_name(&name) || !names.insert(name.clone()) {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E101",
                            "Invalid step name",
                            format!("`{name}` is duplicate or invalid"),
                        )
                        .at_file(path),
                    );
                    continue;
                }
                match tokenize_command(&fence.body) {
                    Ok(tokens) => {
                        if tokens.len() > 1 && tokens.iter().skip(1).any(|token| token == "curl") {
                            diagnostics.push(
                                Diagnostic::error(
                                    "MDOK-E201",
                                    "Forbidden shell construct",
                                    "a curl fence may contain only one simple command",
                                )
                                .at_file(path),
                            );
                            continue;
                        }
                        if tokens.first().is_some_and(|token| token.contains('=')) {
                            diagnostics.push(
                                Diagnostic::error(
                                    "MDOK-E201",
                                    "Forbidden shell construct",
                                    "variable assignments are not allowed in curl fences",
                                )
                                .at_file(path),
                            );
                            continue;
                        }
                        if tokens.first().map(String::as_str) != Some("curl") {
                            diagnostics.push(
                                Diagnostic::error(
                                    "MDOK-E202",
                                    "Invalid curl command",
                                    "the first word must be curl",
                                )
                                .at_file(path),
                            );
                            continue;
                        }
                        let mut step_diagnostics = validate_command(&tokens, path, config);
                        diagnostics.append(&mut step_diagnostics);
                        steps.push(StepPlan {
                            name,
                            command: tokens,
                            raw_command: fence.body,
                            checks: Vec::new(),
                            captures: Vec::new(),
                        });
                    }
                    Err(message) => diagnostics.push(
                        Diagnostic::error(
                            tokenize_error_code(&message),
                            "Invalid curl syntax",
                            message,
                        )
                        .at_file(path),
                    ),
                }
            }
            "jmespath" => {
                if fence.attrs.contains_key("check") && fence.attrs["check"].is_empty()
                    || fence.attrs.contains_key("capture") && fence.attrs["capture"].is_empty()
                {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E100",
                            "Invalid fence metadata",
                            "JMESPath check/capture fences require a target step",
                        )
                        .at_file(path),
                    );
                    continue;
                }
                let role = if let Some(step) = fence.attrs.get("check") {
                    Some((step, false))
                } else {
                    fence.attrs.get("capture").map(|step| (step, true))
                };
                let Some((step_name, capture)) = role else {
                    continue;
                };
                let Some(step) = steps.iter_mut().find(|step| step.name == *step_name) else {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E102",
                            "Unknown step reference",
                            format!("`{step_name}` is not an earlier step"),
                        )
                        .at_file(path),
                    );
                    continue;
                };
                for expression in jmespath_expressions(&fence.body, capture) {
                    let normalized_expression = normalize_jmespath(&expression);
                    if let Err(error) = jmespath::compile(&normalized_expression) {
                        diagnostics.push(
                            Diagnostic::error("MDOK-E500", "Invalid JMESPath", error.to_string())
                                .at_file(path),
                        );
                    }
                    if capture {
                        step.captures.push(expression.to_string());
                    } else {
                        step.checks.push(expression.to_string());
                    }
                }
            }
            _ if fence
                .attrs
                .keys()
                .any(|key| matches!(key.as_str(), "vars" | "name" | "check" | "capture")) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        "MDOK-E100",
                        "Invalid fence metadata",
                        format!("metadata is not valid for `{}` fences", fence.language),
                    )
                    .at_file(path),
                )
            }
            _ => {}
        }
    }
    let capture_names = steps
        .iter()
        .flat_map(|step| {
            step.captures
                .iter()
                .flat_map(|expression| capture_keys(expression))
        })
        .collect::<BTreeSet<_>>();
    let values = variables_to_value_map(&variables);
    for step in &mut steps {
        for token in tokenize_command(&step.raw_command).unwrap_or_default() {
            validate_template_token(
                &token,
                &values,
                &variables,
                &capture_names,
                path,
                &step.name,
                &mut diagnostics,
            );
        }
        let mut ignored = Vec::new();
        step.command = normalize_command(
            &tokenize_command(&step.raw_command).unwrap_or_default(),
            &variables,
            path,
            &mut ignored,
            true,
            true,
        );
        if has_url_glob(&step.command) {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E304",
                    "URL glob denied",
                    "URL glob expansion would create multiple transfers",
                )
                .at_file(path)
                .at_step(step.name.clone()),
            );
        }
        if !step.command.iter().any(|argument| argument.contains("{{"))
            && let Err(error) = CurlPlan::parse(&step.command, &curl_policy(config))
        {
            diagnostics.push(
                curl_diagnostic(error)
                    .at_file(path)
                    .at_step(step.name.clone()),
            );
        }
    }
    let mut plan = DocumentPlan {
        path: path.to_path_buf(),
        steps,
        variables,
    };
    if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        && let Err(error) = &authoritative
    {
        diagnostics.push(markdown_diagnostic(error, path));
    }
    if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        && let Ok(core_plan) = authoritative
    {
        let mut by_name = plan
            .steps
            .iter()
            .map(|step| (step.name.clone(), step.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut authoritative_steps = Vec::with_capacity(core_plan.steps.len());
        for core_step in core_plan.steps {
            let Some(mut step) = by_name.remove(core_step.name.as_str()) else {
                continue;
            };
            step.raw_command = core_step.curl.source;
            step.checks = core_step
                .checks
                .into_iter()
                .map(|check| check.expression)
                .collect();
            step.captures = core_step
                .captures
                .into_iter()
                .map(|capture| capture.expression)
                .collect();
            authoritative_steps.push(step);
        }
        if authoritative_steps.len() == plan.steps.len() {
            plan.steps = authoritative_steps;
        }
    }
    for step in &plan.steps {
        for url in positional_args(&step.command) {
            if let Ok(url) = Url::parse(url)
                && let Err(diagnostic) = enforce_policy(&url, config)
            {
                diagnostics.push(diagnostic.at_file(path).at_step(step.name.clone()));
            }
        }
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        PlanOutcome {
            plan: None,
            diagnostics,
        }
    } else {
        PlanOutcome {
            plan: Some(plan),
            diagnostics,
        }
    }
}

fn read_document_source(path: &Path) -> Result<String, std::io::Error> {
    if path == Path::new("-") {
        let mut source = String::new();
        std::io::stdin().read_to_string(&mut source)?;
        Ok(source)
    } else {
        fs::read_to_string(path)
    }
}

fn markdown_diagnostic(error: &MarkdownError, path: &Path) -> Diagnostic {
    Diagnostic::error(error.code(), "Markdown planning error", error.to_string()).at_file(path)
}

fn jmespath_expressions(body: &str, capture: bool) -> Vec<String> {
    if capture {
        let expression = body.trim();
        if expression.is_empty() {
            Vec::new()
        } else {
            vec![expression.to_string()]
        }
    } else {
        body.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn validate_command(tokens: &[String], path: &Path, config: &EffectiveConfig) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let forbidden = [
        "--parallel",
        "--output",
        "-o",
        "--remote-name",
        "-O",
        "--trace",
        "--write-out",
        "--libcurl",
    ];
    for (index, token) in tokens.iter().enumerate().skip(1) {
        if forbidden.contains(&token.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E301",
                    "Curl option denied",
                    format!("{token} is not allowed in MDOK v1"),
                )
                .at_file(path),
            );
        }
        if matches!(token.as_str(), "--proxy" | "-x") && !config.allow_proxy {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E604",
                    "Proxy denied",
                    "proxy use is disabled by policy",
                )
                .at_file(path),
            );
        }
        if token == "--unix-socket" {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E303",
                    "Unix socket denied",
                    "unix socket transfers are disabled by policy",
                )
                .at_file(path),
            );
        }
        if token == "--config" || token == "-K" {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E303",
                    "Curl config denied",
                    "nested curl configuration files are not allowed",
                )
                .at_file(path),
            );
        }
        if matches!(
            token.as_str(),
            "-d" | "--data" | "--data-raw" | "--data-binary"
        ) && tokens.get(index + 1).is_some_and(|value| value == "@-")
        {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E301",
                    "Stdin body denied",
                    "stdin cannot be used as a request body",
                )
                .at_file(path),
            );
        }
        if matches!(token.as_str(), "-H" | "--header")
            && tokens
                .get(index + 1)
                .is_some_and(|value| value.starts_with('@'))
        {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E303",
                    "Header file denied",
                    "header files are not supported outside the configured read roots",
                )
                .at_file(path),
            );
        }
    }
    let urls = positional_args(tokens).len();
    if urls > 1 {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E304",
                "Multiple transfers",
                "one curl fence may contain only one URL",
            )
            .at_file(path),
        );
    }
    diagnostics
}

fn has_url_glob(tokens: &[String]) -> bool {
    positional_args(tokens)
        .iter()
        .any(|value| value.contains(['[', ']']))
}

fn normalize_command(
    tokens: &[String],
    variables: &BTreeMap<String, Variable>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    redact_secrets: bool,
    preserve_missing: bool,
) -> Vec<String> {
    tokens
        .iter()
        .map(|token| {
            render_templates(
                token,
                variables,
                path,
                diagnostics,
                redact_secrets,
                preserve_missing,
            )
        })
        .collect()
}

fn positional_args(tokens: &[String]) -> Vec<&String> {
    let takes_value = [
        "-X",
        "--request",
        "-H",
        "--header",
        "-d",
        "--data",
        "--data-raw",
        "--data-binary",
        "--data-urlencode",
        "--json",
        "--proxy",
        "-x",
        "--output",
        "-o",
        "--trace",
        "--write-out",
        "--libcurl",
        "--unix-socket",
        "--upload-file",
        "--form",
        "-F",
        "--user",
        "-u",
        "--oauth2-bearer",
        "--max-redirs",
        "--retry",
        "--retry-delay",
        "--retry-max-time",
        "--connect-timeout",
        "--max-time",
        "--cookie",
        "-b",
        "--cookie-jar",
        "-c",
        "--cacert",
        "--cert",
        "--key",
        "--resolve",
        "--connect-to",
        "--user-agent",
        "-A",
        "--referer",
        "-e",
        "--range",
        "-r",
        "--config",
        "-K",
        "--url",
    ];
    let mut values = Vec::new();
    let mut index = 1;
    while index < tokens.len() {
        let token = &tokens[index];
        let option = token
            .split_once('=')
            .map_or(token.as_str(), |(name, _)| name);
        if takes_value.contains(&option) {
            if option == "--url" && token.contains('=') {
                values.push(token);
                index += 1;
            } else {
                index += usize::from(!token.contains('=')) + 1;
            }
        } else if tokens[index].starts_with('-') {
            index += 1;
        } else {
            values.push(&tokens[index]);
            index += 1;
        }
    }
    values
}

fn render_templates(
    input: &str,
    variables: &BTreeMap<String, Variable>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    redact_secrets: bool,
    preserve_missing: bool,
) -> String {
    let parsed = match Template::parse(input) {
        Ok(template) => template,
        Err(error) => {
            push_template_error(error, path, diagnostics);
            return "[INVALID_TEMPLATE]".to_string();
        }
    };
    let values = variables_to_value_map(variables);
    let mut output = String::new();
    for part in &parsed.parts {
        match part {
            TemplatePart::Literal(value) => output.push_str(value),
            TemplatePart::Expression(expression) => {
                let root = template_root(expression);
                let Some(variable) = variables.get(root) else {
                    if preserve_missing {
                        output.push_str(&format_expression(expression));
                    } else {
                        diagnostics.push(
                            Diagnostic::error(
                                "MDOK-E401",
                                "Missing variable",
                                format!("variable `{root}` is not defined"),
                            )
                            .at_file(path),
                        );
                        output.push_str("[MISSING_VARIABLE]");
                    }
                    continue;
                };
                match lookup_template(&values, &expression.path).and_then(|value| {
                    render_expression(expression, &values).map(|rendered| (value, rendered))
                }) {
                    Ok((_, _rendered)) if variable.secret && redact_secrets => {
                        output.push_str("[REDACTED]");
                    }
                    Ok((_, rendered)) => output.push_str(&rendered),
                    Err(error) => {
                        push_template_error(error, path, diagnostics);
                        output.push_str("[INVALID_TEMPLATE]");
                    }
                }
            }
        }
    }
    output
}

fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("password")
        || name.contains("secret")
        || name.contains("token")
        || name.contains("api_key")
        || name.contains("apikey")
}

fn variables_to_value_map(variables: &BTreeMap<String, Variable>) -> BTreeMap<String, Value> {
    variables
        .iter()
        .map(|(key, variable)| (key.clone(), variable.value.clone()))
        .collect()
}

fn template_root(expression: &TemplateExpression) -> &str {
    match expression.path.first() {
        Some(PathPart::Key(key)) => key,
        _ => "",
    }
}

fn format_expression(expression: &TemplateExpression) -> String {
    let mut value = String::from("{{");
    for (index, part) in expression.path.iter().enumerate() {
        match part {
            PathPart::Key(key) if index == 0 => value.push_str(key),
            PathPart::Key(key) => {
                value.push('.');
                value.push_str(key);
            }
            PathPart::Index(index) => value.push_str(&format!("[{index}]")),
        }
    }
    if expression.filter != Filter::String {
        value.push('|');
        value.push_str(expression.filter.as_str());
    }
    value.push_str("}}");
    value
}

fn push_template_error(error: TemplateError, path: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let (code, title) = match &error {
        TemplateError::Syntax(_) => ("MDOK-E400", "Invalid template"),
        TemplateError::MissingVariable(_) => ("MDOK-E401", "Missing variable"),
        TemplateError::Type(_) => ("MDOK-E402", "Template type error"),
        TemplateError::UnsafeHeader => ("MDOK-E403", "Unsafe header value"),
    };
    diagnostics.push(Diagnostic::error(code, title, error.to_string()).at_file(path));
}

fn validate_template_token(
    token: &str,
    values: &BTreeMap<String, Value>,
    variables: &BTreeMap<String, Variable>,
    capture_names: &BTreeSet<String>,
    path: &Path,
    step: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parsed = match Template::parse(token) {
        Ok(template) => template,
        Err(error) => {
            let before = diagnostics.len();
            push_template_error(error, path, diagnostics);
            if diagnostics.len() > before
                && let Some(last) = diagnostics.last_mut()
            {
                last.step = Some(step.to_string());
            }
            return;
        }
    };
    for expression in parsed.expressions() {
        let root = template_root(expression);
        if root.is_empty() {
            continue;
        }
        if capture_names.contains(root) && !variables.contains_key(root) {
            continue;
        }
        if !variables.contains_key(root) {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E401",
                    "Missing variable",
                    format!("variable `{root}` is not defined"),
                )
                .at_file(path)
                .at_step(step.to_string()),
            );
            continue;
        }
        if let Err(error) = lookup_template(values, &expression.path)
            .and_then(|_| render_expression(expression, values))
        {
            let before = diagnostics.len();
            push_template_error(error, path, diagnostics);
            if diagnostics.len() > before
                && let Some(last) = diagnostics.last_mut()
            {
                last.step = Some(step.to_string());
            }
        }
    }
}

fn capture_keys(expression: &str) -> Vec<String> {
    let Some(inner) = expression
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|part| part.split_once(':').map(|(key, _)| key.trim().to_string()))
        .filter(|key| valid_name(key))
        .collect()
}

fn tokenize_command(source: &str) -> Result<Vec<String>, String> {
    let source = source.trim_end_matches(['\r', '\n']);
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut token_started = false;
    let mut quote = None;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if quote.is_none() && character == '{' && chars.peek() == Some(&'{') {
            token.push(character);
            token.push(chars.next().ok_or("unclosed template expression")?);
            token_started = true;
            let mut closed = false;
            while let Some(value) = chars.next() {
                if matches!(value, '\n' | '\r') {
                    return Err("unescaped newline command separator is not allowed".to_string());
                }
                token.push(value);
                if value == '}' && chars.peek() == Some(&'}') {
                    token.push(chars.next().ok_or("unclosed template expression")?);
                    closed = true;
                    break;
                }
            }
            if !closed {
                return Err("unclosed template expression".to_string());
            }
            continue;
        }
        match (quote, character) {
            (Some(current), value) if value == current => {
                quote = None;
            }
            (Some('\''), value) => {
                token.push(value);
                token_started = true;
            }
            (Some(_), '\\') => {
                token.push(chars.next().ok_or("trailing escape")?);
                token_started = true;
            }
            (Some('"'), '$' | '`') => {
                return Err("shell expansion is not allowed".to_string());
            }
            (Some(_), value) => {
                token.push(value);
                token_started = true;
            }
            (None, '\'' | '"') => {
                quote = Some(character);
                token_started = true;
            }
            (None, '\\') => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                } else {
                    token.push(chars.next().ok_or("trailing escape")?);
                    token_started = true;
                }
            }
            (None, '\n' | '\r') => {
                return Err("unescaped newline command separator is not allowed".to_string());
            }
            (None, value) if value.is_whitespace() => {
                if token_started {
                    tokens.push(std::mem::take(&mut token));
                    token_started = false;
                }
            }
            (
                None,
                '|' | ';' | '&' | '`' | '<' | '>' | '$' | '(' | ')' | '{' | '}' | '*' | '?' | '['
                | ']',
            ) => {
                return Err(format!("shell operator `{character}` is not allowed"));
            }
            (None, value) => {
                token.push(value);
                token_started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unclosed shell quote".to_string());
    }
    if token_started {
        tokens.push(token);
    }
    Ok(tokens)
}

fn tokenize_error_code(message: &str) -> &'static str {
    if message.contains("shell") || message.contains("newline") {
        "MDOK-E201"
    } else {
        "MDOK-E200"
    }
}

fn parse_fences(source: &str) -> Vec<Fence> {
    let mut fences = Vec::new();
    let mut lines = source.split_inclusive('\n').enumerate();
    while let Some((index, raw_line)) = lines.next() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        let Some(info) = trimmed.strip_prefix("```") else {
            continue;
        };
        if info.trim().is_empty() {
            continue;
        }
        let mut body = String::new();
        for (_, raw_body) in lines.by_ref() {
            let body_line = raw_body.trim_end_matches(['\r', '\n']);
            if body_line.trim() == "```" {
                break;
            }
            body.push_str(body_line);
            body.push('\n');
        }
        let mut attrs = BTreeMap::new();
        let mut parts = match split_info(info.trim()) {
            Ok(parts) => parts.into_iter(),
            Err(error) => {
                attrs.insert("__metadata_error".to_string(), error);
                Vec::new().into_iter()
            }
        };
        let language = parts.next().unwrap_or_default().to_string();
        if attrs.contains_key("__metadata_error") {
            fences.push(Fence {
                language,
                attrs,
                body,
            });
            continue;
        }
        if parts.next().as_deref() != Some("mdok") {
            continue;
        }
        for part in parts {
            if let Some((key, value)) = part.split_once('=') {
                if attrs.contains_key(key) {
                    attrs.insert(
                        "__metadata_error".to_string(),
                        format!("duplicate metadata attribute `{key}`"),
                    );
                    break;
                }
                attrs.insert(key.to_string(), value.to_string());
            } else if part == "vars" {
                attrs.insert(part, String::new());
            } else {
                attrs.insert(
                    "__metadata_error".to_string(),
                    format!("unknown metadata flag `{part}`"),
                );
                break;
            }
        }
        if attrs.contains_key("check") && attrs.contains_key("capture") {
            attrs.insert(
                "__metadata_error".to_string(),
                "check and capture roles are mutually exclusive".to_string(),
            );
        }
        let _ = index;
        fences.push(Fence {
            language,
            attrs,
            body,
        });
    }
    fences
}

fn split_info(info: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in info.chars() {
        match (quote, character) {
            (Some(current_quote), value) if value == current_quote => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            (_, value) => current.push(value),
        }
    }
    if quote.is_some() {
        return Err("unterminated metadata quote".to_string());
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Ok(parts)
}

fn valid_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(value) if value.is_ascii_alphabetic())
        && value.len() <= 64
        && chars.all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '-'))
}

fn toml_to_json(value: toml::Value) -> Value {
    match value {
        toml::Value::String(value) => Value::String(value),
        toml::Value::Integer(value) => Value::Number(value.into()),
        toml::Value::Float(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(value) => Value::Bool(value),
        toml::Value::Datetime(value) => Value::String(value.to_string()),
        toml::Value::Array(values) => Value::Array(values.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, toml_to_json(value)))
                .collect(),
        ),
    }
}

fn discover_paths(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, Box<CliError>> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    let mut add_path = |path: PathBuf| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };
    for input in inputs {
        if input == Path::new("-") {
            add_path(input.clone());
        } else if input.is_file() {
            // Explicit files are always honored, even when a parent
            // .gitignore would exclude them.
            add_path(input.clone());
        } else if input.is_dir() {
            let mut discovered = Vec::new();
            for entry in WalkDir::new(input)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    if entry.path() == input {
                        return true;
                    }
                    if entry.file_type().is_dir() && ignored_by_gitignore(entry.path(), input) {
                        return false;
                    }
                    let name = entry.file_name().to_string_lossy();
                    !entry.file_type().is_dir()
                        || (!name.starts_with('.')
                            && !matches!(
                                name.as_ref(),
                                "target" | ".git" | "node_modules" | "vendor"
                            ))
                })
            {
                let entry = entry.map_err(|error| {
                    cli_error(
                        EXIT_INPUT,
                        Diagnostic::error("MDOK-E001", "Discovery failed", error.to_string()),
                    )
                })?;
                if entry.file_type().is_file() && !ignored_by_gitignore(entry.path(), input) {
                    let name = entry.file_name().to_string_lossy();
                    if name.ends_with(".md") || name.ends_with(".mdok.md") {
                        discovered.push(entry.path().to_path_buf());
                    }
                }
            }
            discovered.sort();
            for path in discovered {
                add_path(path);
            }
        } else {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Input not found", input.display().to_string()),
            ));
        }
    }
    if paths.is_empty() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "No Markdown documents",
                "the selected paths contain no .md files",
            ),
        ));
    }
    Ok(paths)
}

#[derive(Debug, Clone)]
struct GitIgnoreRule {
    negated: bool,
    directory_only: bool,
    pattern: String,
}

fn ignored_by_gitignore(path: &Path, root: &Path) -> bool {
    if path == root || !path.starts_with(root) {
        return false;
    }

    // An ignored directory prevents Git from visiting anything below it. The
    // ancestor check preserves that rule even when a later negation matches a
    // descendant file.
    let mut ancestor = path.parent();
    while let Some(directory) = ancestor {
        if directory == root {
            break;
        }
        if ignored_path_state(directory, root, true) {
            return true;
        }
        ancestor = directory.parent();
    }
    ignored_path_state(path, root, false)
}

fn ignored_path_state(path: &Path, root: &Path, is_directory: bool) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let mut directories = Vec::new();
    let mut current = Some(parent);
    while let Some(directory) = current {
        directories.push(directory.to_path_buf());
        if directory == root {
            break;
        }
        current = directory.parent();
    }
    directories.reverse();

    let mut ignored = false;
    for directory in directories {
        let ignore_file = directory.join(".gitignore");
        let Ok(contents) = fs::read_to_string(ignore_file) else {
            continue;
        };
        let relative = path
            .strip_prefix(&directory)
            .unwrap_or(path)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        for rule in parse_gitignore(&contents) {
            if gitignore_rule_matches(&rule, &relative, is_directory) {
                ignored = !rule.negated;
            }
        }
    }
    ignored
}

fn parse_gitignore(contents: &str) -> Vec<GitIgnoreRule> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                return None;
            }
            let escaped_comment = line.starts_with(r"\#");
            let line = if escaped_comment { &line[1..] } else { line };
            if !escaped_comment && line.starts_with('#') {
                return None;
            }
            let escaped_negation = line.starts_with(r"\!");
            let negated = !escaped_negation && line.starts_with('!');
            let line = if escaped_negation { &line[1..] } else { line };
            let line = if negated { &line[1..] } else { line };
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let directory_only = line.ends_with('/');
            let pattern = line
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string();
            (!pattern.is_empty()).then_some(GitIgnoreRule {
                negated,
                directory_only,
                pattern,
            })
        })
        .collect()
}

fn gitignore_rule_matches(rule: &GitIgnoreRule, relative: &str, is_directory: bool) -> bool {
    if rule.directory_only && !is_directory {
        return false;
    }
    if rule.pattern.contains('/') {
        glob_match(&rule.pattern, relative)
    } else {
        relative
            .split('/')
            .any(|component| glob_match(&rule.pattern, component))
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    fn matches(
        pattern: &[u8],
        value: &[u8],
        pattern_index: usize,
        value_index: usize,
        memo: &mut [Option<bool>],
    ) -> bool {
        let slot = pattern_index * (value.len() + 1) + value_index;
        if let Some(result) = memo[slot] {
            return result;
        }
        let result = if pattern_index == pattern.len() {
            value_index == value.len()
        } else if pattern[pattern_index] == b'*' {
            let double_star =
                pattern_index + 1 < pattern.len() && pattern[pattern_index + 1] == b'*';
            if double_star {
                let mut end = pattern_index + 2;
                while end < pattern.len() && pattern[end] == b'*' {
                    end += 1;
                }
                if end < pattern.len() && pattern[end] == b'/' {
                    // `**/` may consume zero or more complete path segments.
                    matches(pattern, value, end + 1, value_index, memo)
                        || (value_index..value.len()).any(|index| {
                            value[index] == b'/'
                                && matches(pattern, value, end + 1, index + 1, memo)
                        })
                } else {
                    matches(pattern, value, end, value_index, memo)
                        || value_index < value.len()
                            && matches(pattern, value, pattern_index, value_index + 1, memo)
                }
            } else {
                matches(pattern, value, pattern_index + 1, value_index, memo)
                    || (value_index < value.len()
                        && value[value_index] != b'/'
                        && matches(pattern, value, pattern_index, value_index + 1, memo))
            }
        } else if pattern[pattern_index] == b'?' {
            value_index < value.len()
                && value[value_index] != b'/'
                && matches(pattern, value, pattern_index + 1, value_index + 1, memo)
        } else if pattern[pattern_index] == b'[' {
            if let Some((end, matched)) = class_match(pattern, value, pattern_index, value_index) {
                matched && matches(pattern, value, end, value_index + 1, memo)
            } else {
                value_index < value.len()
                    && pattern[pattern_index] == value[value_index]
                    && matches(pattern, value, pattern_index + 1, value_index + 1, memo)
            }
        } else {
            value_index < value.len()
                && pattern[pattern_index] == value[value_index]
                && matches(pattern, value, pattern_index + 1, value_index + 1, memo)
        };
        memo[slot] = Some(result);
        result
    }

    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut memo = vec![None; (pattern.len() + 1) * (value.len() + 1)];
    matches(pattern, value, 0, 0, &mut memo)
}

fn class_match(
    pattern: &[u8],
    value: &[u8],
    start: usize,
    value_index: usize,
) -> Option<(usize, bool)> {
    if value_index >= value.len() || value[value_index] == b'/' {
        return None;
    }
    let mut index = start + 1;
    let negated = matches!(pattern.get(index), Some(b'!' | b'^'));
    if negated {
        index += 1;
    }
    let mut matched = false;
    let mut has_item = false;
    while index < pattern.len() && pattern[index] != b']' {
        has_item = true;
        let first = pattern[index];
        if index + 2 < pattern.len() && pattern[index + 1] == b'-' && pattern[index + 2] != b']' {
            matched |= (first..=pattern[index + 2]).contains(&value[value_index]);
            index += 3;
        } else {
            matched |= first == value[value_index];
            index += 1;
        }
    }
    (has_item && index < pattern.len())
        .then_some((index + 1, if negated { !matched } else { matched }))
}

fn load_config(
    paths: &[PathBuf],
    options: &CommonOptions,
) -> Result<EffectiveConfig, Box<CliError>> {
    let config_path = if let Some(path) = &options.config {
        Some(path.clone())
    } else {
        find_config(&paths[0])
    };
    let config_root = config_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let file = if let Some(path) = config_path {
        let text = fs::read_to_string(&path).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Cannot read config", error.to_string()),
            )
        })?;
        toml::from_str::<FileConfig>(&text).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Invalid config", error.to_string()),
            )
        })?
    } else {
        FileConfig::default()
    };
    if let Some(language) = &file.language
        && language != "1"
    {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Unsupported language version",
                format!("MDOK language version `{language}` is not supported; expected `1`"),
            ),
        ));
    }
    if let Some(curl_compat) = &file.curl_compat
        && curl_compat != mdok_report::CURL_COMPAT_VERSION
    {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Unsupported curl compatibility",
                format!(
                    "curl compatibility `{curl_compat}` is not supported; expected `{}`",
                    mdok_report::CURL_COMPAT_VERSION
                ),
            ),
        ));
    }
    let profile = options.env.as_ref().and_then(|name| file.env.get(name));
    if options.env.is_some() && profile.is_none() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Unknown environment",
                "selected --env profile is not defined",
            ),
        ));
    }
    let mut vars = BTreeMap::new();
    for (key, value) in file.vars {
        vars.insert(
            key.clone(),
            Variable {
                value: toml_to_json(value),
                secret: is_secret_name(&key),
            },
        );
    }
    if let Some(profile) = profile {
        for (key, value) in &profile.vars {
            vars.insert(
                key.clone(),
                Variable {
                    value: toml_to_json(value.clone()),
                    secret: is_secret_name(key),
                },
            );
        }
        for (key, spec) in &profile.secrets {
            let value = resolve_secret_spec(spec)?;
            vars.insert(
                key.clone(),
                Variable {
                    value: Value::String(value),
                    secret: true,
                },
            );
        }
    }
    for entry in &options.var {
        let (key, value) = parse_assignment(entry)?;
        vars.insert(
            key,
            Variable {
                value: Value::String(value),
                secret: false,
            },
        );
    }
    for entry in &options.secret {
        let (key, value) = parse_assignment(entry)?;
        vars.insert(
            key,
            Variable {
                value: Value::String(resolve_cli_secret(&value)?),
                secret: true,
            },
        );
    }
    let timeout = options
        .timeout
        .as_deref()
        .or(file.execution.total_timeout.as_deref())
        .map(parse_duration)
        .transpose()?
        .unwrap_or(Duration::from_secs(30));
    let connect_timeout = file
        .execution
        .connect_timeout
        .as_deref()
        .map(parse_duration)
        .transpose()?
        .unwrap_or(Duration::from_secs(5));
    let max_body = options
        .max_body
        .or(file.execution.max_body_bytes)
        .unwrap_or(8 * 1024 * 1024);
    let memory_body_threshold = file
        .execution
        .memory_body_threshold_bytes
        .unwrap_or(256 * 1024)
        .min(max_body);
    let allowed_read_roots = file
        .policy
        .allowed_read_paths
        .iter()
        .map(|path| config_root.join(path.trim_end_matches("/**")))
        .collect::<Vec<_>>();
    let allowed_artifact_roots = file
        .policy
        .allowed_write_paths
        .iter()
        .map(|path| config_root.join(path.trim_end_matches("/**")))
        .collect::<Vec<_>>();
    let allow_private_network = file.policy.allow_private_network
        || file
            .policy
            .allowed_hosts
            .iter()
            .any(|host| matches!(host.as_str(), "*" | "localhost" | "127.0.0.1" | "::1"));
    Ok(EffectiveConfig {
        vars,
        allowed_hosts: options
            .allow_host
            .clone()
            .into_iter()
            .chain(file.policy.allowed_hosts)
            .collect(),
        denied_hosts: options.deny_host.clone(),
        allowed_schemes: if !file.execution.allowed_schemes.is_empty() {
            file.execution.allowed_schemes
        } else if file.policy.allowed_schemes.is_empty() {
            vec!["http".to_string(), "https".to_string()]
        } else {
            file.policy.allowed_schemes
        },
        allow_proxy: file.policy.allow_proxy,
        allow_insecure_tls: file.policy.allow_insecure_tls,
        allow_resolve: file.policy.allow_resolve,
        allow_connect_to: file.policy.allow_connect_to,
        allow_private_network,
        allow_file_reads: !allowed_read_roots.is_empty(),
        allowed_read_roots,
        allow_artifact_writes: !allowed_artifact_roots.is_empty(),
        allowed_artifact_roots,
        memory_body_threshold,
        connect_timeout,
        jobs: if options.jobs == 1 {
            file.execution.jobs.max(1)
        } else {
            options.jobs
        },
        fail_fast: options.fail_fast || file.execution.fail_fast,
        timeout,
        max_body,
    })
}

fn find_config(path: &Path) -> Option<PathBuf> {
    let mut directory = if path == Path::new("-") {
        std::env::current_dir().ok()?
    } else if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    loop {
        let candidate = directory.join("mdok.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !directory.pop() {
            break;
        }
    }
    None
}

fn parse_assignment(input: &str) -> Result<(String, String), Box<CliError>> {
    let Some((key, value)) = input.split_once('=') else {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid assignment", "expected KEY=VALUE"),
        ));
    };
    if !valid_name(key) {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid variable name", key.to_string()),
        ));
    }
    Ok((key.to_string(), value.to_string()))
}

fn resolve_secret_spec(spec: &SecretSpec) -> Result<String, Box<CliError>> {
    match spec {
        SecretSpec::Source { from_env } => std::env::var(from_env).map_err(|_| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E001",
                    "Secret unavailable",
                    format!("environment variable `{from_env}` is not set"),
                ),
            )
        }),
        SecretSpec::Value(value) => Ok(value.clone()),
    }
}

fn resolve_cli_secret(value: &str) -> Result<String, Box<CliError>> {
    if let Some(name) = value.strip_prefix("@env:") {
        std::env::var(name).map_err(|_| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E001",
                    "Secret unavailable",
                    format!("environment variable `{name}` is not set"),
                ),
            )
        })
    } else if let Some(path) = value.strip_prefix("@file:") {
        fs::read_to_string(path)
            .map(|value| value.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|error| {
                cli_error(
                    EXIT_INPUT,
                    Diagnostic::error("MDOK-E001", "Secret file unavailable", error.to_string()),
                )
            })
    } else if value == "@prompt" {
        Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Interactive secret unavailable",
                "@prompt is not supported by this non-interactive adapter",
            ),
        ))
    } else {
        Ok(value.to_string())
    }
}

fn parse_duration(value: &str) -> Result<Duration, Box<CliError>> {
    let (number, unit) = value.trim().split_at(
        value
            .trim()
            .find(|character: char| !character.is_ascii_digit() && character != '.')
            .unwrap_or(value.trim().len()),
    );
    let number = number.parse::<f64>().map_err(|_| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid duration", value.to_string()),
        )
    })?;
    let seconds = match unit {
        "ms" => number / 1000.0,
        "s" | "" => number,
        "m" => number * 60.0,
        "h" => number * 3600.0,
        _ => {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Invalid duration", value.to_string()),
            ));
        }
    };
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid duration", value.to_string()),
        ));
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn write_text(path: &Path, text: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn cli_error(code: u8, diagnostic: Diagnostic) -> Box<CliError> {
    Box::new(CliError { code, diagnostic })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_fence_is_one_expression_but_checks_are_line_oriented() {
        let body = "{\n  id: body.id,\n  name: body.name\n}\n";
        assert_eq!(
            jmespath_expressions(body, true),
            vec!["{\n  id: body.id,\n  name: body.name\n}".to_string()]
        );
        assert_eq!(
            jmespath_expressions("status == `200`\nlength(body.items) > `0`\n", false),
            vec!["status == `200`", "length(body.items) > `0`"]
        );
    }

    #[test]
    fn planning_failures_use_input_exit_code_and_execution_failures_use_check_code() {
        let mut planning = Report::now();
        planning.add_document(DocumentReport {
            path: "plan.md".to_string(),
            status: Status::Failed,
            duration_ms: 0,
            steps: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                "MDOK-E500",
                "Invalid JMESPath",
                "invalid expression",
            )],
        });
        assert_eq!(report_exit_code(&planning), EXIT_INPUT);

        let mut execution = Report::now();
        execution.add_document(DocumentReport {
            path: "run.md".to_string(),
            status: Status::Failed,
            duration_ms: 0,
            steps: vec![StepReport {
                name: "request".to_string(),
                status: Status::Failed,
                diagnostics: vec![Diagnostic::error(
                    "MDOK-E501",
                    "Transfer failed",
                    "connection refused",
                )],
                ..StepReport::default()
            }],
            diagnostics: Vec::new(),
        });
        assert_eq!(report_exit_code(&execution), EXIT_CHECK_FAILED);
    }

    #[test]
    fn glob_match_distinguishes_single_segment_and_double_star_patterns() {
        assert!(glob_match("a/**/b.md", "a/b.md"));
        assert!(glob_match("a/**/b.md", "a/one/two/b.md"));
        assert!(!glob_match("a/*/b.md", "a/one/two/b.md"));
        assert!(glob_match("file[0-2].md", "file1.md"));
        assert!(!glob_match("file[0-2].md", "file3.md"));
    }

    #[test]
    fn discovery_honors_gitignore_negation_directory_rules_and_explicit_roots() {
        let root = std::env::temp_dir().join(format!(
            "mdok-cli-discovery-{}-gitignore",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("generated/nested")).unwrap();
        fs::create_dir_all(root.join("important/generated")).unwrap();
        fs::create_dir_all(root.join("docs/nested")).unwrap();
        fs::create_dir_all(root.join("blocked")).unwrap();
        fs::create_dir_all(root.join("vendor")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(
            root.join(".gitignore"),
            "*.md\n!keep.md\nblocked/\n!blocked/keep.md\n**/generated/\n!important/generated/\n!important/generated/keep.md\n/docs/*.md\n!docs/nested/keep.md\nvendor/\n",
        )
        .unwrap();
        fs::write(root.join("keep.md"), "# keep").unwrap();
        fs::write(root.join("generated/drop.md"), "# drop").unwrap();
        fs::write(root.join("generated/nested/drop.md"), "# drop").unwrap();
        fs::write(root.join("important/generated/keep.md"), "# keep").unwrap();
        fs::write(root.join("blocked/keep.md"), "# blocked").unwrap();
        fs::write(root.join("docs/top.md"), "# drop").unwrap();
        fs::write(root.join("docs/nested/keep.md"), "# keep").unwrap();
        fs::write(root.join("docs/nested/drop.md"), "# drop").unwrap();
        fs::write(root.join("vendor/visible.md"), "# vendor").unwrap();
        fs::write(root.join(".hidden/hidden.md"), "# hidden").unwrap();

        let discovered = discover_paths(std::slice::from_ref(&root)).unwrap();
        let mut relative = discovered
            .iter()
            .map(|path| path.strip_prefix(&root).unwrap().display().to_string())
            .collect::<Vec<_>>();
        relative.sort();
        assert_eq!(
            relative,
            vec![
                "docs/nested/keep.md",
                "important/generated/keep.md",
                "keep.md",
            ]
        );

        // The selected root is a discovery boundary: an outer .gitignore and
        // the default vendor/hidden-directory skips do not suppress it.
        assert_eq!(
            discover_paths(&[root.join("vendor")]).unwrap(),
            vec![root.join("vendor/visible.md")]
        );
        assert_eq!(
            discover_paths(&[root.join(".hidden")]).unwrap(),
            vec![root.join(".hidden/hidden.md")]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
