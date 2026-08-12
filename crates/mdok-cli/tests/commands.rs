use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::{TempDir, tempdir};

const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_LINES: usize = 100_000;

fn run_mdok<I, S>(directory: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_mdok"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("mdok command should start")
}

fn run_mdok_with_stdin<I, S>(directory: &Path, args: I, input: &[u8]) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(env!("CARGO_BIN_EXE_mdok"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mdok command should start");
    {
        let mut stdin = child.stdin.take().expect("stdin pipe should be available");
        stdin
            .write_all(input)
            .expect("stdin should accept test input");
    }
    child
        .wait_with_output()
        .expect("mdok command should finish")
}

fn canonical_temp_root(directory: &TempDir) -> PathBuf {
    fs::canonicalize(directory.path()).expect("temporary directory should canonicalize")
}

fn json_output(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "expected JSON-only stderr, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should contain JSON ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn write_exec_config(directory: &Path) -> PathBuf {
    let config = directory.join("mdok.toml");
    let program = serde_json::to_string(env!("CARGO_BIN_EXE_mdok"))
        .expect("mdok executable path should serialize");
    fs::write(
        &config,
        format!(
            "[policy.exec]\nenabled = true\n\n[policy.exec.commands.fixture]\nprogram = {program}\n"
        ),
    )
    .expect("temporary command policy should be writable");
    config
}

fn explicit_record_path(directory: &Path) -> PathBuf {
    directory.join("recording.md")
}

fn record_inline(directory: &Path, output: &Path, source: &str) -> Output {
    run_mdok(
        directory,
        vec![
            OsString::from("record"),
            OsString::from("--output"),
            output.as_os_str().to_os_string(),
            OsString::from("--content"),
            OsString::from(source),
        ],
    )
}

fn assert_private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path)
            .expect("recorded file should have metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "recorded file should be owner-private: {path:?}"
        );
    }
}

#[test]
fn explicit_env_files_load_in_order_and_cli_values_win() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let first = root.join("first.env");
    let second = root.join("second.env");
    let document = root.join("api.md");
    fs::write(
        &first,
        "base_url=https://first.example.test\nregion=first\nAPI_TOKEN=never-print\n",
    )
    .expect("first environment file should be writable");
    fs::write(
        &second,
        "export base_url=https://second.example.test # selected\nregion='quoted value'\n",
    )
    .expect("second environment file should be writable");
    fs::write(
        &document,
        "```curl mdok name=example\ncurl \"{{base_url}}/{{region|url}}/{{mode|url}}\" --header \"Authorization: Bearer {{API_TOKEN|header}}\"\n```\n",
    )
    .expect("Markdown document should be writable");

    let output = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("--env-file"),
            first.as_os_str().to_os_string(),
            OsString::from("--env-file"),
            second.as_os_str().to_os_string(),
            OsString::from("--var"),
            OsString::from("mode=cli"),
            OsString::from("plan"),
            document.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let report = json_output(&output);
    let command = report["documents"][0]["steps"][0]["command"]
        .as_array()
        .expect("planned command should be an array");
    assert_eq!(command[1], "https://second.example.test/quoted%20value/cli");
    assert_eq!(command[3], "Authorization: Bearer [REDACTED]");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print"));
}

#[test]
fn invalid_explicit_env_file_fails_before_planning() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let environment = root.join("invalid.env");
    let document = root.join("api.md");
    fs::write(&environment, "VALID=one\nnot an assignment\n")
        .expect("environment file should be writable");
    fs::write(&document, "# API example\n").expect("Markdown document should be writable");

    let output = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("--env-file"),
            environment.as_os_str().to_os_string(),
            OsString::from("plan"),
            document.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let report = json_output(&output);
    assert_eq!(
        report["diagnostics"][0]["title"],
        "Invalid environment file"
    );
    assert!(
        report["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains(":2: expected NAME=VALUE"))
    );
}

#[test]
fn dotenv_files_are_never_discovered_implicitly() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let document = root.join("api.md");
    fs::write(root.join(".env"), "base_url=https://example.test\n")
        .expect("dotenv file should be writable");
    fs::write(
        &document,
        "```curl mdok name=example\ncurl \"{{base_url}}/users\"\n```\n",
    )
    .expect("Markdown document should be writable");

    let output = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("plan"),
            document.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let report = json_output(&output);
    assert!(
        report["documents"][0]["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| !diagnostics.is_empty())
    );
}

#[test]
fn strict_replay_detects_explicit_env_file_changes() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let environment = root.join("replay.env");
    let recording = root.join("recording.md");
    fs::write(&environment, "region=first\n").expect("environment file should be writable");

    let created = run_mdok(
        &root,
        vec![
            OsString::from("--env-file"),
            environment.as_os_str().to_os_string(),
            OsString::from("record"),
            OsString::from("--output"),
            recording.as_os_str().to_os_string(),
            OsString::from("--content"),
            OsString::from("# Recorded example\n"),
        ],
    );
    assert_eq!(created.status.code(), Some(0));

    let exact = run_mdok(
        &root,
        vec![
            OsString::from("--env-file"),
            environment.as_os_str().to_os_string(),
            OsString::from("replay"),
            OsString::from("--strict"),
            recording.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(exact.status.code(), Some(0));

    fs::write(&environment, "region=changed\n").expect("environment file should be writable");
    let changed = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("--env-file"),
            environment.as_os_str().to_os_string(),
            OsString::from("replay"),
            OsString::from("--strict"),
            recording.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(changed.status.code(), Some(2));
    assert_eq!(
        json_output(&changed)["diagnostics"][0]["message"],
        "recording source, configuration, or secret input identifiers changed"
    );
}

#[test]
fn postman_import_writes_canonical_markdown_and_manifest() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let input = root.join("collection.json");
    let output_path = root.join("collection.md");
    let collection = serde_json::json!({
        "info": {
            "name": "Imported API",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "variable": [{"key": "base_url", "value": "https://example.test"}],
        "item": [{
            "name": "Health",
            "request": {
                "method": "GET",
                "url": "{{base_url}}/health",
                "header": [{"key": "X-Test", "value": "mdok"}]
            },
            "event": [{
                "listen": "test",
                "script": {"exec": ["pm.response.to.have.status(200);"]}
            }]
        }]
    });
    fs::write(
        &input,
        serde_json::to_vec(&collection).expect("collection should serialize"),
    )
    .expect("collection should be writable");
    let result = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("import"),
            OsString::from("postman"),
            input.as_os_str().to_os_string(),
            OsString::from("--out"),
            output_path.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(
        result.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result_json = json_output(&result);
    assert_eq!(result_json["operation"], "import");
    assert_eq!(result_json["success"], true);
    assert_eq!(result_json["generated_steps"], 1);
    let markdown = fs::read_to_string(&output_path).expect("generated Markdown should exist");
    assert!(markdown.contains("curl mdok name=health"));
    assert!(markdown.contains("status == "));
    let lint = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("lint"),
            output_path.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(
        lint.status.code(),
        Some(0),
        "generated Markdown should lint: {}",
        String::from_utf8_lossy(&lint.stdout)
    );
    let manifest_path = PathBuf::from(
        result_json["manifest"]
            .as_str()
            .expect("manifest path should be reported"),
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(manifest_path).expect("manifest should exist"))
            .expect("manifest should be JSON");
    assert_eq!(
        manifest["generated_steps"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn postman_import_strict_mode_preserves_review_manifest() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let input = root.join("collection.json");
    let output_path = root.join("collection.md");
    let collection = serde_json::json!({
        "info": {
            "name": "Needs review",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [{
            "name": "Health",
            "request": "https://example.test/health",
            "event": [{
                "listen": "prerequest",
                "script": {"exec": ["pm.variables.set(\"nonce\", \"dynamic\");"]}
            }]
        }]
    });
    fs::write(
        &input,
        serde_json::to_vec(&collection).expect("collection should serialize"),
    )
    .expect("collection should be writable");
    let strict = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("import"),
            OsString::from("postman"),
            input.as_os_str().to_os_string(),
            OsString::from("--out"),
            output_path.as_os_str().to_os_string(),
        ],
    );
    assert_eq!(strict.status.code(), Some(2));
    let strict_json = json_output(&strict);
    assert_eq!(strict_json["diagnostics"][0]["code"], "MDOK-PM-REVIEW");
    assert!(!output_path.exists());
    let manifest_path = PathBuf::from(format!("{}.import.json", output_path.display()));
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("review manifest should exist"))
            .expect("review manifest should be JSON");
    assert_eq!(manifest["issues"][0]["code"], "MDOK-PM-PREREQUEST");

    let lossy = run_mdok(
        &root,
        vec![
            OsString::from("--json"),
            OsString::from("import"),
            OsString::from("postman"),
            input.as_os_str().to_os_string(),
            OsString::from("--out"),
            output_path.as_os_str().to_os_string(),
            OsString::from("--allow-lossy"),
            OsString::from("--force"),
        ],
    );
    assert_eq!(lossy.status.code(), Some(0));
    assert!(output_path.exists());
}

#[test]
fn direct_call_validation_and_output_contract_are_offline() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);

    let mut too_many_args = vec!["--json", "call", "--", "fixture"];
    too_many_args.extend(std::iter::repeat_n("argument", 64));
    let validation = run_mdok(&root, too_many_args);
    assert_eq!(validation.status.code(), Some(2));
    let validation_json = json_output(&validation);
    assert_eq!(validation_json["schema_version"], "1");
    assert_eq!(validation_json["diagnostics"][0]["code"], "MDOK-E001");
    assert!(
        validation_json["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("maximum is 64"))
    );

    let config = write_exec_config(&root);
    let structured = run_mdok(
        &root,
        vec![
            OsStr::new("--config"),
            config.as_os_str(),
            OsStr::new("call"),
            OsStr::new("--"),
            OsStr::new("fixture"),
            OsStr::new("version"),
            OsStr::new("--json"),
        ],
    );
    assert_eq!(structured.status.code(), Some(0));
    let structured_json = json_output(&structured);
    assert_eq!(structured_json["schema_version"], "1");
    assert_eq!(structured_json["operation"], "call");
    assert_eq!(structured_json["success"], true);
    assert_eq!(structured_json["result_kind"], "command");
    assert_eq!(structured_json["request"]["adapter"], "exec");
    assert_eq!(
        structured_json["request"]["argv"],
        serde_json::json!(["fixture", "version", "--json"])
    );
    assert_eq!(structured_json["response"]["kind"], "exec");
    assert_eq!(structured_json["response"]["success"], true);
    assert_eq!(structured_json["response"]["exit_code"], 0);
    assert_eq!(
        structured_json["response"]["stdout_json"]["mdok_version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        structured_json["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
}

#[test]
fn run_accepts_inline_and_stdin_and_enforces_input_limits() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);

    let inline = run_mdok(&root, ["--json", "run", "--content", "# inline input\n"]);
    assert_eq!(inline.status.code(), Some(0));
    let inline_json = json_output(&inline);
    assert_eq!(inline_json["documents"][0]["path"], "<inline>");
    assert_eq!(inline_json["documents"][0]["status"], "passed");

    let stdin = run_mdok_with_stdin(&root, ["--json", "run"], b"# stdin input\n");
    assert_eq!(stdin.status.code(), Some(0));
    let stdin_json = json_output(&stdin);
    assert_eq!(stdin_json["documents"][0]["path"], "<stdin>");
    assert_eq!(stdin_json["documents"][0]["status"], "passed");

    // Use stdin for the line-limit payload because Linux limits each execve
    // argument to less than this otherwise valid MDOK input size.
    let oversized_lines = "x\n".repeat(MAX_SOURCE_LINES + 1);
    let line_limit = run_mdok_with_stdin(&root, ["--json", "run"], oversized_lines.as_bytes());
    assert_eq!(line_limit.status.code(), Some(2));
    let line_limit_json = json_output(&line_limit);
    assert_eq!(
        line_limit_json["documents"][0]["diagnostics"][0]["code"],
        "MDOK-E700"
    );
    assert!(
        line_limit_json["documents"][0]["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("maximum is 100000"))
    );

    let oversized_stdin = vec![b'x'; MAX_SOURCE_BYTES + 1];
    let stdin_limit = run_mdok_with_stdin(&root, ["--json", "run"], &oversized_stdin);
    assert_eq!(stdin_limit.status.code(), Some(2));
    let stdin_limit_json = json_output(&stdin_limit);
    assert_eq!(
        stdin_limit_json["diagnostics"][0]["title"],
        "Markdown source limit exceeded"
    );
    assert!(
        stdin_limit_json["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("8388608 bytes"))
    );
}

#[test]
fn record_default_and_explicit_paths_are_private_and_have_manifests() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);

    let default_source = "# default recording\n";
    let default_output = run_mdok(
        &root,
        vec![
            OsStr::new("record"),
            OsStr::new("--content"),
            OsStr::new(default_source),
        ],
    );
    assert_eq!(default_output.status.code(), Some(0));
    let default_json = json_output(&default_output);
    let default_path = PathBuf::from(
        default_json["recording"]["path"]
            .as_str()
            .expect("default recording path should be reported"),
    );
    assert!(default_path.starts_with(root.join(".mdok/records")));
    assert_eq!(
        default_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("md")
    );
    assert_eq!(
        fs::read(&default_path).expect("default recording should exist"),
        default_source.as_bytes()
    );
    let default_manifest = PathBuf::from(
        default_json["recording"]["manifest_path"]
            .as_str()
            .expect("default manifest path should be reported"),
    );
    assert_eq!(
        default_manifest,
        PathBuf::from(format!("{}.json", default_path.display()))
    );
    assert!(default_manifest.is_file());
    assert_private_file(&default_path);
    assert_private_file(&default_manifest);

    let requested = root.join("explicit-recording");
    let explicit_source = "# explicit recording\n";
    let explicit_output = record_inline(&root, &requested, explicit_source);
    assert_eq!(explicit_output.status.code(), Some(0));
    let explicit_json = json_output(&explicit_output);
    let explicit_path = requested.with_extension("md");
    let explicit_manifest = PathBuf::from(format!("{}.json", explicit_path.display()));
    assert_eq!(
        explicit_json["recording"]["path"],
        explicit_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        fs::read(&explicit_path).unwrap(),
        explicit_source.as_bytes()
    );
    assert!(explicit_manifest.is_file());
    assert_private_file(&explicit_path);
    assert_private_file(&explicit_manifest);

    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&explicit_manifest).expect("explicit manifest should be readable"),
    )
    .expect("explicit manifest should be JSON");
    assert_eq!(manifest["schema_version"], "1");
    assert_eq!(manifest["source_kind"], "inline");
    assert_eq!(manifest["source_sha256"].as_str().map(str::len), Some(64));
}

#[test]
fn record_refuses_to_overwrite_without_force_and_preserves_private_files() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let requested = explicit_record_path(&root);
    let first_source = "# first recording\n";
    let second_source = "# second recording\n";

    let first = record_inline(&root, &requested, first_source);
    assert_eq!(first.status.code(), Some(0));
    let recorded = requested.clone();
    let manifest = PathBuf::from(format!("{}.json", recorded.display()));
    let before_source = fs::read(&recorded).expect("first recording should exist");
    let before_manifest = fs::read(&manifest).expect("first manifest should exist");

    let second = record_inline(&root, &requested, second_source);
    assert_eq!(second.status.code(), Some(2));
    let second_json = json_output(&second);
    assert_eq!(second_json["diagnostics"][0]["code"], "MDOK-E001");
    assert_eq!(fs::read(&recorded).unwrap(), before_source);
    assert_eq!(fs::read(&manifest).unwrap(), before_manifest);
    assert_private_file(&recorded);
    assert_private_file(&manifest);

    let forced = run_mdok(
        &root,
        vec![
            OsStr::new("record"),
            OsStr::new("--force"),
            OsStr::new("--output"),
            requested.as_os_str(),
            OsStr::new("--content"),
            OsStr::new(second_source),
        ],
    );
    assert_eq!(forced.status.code(), Some(0));
    assert_eq!(fs::read(&recorded).unwrap(), second_source.as_bytes());
    assert_private_file(&recorded);
    assert_private_file(&manifest);
}

#[test]
fn replay_strict_fails_closed_for_missing_invalid_and_tampered_manifests() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let requested = explicit_record_path(&root);
    let recorded_source = "# replayable recording\n";
    let created = record_inline(&root, &requested, recorded_source);
    assert_eq!(created.status.code(), Some(0));
    let manifest = PathBuf::from(format!("{}.json", requested.display()));

    fs::remove_file(&manifest).expect("manifest should be removable for the missing case");
    let missing = run_mdok(
        &root,
        vec![
            OsStr::new("--json"),
            OsStr::new("replay"),
            OsStr::new("--strict"),
            requested.as_os_str(),
        ],
    );
    assert_eq!(missing.status.code(), Some(2));
    let missing_json = json_output(&missing);
    assert_eq!(missing_json["diagnostics"][0]["code"], "MDOK-E001");
    assert!(
        missing_json["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("manifest is missing"))
    );

    let non_strict = run_mdok(&root, vec![OsStr::new("replay"), requested.as_os_str()]);
    assert_eq!(non_strict.status.code(), Some(0));
    let non_strict_json = json_output(&non_strict);
    assert_eq!(non_strict_json["operation"], "replay");
    assert_eq!(non_strict_json["success"], true);
    assert_eq!(
        non_strict_json["recording"]["path"],
        requested.to_string_lossy().as_ref()
    );
    assert_eq!(
        non_strict_json["recording"]["manifest_path"],
        manifest.to_string_lossy().as_ref()
    );
    assert_eq!(non_strict_json["recording"]["drift"]["status"], "unknown");

    fs::write(
        &manifest,
        r#"{"schema_version":"1","source_sha256":"0000000000000000000000000000000000000000000000000000000000000000","provenance_sha256":"0000000000000000000000000000000000000000000000000000000000000000","provenance":{"secret_source_ids":[]}}"#,
    )
    .expect("tampered manifest should be writable");
    let tampered = run_mdok(
        &root,
        vec![
            OsStr::new("--json"),
            OsStr::new("replay"),
            OsStr::new("--strict"),
            requested.as_os_str(),
        ],
    );
    assert_eq!(tampered.status.code(), Some(2));
    let tampered_json = json_output(&tampered);
    assert_eq!(tampered_json["diagnostics"][0]["code"], "MDOK-E001");
    assert_eq!(
        tampered_json["diagnostics"][0]["message"],
        "recording source, configuration, or secret input identifiers changed"
    );

    fs::write(&manifest, b"{not valid JSON").expect("invalid manifest should be writable");
    let invalid = run_mdok(
        &root,
        vec![
            OsStr::new("--json"),
            OsStr::new("replay"),
            OsStr::new("--strict"),
            requested.as_os_str(),
        ],
    );
    assert_eq!(invalid.status.code(), Some(2));
    let invalid_json = json_output(&invalid);
    assert_eq!(invalid_json["diagnostics"][0]["code"], "MDOK-E001");
    assert!(
        invalid_json["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("expected") || message.contains("key"))
    );
}

#[test]
fn raw_call_output_is_exactly_the_structured_command_stdout() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let config = write_exec_config(&root);
    let structured = run_mdok(
        &root,
        vec![
            OsStr::new("--config"),
            config.as_os_str(),
            OsStr::new("call"),
            OsStr::new("--"),
            OsStr::new("fixture"),
            OsStr::new("version"),
            OsStr::new("--json"),
        ],
    );
    assert_eq!(structured.status.code(), Some(0));
    let structured_json = json_output(&structured);
    let expected_raw = structured_json["response"]["stdout"]
        .as_str()
        .expect("structured response should contain command stdout");

    let raw = run_mdok(
        &root,
        vec![
            OsStr::new("--config"),
            config.as_os_str(),
            OsStr::new("call"),
            OsStr::new("--raw"),
            OsStr::new("--"),
            OsStr::new("fixture"),
            OsStr::new("version"),
            OsStr::new("--json"),
        ],
    );
    assert_eq!(raw.status.code(), Some(0));
    assert!(raw.stderr.is_empty());
    assert_eq!(String::from_utf8(raw.stdout).unwrap(), expected_raw);
    let raw_json: Value =
        serde_json::from_str(expected_raw).expect("raw output should remain JSON");
    assert_eq!(
        raw_json["mdok_version"],
        structured_json["response"]["stdout_json"]["mdok_version"]
    );
}

#[cfg(unix)]
#[test]
fn direct_argv_template_delimiters_remain_data() {
    let directory = tempdir().expect("temporary directory should be available");
    let root = canonical_temp_root(&directory);
    let config = root.join("mdok.toml");
    fs::write(
        &config,
        "[policy.exec]\nenabled = true\n\n[policy.exec.commands.printf]\nprogram = \"/usr/bin/printf\"\n",
    )
    .expect("printf policy should be writable");
    let output = run_mdok(
        &root,
        vec![
            OsStr::new("--config"),
            config.as_os_str(),
            OsStr::new("call"),
            OsStr::new("--"),
            OsStr::new("printf"),
            OsStr::new("%s"),
            OsStr::new("{{literal}}"),
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let value = json_output(&output);
    assert_eq!(value["response"]["stdout"], "{{literal}}");
}
