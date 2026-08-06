//! `mdok-pm-probe` — CLI probe for the QuickJS Postman facade.
//!
//! ```
//! mdok-pm-probe --case PATH.json [--network offline|fetch] [--timeout-ms N]
//! mdok-pm-probe --case -            # read case from stdin
//! mdok-pm-probe --list-api          # emit supported API surface JSON, exit 0
//! ```
//!
//! Errors print `{"ok":false,"error":"..."}` to stdout and exit 1. Normal runs
//! print the probe output JSON to stdout and exit 0.

use std::io::Read;
use std::process::ExitCode;
use std::time::Duration;

use mdok_quickjs::{ProbeInput, fetch_executor, list_api, run_script, run_script_with_executor};

const USAGE: &str = "usage: mdok-pm-probe --case PATH.json|--case - [--network offline|fetch] [--timeout-ms N] | --list-api";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            println!(
                r#"{{"ok":false,"error":{}}}"#,
                serde_json::to_string(&message).unwrap()
            );
            ExitCode::FAILURE
        }
    }
}

struct Args {
    case: Option<String>,
    network: String,
    timeout_ms: Option<u64>,
    list_api: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut out = Args {
        case: None,
        network: "offline".to_string(),
        timeout_ms: None,
        list_api: false,
    };
    let mut saw_dashdash = false;
    while let Some(arg) = args.next() {
        if saw_dashdash {
            if out.case.is_none() {
                out.case = Some(arg);
            } else {
                return Err(format!("unexpected positional argument: {arg}\n{USAGE}"));
            }
            continue;
        }
        match arg.as_str() {
            "--case" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("--case requires a value\n{USAGE}"))?;
                out.case = Some(value);
            }
            "--network" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("--network requires a value\n{USAGE}"))?;
                if value != "offline" && value != "fetch" {
                    return Err(format!("--network must be offline or fetch, got {value:?}"));
                }
                out.network = value;
            }
            "--timeout-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("--timeout-ms requires a value\n{USAGE}"))?;
                out.timeout_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid --timeout-ms value: {value:?}"))?,
                );
            }
            "--list-api" => out.list_api = true,
            "--" => saw_dashdash = true,
            other => return Err(format!("unknown argument: {other}\n{USAGE}")),
        }
    }
    Ok(out)
}

fn read_case(path: &str) -> Result<ProbeInput, String> {
    let mut source = String::new();
    if path == "-" {
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| format!("failed to read case from stdin: {e}"))?;
    } else {
        source = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read case file {path:?}: {e}"))?;
    }
    serde_json::from_str(&source).map_err(|e| format!("invalid case JSON: {e}"))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.list_api {
        let value = list_api();
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let case_path = args
        .case
        .as_deref()
        .ok_or_else(|| format!("missing --case\n{USAGE}"))?;
    let mut input = read_case(case_path)?;
    if let Some(timeout_ms) = args.timeout_ms {
        input.profile.script_timeout_ms = timeout_ms;
    }
    let output = if args.network == "fetch" {
        let timeout = Duration::from_millis(input.profile.script_timeout_ms.max(1));
        let mut executor = fetch_executor(timeout);
        run_script_with_executor(&input, &mut executor)
    } else {
        run_script(&input)
    };
    if output.ok {
        println!(
            "{}",
            serde_json::to_string(&output).map_err(|e| e.to_string())?
        );
        Ok(())
    } else {
        let message = output
            .transcript
            .errors
            .first()
            .cloned()
            .unwrap_or_else(|| "probe run failed".to_string());
        Err(message)
    }
}
