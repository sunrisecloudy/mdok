//! Model Context Protocol server for mdok.
//!
//! The MCP surface intentionally mirrors the stable JSON-oriented CLI rather
//! than exposing internal Rust implementation details. Document operations
//! execute the current `mdok` binary in a child process, which keeps command
//! behavior, configuration discovery, policy enforcement, and redaction
//! identical between a terminal and an MCP client. Probe and Postman import
//! use their typed library APIs directly.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use mdok_quickjs::{ProbeInput, Profile, fetch_executor, run_script, run_script_with_executor};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

const DEFAULT_CHILD_TIMEOUT_SECS: u64 = 120;
const MAX_CHILD_TIMEOUT_SECS: u64 = 600;
const MAX_PROBE_TIMEOUT_MS: u64 = 30_000;
const MAX_MCP_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// Start the stdio MCP server and block until the client closes the session.
pub fn serve() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot create MCP runtime: {error}"))?;
    runtime.block_on(async {
        let service = MdokMcp::new()
            .serve(stdio())
            .await
            .map_err(|error| format!("MCP transport failed: {error}"))?;
        service
            .waiting()
            .await
            .map(|_| ())
            .map_err(|error| format!("MCP server stopped: {error}"))
    })
}

#[derive(Clone)]
pub struct MdokMcp {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct DocumentToolArgs {
    /// Markdown files or directories. Paths are resolved from the MCP server's working directory.
    pub paths: Vec<String>,
    /// Non-secret variables passed as KEY=VALUE to mdok.
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    /// Secret variables. Values are passed through a child environment, not argv.
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    /// Additional destination host patterns allowed by the mdok policy.
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    /// Destination host patterns denied by the mdok policy.
    #[serde(default)]
    pub deny_hosts: Vec<String>,
    /// Optional mdok.toml path.
    #[serde(default)]
    pub config: Option<String>,
    /// Optional named environment profile from mdok.toml.
    #[serde(default)]
    pub environment: Option<String>,
    /// Refuse network execution when true.
    #[serde(default)]
    pub offline: bool,
    /// Child-process wall-clock timeout in seconds (capped at 600).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ProbeToolArgs {
    /// Postman JavaScript source.
    pub script: String,
    /// `test` or `prerequest` (also accepts another event name).
    #[serde(default)]
    pub phase: String,
    /// Canonical ProbeInput request object, when this is a request script.
    #[serde(default)]
    pub request: Option<Value>,
    /// Canonical ProbeInput response object, when this is a test script.
    #[serde(default)]
    pub response: Option<Value>,
    /// VariableSet object with global, collection, environment, data, and local maps.
    #[serde(default)]
    pub variables: Option<Value>,
    /// Names whose values must be redacted in transcript output.
    #[serde(default)]
    pub secrets: Vec<String>,
    /// `offline` (default) or `fetch` for pm.sendRequest effects.
    #[serde(default = "default_network")]
    pub network: String,
    /// Override script timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Record API accesses in the returned used_api list.
    #[serde(default = "default_true")]
    pub coverage: bool,
    /// Optional full QuickJS profile override.
    #[serde(default)]
    pub profile: Option<ProfileInput>,
}

/// A JSON representation of a QuickJS profile. Keeping this local gives MCP
/// clients a generated schema while preserving the library's canonical types.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ProfileInput {
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub script_timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_stack_bytes: Option<usize>,
    #[serde(default)]
    pub max_memory_bytes: Option<usize>,
    #[serde(default)]
    pub max_log_entries: Option<usize>,
    #[serde(default)]
    pub max_log_entry_bytes: Option<usize>,
    #[serde(default)]
    pub max_transcript_bytes: Option<usize>,
    #[serde(default)]
    pub max_visualizer_template_bytes: Option<usize>,
    #[serde(default)]
    pub max_visualizer_data_bytes: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ImportToolArgs {
    /// Postman Collection v2.1 JSON text. Exactly one of this and `path` is required.
    #[serde(default)]
    pub collection_json: Option<String>,
    /// Local Postman Collection v2.1 JSON path, resolved from the server working directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Return generated Markdown even when blocking review issues exist.
    #[serde(default)]
    pub allow_lossy: bool,
}

fn default_network() -> String {
    "offline".to_owned()
}

fn default_true() -> bool {
    true
}

#[tool_router]
impl MdokMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Parse, execute, and assert one or more Markdown API documents.
    #[tool(
        description = "Run mdok test and return its redacted JSON report. Use this for end-to-end API tests."
    )]
    async fn mdok_test(
        &self,
        Parameters(args): Parameters<DocumentToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.document_tool("test", args).await
    }

    /// Statistically validate Markdown API documents without network execution.
    #[tool(
        description = "Run mdok lint and return its JSON report. Use this to validate a reusable API workflow before execution."
    )]
    async fn mdok_lint(
        &self,
        Parameters(args): Parameters<DocumentToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.document_tool("lint", args).await
    }

    /// Print normalized, redacted execution plans for Markdown API documents.
    #[tool(
        description = "Run mdok plan and return the normalized redacted execution plan as JSON."
    )]
    async fn mdok_plan(
        &self,
        Parameters(args): Parameters<DocumentToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.document_tool("plan", args).await
    }

    /// List documents, requests, checks, and captures in Markdown API documents.
    #[tool(description = "Run mdok list and return the document workflow inventory as JSON.")]
    async fn mdok_list(
        &self,
        Parameters(args): Parameters<DocumentToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.document_tool("list", args).await
    }

    /// Execute a Postman-compatible JavaScript probe in the QuickJS sandbox.
    #[tool(
        description = "Run a Postman pre-request or test script in mdok's bounded QuickJS sandbox and return transcript, diagnostics, and API coverage."
    )]
    async fn mdok_probe(
        &self,
        Parameters(args): Parameters<ProbeToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        let input = match build_probe_input(args.clone()) {
            Ok(input) => input,
            Err(error) => return Ok(tool_error(error)),
        };
        let network = args.network;
        let timeout =
            Duration::from_millis(input.profile.script_timeout_ms.max(1).saturating_add(5_000));
        let output = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                if network == "fetch" {
                    let request_timeout =
                        Duration::from_millis(input.profile.script_timeout_ms.max(1));
                    let mut executor = fetch_executor(request_timeout);
                    run_script_with_executor(&input, &mut executor)
                } else {
                    run_script(&input)
                }
            }),
        )
        .await;
        match output {
            Ok(Ok(probe)) => Ok(json_result(&probe)),
            Ok(Err(error)) => Ok(tool_error(format!("probe worker failed: {error}"))),
            Err(_) => Ok(tool_error("probe exceeded its MCP wall-clock timeout")),
        }
    }

    /// Convert a Postman Collection v2.1 JSON document into Markdown and a review manifest.
    #[tool(
        description = "Import a Postman Collection v2.1 from JSON text or a local path; return generated Markdown and the redacted import manifest without overwriting files."
    )]
    async fn mdok_import_postman(
        &self,
        Parameters(args): Parameters<ImportToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        let result = tokio::task::spawn_blocking(move || import_postman(args)).await;
        match result {
            Ok(Ok(value)) => Ok(json_result(&value)),
            Ok(Err(error)) => Ok(tool_error(error)),
            Err(error) => Ok(tool_error(format!("import worker failed: {error}"))),
        }
    }

    /// Return mdok and Postman runtime compatibility versions.
    #[tool(
        description = "Return mdok version, curl compatibility, and the Postman QuickJS profile version."
    )]
    async fn mdok_version(&self) -> Result<CallToolResult, McpError> {
        Ok(json_result(&json!({
            "mdok_version": mdok_report::MDOK_VERSION,
            "curl_version": mdok_report::CURL_COMPAT_VERSION,
            "libcurl": mdok_report::LIBCURL_VERSION,
            "tls": mdok_report::TLS_BACKEND,
            "postman_profile": mdok_quickjs::PROFILE_API_VERSION,
            "mcp": true,
        })))
    }

    async fn document_tool(
        &self,
        operation: &'static str,
        args: DocumentToolArgs,
    ) -> Result<CallToolResult, McpError> {
        if args.paths.is_empty() {
            return Ok(tool_error("paths must contain at least one Markdown path"));
        }
        let timeout_secs = args
            .timeout_secs
            .unwrap_or(DEFAULT_CHILD_TIMEOUT_SECS)
            .clamp(1, MAX_CHILD_TIMEOUT_SECS);
        let (argv, env) = match build_document_argv(operation, &args) {
            Ok(value) => value,
            Err(error) => return Ok(tool_error(error)),
        };
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                return Ok(tool_error(format!(
                    "cannot locate mdok executable: {error}"
                )));
            }
        };
        let mut command = tokio::process::Command::new(executable);
        command
            .args(&argv)
            .envs(env)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return Ok(tool_error(format!("cannot start mdok child: {error}"))),
        };
        let output =
            tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;
        match output {
            Ok(Ok(output)) => Ok(document_result(operation, output)),
            Ok(Err(error)) => Ok(tool_error(format!("mdok child failed: {error}"))),
            Err(_) => Ok(tool_error(format!(
                "mdok {operation} exceeded {timeout_secs}s timeout"
            ))),
        }
    }
}

#[tool_handler]
impl ServerHandler for MdokMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(
            Implementation::new("mdok", env!("CARGO_PKG_VERSION"))
                .with_title("mdok API testing")
                .with_description("Markdown-native API testing, Postman compatibility, and reusable workflows"),
        )
        .with_instructions(
            "Use mdok_lint and mdok_plan before mdok_test. Prefer Markdown documents for reusable API workflows; use mdok_import_postman to migrate collections. Network and secrets remain subject to mdok policy and report redaction.".to_owned(),
        )
    }
}

fn build_document_argv(
    operation: &str,
    args: &DocumentToolArgs,
) -> Result<(Vec<String>, BTreeMap<String, String>), String> {
    let mut argv = vec![operation.to_owned(), "--json".to_owned()];
    if args.offline {
        argv.push("--offline".to_owned());
    }
    if let Some(config) = &args.config {
        if config.is_empty() {
            return Err("config must not be empty".to_owned());
        }
        argv.extend(["--config".to_owned(), config.clone()]);
    }
    if let Some(environment) = &args.environment {
        argv.extend(["--env".to_owned(), environment.clone()]);
    }
    for (key, value) in &args.vars {
        validate_env_key(key)?;
        argv.extend(["--var".to_owned(), format!("{key}={value}")]);
    }
    let mut env = BTreeMap::new();
    for (index, (key, value)) in args.secrets.iter().enumerate() {
        validate_env_key(key)?;
        let env_key = format!("MDOK_MCP_SECRET_{index}");
        argv.extend(["--secret".to_owned(), format!("{key}=@env:{env_key}")]);
        env.insert(env_key, value.clone());
    }
    for host in &args.allow_hosts {
        argv.extend(["--allow-host".to_owned(), host.clone()]);
    }
    for host in &args.deny_hosts {
        argv.extend(["--deny-host".to_owned(), host.clone()]);
    }
    if let Some(timeout_secs) = args.timeout_secs {
        let timeout_secs = timeout_secs.clamp(1, MAX_CHILD_TIMEOUT_SECS);
        argv.extend(["--timeout".to_owned(), format!("{timeout_secs}s")]);
    }
    argv.push("--".to_owned());
    argv.extend(args.paths.iter().cloned());
    Ok((argv, env))
}

fn validate_env_key(key: &str) -> Result<(), String> {
    let mut characters = key.chars();
    let valid = matches!(characters.next(), Some(character) if character.is_ascii_alphabetic())
        && key.len() <= 64
        && characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if !valid {
        return Err(format!("invalid variable name {key:?}"));
    }
    Ok(())
}

fn document_result(operation: &str, output: std::process::Output) -> CallToolResult {
    let stdout = &output.stdout[..output.stdout.len().min(MAX_MCP_OUTPUT_BYTES)];
    let result = serde_json::from_slice::<Value>(stdout).unwrap_or_else(|_| {
        json!({
            "error": "mdok child produced no JSON report",
        })
    });
    json_result(&json!({
        "operation": operation,
        "ok": output.status.success(),
        "exit_code": output.status.code(),
        "result": result,
    }))
}

fn build_probe_input(args: ProbeToolArgs) -> Result<ProbeInput, String> {
    if args.network != "offline" && args.network != "fetch" {
        return Err("network must be offline or fetch".to_owned());
    }
    let mut profile = if let Some(profile) = args.profile {
        profile.into_profile()
    } else {
        Profile::default()
    };
    if let Some(timeout_ms) = args.timeout_ms {
        profile.script_timeout_ms = timeout_ms;
    }
    if profile.script_timeout_ms == 0 {
        return Err("timeout_ms must be at least 1".to_owned());
    }
    if profile.script_timeout_ms > MAX_PROBE_TIMEOUT_MS {
        return Err(format!("timeout_ms must not exceed {MAX_PROBE_TIMEOUT_MS}"));
    }
    let request = decode_optional(args.request, "request")?;
    let response = decode_optional(args.response, "response")?;
    let variables = decode_optional(args.variables, "variables")?.unwrap_or_default();
    Ok(ProbeInput {
        script: args.script,
        phase: args.phase,
        request,
        response,
        variables,
        secrets: args.secrets,
        profile,
        coverage: args.coverage,
    })
}

impl ProfileInput {
    fn into_profile(self) -> Profile {
        let mut profile = Profile::default();
        if let Some(value) = self.api_version {
            profile.api_version = value;
        }
        if let Some(value) = self.script_timeout_ms {
            profile.script_timeout_ms = value;
        }
        if let Some(value) = self.max_stack_bytes {
            profile.max_stack_bytes = value;
        }
        if let Some(value) = self.max_memory_bytes {
            profile.max_memory_bytes = value;
        }
        if let Some(value) = self.max_log_entries {
            profile.max_log_entries = value;
        }
        if let Some(value) = self.max_log_entry_bytes {
            profile.max_log_entry_bytes = value;
        }
        if let Some(value) = self.max_transcript_bytes {
            profile.max_transcript_bytes = value;
        }
        if let Some(value) = self.max_visualizer_template_bytes {
            profile.max_visualizer_template_bytes = value;
        }
        if let Some(value) = self.max_visualizer_data_bytes {
            profile.max_visualizer_data_bytes = value;
        }
        profile
    }
}

fn decode_optional<T: DeserializeOwned>(
    value: Option<Value>,
    field: &str,
) -> Result<Option<T>, String> {
    value
        .map(|value| {
            serde_json::from_value(value).map_err(|error| format!("invalid {field}: {error}"))
        })
        .transpose()
}

fn import_postman(args: ImportToolArgs) -> Result<Value, String> {
    let (bytes, source_path): (Vec<u8>, Option<PathBuf>) = match (args.collection_json, args.path) {
        (Some(_), Some(_)) => return Err("provide collection_json or path, not both".to_owned()),
        (None, None) => return Err("one of collection_json or path is required".to_owned()),
        (Some(json), None) => (json.into_bytes(), None),
        (None, Some(path)) => {
            let path_buf = PathBuf::from(path);
            let bytes = std::fs::read(&path_buf)
                .map_err(|error| format!("cannot read collection: {error}"))?;
            (bytes, Some(path_buf))
        }
    };
    let output = mdok_postman::import_collection_bytes(
        &bytes,
        source_path.as_deref(),
        &mdok_postman::ImportOptions {
            allow_lossy: args.allow_lossy,
        },
    )
    .map_err(|error| error.to_string())?;
    let requires_review = output.has_blockers();
    Ok(json!({
        "ok": !requires_review || args.allow_lossy,
        "requires_review": requires_review,
        "markdown": output.markdown,
        "manifest": output.manifest,
    }))
}

fn json_result<T: serde::Serialize>(value: &T) -> CallToolResult {
    let encoded = serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("{{\"error\":{error:?}}}"));
    CallToolResult::success(vec![ContentBlock::text(encoded)])
}

fn tool_error(error: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.into())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn run_bounded<T>(test: impl FnOnce() -> T + Send + 'static) -> T
    where
        T: Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let value = test();
            let _ = sender.send(value);
        });
        receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("test exceeded 30-second watchdog")
    }

    #[test]
    fn document_argv_keeps_secrets_out_of_argv_values() {
        run_bounded(|| {
            let args = DocumentToolArgs {
                paths: vec!["api.md".to_owned()],
                vars: BTreeMap::new(),
                secrets: [("TOKEN".to_owned(), "secret-value".to_owned())]
                    .into_iter()
                    .collect(),
                allow_hosts: vec!["api.example.test".to_owned()],
                deny_hosts: Vec::new(),
                config: None,
                environment: None,
                offline: false,
                timeout_secs: Some(5),
            };
            let (argv, env) = build_document_argv("test", &args).expect("argv");
            let joined = argv.join(" ");
            assert!(!joined.contains("secret-value"));
            assert!(joined.contains("@env:MDOK_MCP_SECRET_0"));
            assert_eq!(
                env.get("MDOK_MCP_SECRET_0"),
                Some(&"secret-value".to_owned())
            );
        });
    }

    #[test]
    fn probe_input_defaults_and_validates_network() {
        run_bounded(|| {
            let input = build_probe_input(ProbeToolArgs {
                script: "pm.test('ok', () => pm.expect(1).to.eql(1));".to_owned(),
                phase: "test".to_owned(),
                request: None,
                response: None,
                variables: None,
                secrets: Vec::new(),
                network: "offline".to_owned(),
                timeout_ms: Some(100),
                coverage: true,
                profile: None,
            })
            .expect("probe input");
            assert_eq!(input.profile.script_timeout_ms, 100);
            assert!(
                build_probe_input(ProbeToolArgs {
                    network: "internet".to_owned(),
                    ..ProbeToolArgs {
                        script: String::new(),
                        phase: String::new(),
                        request: None,
                        response: None,
                        variables: None,
                        secrets: Vec::new(),
                        network: "offline".to_owned(),
                        timeout_ms: None,
                        coverage: true,
                        profile: None,
                    }
                })
                .is_err()
            );
        });
    }

    #[test]
    fn watchdog_constants_are_finite() {
        run_bounded(|| {
            assert!(Duration::from_secs(MAX_CHILD_TIMEOUT_SECS).as_secs() <= 600);
        });
    }
}
