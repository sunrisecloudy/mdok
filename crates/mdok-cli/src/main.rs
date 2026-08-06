//! `mdok` command line adapter.
//!
//! Rust owns Markdown planning, typed templates, checks, captures, policy, and
//! reporting. Native vendored libcurl is used for the conservative plain-GET
//! fast path; the compatibility adapter remains the fallback for the broader
//! supported option surface.

#![forbid(unsafe_code)]

mod transient;

use base64::Engine as _;
use clap::{Args, Parser, Subcommand};
use mdok_command::{CommandPolicy, ProcessOutput, run as run_external_command};
use mdok_curl::{BodyArtifact, CurlError, CurlPlan, CurlPolicy, ExecutionSession};
use mdok_markdown::{
    MAX_EXECUTABLE_BLOCKS, MAX_FENCE_BODY_BYTES, MAX_FENCES, MAX_SOURCE_BYTES, MAX_STEPS,
    MarkdownError, parse, plan_document,
};
use mdok_report::{
    CheckReport, Diagnostic, DocumentReport, Event, EventMetadata, ExternalExecutionResult,
    Redactor, Report, Severity, Status, StepExecutionMetadata, StepKind as ReportStepKind,
    StepReport, write_atomic_json,
};
use mdok_template::{
    Filter, PathPart, Template, TemplateError, TemplateExpression, TemplatePart,
    lookup as lookup_template, render_expression_with_limit,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use url::Url;
use walkdir::WalkDir;

const EXIT_OK: u8 = 0;
const EXIT_CHECK_FAILED: u8 = 1;
const EXIT_INPUT: u8 = 2;
const EXIT_POLICY: u8 = 3;
const EXIT_INTERNAL: u8 = 4;
#[allow(dead_code)]
const EXIT_INTERRUPTED: u8 = 130;
const MAX_RENDERED_ARG_COUNT: usize = 256;
const MAX_RENDERED_ARGUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_RENDERED_ARGV_BYTES: usize = 8 * 1024 * 1024;
const MAX_CAPTURE_KEYS: usize = 256;
const MAX_CAPTURE_DEPTH: usize = 32;
const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CAPTURE_TOTAL_BYTES: usize = 8 * 1024 * 1024;

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
    /// Persist the response body as a durable artifact under configured write roots.
    #[arg(long, global = true, value_name = "PATH")]
    artifact: Option<PathBuf>,
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

#[derive(Args, Clone, Debug, Default)]
struct MarkdownInputArgs {
    /// Inline Markdown source. Prefer stdin for large or secret-bearing input.
    #[arg(long, value_name = "MARKDOWN", conflicts_with = "path")]
    content: Option<String>,
    /// Markdown path, or `-` for stdin. With no path, stdin is used.
    #[arg(value_name = "PATH", conflicts_with = "content")]
    path: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Default)]
struct RecordArgs {
    /// Destination Markdown path. Otherwise use .mdok/records/.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Replace an existing recording.
    #[arg(long)]
    force: bool,
    /// Emit the exact response body after recording instead of the JSON envelope.
    #[arg(long)]
    raw: bool,
    /// Inline Markdown source.
    #[arg(long, value_name = "MARKDOWN", conflicts_with_all = ["path", "argv"])]
    content: Option<String>,
    /// Markdown path, or `-` for stdin.
    #[arg(value_name = "PATH", conflicts_with = "content")]
    path: Option<PathBuf>,
    /// Direct argv after `--`; it is converted to canonical Markdown.
    #[arg(
        last = true,
        allow_hyphen_values = true,
        conflicts_with_all = ["content", "path"]
    )]
    argv: Vec<String>,
}

#[derive(Args, Clone, Debug)]
struct ReplayArgs {
    /// Recorded Markdown path.
    path: PathBuf,
    /// Fail if the recording source/configuration has drifted.
    #[arg(long)]
    strict: bool,
}

#[derive(Args, Clone, Debug)]
struct PostmanImportArgs {
    /// Postman Collection v2.1 JSON file.
    input: PathBuf,
    /// Generated MDOK Markdown destination.
    #[arg(long, value_name = "PATH")]
    out: PathBuf,
    /// Write generated Markdown despite blocking import diagnostics.
    #[arg(long)]
    allow_lossy: bool,
    /// Replace existing Markdown and manifest files.
    #[arg(long)]
    force: bool,
    /// Import manifest destination. Defaults to <out>.import.json.
    #[arg(long, value_name = "PATH")]
    manifest: Option<PathBuf>,
}

#[derive(Subcommand, Clone, Debug)]
enum ImportCommand {
    /// Import a Postman Collection v2.1 JSON file.
    Postman(PostmanImportArgs),
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse, plan, execute, and check documents.
    Test { paths: Vec<PathBuf> },
    /// Execute transient Markdown from inline content or stdin.
    Run(MarkdownInputArgs),
    /// Execute one direct curl or trusted-profile command.
    Call {
        /// Emit the exact response body instead of the JSON invocation envelope.
        #[arg(long)]
        raw: bool,
        /// Direct argv after `--`.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Record transient Markdown or direct argv, then execute it.
    Record(RecordArgs),
    /// Re-run a recorded Markdown document.
    Replay(ReplayArgs),
    /// Import an external API collection into canonical MDOK Markdown.
    Import {
        #[command(subcommand)]
        format: ImportCommand,
    },
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
    #[serde(default)]
    command_timeout: Option<String>,
    #[serde(default = "default_command_output_bytes")]
    max_command_output_bytes: usize,
    #[serde(default = "default_command_args")]
    max_command_args: usize,
    #[serde(default = "default_command_arg_bytes")]
    max_command_arg_bytes: usize,
    #[serde(default = "default_command_argv_bytes")]
    max_command_argv_bytes: usize,
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
            command_timeout: None,
            max_command_output_bytes: default_command_output_bytes(),
            max_command_args: default_command_args(),
            max_command_arg_bytes: default_command_arg_bytes(),
            max_command_argv_bytes: default_command_argv_bytes(),
        }
    }
}

fn default_command_output_bytes() -> usize {
    1024 * 1024
}

fn default_command_args() -> usize {
    64
}

fn default_command_arg_bytes() -> usize {
    64 * 1024
}

fn default_command_argv_bytes() -> usize {
    1024 * 1024
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
    #[serde(default)]
    exec: ExecPolicyConfig,
    /// Legacy exact command entries are accepted only when they are absolute
    /// executable paths. New documents should use `[policy.exec.commands]`.
    #[serde(default)]
    allowed_commands: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ExecPolicyConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    commands: BTreeMap<String, CommandProfileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CommandProfileConfig {
    program: String,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    secret_env: BTreeMap<String, String>,
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
    config_path: Option<PathBuf>,
    config_root: PathBuf,
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
    artifact_path: Option<PathBuf>,
    command_timeout: Duration,
    max_command_output_bytes: usize,
    max_command_args: usize,
    max_command_arg_bytes: usize,
    max_command_argv_bytes: usize,
    exec_enabled: bool,
    command_working_directory: Option<PathBuf>,
    command_profiles: BTreeMap<String, ResolvedCommandProfile>,
}

#[derive(Clone, Debug)]
struct ResolvedCommandProfile {
    program: PathBuf,
    env: BTreeMap<String, String>,
    secret_env: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct Variable {
    value: Value,
    secret: bool,
}

impl EffectiveConfig {
    fn secret_values(&self) -> Vec<String> {
        collect_secret_values(&self.vars)
    }
}

#[derive(Clone, Debug)]
struct DocumentPlan {
    path: PathBuf,
    steps: Vec<StepPlan>,
    variables: BTreeMap<String, Variable>,
    jmespath: BTreeMap<String, jmespath::Expression<'static>>,
}

#[derive(Clone, Debug)]
struct StepPlan {
    name: String,
    kind: StepKind,
    command: Vec<String>,
    raw_tokens: Vec<String>,
    templates: Vec<Option<Template>>,
    checks: Vec<String>,
    captures: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepKind {
    Curl,
    Exec,
}

#[derive(Clone, Debug)]
struct PlanOutcome {
    plan: Option<DocumentPlan>,
    diagnostics: Vec<Diagnostic>,
    secret_values: Vec<String>,
}

#[derive(Clone, Debug)]
struct DocumentRun {
    report: DocumentReport,
    executions: Vec<Option<ExternalExecutionResult>>,
    contexts: Vec<Option<Value>>,
    secret_values: Vec<String>,
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

#[derive(Debug)]
enum SourceReadError {
    Io(io::Error),
    TooLarge { limit: usize, observed: usize },
}

impl std::fmt::Display for SourceReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::TooLarge { limit, observed } => write!(
                formatter,
                "source is at least {observed} bytes; the maximum is {limit} bytes"
            ),
        }
    }
}

impl std::error::Error for SourceReadError {}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.options.json;
    let json_lines = cli.options.json_lines;
    let invocation_error_context = invocation_error_context(&cli.command);
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let CliError { code, diagnostic } = *error;
            if let Some((operation, raw, argv)) = invocation_error_context
                && !raw
            {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&invocation_error_envelope(
                        operation,
                        &argv,
                        &diagnostic,
                    ))
                    .unwrap_or_else(|_| {
                        serde_json::json!({
                            "schema_version": "1",
                            "operation": invocation_operation_name(operation),
                            "success": false,
                            "result_kind": "none",
                            "diagnostics": [&diagnostic]
                        })
                        .to_string()
                    })
                );
            } else if json {
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

fn invocation_error_context(
    command: &Option<Command>,
) -> Option<(InvocationOperation, bool, Vec<String>)> {
    match command.as_ref()? {
        Command::Call { raw, argv } => Some((InvocationOperation::Call, *raw, argv.clone())),
        Command::Record(args) => Some((
            InvocationOperation::Record,
            args.raw,
            if args.argv.is_empty() {
                vec!["<unavailable>".to_string()]
            } else {
                args.argv.clone()
            },
        )),
        Command::Replay(_) => Some((
            InvocationOperation::Replay,
            false,
            vec!["<recording>".to_string()],
        )),
        Command::Run(_)
        | Command::Test { .. }
        | Command::Lint { .. }
        | Command::Plan { .. }
        | Command::List { .. }
        | Command::Import { .. }
        | Command::Version => None,
    }
}

fn run(cli: Cli) -> Result<u8, Box<CliError>> {
    let (mode, paths) = match cli.command {
        Some(Command::Version) => return print_version(&cli.options),
        Some(Command::Run(input)) => return run_transient_input(input, &cli.options),
        Some(Command::Call { raw, argv }) => return run_direct_call(argv, &cli.options, raw),
        Some(Command::Record(args)) => return run_record(args, &cli.options),
        Some(Command::Replay(args)) => return run_replay(args, &cli.options),
        Some(Command::Import {
            format: ImportCommand::Postman(args),
        }) => return run_postman_import(args, &cli.options),
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
    let mut secret_values = config.secret_values();
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
            secret_values.extend(result.secret_values.iter().cloned());
            let failed = result.report.status.is_failure();
            let event_start = report.events.len();
            append_document_event(&mut report, &result.report, document_ordinal);
            stream_event_range(&report, event_start)?;
            attach_execution_metadata(&mut report, document_ordinal, &result);
            report.add_document(result.report);
            if failed && (cli.options.fail_fast || config.fail_fast) {
                break;
            }
        }
    } else {
        let results = if sequential {
            let mut results = Vec::with_capacity(paths.len());
            for path in paths {
                let result = process_document(&path, mode, &config, &cli.options);
                let failed = result.report.status.is_failure();
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
            secret_values.extend(document.secret_values.iter().cloned());
            append_report_document(&mut report, document, document_ordinal);
        }
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    let redactor = Redactor::new(secret_values);
    let report = redactor.redact_report(&report).map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Report error", error.to_string()),
        )
    })?;
    emit_report(&report, &cli.options, mode, stream_jsonl)?;
    Ok(report_exit_code(&report))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationOperation {
    Run,
    Call,
    Record,
    Replay,
}

#[derive(Clone, Debug)]
struct TransientDocument {
    source: Vec<u8>,
    label: PathBuf,
    config_anchor: PathBuf,
    source_kind: &'static str,
    argv: Option<Vec<String>>,
    replay_drift: Option<Value>,
}

#[derive(Clone, Debug)]
struct RecordingInfo {
    path: PathBuf,
    manifest_path: PathBuf,
    source_sha256: String,
    replay_command: String,
}

fn run_transient_input(
    input: MarkdownInputArgs,
    options: &CommonOptions,
) -> Result<u8, Box<CliError>> {
    validate_invocation_options(options)?;
    let source = read_transient_markdown(input)?;
    run_invocation(
        source,
        InvocationOperation::Run,
        options,
        false,
        None,
        false,
    )
}

fn validate_invocation_options(options: &CommonOptions) -> Result<(), Box<CliError>> {
    if options.jobs == 0 {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Invalid jobs value",
                "--jobs must be at least 1",
            ),
        ));
    }
    if options.json && options.json_lines {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Conflicting output modes",
                "--json and --json-lines cannot be selected together",
            ),
        ));
    }
    Ok(())
}

fn run_direct_call(
    argv: Vec<String>,
    options: &CommonOptions,
    raw: bool,
) -> Result<u8, Box<CliError>> {
    validate_invocation_options(options)?;
    let source = direct_transient_document(argv)?;
    run_invocation(source, InvocationOperation::Call, options, raw, None, false)
}

fn run_record(args: RecordArgs, options: &CommonOptions) -> Result<u8, Box<CliError>> {
    validate_invocation_options(options)?;
    let (source, force, raw) = if !args.argv.is_empty() {
        (direct_transient_document(args.argv)?, args.force, args.raw)
    } else {
        let input = MarkdownInputArgs {
            content: args.content,
            path: args.path,
        };
        (read_transient_markdown(input)?, args.force, args.raw)
    };
    run_invocation(
        source,
        InvocationOperation::Record,
        options,
        raw,
        args.output,
        force,
    )
}

fn run_replay(args: ReplayArgs, options: &CommonOptions) -> Result<u8, Box<CliError>> {
    validate_invocation_options(options)?;
    let path = args.path;
    let mut source = read_transient_markdown(MarkdownInputArgs {
        content: None,
        path: Some(path),
    })?;
    source.source_kind = "recording";
    let config = load_config(std::slice::from_ref(&source.config_anchor), options)?;
    let drift = replay_drift(&source, options, &config)?;
    source.replay_drift = Some(drift.clone());
    if args.strict && drift.get("status").and_then(Value::as_str) != Some("exact") {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Recording drift detected",
                drift
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("recording provenance does not match the current inputs"),
            ),
        ));
    }
    run_invocation(
        source,
        InvocationOperation::Replay,
        options,
        false,
        None,
        false,
    )
}

fn run_postman_import(
    args: PostmanImportArgs,
    options: &CommonOptions,
) -> Result<u8, Box<CliError>> {
    validate_invocation_options(options)?;
    if args.input == args.out {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-PM-IMPORT",
                "Invalid Postman import destination",
                "input and --out must be different paths",
            ),
        ));
    }
    let import_options = mdok_postman::ImportOptions {
        allow_lossy: args.allow_lossy,
    };
    let output =
        mdok_postman::import_collection_file(&args.input, &import_options).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-PM-IMPORT", "Postman import failed", error.to_string())
                    .at_file(&args.input),
            )
        })?;
    let manifest_path = args
        .manifest
        .clone()
        .or_else(|| options.report.clone())
        .unwrap_or_else(|| PathBuf::from(format!("{}.import.json", args.out.display())));
    if output.has_blockers() && !args.allow_lossy {
        if manifest_path.exists() && !args.force {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-PM-IMPORT",
                    "Refusing to overwrite import manifest",
                    format!(
                        "{} already exists; pass --force to replace it",
                        manifest_path.display()
                    ),
                )
                .at_file(&manifest_path),
            ));
        }
        let manifest_bytes = serde_json::to_vec_pretty(&output.manifest).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error(
                    "MDOK-PM-IMPORT",
                    "Cannot encode import manifest",
                    error.to_string(),
                ),
            )
        })?;
        fs::write(&manifest_path, manifest_bytes).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-PM-IMPORT",
                    "Cannot write import manifest",
                    error.to_string(),
                )
                .at_file(&manifest_path),
            )
        })?;
        let summary = output
            .manifest
            .issues
            .iter()
            .filter(|issue| issue.severity == mdok_postman::IssueSeverity::Error)
            .map(|issue| format!("{}: {}", issue.code, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-PM-REVIEW",
                "Postman import requires review",
                format!(
                    "{} blocking issue(s); review {} and rerun with --allow-lossy only after deciding how to handle them: {}",
                    output
                        .manifest
                        .issues
                        .iter()
                        .filter(|issue| issue.severity == mdok_postman::IssueSeverity::Error)
                        .count(),
                    manifest_path.display(),
                    summary
                ),
            )
            .at_file(&args.input),
        ));
    }
    for path in [&args.out, &manifest_path] {
        if path.exists() && !args.force {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-PM-IMPORT",
                    "Refusing to overwrite import output",
                    format!(
                        "{} already exists; pass --force to replace it",
                        path.display()
                    ),
                )
                .at_file(path),
            ));
        }
    }
    let manifest_bytes = serde_json::to_vec_pretty(&output.manifest).map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error(
                "MDOK-PM-IMPORT",
                "Cannot encode import manifest",
                error.to_string(),
            ),
        )
    })?;
    fs::write(&args.out, output.markdown.as_bytes()).map_err(|error| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-PM-IMPORT",
                "Cannot write imported Markdown",
                error.to_string(),
            )
            .at_file(&args.out),
        )
    })?;
    fs::write(&manifest_path, manifest_bytes).map_err(|error| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-PM-IMPORT",
                "Cannot write import manifest",
                error.to_string(),
            )
            .at_file(&manifest_path),
        )
    })?;
    let blocking_count = output
        .manifest
        .issues
        .iter()
        .filter(|issue| issue.severity == mdok_postman::IssueSeverity::Error)
        .count();
    let warning_count = output
        .manifest
        .issues
        .iter()
        .filter(|issue| issue.severity == mdok_postman::IssueSeverity::Warning)
        .count();
    let result = json!({
        "operation": "import",
        "format": "postman_collection_v2.1",
        "success": true,
        "input": args.input,
        "output": args.out,
        "manifest": manifest_path,
        "generated_steps": output.manifest.generated_steps.len(),
        "blocking_issues": blocking_count,
        "warnings": warning_count,
    });
    if options.json || options.json_lines {
        println!("{}", result);
    } else {
        println!(
            "Imported {} step(s) to {} (manifest: {})",
            output.manifest.generated_steps.len(),
            args.out.display(),
            manifest_path.display()
        );
        if blocking_count > 0 || warning_count > 0 {
            eprintln!(
                "review required: {} blocking issue(s), {} warning(s)",
                blocking_count, warning_count
            );
        }
    }
    Ok(EXIT_OK)
}

fn read_transient_markdown(input: MarkdownInputArgs) -> Result<TransientDocument, Box<CliError>> {
    if let Some(content) = input.content {
        let source = bounded_source_bytes(content.into_bytes())?;
        return Ok(TransientDocument {
            source,
            label: PathBuf::from("<inline>"),
            config_anchor: PathBuf::from("-"),
            source_kind: "inline",
            argv: None,
            replay_drift: None,
        });
    }
    let path = input.path.unwrap_or_else(|| PathBuf::from("-"));
    let source = if path == Path::new("-") {
        read_bounded_source(io::stdin()).map_err(source_read_diagnostic)?
    } else {
        let file = fs::File::open(&path).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Cannot read document", error.to_string())
                    .at_file(&path),
            )
        })?;
        read_bounded_source(file).map_err(source_read_diagnostic)?
    };
    Ok(TransientDocument {
        source,
        label: if path == Path::new("-") {
            PathBuf::from("<stdin>")
        } else {
            path.clone()
        },
        config_anchor: path.clone(),
        source_kind: if path == Path::new("-") {
            "stdin"
        } else {
            "path"
        },
        argv: None,
        replay_drift: None,
    })
}

fn direct_transient_document(argv: Vec<String>) -> Result<TransientDocument, Box<CliError>> {
    if argv.is_empty() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Empty command", "provide argv after `--`"),
        ));
    }
    let source = transient::canonical_command_markdown(&argv).map_err(|message| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid direct command", message),
        )
    })?;
    Ok(TransientDocument {
        source: bounded_source_bytes(source.into_bytes())?,
        label: PathBuf::from("<argv>"),
        config_anchor: PathBuf::from("-"),
        source_kind: "argv",
        argv: Some(argv),
        replay_drift: None,
    })
}

fn bounded_source_bytes(source: Vec<u8>) -> Result<Vec<u8>, Box<CliError>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E700",
                "Markdown source limit exceeded",
                format!("source exceeds {MAX_SOURCE_BYTES} bytes"),
            ),
        ));
    }
    Ok(source)
}

fn source_read_diagnostic(error: SourceReadError) -> Box<CliError> {
    let too_large = matches!(&error, SourceReadError::TooLarge { .. });
    let (title, message) = match error {
        SourceReadError::Io(error) => ("Cannot read document", error.to_string()),
        SourceReadError::TooLarge { limit, observed } => (
            "Markdown source limit exceeded",
            format!("source is at least {observed} bytes; the maximum is {limit} bytes"),
        ),
    };
    let code = if too_large { "MDOK-E700" } else { "MDOK-E001" };
    cli_error(EXIT_INPUT, Diagnostic::error(code, title, message))
}

fn run_invocation(
    source: TransientDocument,
    operation: InvocationOperation,
    options: &CommonOptions,
    raw: bool,
    output: Option<PathBuf>,
    force: bool,
) -> Result<u8, Box<CliError>> {
    let config = load_config(std::slice::from_ref(&source.config_anchor), options)?;
    let source_sha256 = sha256_hex(&source.source);
    let recording = match operation {
        InvocationOperation::Record => {
            if let Some(argv) = &source.argv {
                validate_recordable_argv(argv)?;
            }
            validate_recordable_source(&source.source, &config)?;
            Some(write_recording(
                &source,
                output,
                force,
                options,
                &source_sha256,
                &config,
            )?)
        }
        InvocationOperation::Replay => Some(RecordingInfo {
            path: source.label.clone(),
            manifest_path: recording_manifest_path(&source.label),
            source_sha256: source_sha256.clone(),
            replay_command: format!("mdok replay {}", record_path_string(&source.label)),
        }),
        InvocationOperation::Run | InvocationOperation::Call => None,
    };
    let mut temporary = NamedTempFile::new().map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Transient source failed", error.to_string()),
        )
    })?;
    temporary.write_all(&source.source).map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Transient source failed", error.to_string()),
        )
    })?;
    temporary.flush().map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Transient source failed", error.to_string()),
        )
    })?;

    let mut document = process_document(temporary.path(), Mode::Test, &config, options);
    document.report.path = source.label.display().to_string();
    let mut secret_values = config.secret_values();
    secret_values.extend(document.secret_values.iter().cloned());
    let redactor = Redactor::new(secret_values);
    if operation == InvocationOperation::Run {
        let mut report = Report::now();
        append_report_document(&mut report, document, 0);
        report.duration_ms = report
            .documents
            .first()
            .map(|document| document.duration_ms)
            .unwrap_or_default();
        let report = redactor.redact_report(&report).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Report error", error.to_string()),
            )
        })?;
        emit_report(&report, options, Mode::Test, false)?;
        return Ok(report_exit_code(&report));
    }

    let context = document.contexts.first().and_then(Clone::clone);
    let envelope = invocation_envelope(
        &document,
        context.as_ref(),
        &source,
        operation,
        recording.as_ref(),
        &source_sha256,
        &redactor,
    );
    if raw {
        write_raw_context(context.as_ref(), &config)?;
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error(
                        "MDOK-E800",
                        "Invocation serialization failed",
                        error.to_string(),
                    ),
                )
            })?
        );
    }
    Ok(if document.report.status.is_failure() {
        report_exit_code_from_document(&document.report)
    } else {
        EXIT_OK
    })
}

fn report_exit_code_from_document(document: &DocumentReport) -> u8 {
    let policy = document
        .diagnostics
        .iter()
        .chain(
            document
                .steps
                .iter()
                .flat_map(|step| step.diagnostics.iter()),
        )
        .any(|diagnostic| is_policy_code(&diagnostic.code));
    if policy {
        EXIT_POLICY
    } else if document.status == Status::Error {
        EXIT_INPUT
    } else {
        EXIT_CHECK_FAILED
    }
}

fn invocation_operation_name(operation: InvocationOperation) -> &'static str {
    match operation {
        InvocationOperation::Run => "run",
        InvocationOperation::Call => "call",
        InvocationOperation::Record => "record",
        InvocationOperation::Replay => "replay",
    }
}

fn invocation_error_envelope(
    operation: InvocationOperation,
    argv: &[String],
    diagnostic: &Diagnostic,
) -> Value {
    let source_bytes = serde_json::to_vec(argv).unwrap_or_default();
    let source_sha256 = sha256_hex(&source_bytes);
    let adapter = if argv.first().map(String::as_str) == Some("curl") {
        "curl"
    } else {
        "exec"
    };
    let redactor = Redactor::new(std::iter::empty::<String>());
    let diagnostic_value = invocation_diagnostic_value(diagnostic);
    let policy = is_policy_code(&diagnostic.code);
    redactor.redact_value(&json!({
        "schema_version": "1",
        "operation": invocation_operation_name(operation),
        "run_id": format!("error-{}", &source_sha256[..16]),
        "success": false,
        "result_kind": "none",
        "request": {
            "adapter": adapter,
            "argv": redact_argv_for_output(argv, &redactor),
            "source": {
                "kind": match operation {
                    InvocationOperation::Replay => "recording",
                    _ => "argv",
                },
                "path": Value::Null,
                "sha256": source_sha256,
            },
        },
        "response": Value::Null,
        "execution": {
            "started_at": now_string(),
            "duration_ms": 0,
            "timed_out": false,
            "policy_allowed": !policy,
            "policy_reason": if policy {
                Value::String(diagnostic.message.clone())
            } else {
                Value::Null
            },
            "exit_code": Value::Null,
        },
        "recording": Value::Null,
        "artifacts": [],
        "diagnostics": [diagnostic_value],
    }))
}

fn invocation_diagnostic_value(diagnostic: &Diagnostic) -> Value {
    json!({
        "severity": diagnostic.severity,
        "code": diagnostic.code,
        "message": diagnostic.message,
        "hint": diagnostic.hint,
    })
}

fn is_policy_code(code: &str) -> bool {
    matches!(
        code,
        "MDOK-E302"
            | "MDOK-E303"
            | "MDOK-E304"
            | "MDOK-E306"
            | "MDOK-E307"
            | "MDOK-E312"
            | "MDOK-E404"
            | "MDOK-E602"
            | "MDOK-E603"
            | "MDOK-E604"
    )
}

fn invocation_envelope(
    document: &DocumentRun,
    context: Option<&Value>,
    source: &TransientDocument,
    operation: InvocationOperation,
    recording: Option<&RecordingInfo>,
    source_sha256: &str,
    redactor: &Redactor,
) -> Value {
    let step = document.report.steps.first();
    let argv = source
        .argv
        .clone()
        .or_else(|| step.map(|step| step.command.clone()))
        .unwrap_or_else(|| vec!["<unavailable>".to_string()]);
    let adapter = context
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        .map_or_else(
            || {
                if source
                    .argv
                    .as_ref()
                    .and_then(|argv| argv.first())
                    .map(String::as_str)
                    == Some("curl")
                {
                    "curl"
                } else {
                    "exec"
                }
            },
            |kind| if kind == "exec" { "exec" } else { "curl" },
        );
    let response = context.cloned();
    let result_kind = if context.is_none() {
        "none"
    } else if adapter == "curl" {
        "http"
    } else {
        "command"
    };
    let diagnostics = document
        .report
        .diagnostics
        .iter()
        .chain(
            document
                .report
                .steps
                .iter()
                .flat_map(|step| step.diagnostics.iter()),
        )
        .map(invocation_diagnostic_value)
        .collect::<Vec<_>>();
    let raw = json!({
        "schema_version": "1",
        "operation": invocation_operation_name(operation),
        "run_id": format!("run-{}", source_sha256.get(..16).unwrap_or(source_sha256)),
        "success": document.report.status == Status::Passed,
        "result_kind": result_kind,
        "request": {
            "adapter": adapter,
            "argv": redact_argv_for_output(&argv, redactor),
            "source": {
                "kind": source.source_kind,
                "path": source.label,
                "sha256": source_sha256,
            },
        },
        "response": response,
        "execution": {
            "started_at": now_string(),
            "duration_ms": document.report.duration_ms,
            "timed_out": context.and_then(|value| value.get("timed_out")).and_then(Value::as_bool).unwrap_or(false),
            "policy_allowed": !diagnostics.iter().any(|value| value.get("code").and_then(Value::as_str).is_some_and(is_policy_code)),
            "policy_reason": diagnostics.iter().find(|value| value.get("code").and_then(Value::as_str).is_some_and(is_policy_code)).and_then(|value| value.get("message")).cloned().unwrap_or(Value::Null),
            "exit_code": context.and_then(|value| value.get("exit_code")).cloned().unwrap_or(Value::Null),
        },
        "recording": recording.map(|recording| json!({
            "path": record_path_string(&recording.path),
            "manifest_path": record_path_string(&recording.manifest_path),
            "source_sha256": recording.source_sha256,
            "replay_command": recording.replay_command,
            "drift": source.replay_drift.clone().unwrap_or_else(|| {
                json!({
                    "status": "not_checked",
                    "source_changed": false,
                    "configuration_changed": false,
                    "inputs_changed": false,
                    "message": "replay provenance was not checked"
                })
            }),
        })).unwrap_or(Value::Null),
        "artifacts": response
            .as_ref()
            .and_then(|value| value.get("body_metadata"))
            .and_then(|value| value.get("artifact"))
            .filter(|value| !value.is_null())
            .map(|value| vec![value.clone()])
            .unwrap_or_default(),
        "diagnostics": diagnostics,
    });
    redactor.redact_value(&raw)
}

fn write_raw_context(
    context: Option<&Value>,
    config: &EffectiveConfig,
) -> Result<(), Box<CliError>> {
    let Some(context) = context else {
        return Ok(());
    };
    if context.get("kind").and_then(Value::as_str) == Some("exec")
        && context.get("secret_tainted").and_then(Value::as_bool) == Some(true)
    {
        return Err(cli_error(
            EXIT_POLICY,
            Diagnostic::error(
                "MDOK-E404",
                "Raw secret-tainted output denied",
                "structured output is required when a trusted command receives secret environment values",
            ),
        ));
    }
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if let Some(path) = context
        .get("body_metadata")
        .and_then(|value| value.get("artifact"))
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
    {
        let artifact_path = config.config_root.join(path);
        let mut file = fs::File::open(&artifact_path).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
            )
        })?;
        let mut buffer = [0u8; 16 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
                )
            })?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count]).map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
                )
            })?;
        }
    } else if let Some(text) = context.get("body_text").and_then(Value::as_str) {
        output.write_all(text.as_bytes()).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
            )
        })?;
    } else if let Some(encoded) = context.get("body_base64").and_then(Value::as_str) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
                )
            })?;
        output.write_all(&bytes).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
            )
        })?;
    } else if let Some(stdout_text) = context.get("stdout").and_then(Value::as_str) {
        output.write_all(stdout_text.as_bytes()).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
            )
        })?;
    }
    output.flush().map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Raw output failed", error.to_string()),
        )
    })?;
    Ok(())
}

fn redact_argv_for_output(argv: &[String], redactor: &Redactor) -> Vec<String> {
    let mut redacted = Vec::with_capacity(argv.len());
    let mut redact_next = false;
    for argument in argv {
        if redact_next {
            redacted.push("[REDACTED]".to_string());
            redact_next = false;
            continue;
        }
        let lower = argument.to_ascii_lowercase();
        let option = lower
            .split_once('=')
            .map_or(lower.as_str(), |(name, _)| name);
        if matches!(
            option,
            "-u" | "--user" | "--cookie" | "-b" | "--proxy" | "-x" | "--oauth2-bearer"
        ) {
            if argument.contains('=') {
                redacted.push(format!(
                    "{}=[REDACTED]",
                    argument
                        .split_once('=')
                        .map(|(name, _)| name)
                        .unwrap_or(argument)
                ));
            } else {
                redacted.push(argument.clone());
                redact_next = true;
            }
            continue;
        }
        if option == "-h" || option == "--header" {
            if let Some((name, value)) = argument.split_once('=') {
                redacted.push(format!("{name}={}", redact_header_value(value, redactor)));
            } else {
                redacted.push(argument.clone());
                redact_next = true;
            }
            continue;
        }
        if let Some((name, _value)) = argument.split_once(':')
            && is_sensitive_header(name)
        {
            redacted.push(format!("{name}: [REDACTED]"));
            continue;
        }
        let redacted_value = redact_url_credentials(argument, redactor);
        redacted.push(redacted_value);
    }
    redacted
}

fn redact_header_value(value: &str, redactor: &Redactor) -> String {
    if let Some((name, _)) = value.split_once(':')
        && is_sensitive_header(name)
    {
        return format!("{name}: [REDACTED]");
    }
    redactor.redact_text(value)
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "x-auth-token"
    )
}

fn redact_url_credentials(value: &str, redactor: &Redactor) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return redactor.redact_text(value);
    };
    if !url.username().is_empty() || url.password().is_some() {
        let _ = url.set_username("[REDACTED]");
        let _ = url.set_password(Some("[REDACTED]"));
        return url.to_string();
    }
    redactor.redact_text(value)
}

fn write_recording(
    source: &TransientDocument,
    output: Option<PathBuf>,
    force: bool,
    options: &CommonOptions,
    source_sha256: &str,
    config: &EffectiveConfig,
) -> Result<RecordingInfo, Box<CliError>> {
    let path = output.unwrap_or_else(|| {
        let root = options
            .config
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        root.join(".mdok").join("records").join(format!(
            "record-{}-{}.md",
            unix_timestamp(),
            &source_sha256[..8]
        ))
    });
    let path = if path.extension().is_none() {
        path.with_extension("md")
    } else {
        path
    };
    write_private_atomic(&path, &source.source, force)?;
    let manifest_path = recording_manifest_path(&path);
    let provenance = provenance_snapshot(config, options);
    let provenance_sha256 = sha256_json(&provenance);
    let manifest = json!({
        "schema_version": "1",
        "source_sha256": source_sha256,
        "source_kind": source.source_kind,
        "mdok_version": mdok_report::MDOK_VERSION,
        "curl_compatibility": mdok_report::CURL_COMPAT_VERSION,
        "created_at": now_string(),
        "provenance": provenance,
        "provenance_sha256": provenance_sha256,
    });
    if let Err(error) = write_private_atomic(
        &manifest_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).unwrap_or_default()
        )
        .as_bytes(),
        force,
    ) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(RecordingInfo {
        replay_command: format!("mdok replay {}", record_path_string(&path)),
        path,
        manifest_path,
        source_sha256: source_sha256.to_owned(),
    })
}

fn write_private_atomic(path: &Path, bytes: &[u8], force: bool) -> Result<(), Box<CliError>> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_private_directory(parent).map_err(|error| {
        cli_error(
            EXIT_POLICY,
            Diagnostic::error("MDOK-E303", "Recording path denied", error.to_string()),
        )
    })?;
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(cli_error(
            EXIT_POLICY,
            Diagnostic::error(
                "MDOK-E303",
                "Recording path denied",
                "recording destination cannot be a symbolic link",
            ),
        ));
    }
    if !force && path.exists() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Recording already exists",
                path.display().to_string(),
            ),
        ));
    }
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| {
        cli_error(
            EXIT_INTERNAL,
            Diagnostic::error("MDOK-E800", "Recording write failed", error.to_string()),
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                cli_error(
                    EXIT_INTERNAL,
                    Diagnostic::error(
                        "MDOK-E800",
                        "Recording permission failed",
                        error.to_string(),
                    ),
                )
            })?;
    }
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file_mut().sync_all())
        .map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Recording write failed", error.to_string()),
            )
        })?;
    if force {
        temporary.persist(path).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error(
                    "MDOK-E800",
                    "Recording rename failed",
                    error.error.to_string(),
                ),
            )
        })?;
    } else {
        temporary.persist_noclobber(path).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E001",
                    "Recording already exists",
                    error.error.to_string(),
                ),
            )
        })?;
    }
    let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
    Ok(())
}

fn validate_recordable_argv(argv: &[String]) -> Result<(), Box<CliError>> {
    for (index, argument) in argv.iter().enumerate() {
        let lower = argument.to_ascii_lowercase();
        if lower.contains("authorization:")
            || lower.contains("cookie:")
            || lower.contains("x-api-key:")
            || matches!(lower.as_str(), "-u" | "--user" | "--cookie" | "--proxy")
            || (index > 0 && argv[index - 1] == "-u")
            || (index > 0 && argv[index - 1] == "--user")
            || (index > 0 && argv[index - 1] == "--cookie")
        {
            return Err(cli_error(
                EXIT_POLICY,
                Diagnostic::error(
                    "MDOK-E404",
                    "Secret-bearing command cannot be recorded",
                    "use a named secret and a Markdown template instead of persisting a literal credential",
                ),
            ));
        }
    }
    Ok(())
}

fn validate_recordable_source(
    source: &[u8],
    config: &EffectiveConfig,
) -> Result<(), Box<CliError>> {
    let text = std::str::from_utf8(source).map_err(|_| {
        cli_error(
            EXIT_POLICY,
            Diagnostic::error(
                "MDOK-E404",
                "Secret-bearing document cannot be recorded",
                "recordings must be valid UTF-8 Markdown so their source can be inspected safely",
            ),
        )
    })?;
    let known_secrets = config.secret_values();
    if known_secrets
        .iter()
        .any(|secret| !secret.is_empty() && text.contains(secret))
    {
        return Err(cli_error(
            EXIT_POLICY,
            Diagnostic::error(
                "MDOK-E404",
                "Secret-bearing document cannot be recorded",
                "the Markdown source contains a resolved secret value; use a named template variable",
            ),
        ));
    }
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        let sensitive_header = [
            "authorization:",
            "proxy-authorization:",
            "cookie:",
            "set-cookie:",
            "x-api-key:",
            "x-auth-token:",
        ]
        .iter()
        .any(|marker| lower.contains(marker));
        if sensitive_header && !line.contains("{{") {
            return Err(cli_error(
                EXIT_POLICY,
                Diagnostic::error(
                    "MDOK-E404",
                    "Secret-bearing document cannot be recorded",
                    "replace literal authorization, cookie, or API-key values with a named template variable",
                ),
            ));
        }
        if (lower.contains("--user ")
            || lower.contains("--cookie ")
            || lower.contains(" -b ")
            || lower.contains("--oauth2-bearer "))
            && !line.contains("{{")
        {
            return Err(cli_error(
                EXIT_POLICY,
                Diagnostic::error(
                    "MDOK-E404",
                    "Secret-bearing document cannot be recorded",
                    "replace literal credential options with a named template variable",
                ),
            ));
        }
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if !fs::metadata(&current).is_ok_and(|target| target.is_dir()) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "directory component is not a directory: {}",
                            current.display()
                        ),
                    ));
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "directory component is not a directory: {}",
                        current.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&current, fs::Permissions::from_mode(0o700))?;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn provenance_snapshot(config: &EffectiveConfig, options: &CommonOptions) -> Value {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|path| fs::canonicalize(path).ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut profiles = Map::new();
    for (name, profile) in &config.command_profiles {
        profiles.insert(
            name.clone(),
            json!({
                "program": profile.program,
                "program_sha256": sha256_file(&profile.program),
                "env_keys": profile.env.keys().collect::<Vec<_>>(),
                "secret_env_keys": profile.secret_env.keys().collect::<Vec<_>>(),
            }),
        );
    }
    let secret_sources = config
        .vars
        .iter()
        .filter(|(_, variable)| variable.secret)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let input_ids = options
        .secret
        .iter()
        .filter_map(|entry| entry.split_once('=').map(|(name, _)| name.to_string()))
        .chain(secret_sources.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let non_secret_variables = config
        .vars
        .iter()
        .filter(|(_, variable)| !variable.secret)
        .map(|(name, variable)| (name.clone(), variable.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let secret_value_digests = config
        .vars
        .iter()
        .filter(|(_, variable)| variable.secret)
        .map(|(name, variable)| (name.clone(), sha256_json(&variable.value)))
        .collect::<BTreeMap<_, _>>();
    json!({
        "mdok_version": mdok_report::MDOK_VERSION,
        "curl_compatibility": mdok_report::CURL_COMPAT_VERSION,
        "libcurl": mdok_report::LIBCURL_VERSION,
        "tls": mdok_report::TLS_BACKEND,
        "config_path": config.config_path,
        "config_sha256": config.config_path.as_deref().and_then(sha256_file),
        "config_root": config.config_root,
        "working_directory": cwd,
        "environment_profile": options.env,
        "allowed_hosts": config.allowed_hosts,
        "denied_hosts": config.denied_hosts,
        "allowed_schemes": config.allowed_schemes,
        "allow_private_network": config.allow_private_network,
        "allow_proxy": config.allow_proxy,
        "allow_insecure_tls": config.allow_insecure_tls,
        "allow_resolve": config.allow_resolve,
        "allow_connect_to": config.allow_connect_to,
        "allow_file_reads": config.allow_file_reads,
        "allowed_read_roots": config.allowed_read_roots,
        "allow_artifact_writes": config.allow_artifact_writes,
        "allowed_artifact_roots": config.allowed_artifact_roots,
        "artifact_path": config.artifact_path,
        "timeouts": {
            "connect_ms": config.connect_timeout.as_millis(),
            "total_ms": config.timeout.as_millis(),
            "command_ms": config.command_timeout.as_millis(),
        },
        "limits": {
            "max_body": config.max_body,
            "max_command_output_bytes": config.max_command_output_bytes,
            "max_command_args": config.max_command_args,
            "max_command_arg_bytes": config.max_command_arg_bytes,
            "max_command_argv_bytes": config.max_command_argv_bytes,
        },
        "profiles": profiles,
        "secret_source_ids": input_ids,
        "secret_value_digests": secret_value_digests,
        "non_secret_variables_sha256": sha256_json(&Value::Object(
            non_secret_variables.into_iter().collect()
        )),
    })
}

fn sha256_json(value: &Value) -> String {
    serde_json::to_vec(value)
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_default()
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Some(hex_digest(&digest.finalize()))
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn recording_manifest_path(source_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.json", source_path.display()))
}

fn replay_drift(
    source: &TransientDocument,
    options: &CommonOptions,
    config: &EffectiveConfig,
) -> Result<Value, Box<CliError>> {
    let manifest_path = recording_manifest_path(&source.label);
    if !manifest_path.is_file() {
        return Ok(json!({
            "status": "unknown",
            "source_changed": false,
            "configuration_changed": false,
            "inputs_changed": false,
            "message": "recording manifest is missing"
        }));
    }
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E001",
                    "Cannot read recording manifest",
                    error.to_string(),
                ),
            )
        })?)
        .map_err(|error| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error("MDOK-E001", "Invalid recording manifest", error.to_string()),
            )
        })?;
    if manifest.get("schema_version").and_then(Value::as_str) != Some("1") {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Unsupported recording manifest",
                "recording manifest schema_version must be `1`",
            ),
        ));
    }
    let expected = manifest.get("source_sha256").and_then(Value::as_str);
    let actual = sha256_hex(&source.source);
    if expected.is_none() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E001",
                "Invalid recording manifest",
                "recording manifest is missing source_sha256",
            ),
        ));
    }
    let current_provenance = provenance_snapshot(config, options);
    let expected_provenance = manifest
        .get("provenance_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E001",
                    "Invalid recording manifest",
                    "recording manifest is missing provenance_sha256",
                ),
            )
        })?;
    let actual_provenance = sha256_json(&current_provenance);
    let source_changed = expected != Some(actual.as_str());
    let configuration_changed = expected_provenance != actual_provenance;
    let expected_inputs = manifest
        .get("provenance")
        .and_then(|value| value.get("secret_source_ids"));
    let actual_inputs = current_provenance.get("secret_source_ids");
    let inputs_changed = expected_inputs != actual_inputs;
    let changed = source_changed || configuration_changed || inputs_changed;
    Ok(json!({
        "status": if changed { "changed" } else { "exact" },
        "source_changed": source_changed,
        "configuration_changed": configuration_changed,
        "inputs_changed": inputs_changed,
        "message": if changed {
            "recording source, configuration, or secret input identifiers changed"
        } else {
            "recording source and provenance match the current inputs"
        },
        "expected_source_sha256": expected,
        "actual_source_sha256": actual,
        "expected_provenance_sha256": expected_provenance,
        "actual_provenance_sha256": actual_provenance,
    }))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;

    // Proleptic Gregorian conversion from Unix days. Keeping this local
    // avoids adding a date dependency to the small CLI binary while emitting
    // the RFC 3339 form required by the invocation schema.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * month_part + 2).div_euclid(5) + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn record_path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn report_exit_code(report: &Report) -> u8 {
    let diagnostics = all_report_diagnostics(report);
    let policy_failure = diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "MDOK-E302"
                | "MDOK-E303"
                | "MDOK-E304"
                | "MDOK-E306"
                | "MDOK-E602"
                | "MDOK-E603"
                | "MDOK-E604"
        )
    });
    if policy_failure {
        EXIT_POLICY
    } else if diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            && (diagnostic.code == "MDOK-E800"
                || diagnostic.code == "MDOK-E500"
                || diagnostic.code == "MDOK-E700"
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

fn append_report_document(report: &mut Report, document: DocumentRun, document_ordinal: usize) {
    attach_execution_metadata(report, document_ordinal, &document);
    for (step_ordinal, step) in document.report.steps.iter().enumerate() {
        append_step_event(
            report,
            &document.report.path,
            document_ordinal,
            step_ordinal,
            step,
        );
    }
    append_document_event(report, &document.report, document_ordinal);
    report.add_document(document.report);
}

fn attach_execution_metadata(report: &mut Report, document_ordinal: usize, document: &DocumentRun) {
    for (step_ordinal, execution) in document.executions.iter().enumerate() {
        let Some(execution) = execution else {
            continue;
        };
        report.set_step_execution_metadata(StepExecutionMetadata::new(
            document_ordinal,
            step_ordinal,
            Some(ReportStepKind::Exec),
            Some(execution.clone()),
        ));
    }
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
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let end = report.events.len();
    report
        .write_json_lines_range(start..end, &mut output)
        .map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Event serialization failed", error.to_string()),
            )
        })?;
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
        let stdout = io::stdout();
        let mut output = stdout.lock();
        report.write_json_lines(&mut output).map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Event serialization failed", error.to_string()),
            )
        })?;
        output.flush().map_err(|error| {
            cli_error(
                EXIT_INTERNAL,
                Diagnostic::error("MDOK-E800", "Event output failed", error.to_string()),
            )
        })?;
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
) -> Vec<DocumentRun> {
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
) -> DocumentRun {
    process_document_with_hook(path, mode, config, options, |_, _| {})
}

fn process_document_with_hook<F>(
    path: &Path,
    mode: Mode,
    config: &EffectiveConfig,
    options: &CommonOptions,
    mut on_step: F,
) -> DocumentRun
where
    F: FnMut(usize, &StepReport),
{
    let started = Instant::now();
    let outcome = build_plan(path, config);
    let Some(plan) = outcome.plan else {
        return DocumentRun {
            report: DocumentReport {
                path: path.display().to_string(),
                status: if outcome.diagnostics.iter().any(|diagnostic| {
                    matches!(
                        diagnostic.code.as_str(),
                        "MDOK-E302"
                            | "MDOK-E303"
                            | "MDOK-E304"
                            | "MDOK-E306"
                            | "MDOK-E312"
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
            },
            executions: Vec::new(),
            contexts: Vec::new(),
            secret_values: outcome.secret_values,
        };
    };
    let mut document = match mode {
        Mode::Test => execute_plan_with_hook(&plan, config, options, &mut on_step),
        Mode::Lint => DocumentRun {
            report: plan_report(&plan, Status::Passed, true),
            executions: Vec::new(),
            contexts: Vec::new(),
            secret_values: collect_secret_values(&plan.variables),
        },
        Mode::Plan => DocumentRun {
            report: plan_report(&plan, Status::Planned, true),
            executions: Vec::new(),
            contexts: Vec::new(),
            secret_values: collect_secret_values(&plan.variables),
        },
        Mode::List => DocumentRun {
            report: plan_report(&plan, Status::Planned, false),
            executions: Vec::new(),
            contexts: Vec::new(),
            secret_values: collect_secret_values(&plan.variables),
        },
    };
    if mode != Mode::Test {
        for (step_ordinal, step) in document.report.steps.iter().enumerate() {
            on_step(step_ordinal, step);
        }
    }
    document.report.diagnostics.extend(outcome.diagnostics);
    document.report.path = path.display().to_string();
    document.report.duration_ms = started.elapsed().as_millis() as u64;
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
) -> DocumentRun
where
    F: FnMut(usize, &StepReport),
{
    let mut variables = plan.variables.clone();
    let mut step_summaries = Map::new();
    let mut session = ExecutionSession::new();
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
                            "--offline prevents curl transfers and external commands in test mode",
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
        return DocumentRun {
            report: document,
            executions: vec![None; plan.steps.len()],
            contexts: vec![None; plan.steps.len()],
            secret_values: collect_secret_values(&variables),
        };
    }
    let mut steps = Vec::new();
    let mut executions = Vec::new();
    let mut contexts = Vec::new();
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
        let tokens = &step.raw_tokens;
        if step.kind == StepKind::Exec
            && step
                .templates
                .iter()
                .flatten()
                .flat_map(Template::expressions)
                .any(|expression| template_expression_is_secret(expression, &variables))
        {
            report.diagnostics.push(
                Diagnostic::error(
                    "MDOK-E404",
                    "Secret in external command argv",
                    "secret values may only enter an exec process through a declared secret_env mapping",
                )
                .at_file(&plan.path)
                .at_step(step.name.clone()),
            );
        }
        let rendered_command = normalize_command(
            tokens,
            Some(&step.templates),
            &variables,
            &plan.path,
            &mut report.diagnostics,
            false,
            false,
        );
        let mut display_diagnostics = Vec::new();
        report.command = normalize_command(
            tokens,
            Some(&step.templates),
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
            executions.push(None);
            contexts.push(None);
            on_step(step_ordinal, steps.last().expect("step was just pushed"));
            if options.fail_fast || config.fail_fast {
                break;
            }
            continue;
        }
        let context_result = match step.kind {
            StepKind::Curl => transfer(
                &rendered_command,
                config,
                &variables_to_value(&variables),
                &Value::Object(step_summaries.clone()),
                &mut session,
            ),
            StepKind::Exec => execute_external_command(
                &rendered_command,
                config,
                &variables_to_value(&variables),
                &Value::Object(step_summaries.clone()),
            ),
        };
        let mut execution = None;
        let mut context_for_step = None;
        match context_result {
            Ok(context) => {
                context_for_step = Some(context.clone());
                if step.kind == StepKind::Exec {
                    execution = execution_result_from_context(&context, &report.command);
                }
                if step.kind == StepKind::Exec
                    && context.get("success").and_then(Value::as_bool) == Some(false)
                {
                    report.status = Status::Failed;
                    let timed_out = context
                        .get("timed_out")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let output_limit_exceeded = context
                        .get("output_limit_exceeded")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let code = if timed_out {
                        mdok_command::E_TIMEOUT
                    } else if output_limit_exceeded {
                        mdok_command::E_LIMIT
                    } else {
                        mdok_command::E_EXIT
                    };
                    let message = if timed_out {
                        "external command exceeded its configured timeout".to_owned()
                    } else if output_limit_exceeded {
                        "external command exceeded its combined output limit".to_owned()
                    } else {
                        format!(
                            "external command exited with status {}",
                            context
                                .get("exit_code")
                                .and_then(Value::as_i64)
                                .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                        )
                    };
                    report.diagnostics.push(
                        Diagnostic::error(code, "External command failed", message)
                            .at_file(&plan.path)
                            .at_step(step.name.clone()),
                    );
                }
                for expression in &step.checks {
                    let result = compiled_jmespath(&plan.jmespath, expression)
                        .and_then(|compiled| evaluate_check(compiled, &context));
                    match result {
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
                    if step.kind == StepKind::Exec
                        && context.get("secret_tainted").and_then(Value::as_bool) == Some(true)
                        && !step.captures.is_empty()
                    {
                        report.diagnostics.push(
                            Diagnostic::error(
                                "MDOK-E404",
                                "Secret-tainted command output cannot be captured",
                                "remove captures or use a non-secret command profile",
                            )
                            .at_file(&plan.path)
                            .at_step(step.name.clone()),
                        );
                    } else {
                        publish_captures(
                            &step.captures,
                            &context,
                            &mut variables,
                            &plan.jmespath,
                            &plan.path,
                            &step.name,
                            &mut report.diagnostics,
                        );
                    }
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
        executions.push(execution);
        contexts.push(context_for_step);
        on_step(step_ordinal, steps.last().expect("step was just pushed"));
        if failed && (options.fail_fast || config.fail_fast) {
            break;
        }
    }
    DocumentRun {
        report: DocumentReport {
            path: plan.path.display().to_string(),
            status: if steps.iter().any(|step| step.status.is_failure()) {
                Status::Failed
            } else {
                Status::Passed
            },
            duration_ms: 0,
            steps,
            diagnostics: Vec::new(),
        },
        executions,
        contexts,
        secret_values: collect_secret_values(&variables),
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
    session: &mut ExecutionSession,
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
    let response = plan
        .execute_in_session(&policy, session)
        .map_err(curl_diagnostic)?;
    let artifact = if let Some(destination) = &config.artifact_path {
        let artifact = response
            .body
            .promote_to_artifact(destination, &policy, config.max_body as u64)
            .map_err(curl_diagnostic)?;
        Some(relative_artifact_reference(artifact, &config.config_root))
    } else {
        None
    };
    response
        .evaluation_json_limited_with_artifact(variables, steps, config.max_body, artifact.as_ref())
        .map_err(curl_diagnostic)
}

fn relative_artifact_reference(mut artifact: BodyArtifact, root: &Path) -> BodyArtifact {
    if let Ok(relative) = artifact.path.strip_prefix(root) {
        artifact.path = relative.to_path_buf();
    } else if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = artifact.path.strip_prefix(cwd)
    {
        artifact.path = relative.to_path_buf();
    }
    artifact
}

#[allow(clippy::result_large_err)]
fn execute_external_command(
    argv: &[String],
    config: &EffectiveConfig,
    variables: &Value,
    steps: &Value,
) -> Result<Value, Diagnostic> {
    let policy = CommandPolicy {
        profiles: config
            .command_profiles
            .iter()
            .map(|(name, profile)| {
                (
                    name.clone(),
                    mdok_command::CommandProfile {
                        program: profile.program.clone(),
                        env: profile
                            .env
                            .iter()
                            .map(|(key, value)| (key.clone(), OsString::from(value)))
                            .collect(),
                        secret_env: profile
                            .secret_env
                            .iter()
                            .map(|(key, value)| (key.clone(), OsString::from(value)))
                            .collect(),
                        working_directory: config.command_working_directory.clone(),
                    },
                )
            })
            .collect(),
        timeout: config.command_timeout,
        max_output_bytes: config.max_command_output_bytes,
        max_args: config.max_command_args,
        max_arg_bytes: config.max_command_arg_bytes,
        max_argv_bytes: config.max_command_argv_bytes,
    };
    let output = run_external_command(argv, &policy).map_err(command_diagnostic)?;
    Ok(command_context(&output, variables, steps))
}

fn command_context(output: &ProcessOutput, variables: &Value, steps: &Value) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout_json = serde_json::from_str::<Value>(&stdout).ok();
    json!({
        "kind": "exec",
        "program": output.program.display().to_string(),
        "argv": &output.argv,
        "exit_code": output.exit_code,
        "signal": output.signal,
        "success": output.success,
        "timed_out": output.timed_out,
        "output_limit_exceeded": output.output_limit_exceeded,
        "output_truncated": output.output_truncated,
        "stdout": stdout,
        "stderr": stderr,
        "stdout_json": stdout_json,
        "stdout_bytes": output.stdout_bytes,
        "stderr_bytes": output.stderr_bytes,
        "secret_env_used": output.secret_env_used,
        "secret_tainted": output.secret_env_used,
        "duration_ms": output.duration.as_millis() as u64,
        "variables": variables,
        "steps": steps,
    })
}

fn execution_result_from_context(
    context: &Value,
    redacted_argv: &[String],
) -> Option<ExternalExecutionResult> {
    (context.get("kind")?.as_str()? == "exec").then(|| ExternalExecutionResult {
        program: context
            .get("program")
            .and_then(Value::as_str)
            .unwrap_or("<configured command>")
            .to_owned(),
        argv: redacted_argv.to_vec(),
        exit_code: context
            .get("exit_code")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        signal: context
            .get("signal")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        timed_out: context
            .get("timed_out")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        output_limit_exceeded: context
            .get("output_limit_exceeded")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        output_truncated: context
            .get("output_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        stdout_bytes: context
            .get("stdout_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        stderr_bytes: context
            .get("stderr_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        duration_ms: context
            .get("duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

fn command_diagnostic(error: mdok_command::CommandError) -> Diagnostic {
    Diagnostic::error(error.code, "External command error", error.message)
}

fn curl_diagnostic(error: CurlError) -> Diagnostic {
    Diagnostic::error(error.code, "Curl transfer error", error.message)
}

fn publish_captures(
    captures: &[String],
    context: &Value,
    variables: &mut BTreeMap<String, Variable>,
    compiled_expressions: &BTreeMap<String, jmespath::Expression<'static>>,
    path: &Path,
    step_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic_start = diagnostics.len();
    let mut published = BTreeMap::new();
    let mut total_capture_bytes = variables
        .values()
        .try_fold(0usize, |total, variable| {
            let size = serde_json::to_vec(&variable.value).ok()?.len();
            total.checked_add(size)
        })
        .unwrap_or(usize::MAX);
    for expression in captures {
        let compiled = match compiled_jmespath(compiled_expressions, expression) {
            Ok(compiled) => compiled,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("MDOK-E501", "Capture expression unavailable", error)
                        .at_file(path)
                        .at_step(step_name.to_string()),
                );
                continue;
            }
        };
        let result = match compiled.search(context) {
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
        let serialized = result.to_string();
        if serialized.len() > MAX_CAPTURE_BYTES {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E700",
                    "Capture resource limit exceeded",
                    format!("capture result exceeds {MAX_CAPTURE_BYTES} bytes"),
                )
                .at_file(path)
                .at_step(step_name.to_string()),
            );
            continue;
        }
        let json: Value = match serde_json::from_str(&serialized) {
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
        if object.len() > MAX_CAPTURE_KEYS || value_depth(&json) > MAX_CAPTURE_DEPTH {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E700",
                    "Capture resource limit exceeded",
                    format!(
                        "capture must contain at most {MAX_CAPTURE_KEYS} keys and depth {MAX_CAPTURE_DEPTH}"
                    ),
                )
                .at_file(path)
                .at_step(step_name.to_string()),
            );
            continue;
        }
        if total_capture_bytes
            .checked_add(serialized.len())
            .is_none_or(|size| size > MAX_CAPTURE_TOTAL_BYTES)
        {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E700",
                    "Capture resource limit exceeded",
                    format!("captured variables exceed {MAX_CAPTURE_TOTAL_BYTES} bytes"),
                )
                .at_file(path)
                .at_step(step_name.to_string()),
            );
            continue;
        }
        total_capture_bytes += serialized.len();
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
                    secret: is_secret_name(key)
                        || capture_result_is_secret(expression, &json, variables, context),
                },
            );
        }
    }
    if diagnostics.len() == diagnostic_start {
        variables.extend(published);
    }
}

fn compiled_jmespath<'a>(
    compiled_expressions: &'a BTreeMap<String, jmespath::Expression<'static>>,
    source: &str,
) -> Result<&'a jmespath::Expression<'static>, String> {
    let normalized = normalize_jmespath(source);
    compiled_expressions
        .get(&normalized)
        .ok_or_else(|| format!("compiled JMESPath expression is missing: {normalized}"))
}

fn evaluate_check(
    expression: &jmespath::Expression<'static>,
    context: &Value,
) -> Result<bool, String> {
    let result = expression
        .search(context)
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
        Err(SourceReadError::TooLarge { limit, observed }) => {
            return PlanOutcome {
                plan: None,
                diagnostics: vec![
                    Diagnostic::error(
                        "MDOK-E700",
                        "Markdown source limit exceeded",
                        format!(
                            "source is at least {observed} bytes; the maximum is {limit} bytes"
                        ),
                    )
                    .at_file(path),
                ],
                secret_values: config.secret_values(),
            };
        }
        Err(error) => {
            return PlanOutcome {
                plan: None,
                diagnostics: vec![
                    Diagnostic::error("MDOK-E001", "Cannot read document", error.to_string())
                        .at_file(path),
                ],
                secret_values: config.secret_values(),
            };
        }
    };
    let fences = match parse_fences(&source) {
        Ok(fences) => fences,
        Err(error) => {
            return PlanOutcome {
                plan: None,
                diagnostics: vec![
                    Diagnostic::error("MDOK-E700", "Markdown resource limit exceeded", error)
                        .at_file(path),
                ],
                secret_values: config.secret_values(),
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
            "curl" | "exec" => {
                let kind = if fence.language == "curl" {
                    StepKind::Curl
                } else {
                    StepKind::Exec
                };
                if steps.len() >= MAX_STEPS {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E700",
                            "Markdown resource limit exceeded",
                            format!("document contains more than {MAX_STEPS} steps"),
                        )
                        .at_file(path),
                    );
                    continue;
                }
                let Some(name) = fence.attrs.get("name").cloned() else {
                    diagnostics.push(
                        Diagnostic::error(
                            "MDOK-E100",
                            "Missing step name",
                            format!("{} fences require name=...", fence.language),
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
                            format!("{} fence name cannot be empty", fence.language),
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
                        if kind == StepKind::Curl
                            && tokens.len() > 1
                            && tokens.iter().skip(1).any(|token| token == "curl")
                        {
                            diagnostics.push(
                                Diagnostic::error(
                                    "MDOK-E201",
                                    "Forbidden shell construct",
                                    "a command fence may contain only one simple command",
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
                                    "variable assignments are not allowed in command fences",
                                )
                                .at_file(path),
                            );
                            continue;
                        }
                        if kind == StepKind::Curl
                            && tokens.first().map(String::as_str) != Some("curl")
                        {
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
                        let mut step_diagnostics = if kind == StepKind::Curl {
                            validate_command(&tokens, path, config)
                        } else {
                            validate_exec_command(&tokens, path, config)
                        };
                        diagnostics.append(&mut step_diagnostics);
                        steps.push(StepPlan {
                            name,
                            kind,
                            command: tokens,
                            raw_tokens: Vec::new(),
                            templates: Vec::new(),
                            checks: Vec::new(),
                            captures: Vec::new(),
                        });
                    }
                    Err(message) => diagnostics.push(
                        Diagnostic::error(
                            tokenize_error_code(&message),
                            if kind == StepKind::Curl {
                                "Invalid curl syntax"
                            } else {
                                "Invalid external command syntax"
                            },
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
        let tokens = std::mem::take(&mut step.command);
        for token in &tokens {
            validate_template_token(
                token,
                &values,
                &variables,
                &capture_names,
                path,
                &step.name,
                &mut diagnostics,
            );
        }
        let mut ignored = Vec::new();
        let templates = tokens
            .iter()
            .map(|token| {
                if token.contains("{{") {
                    Template::parse(token).ok()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        step.command = normalize_command(
            &tokens,
            Some(&templates),
            &variables,
            path,
            &mut ignored,
            true,
            true,
        );
        step.raw_tokens = tokens;
        step.templates = templates;
        if step.kind == StepKind::Curl && has_url_glob(&step.command) {
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
        if !step.command.iter().any(|argument| argument.contains("{{")) {
            match step.kind {
                StepKind::Curl => {
                    if let Err(error) = CurlPlan::parse(&step.command, &curl_policy(config)) {
                        diagnostics.push(
                            curl_diagnostic(error)
                                .at_file(path)
                                .at_step(step.name.clone()),
                        );
                    }
                }
                StepKind::Exec => {
                    diagnostics.extend(
                        validate_exec_command(&step.command, path, config)
                            .into_iter()
                            .map(|diagnostic| diagnostic.at_step(step.name.clone())),
                    );
                }
            }
        }
    }
    let mut plan = DocumentPlan {
        path: path.to_path_buf(),
        steps,
        variables,
        jmespath: BTreeMap::new(),
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
    let mut compiled_expressions = BTreeMap::new();
    for step in &plan.steps {
        for expression in step.checks.iter().chain(step.captures.iter()) {
            let normalized = normalize_jmespath(expression);
            if compiled_expressions.contains_key(&normalized) {
                continue;
            }
            match jmespath::compile(&normalized) {
                Ok(compiled) => {
                    compiled_expressions.insert(normalized, compiled);
                }
                Err(error) => diagnostics.push(
                    Diagnostic::error("MDOK-E500", "Invalid JMESPath", error.to_string())
                        .at_file(path)
                        .at_step(step.name.clone()),
                ),
            }
        }
    }
    plan.jmespath = compiled_expressions;
    for step in &plan.steps {
        if step.kind == StepKind::Curl {
            for url in positional_args(&step.command) {
                if let Ok(url) = Url::parse(url)
                    && let Err(diagnostic) = enforce_policy(&url, config)
                {
                    diagnostics.push(diagnostic.at_file(path).at_step(step.name.clone()));
                }
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
            secret_values: collect_secret_values(&plan.variables),
        }
    } else {
        let secret_values = collect_secret_values(&plan.variables);
        PlanOutcome {
            plan: Some(plan),
            diagnostics,
            secret_values,
        }
    }
}

fn read_document_source(path: &Path) -> Result<String, SourceReadError> {
    let bytes = if path == Path::new("-") {
        read_bounded_source(io::stdin())?
    } else {
        let file = fs::File::open(path).map_err(SourceReadError::Io)?;
        read_bounded_source(file)?
    };
    String::from_utf8(bytes)
        .map_err(|error| SourceReadError::Io(io::Error::new(io::ErrorKind::InvalidData, error)))
}

fn read_bounded_source<R: Read>(reader: R) -> Result<Vec<u8>, SourceReadError> {
    let mut bytes = Vec::with_capacity((MAX_SOURCE_BYTES + 1).min(64 * 1024));
    reader
        .take((MAX_SOURCE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(SourceReadError::Io)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(SourceReadError::TooLarge {
            limit: MAX_SOURCE_BYTES,
            observed: bytes.len(),
        });
    }
    Ok(bytes)
}

fn markdown_diagnostic(error: &MarkdownError, path: &Path) -> Diagnostic {
    let title = if error.code() == "MDOK-E700" {
        "Markdown resource limit exceeded"
    } else {
        "Markdown planning error"
    };
    Diagnostic::error(error.code(), title, error.to_string()).at_file(path)
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

fn validate_exec_command(
    tokens: &[String],
    path: &Path,
    config: &EffectiveConfig,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(executable) = tokens.first() else {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E202",
                "Invalid external command",
                "an exec fence must contain one non-empty command",
            )
            .at_file(path),
        );
        return diagnostics;
    };
    if executable.contains("{{") || executable.contains('\0') {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E202",
                "Invalid external command",
                "the executable name must be a literal without templates or NUL bytes",
            )
            .at_file(path),
        );
    }
    if executable.starts_with('-') {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E202",
                "Invalid external command",
                "the executable name cannot start with `-`",
            )
            .at_file(path),
        );
    }
    if is_shell_interpreter(executable) {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E307",
                "Shell interpreter denied",
                "shell interpreters cannot be selected by an exec profile",
            )
            .at_file(path),
        );
    }
    if !config.exec_enabled {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E306",
                "External command denied",
                "external commands are disabled; enable policy.exec.enabled",
            )
            .at_file(path),
        );
    } else if !config.command_profiles.contains_key(executable) {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E306",
                "External command denied",
                if config.command_profiles.is_empty() {
                    "no trusted command profiles are configured".to_owned()
                } else {
                    format!("`{executable}` is not in policy.exec.commands")
                },
            )
            .at_file(path),
        );
    }
    diagnostics
}

fn is_shell_interpreter(executable: &str) -> bool {
    let basename = Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();
    matches!(
        basename.as_str(),
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
    )
}

fn has_url_glob(tokens: &[String]) -> bool {
    positional_args(tokens)
        .iter()
        .any(|value| value.contains(['[', ']']))
}

fn normalize_command(
    tokens: &[String],
    templates: Option<&[Option<Template>]>,
    variables: &BTreeMap<String, Variable>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    redact_secrets: bool,
    preserve_missing: bool,
) -> Vec<String> {
    if tokens.len() > MAX_RENDERED_ARG_COUNT {
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E405",
                "Command argv limit exceeded",
                format!("command has more than {MAX_RENDERED_ARG_COUNT} arguments"),
            )
            .at_file(path),
        );
        return Vec::new();
    }
    let mut rendered_tokens = Vec::with_capacity(tokens.len());
    let mut total_bytes = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        let parsed = templates
            .and_then(|templates| templates.get(index))
            .and_then(Option::as_ref);
        let rendered = render_templates(
            token,
            parsed,
            variables,
            path,
            diagnostics,
            redact_secrets,
            preserve_missing,
            MAX_RENDERED_ARGUMENT_BYTES,
        );
        if rendered.len() > MAX_RENDERED_ARGUMENT_BYTES {
            diagnostics.push(
                Diagnostic::error(
                    "MDOK-E405",
                    "Command argv limit exceeded",
                    format!("one argument exceeds {MAX_RENDERED_ARGUMENT_BYTES} bytes"),
                )
                .at_file(path),
            );
            return Vec::new();
        }
        total_bytes = match total_bytes.checked_add(rendered.len()) {
            Some(total) if total <= MAX_RENDERED_ARGV_BYTES => total,
            _ => {
                diagnostics.push(
                    Diagnostic::error(
                        "MDOK-E405",
                        "Command argv limit exceeded",
                        format!("command argv exceeds {MAX_RENDERED_ARGV_BYTES} bytes"),
                    )
                    .at_file(path),
                );
                return Vec::new();
            }
        };
        rendered_tokens.push(rendered);
    }
    rendered_tokens
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
        "-m",
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

#[allow(clippy::too_many_arguments)]
fn render_templates(
    input: &str,
    parsed_template: Option<&Template>,
    variables: &BTreeMap<String, Variable>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    redact_secrets: bool,
    preserve_missing: bool,
    max_bytes: usize,
) -> String {
    if !input.contains("{{") {
        if input.len() <= max_bytes {
            return input.to_owned();
        }
        diagnostics.push(
            Diagnostic::error(
                "MDOK-E405",
                "Command argv limit exceeded",
                format!("one argument exceeds {max_bytes} bytes"),
            )
            .at_file(path),
        );
        return "[TEMPLATE_LIMIT]".to_string();
    }
    if let Some(parsed) = parsed_template {
        return render_parsed_template(
            parsed,
            variables,
            path,
            diagnostics,
            redact_secrets,
            preserve_missing,
            max_bytes,
        );
    }
    let parsed = match Template::parse(input) {
        Ok(template) => template,
        Err(error) => {
            push_template_error(error, path, diagnostics);
            return "[INVALID_TEMPLATE]".to_string();
        }
    };
    render_parsed_template(
        &parsed,
        variables,
        path,
        diagnostics,
        redact_secrets,
        preserve_missing,
        max_bytes,
    )
}

fn render_parsed_template(
    parsed: &Template,
    variables: &BTreeMap<String, Variable>,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    redact_secrets: bool,
    preserve_missing: bool,
    max_bytes: usize,
) -> String {
    let values = variables_to_value_map(variables);
    let mut output = String::with_capacity(parsed.source.len().min(max_bytes));
    for part in &parsed.parts {
        match part {
            TemplatePart::Literal(value) => {
                if !append_rendered_text(&mut output, value, max_bytes) {
                    push_render_limit_diagnostic(path, diagnostics, max_bytes);
                    return "[TEMPLATE_LIMIT]".to_string();
                }
            }
            TemplatePart::Expression(expression) => {
                let root = template_root(expression);
                let Some(_variable) = variables.get(root) else {
                    if preserve_missing {
                        let formatted = format_expression(expression);
                        if !append_rendered_text(&mut output, &formatted, max_bytes) {
                            push_render_limit_diagnostic(path, diagnostics, max_bytes);
                            return "[TEMPLATE_LIMIT]".to_string();
                        }
                    } else {
                        diagnostics.push(
                            Diagnostic::error(
                                "MDOK-E401",
                                "Missing variable",
                                format!("variable `{root}` is not defined"),
                            )
                            .at_file(path),
                        );
                        if !append_rendered_text(&mut output, "[MISSING_VARIABLE]", max_bytes) {
                            push_render_limit_diagnostic(path, diagnostics, max_bytes);
                            return "[TEMPLATE_LIMIT]".to_string();
                        }
                    }
                    continue;
                };
                let remaining = max_bytes.saturating_sub(output.len());
                match lookup_template(&values, &expression.path).and_then(|value| {
                    render_expression_with_limit(expression, &values, remaining)
                        .map(|rendered| (value, rendered))
                }) {
                    Ok((_, _rendered))
                        if redact_secrets
                            && template_expression_is_secret(expression, variables) =>
                    {
                        if !append_rendered_text(&mut output, "[REDACTED]", max_bytes) {
                            push_render_limit_diagnostic(path, diagnostics, max_bytes);
                            return "[TEMPLATE_LIMIT]".to_string();
                        }
                    }
                    Ok((_, rendered)) => {
                        if !append_rendered_text(&mut output, &rendered, max_bytes) {
                            push_render_limit_diagnostic(path, diagnostics, max_bytes);
                            return "[TEMPLATE_LIMIT]".to_string();
                        }
                    }
                    Err(error) => {
                        push_template_error(error, path, diagnostics);
                        if !append_rendered_text(&mut output, "[INVALID_TEMPLATE]", max_bytes) {
                            push_render_limit_diagnostic(path, diagnostics, max_bytes);
                            return "[TEMPLATE_LIMIT]".to_string();
                        }
                    }
                }
            }
        }
    }
    output
}

fn append_rendered_text(output: &mut String, value: &str, max_bytes: usize) -> bool {
    let Some(total) = output.len().checked_add(value.len()) else {
        return false;
    };
    if total > max_bytes {
        return false;
    }
    output.push_str(value);
    true
}

fn push_render_limit_diagnostic(path: &Path, diagnostics: &mut Vec<Diagnostic>, max_bytes: usize) {
    diagnostics.push(
        Diagnostic::error(
            "MDOK-E405",
            "Command argv limit exceeded",
            format!("rendered argument exceeds {max_bytes} bytes"),
        )
        .at_file(path),
    );
}

fn is_secret_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '_'], "");
    normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("apikey")
}

fn template_expression_is_secret(
    expression: &TemplateExpression,
    variables: &BTreeMap<String, Variable>,
) -> bool {
    let Some(PathPart::Key(root)) = expression.path.first() else {
        return false;
    };
    variables
        .get(root)
        .is_some_and(|variable| variable_path_is_secret(variable, &expression.path[1..]))
}

fn variable_path_is_secret(variable: &Variable, path: &[PathPart]) -> bool {
    if variable.secret {
        return true;
    }
    let mut value = &variable.value;
    for part in path {
        match part {
            PathPart::Key(key) => {
                if is_secret_name(key) {
                    return true;
                }
                let Some(next) = value.get(key) else {
                    return false;
                };
                value = next;
            }
            PathPart::Index(index) => {
                let Some(next) = value.get(*index) else {
                    return false;
                };
                value = next;
            }
        }
    }
    value_contains_secret_field(value)
}

fn value_contains_secret_field(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(value_contains_secret_field),
        Value::Object(object) => object
            .iter()
            .any(|(key, value)| is_secret_name(key) || value_contains_secret_field(value)),
        _ => false,
    }
}

fn collect_secret_values(variables: &BTreeMap<String, Variable>) -> Vec<String> {
    let mut values = Vec::new();
    for (name, variable) in variables {
        collect_tainted_strings(
            &variable.value,
            variable.secret || is_secret_name(name),
            &mut values,
        );
    }
    values
}

fn collect_tainted_strings(value: &Value, inherited_secret: bool, values: &mut Vec<String>) {
    match value {
        Value::String(text) if inherited_secret && !text.is_empty() => {
            values.push(text.clone());
        }
        Value::Array(items) => {
            for item in items {
                collect_tainted_strings(item, inherited_secret, values);
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                collect_tainted_strings(child, inherited_secret || is_secret_name(key), values);
            }
        }
        _ => {}
    }
}

fn expression_mentions_secret(expression: &str, variables: &BTreeMap<String, Variable>) -> bool {
    expression
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        })
        .filter(|identifier| !identifier.is_empty())
        .any(|identifier| {
            is_secret_name(identifier)
                || variables
                    .get(identifier)
                    .is_some_and(|variable| variable.secret)
        })
}

fn value_contains_known_secret(value: &Value, secrets: &[String]) -> bool {
    match value {
        Value::String(text) => secrets
            .iter()
            .filter(|secret| !secret.is_empty())
            .any(|secret| text == secret || text.contains(secret)),
        Value::Array(items) => items
            .iter()
            .any(|item| value_contains_known_secret(item, secrets)),
        Value::Object(object) => object
            .values()
            .any(|item| value_contains_known_secret(item, secrets)),
        _ => false,
    }
}

fn value_depth(value: &Value) -> usize {
    value_depth_limited(value, MAX_CAPTURE_DEPTH + 1)
}

fn value_depth_limited(value: &Value, remaining: usize) -> usize {
    if remaining == 0 {
        return MAX_CAPTURE_DEPTH + 1;
    }
    match value {
        Value::Array(items) => {
            1 + items
                .iter()
                .map(|item| value_depth_limited(item, remaining - 1))
                .max()
                .unwrap_or(0)
        }
        Value::Object(object) => {
            1 + object
                .values()
                .map(|item| value_depth_limited(item, remaining - 1))
                .max()
                .unwrap_or(0)
        }
        _ => 1,
    }
}

fn capture_result_is_secret(
    expression: &str,
    result: &Value,
    variables: &BTreeMap<String, Variable>,
    context: &Value,
) -> bool {
    if context.get("secret_tainted").and_then(Value::as_bool) == Some(true)
        || expression_mentions_secret(expression, variables)
        || value_contains_secret_field(result)
    {
        return true;
    }
    let mut known_secrets = collect_secret_values(variables);
    collect_tainted_strings(context, false, &mut known_secrets);
    value_contains_known_secret(result, &known_secrets)
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
        TemplateError::Limit(_) => ("MDOK-E404", "Template resource limit exceeded"),
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
        if let Err(error) = lookup_template(values, &expression.path).and_then(|_| {
            render_expression_with_limit(expression, values, MAX_RENDERED_ARGUMENT_BYTES)
        }) {
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

fn parse_fences(source: &str) -> Result<Vec<Fence>, String> {
    let mut fences = Vec::new();
    let mut fence_count = 0;
    let mut lines = source.split_inclusive('\n');
    while let Some(raw_line) = lines.next() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        let Some(info) = trimmed.strip_prefix("```") else {
            continue;
        };
        if info.trim().is_empty() {
            continue;
        }
        fence_count += 1;
        if fence_count > MAX_FENCES {
            return Err(format!(
                "document contains more than {MAX_FENCES} fenced code blocks"
            ));
        }
        let mut body = String::new();
        for raw_body in lines.by_ref() {
            let body_line = raw_body.trim_end_matches(['\r', '\n']);
            if body_line.trim() == "```" {
                break;
            }
            let body_bytes = body_line.len().checked_add(1).ok_or_else(|| {
                format!("fenced code block exceeds the {MAX_FENCE_BODY_BYTES}-byte budget")
            })?;
            if body
                .len()
                .checked_add(body_bytes)
                .is_none_or(|size| size > MAX_FENCE_BODY_BYTES)
            {
                return Err(format!(
                    "fenced code block exceeds the {MAX_FENCE_BODY_BYTES}-byte budget"
                ));
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
        if fences.len() >= MAX_EXECUTABLE_BLOCKS {
            return Err(format!(
                "document contains more than {MAX_EXECUTABLE_BLOCKS} executable blocks"
            ));
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
        fences.push(Fence {
            language,
            attrs,
            body,
        });
    }
    Ok(fences)
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
    let file = if let Some(path) = config_path.clone() {
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
    let command_timeout = options
        .timeout
        .as_deref()
        .or(file.execution.command_timeout.as_deref())
        .map(parse_duration)
        .transpose()?
        .unwrap_or(timeout);
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
    let artifact_path = options.artifact.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| config_root.clone())
                .join(path)
        }
    });
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
    let command_working_directory = file
        .policy
        .exec
        .working_directory
        .as_deref()
        .map(|path| resolve_command_directory(&config_root, path))
        .transpose()?;
    let mut command_profiles =
        resolve_command_profiles(&file.policy.exec, &config_root, &mut vars)?;
    for configured in &file.policy.allowed_commands {
        let program = resolve_command_program(&config_root, configured)?;
        command_profiles
            .entry(configured.clone())
            .or_insert(ResolvedCommandProfile {
                program,
                env: BTreeMap::new(),
                secret_env: BTreeMap::new(),
            });
    }
    let allow_private_network = file.policy.allow_private_network
        || file
            .policy
            .allowed_hosts
            .iter()
            .any(|host| matches!(host.as_str(), "*" | "localhost" | "127.0.0.1" | "::1"));
    Ok(EffectiveConfig {
        config_path: config_path.map(|path| fs::canonicalize(&path).unwrap_or(path)),
        config_root,
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
        artifact_path,
        command_timeout,
        max_command_output_bytes: file.execution.max_command_output_bytes.max(1),
        max_command_args: file.execution.max_command_args.max(1),
        max_command_arg_bytes: file.execution.max_command_arg_bytes.max(1),
        max_command_argv_bytes: file.execution.max_command_argv_bytes.max(1),
        exec_enabled: file.policy.exec.enabled || !file.policy.allowed_commands.is_empty(),
        command_working_directory,
        command_profiles,
    })
}

fn resolve_command_profiles(
    policy: &ExecPolicyConfig,
    config_root: &Path,
    variables: &mut BTreeMap<String, Variable>,
) -> Result<BTreeMap<String, ResolvedCommandProfile>, Box<CliError>> {
    let mut profiles = BTreeMap::new();
    for (name, profile) in &policy.commands {
        if !valid_name(name) {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E312",
                    "Invalid command profile",
                    format!("command profile `{name}` is not a valid name"),
                ),
            ));
        }
        let program = resolve_command_program(config_root, &profile.program)?;
        let mut env = BTreeMap::new();
        for (key, value) in &profile.env {
            validate_command_environment_name(key)?;
            env.insert(key.clone(), value.clone());
        }
        let mut secret_env = BTreeMap::new();
        for (key, variable_name) in &profile.secret_env {
            validate_command_environment_name(key)?;
            let Some(variable) = variables.get_mut(variable_name) else {
                return Err(cli_error(
                    EXIT_INPUT,
                    Diagnostic::error(
                        "MDOK-E404",
                        "Missing command secret",
                        format!(
                            "command profile `{name}` maps `{key}` from undefined variable `{variable_name}`"
                        ),
                    ),
                ));
            };
            // An explicit secret_env mapping is a taint boundary even when
            // the source variable has a neutral name such as `value`. Mark
            // it before reports and invocation envelopes collect secrets.
            variable.secret = true;
            let Some(value) = variable.value.as_str() else {
                return Err(cli_error(
                    EXIT_INPUT,
                    Diagnostic::error(
                        "MDOK-E404",
                        "Invalid command secret",
                        format!(
                            "command profile `{name}` secret variable `{variable_name}` must be a string"
                        ),
                    ),
                ));
            };
            secret_env.insert(key.clone(), value.to_owned());
        }
        profiles.insert(
            name.clone(),
            ResolvedCommandProfile {
                program,
                env,
                secret_env,
            },
        );
    }
    Ok(profiles)
}

fn resolve_command_program(config_root: &Path, configured: &str) -> Result<PathBuf, Box<CliError>> {
    if configured.is_empty() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command profile",
                "command profile program cannot be empty",
            ),
        ));
    }
    let configured_path = PathBuf::from(configured);
    let candidate = if configured_path.is_absolute() {
        configured_path
    } else {
        config_root.join(configured_path)
    };
    let canonical = canonicalize_command_program(&candidate).map_err(|error| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command profile",
                format!("cannot resolve `{}`: {error}", candidate.display()),
            ),
        )
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command profile",
                format!("cannot inspect `{}`: {error}", canonical.display()),
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command profile",
                format!("`{}` is not a regular file", canonical.display()),
            ),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(cli_error(
                EXIT_INPUT,
                Diagnostic::error(
                    "MDOK-E312",
                    "Invalid command profile",
                    format!("`{}` is not executable", canonical.display()),
                ),
            ));
        }
    }
    Ok(canonical)
}

fn canonicalize_command_program(candidate: &Path) -> Result<PathBuf, std::io::Error> {
    #[cfg(windows)]
    {
        match fs::canonicalize(candidate) {
            Ok(path) => Ok(path),
            Err(original) => {
                let mut with_extension = candidate.to_path_buf();
                if with_extension.extension().is_none() {
                    with_extension.set_extension("exe");
                    fs::canonicalize(with_extension).map_err(|_| original)
                } else {
                    Err(original)
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        fs::canonicalize(candidate)
    }
}

fn resolve_command_directory(
    config_root: &Path,
    configured: &str,
) -> Result<PathBuf, Box<CliError>> {
    let configured_path = PathBuf::from(configured);
    let candidate = if configured_path.is_absolute() {
        configured_path
    } else {
        config_root.join(configured_path)
    };
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command working directory",
                format!("cannot resolve `{}`: {error}", candidate.display()),
            ),
        )
    })?;
    if !canonical.is_dir() {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command working directory",
                format!("`{}` is not a directory", canonical.display()),
            ),
        ));
    }
    Ok(canonical)
}

fn validate_command_environment_name(name: &str) -> Result<(), Box<CliError>> {
    let upper = name.to_ascii_uppercase();
    let blocked = matches!(
        upper.as_str(),
        "LD_PRELOAD" | "LD_LIBRARY_PATH" | "DYLD_INSERT_LIBRARIES" | "DYLD_LIBRARY_PATH"
    ) || upper.starts_with("PYTHONPATH")
        || upper == "PYTHONINSPECT"
        || upper == "NODE_OPTIONS"
        || upper == "RUBYOPT"
        || upper == "PERL5OPT"
        || upper == "BASH_ENV"
        || upper == "ENV";
    if name.is_empty() || name.contains('=') || name.contains('\0') || blocked {
        return Err(cli_error(
            EXIT_INPUT,
            Diagnostic::error(
                "MDOK-E312",
                "Invalid command environment",
                format!("environment variable `{name}` is not permitted"),
            ),
        ));
    }
    Ok(())
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
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        cli_error(
            EXIT_INPUT,
            Diagnostic::error("MDOK-E001", "Invalid duration", value.to_string()),
        )
    })
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
    fn nested_secret_paths_are_tainted_at_argv_and_report_boundaries() {
        let variables = [(
            "credentials".to_string(),
            Variable {
                value: json!({
                    "public": "kept",
                    "nested": {"token": "nested-secret"}
                }),
                secret: false,
            },
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let secret = Template::parse("tool {{credentials.nested.token}}").unwrap();
        let public = Template::parse("tool {{credentials.public}}").unwrap();
        assert!(template_expression_is_secret(
            secret.expressions().next().unwrap(),
            &variables
        ));
        assert!(!template_expression_is_secret(
            public.expressions().next().unwrap(),
            &variables
        ));

        let mut diagnostics = Vec::new();
        let redacted = normalize_command(
            &["tool {{credentials.nested.token}}".to_string()],
            Some(&[Some(secret)]),
            &variables,
            Path::new("nested.md"),
            &mut diagnostics,
            true,
            false,
        );
        assert_eq!(redacted, vec!["tool [REDACTED]".to_string()]);
        assert!(diagnostics.is_empty());
        assert_eq!(collect_secret_values(&variables), vec!["nested-secret"]);
        assert_eq!(
            Redactor::new(collect_secret_values(&variables)).redact_text("nested-secret"),
            "[REDACTED]"
        );
    }

    #[test]
    fn rendered_argv_has_aggregate_count_and_byte_limits() {
        let tokens = (0..=MAX_RENDERED_ARG_COUNT)
            .map(|index| {
                if index == 0 {
                    "curl".to_string()
                } else {
                    "value".to_string()
                }
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        assert!(
            normalize_command(
                &tokens,
                None,
                &BTreeMap::new(),
                Path::new("argv.md"),
                &mut diagnostics,
                false,
                false,
            )
            .is_empty()
        );
        assert_eq!(diagnostics.last().unwrap().code, "MDOK-E405");

        let value = "x".repeat(1024 * 1024);
        let variables = [(
            "value".to_string(),
            Variable {
                value: Value::String(value),
                secret: false,
            },
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let template = Template::parse("{{value}}").unwrap();
        let tokens = std::iter::once("curl".to_string())
            .chain((0..9).map(|_| "{{value}}".to_string()))
            .collect::<Vec<_>>();
        let templates = std::iter::once(None)
            .chain((0..9).map(|_| Some(template.clone())))
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        assert!(
            normalize_command(
                &tokens,
                Some(&templates),
                &variables,
                Path::new("argv-bytes.md"),
                &mut diagnostics,
                false,
                false,
            )
            .is_empty()
        );
        assert_eq!(diagnostics.last().unwrap().code, "MDOK-E405");
    }

    #[test]
    fn derived_capture_values_remain_secret_tainted_and_bounded() {
        assert!(value_contains_known_secret(
            &json!("prefix-nested-secret-suffix"),
            &["nested-secret".to_string()]
        ));
        let context = json!({"secret_tainted": true});
        assert!(capture_result_is_secret(
            "join(@, `x`)",
            &json!({"value": "derived"}),
            &BTreeMap::new(),
            &context,
        ));
        let mut nested = json!("leaf");
        for _ in 0..=MAX_CAPTURE_DEPTH {
            nested = json!({"nested": nested});
        }
        assert!(value_depth(&nested) > MAX_CAPTURE_DEPTH);
    }

    #[test]
    fn oversized_cli_durations_are_normal_input_errors() {
        let error = parse_duration("1e300").unwrap_err();
        assert_eq!(error.diagnostic.code, "MDOK-E001");
    }

    #[test]
    fn bounded_source_read_rejects_input_before_parser_use() {
        let source = vec![b'x'; MAX_SOURCE_BYTES + 1];
        let error = read_bounded_source(io::Cursor::new(source)).unwrap_err();
        assert!(matches!(error, SourceReadError::TooLarge { .. }));
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
