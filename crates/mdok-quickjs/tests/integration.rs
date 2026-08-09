//! Integration tests for the mdok-quickjs Postman facade (spec section 7).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use mdok_quickjs::effect::{ChildRequest, ChildRequestResult};
use mdok_quickjs::{
    Outcome, ProbeInput, Profile, RequestBody, RequestData, ResponseData, VariableSet, run_script,
    run_script_with_executor,
};

fn str_map(pairs: &[(&str, &str)]) -> BTreeMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
        .collect()
}

/// Hard 30-second watchdog for every test in this suite. A test that exceeds
/// the budget (for example a QuickJS interrupt regression) fails loudly
/// instead of hanging `cargo test`. All tests in this crate are wrapped in
/// `run_bounded`, so the whole suite is bounded by 30s per test no matter what
/// the sandbox does.
fn run_bounded<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = tx.send(catch_unwind(AssertUnwindSafe(f)));
    });
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(())) => {}
        Ok(Err(payload)) => std::panic::resume_unwind(payload),
        Err(_) => {
            // The worker may be stuck in a tight loop (e.g. a broken QuickJS
            // interrupt handler); detach it and fail the test loudly.
            drop(handle);
            panic!("test exceeded the 30s hard timeout");
        }
    }
}

fn case(script: &str) -> ProbeInput {
    ProbeInput {
        script: script.to_string(),
        phase: "test".to_string(),
        request: Some(RequestData {
            name: "Get user".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.test/users/1".to_string(),
            headers: vec![],
            body: None,
        }),
        response: Some(ResponseData {
            code: Some(200),
            status: "OK".to_string(),
            headers: vec![],
            body: r#"{"id":1,"name":"ada"}"#.to_string(),
            response_time_ms: Some(12),
            response_size_bytes: Some(42),
        }),
        variables: VariableSet::default(),
        secrets: vec![],
        profile: Profile {
            script_timeout_ms: 2000,
            ..Profile::default()
        },
        coverage: true,
    }
}

/// A tiny HTTP/1.1 client over std TcpStream, used as the child-request
/// executor in the fetch-mode test.
fn tcp_executor(addr: std::net::SocketAddr) -> impl FnMut(&ChildRequest) -> ChildRequestResult {
    move |req: &ChildRequest| {
        let path = req
            .url
            .split_once("://")
            .and_then(|(_, rest)| rest.find('/').map(|i| &rest[i..]))
            .unwrap_or("/");
        let mut raw = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            req.method, path, addr
        );
        for (k, v) in &req.headers {
            raw.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(body) = &req.body {
            raw.push_str(&format!("Content-Length: {}\r\n", body.len()));
            raw.push_str("\r\n");
            raw.push_str(body);
        } else {
            raw.push_str("\r\n");
        }
        let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            Ok(s) => s,
            Err(e) => {
                return ChildRequestResult {
                    op: req.op,
                    ok: false,
                    status: None,
                    status_text: None,
                    headers: vec![],
                    body: None,
                    error: Some(format!("connect failed: {e}")),
                    response_time_ms: None,
                };
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(1))).ok();
        if stream.write_all(raw.as_bytes()).is_err() {
            return ChildRequestResult {
                op: req.op,
                ok: false,
                status: None,
                status_text: None,
                headers: vec![],
                body: None,
                error: Some("write failed".to_string()),
                response_time_ms: None,
            };
        }
        // Read headers, then exactly Content-Length bytes (no EOF stall).
        let mut response = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end;
        loop {
            if let Some(pos) = response.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
            match stream.read(&mut chunk) {
                Ok(0) => {
                    header_end = response.len();
                    break;
                }
                Ok(n) => response.extend_from_slice(&chunk[..n]),
                Err(_) => {
                    header_end = response.len();
                    break;
                }
            }
        }
        let header_text = String::from_utf8_lossy(&response[..header_end]).into_owned();
        let mut content_length = 0usize;
        for line in header_text.split("\r\n").skip(1) {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case("content-length") {
                    content_length = v.trim().parse::<usize>().unwrap_or(0);
                }
            }
        }
        while response.len() < header_end + content_length {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => response.extend_from_slice(&chunk[..n]),
            }
        }
        let text = String::from_utf8_lossy(&response).into_owned();
        let mut lines = text.split("\r\n");
        let status_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        let status = parts.get(1).and_then(|s| s.parse::<u16>().ok());
        let mut headers = Vec::new();
        let mut body = String::new();
        let mut in_body = false;
        for line in lines {
            if in_body {
                body.push_str(line);
                body.push('\n');
            } else if line.is_empty() {
                in_body = true;
            } else if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        body = body.trim_end_matches('\n').to_string();
        ChildRequestResult {
            op: req.op,
            ok: status.is_some(),
            status,
            status_text: parts.get(2).map(|s| s.to_string()),
            headers,
            body: Some(body),
            error: if status.is_some() {
                None
            } else {
                Some("malformed response".to_string())
            },
            response_time_ms: Some(1),
        }
    }
}

/// Spin up a loopback server that answers a canned JSON response.
///
/// Every blocking call is bounded (1s I/O timeouts, 10s accept window) so the
/// test can never hang the `cargo test` run (30s budget rule).
fn loopback_server() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let addr = listener.local_addr().expect("local addr");
    let handle = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut served = 0;
        let mut last_accept = std::time::Instant::now();
        while std::time::Instant::now() < deadline && served < 8 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    last_accept = std::time::Instant::now();
                    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
                    stream.set_write_timeout(Some(Duration::from_secs(1))).ok();
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    loop {
                        match stream.read(&mut chunk) {
                            Ok(0) => break,
                            Ok(n) => {
                                buf.extend_from_slice(&chunk[..n]);
                                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    let body = r#"{"ok":true,"echo":"pong"}"#;
                    let response = format!(
                        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nX-Echo: 1\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    served += 1;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Exit once the test's requests have been served and the
                    // connection has been idle, so join() never hangs.
                    if std::time::Instant::now() - last_accept > Duration::from_secs(1) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (addr, handle)
}

// ---------------------------------------------------------------------------
// 1. pm.test pass/fail + response chains + expect matchers
// ---------------------------------------------------------------------------

#[test]
fn test_response_chains_and_expect_matchers() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.test("status is 200", function () {
            pm.response.to.have.status(200);
            pm.response.to.be.ok;
            pm.response.to.have.jsonBody();
            pm.expect(pm.response.json()).to.have.property("id", 1);
        });
        pm.test("expect matchers", function () {
            pm.expect({ a: 1 }).to.eql({ a: 1 });
            pm.expect("hello world").to.include("world");
            pm.expect([1, 2, 3]).to.be.oneOf ? 0 : 0;
            pm.expect(1).to.be.oneOf([1, 2]);
            pm.expect("ada").to.be.a("string");
            pm.expect([1, 2, 3]).to.have.lengthOf(3);
        });
        pm.test("failing test", function () {
            pm.expect(1).to.equal(2);
        });
        "#,
        );
        let output = run_script(&input);
        assert!(output.ok);
        assert_eq!(output.outcome, Outcome::Failed);
        assert_eq!(output.transcript.tests.len(), 3);
        assert!(output.transcript.tests[0].passed);
        assert!(output.transcript.tests[1].passed);
        assert!(!output.transcript.tests[2].passed);
        let err = output.transcript.tests[2].error.clone().unwrap();
        assert!(err.contains("expected 1 to equal 2"), "error: {err}");
    });
}

// ---------------------------------------------------------------------------
// 2. variable precedence + writes + replaceIn
// ---------------------------------------------------------------------------

#[test]
fn test_variable_precedence_and_scopes() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.environment.set("user_id", "42");
        pm.variables.set("local_only", "L");
        pm.globals.set("from_global", "G");
        pm.test("precedence", function () {
            pm.expect(pm.variables.get("user_id")).to.equal("42");
            pm.expect(pm.environment.get("user_id")).to.equal("42");
            // global shadows nothing: global -> collection -> environment -> data -> local
            pm.expect(pm.variables.get("from_global")).to.equal("G");
            var tpl = pm.variables.replaceIn("u={{user_id}} g={{from_global}} missing={{nope}}");
            pm.expect(tpl).to.equal("u=42 g=G missing={{nope}}");
            pm.expect(pm.variables.has("local_only")).to.be.true;
            pm.expect(pm.environment.has("local_only")).to.be.false;
        });
        "#,
        );
        input.variables.global = str_map(&[("from_global", "G"), ("shared", "global")]);
        input.variables.collection = str_map(&[("shared", "collection")]);
        input.variables.environment = str_map(&[("shared", "environment")]);
        input.variables.data = str_map(&[("shared", "data")]);
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert_eq!(output.transcript.tests.len(), 1);
        assert!(output.transcript.tests[0].passed);
        // environment -> local -> global writes recorded in order
        let scopes: Vec<String> = output
            .transcript
            .scope_writes
            .iter()
            .map(|w| w.scope.clone())
            .collect();
        assert_eq!(scopes, vec!["environment", "local", "global"]);
        // scope writes recorded with rendered values
        let env_write = &output.transcript.scope_writes[0];
        assert_eq!(env_write.key, "user_id");
        assert_eq!(env_write.value, "42");
    });
}

#[test]
fn test_variable_precedence_global_wins() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.test("precedence", function () {
            pm.expect(pm.variables.get("shared")).to.equal("global");
        });
        "#,
        );
        input.variables.global = str_map(&[("shared", "global")]);
        input.variables.collection = str_map(&[("shared", "collection")]);
        input.variables.environment = str_map(&[("shared", "environment")]);
        input.variables.data = str_map(&[("shared", "data")]);
        input.variables.local = str_map(&[("shared", "local")]);
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed);
    });
}

// ---------------------------------------------------------------------------
// 3. sendRequest offline + fetch
// ---------------------------------------------------------------------------

#[test]
fn test_send_request_offline_rejects() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.sendRequest("https://api.example.test/x").then(function (res) {
            pm.test("should not resolve", function () { pm.expect(true).to.be.false; });
        }).catch(function (err) {
            pm.test("offline rejected", function () {
                pm.expect(String(err)).to.include("MDOK-PM-NETWORK-OFFLINE");
            });
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        let req = &output.transcript.child_requests[0];
        assert_eq!(req.method, "GET");
        assert!(!(req.resolved));
        assert!(
            req.error
                .as_ref()
                .unwrap()
                .contains("MDOK-PM-NETWORK-OFFLINE")
        );
        let diag = output
            .diagnostics
            .iter()
            .find(|d| d.code == "MDOK-PM-NETWORK-OFFLINE");
        assert!(diag.is_some(), "expected offline diagnostic");
    });
}

#[test]
fn test_send_request_fetch_loopback() {
    run_bounded(|| {
        let (addr, server) = loopback_server();
        let mut input = case(&format!(
            r#"
        pm.sendRequest({{ url: "http://{addr}/ping", method: "POST", header: {{ "X-Test": "1" }}, body: {{ mode: "raw", raw: "hello" }} }}).then(function (res) {{
            pm.test("child status", function () {{ pm.expect(res.code).to.equal(201); }});
            pm.test("child body", function () {{
                pm.expect(res.text()).to.include("pong");
                pm.expect(res.json().ok).to.be.true;
            }});
            pm.test("child header", function () {{
                pm.expect(res.headers.get("X-Echo")).to.equal("1");
            }});
            // nested sendRequest from a callback keeps op ids increasing
            pm.sendRequest("http://{addr}/second").then(function (res2) {{
                pm.test("nested child", function () {{ pm.expect(res2.code).to.equal(201); }});
            }});
        }}).catch(function (err) {{
            pm.test("fetch failed", function () {{ throw err; }});
        }});
        "#,
        ));
        // The executor may legitimately spend up to ~1s per request on I/O; give
        // the script a generous wall-clock budget so network time is not counted
        // as script CPU time (the test still completes in a couple of seconds).
        input.profile.script_timeout_ms = 15_000;
        let mut executor = tcp_executor(addr);
        let output = run_script_with_executor(&input, &mut executor);
        server.join().unwrap();
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        let tests: Vec<(String, bool)> = output
            .transcript
            .tests
            .iter()
            .map(|t| (t.name.clone(), t.passed))
            .collect();
        assert!(
            tests.iter().all(|(_, passed)| *passed),
            "failed tests: {tests:?}"
        );
        assert_eq!(output.transcript.child_requests.len(), 2);
        assert_eq!(output.transcript.child_requests[0].op, 1);
        assert_eq!(output.transcript.child_requests[1].op, 2);
        assert!(output.transcript.child_requests.iter().all(|r| r.resolved));
    });
}

// ---------------------------------------------------------------------------
// 4. coverage: exact used_api
// ---------------------------------------------------------------------------

#[test]
fn test_coverage_used_api_exact() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.test("status", function () { pm.response.to.have.status(200); });
        pm.environment.set("k", "v");
        var v = pm.variables.get("k");
        console.log("ok");
        "#,
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed);
        assert_eq!(
            output.used_api,
            vec![
                "pm.test",
                "pm.response.to.have.status",
                "pm.environment.set",
                "pm.variables.get",
            ]
        );
    });
}

#[test]
fn test_coverage_disabled() {
    run_bounded(|| {
        let mut input = case("pm.test('x', function() { pm.response.to.have.status(200); });");
        input.coverage = false;
        let output = run_script(&input);
        assert!(output.used_api.is_empty());
    });
}

// ---------------------------------------------------------------------------
// 5. secrets redaction
// ---------------------------------------------------------------------------

#[test]
fn test_secrets_never_leak() {
    run_bounded(|| {
        let mut input = case(
            r#"
        var tok = pm.variables.get("token");
        console.log("token:", tok);
        pm.test("secret", function () {
            pm.expect(tok).to.equal("opaque-secret");
            throw new Error("boom " + tok);
        });
        pm.environment.set("token", tok);
        pm.sendRequest({ url: "https://api.example.test/leak?t=" + tok }).catch(function () {});
        "#,
        );
        input.variables.environment = str_map(&[("token", "opaque-secret")]);
        input.secrets = vec!["token".to_string(), "X-Token".to_string()];
        let output = run_script(&input);
        let blob = serde_json::to_string(&output).unwrap();
        assert!(
            !blob.contains("opaque-secret"),
            "tainted value leaked: {blob}"
        );
        // the transcript still records the structure with [redacted]
        assert!(blob.contains("[redacted]"));
        assert!(output.transcript.logs[0].message.contains("[redacted]"));
        let write = &output.transcript.scope_writes[0];
        assert!(write.redacted);
        assert_eq!(write.value, "[redacted]");
        assert!(
            output.transcript.tests[0]
                .error
                .as_ref()
                .unwrap()
                .contains("[redacted]")
        );
        let child = &output.transcript.child_requests[0];
        assert!(child.redacted);
        assert!(child.url.contains("[redacted]"));
    });
}

// ---------------------------------------------------------------------------
// 6. timeout
// ---------------------------------------------------------------------------

#[test]
fn test_busy_loop_times_out() {
    run_bounded(|| {
        let mut input = case("while (true) {}");
        input.profile.script_timeout_ms = 200;
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Timeout);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == "MDOK-PM-TIMEOUT")
        );
    });
}

#[test]
fn test_promise_job_times_out() {
    run_bounded(|| {
        // A job (promise callback) that never yields must trip the interrupt too.
        let mut input = case(
            "pm.sendRequest('https://api.example.test/x').catch(function () { while (true) {} });",
        );
        input.profile.script_timeout_ms = 300;
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Timeout);
    });
}

// ---------------------------------------------------------------------------
// 7. expect failure -> failed test, not a crash
// ---------------------------------------------------------------------------

#[test]
fn test_expect_failure_is_failed_test() {
    run_bounded(|| {
        let input = case(
            "pm.test('fail', function () { pm.expect(1).to.equal(2); });\npm.test('ok', function () { pm.expect(1).to.equal(1); });",
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Failed);
        assert!(output.transcript.errors.is_empty());
        assert!(!output.transcript.tests[0].passed);
        assert!(output.transcript.tests[1].passed);
    });
}

#[test]
fn test_exception_inside_test_fn_is_failed_test() {
    run_bounded(|| {
        let input = case("pm.test('boom', function () { throw new Error('inside'); });");
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Failed);
        assert!(output.transcript.errors.is_empty());
        assert!(!output.transcript.tests[0].passed);
        assert!(
            output.transcript.tests[0]
                .error
                .as_ref()
                .unwrap()
                .contains("inside")
        );
    });
}

// ---------------------------------------------------------------------------
// 8. lodash require
// ---------------------------------------------------------------------------

#[test]
fn test_lodash_require() {
    run_bounded(|| {
        let input = case(
            r#"
        var _ = require("lodash");
        pm.test("lodash works", function () {
            pm.expect(_.get({ a: { b: 2 } }, "a.b")).to.equal(2);
            pm.expect(_.toUpper("hi")).to.equal("HI");
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert!(output.used_api.contains(&"require:lodash".to_string()));
    });
}

#[test]
fn test_unknown_require_diagnostic() {
    run_bounded(|| {
        let input = case(
            r#"
        try {
            require("no-such-module");
        } catch (e) {
            pm.test("refused", function () { pm.expect(String(e)).to.include("MDOK-PM-REQUIRE"); });
        }
        "#,
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == "MDOK-PM-REQUIRE")
        );
        assert!(
            output
                .used_api
                .contains(&"require:no-such-module".to_string())
        );
    });
}

// ---------------------------------------------------------------------------
// 9. unknown pm member
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_pm_member_diagnostic_and_throw() {
    run_bounded(|| {
        let input = case("pm.foo.bar();");
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Error);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == "MDOK-PM-UNSUPPORTED" && d.api == "pm.foo")
        );
        assert!(output.used_api.contains(&"pm.foo".to_string()));
        assert!(!output.transcript.errors.is_empty());
    });
}

// ---------------------------------------------------------------------------
// 10. misc surface: cookies, control flow, visualizer, vault, eval
// ---------------------------------------------------------------------------

#[test]
fn test_cookies_and_control_flow_and_visualizer() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.test("cookies", function () {
            pm.expect(pm.cookies.get("sid")).to.equal("abc123");
            pm.expect(pm.cookies.has("sid")).to.be.true;
        });
        pm.execution.setNextRequest("Next");
        pm.execution.skipRequest();
        pm.visualizer.set("<b>{{x}}</b>", { x: 1 });
        "#,
        );
        input.response.as_mut().unwrap().headers = vec![mdok_quickjs::Header {
            key: "Set-Cookie".into(),
            value: "sid=abc123; Path=/".into(),
        }];
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert_eq!(output.transcript.control_flow.len(), 2);
        assert_eq!(output.transcript.control_flow[0].action, "set_next_request");
        assert!(output.transcript.control_flow[0].supported);
        assert_eq!(output.transcript.control_flow[1].action, "skip_request");
        let viz = output.transcript.visualizer.as_ref().unwrap();
        assert_eq!(viz.template, "<b>{{x}}</b>");
        assert_eq!(viz.data, r#"{"x":1}"#);
    });
}

#[test]
fn test_run_request_unsupported() {
    run_bounded(|| {
        let input = case(
            r#"
        try {
            pm.execution.runRequest("Other");
        } catch (e) {
            pm.test("runRequest refused", function () {
                pm.expect(String(e)).to.include("MDOK-PM-UNSUPPORTED");
            });
        }
        "#,
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed);
        assert_eq!(output.transcript.control_flow[0].action, "run_request");
        assert!(!output.transcript.control_flow[0].supported);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == "MDOK-PM-UNSUPPORTED")
        );
    });
}

#[test]
fn test_vault_granted_and_denied() {
    run_bounded(|| {
        // F6: pm.vault.get now resolves with an opaque redacted placeholder for
        // both granted and denied names (no enumeration oracle), and never
        // returns the raw secret value.
        let mut input = case(
            r#"
        var results = {};
        pm.vault.get("token").then(function (v) {
            results.token = v;
            pm.test("token resolved placeholder", function () {
                pm.expect(v).to.equal("[redacted-vault-value]");
            });
        });
        pm.vault.get("missing").then(function (v) {
            results.missing = v;
            pm.test("missing resolved placeholder", function () {
                pm.expect(v).to.equal("[redacted-vault-value]");
            });
        });
        "#,
        );
        input.variables.environment = str_map(&[("token", "opaque-secret")]);
        input.secrets = vec!["token".to_string()];
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        let blob = serde_json::to_string(&output).unwrap();
        // The raw secret value must never appear in the transcript/output.
        assert!(!blob.contains("opaque-secret"));
    });
}

#[test]
fn test_eval_and_function_disabled() {
    run_bounded(|| {
        let input = case(
            r#"
        try { eval("1+1"); } catch (e) {
            pm.test("eval", function () { pm.expect(String(e)).to.include("MDOK-PM-EVAL"); });
        }
        try { new Function("return 1"); } catch (e) {
            pm.test("function", function () { pm.expect(String(e)).to.include("MDOK-PM-EVAL"); });
        }
        "#,
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed);
        assert!(output.diagnostics.iter().any(|d| d.code == "MDOK-PM-EVAL"));
    });
}

#[test]
fn test_script_syntax_error() {
    run_bounded(|| {
        let input = case("function ( {");
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Error);
        assert!(!output.transcript.errors.is_empty());
    });
}

#[test]
fn test_console_logging_bounded() {
    run_bounded(|| {
        let input = case(
            r#"
        for (var i = 0; i < 500; i++) { console.log("line " + i); }
        "#,
        );
        let output = run_script(&input);
        assert!(output.transcript.logs.len() <= 100, "log cap exceeded");
        assert!(output.diagnostics.iter().any(|d| d.code == "MDOK-PM-LIMIT"));
    });
}

#[test]
fn test_request_facade_shape() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.test("request", function () {
            pm.expect(pm.request.method).to.equal("GET");
            pm.expect(pm.request.url).to.equal("https://api.example.test/users/1");
            pm.expect(pm.request.headers.has("X-Token")).to.be.true;
            pm.expect(pm.request.headers.get("x-token")).to.equal("tok-value");
            pm.expect(pm.request.headers.count()).to.equal(1);
            pm.expect(pm.request.body).to.be.null;
            pm.expect(pm.request.auth).to.be.null;
        });
        "#,
        );
        input.request.as_mut().unwrap().headers = vec![mdok_quickjs::Header {
            key: "X-Token".into(),
            value: "tok-value".into(),
        }];
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_request_body_and_data_alias() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.test("body", function () {
            pm.expect(pm.request.body.mode).to.equal("raw");
            pm.expect(pm.request.body.raw).to.equal("{\"a\":1}");
            pm.expect(pm.request.data).to.eql({ mode: "raw", raw: "{\"a\":1}", toJSON: pm.request.body.toJSON });
            pm.expect(pm.request.body.toJSON()).to.eql({ mode: "raw", raw: "{\"a\":1}" });
        });
        "#,
        );
        input.request.as_mut().unwrap().body = Some(RequestBody {
            mode: "raw".into(),
            raw: Some(r#"{"a":1}"#.into()),
        });
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_response_to_have_chains_fail_outside_test() {
    run_bounded(|| {
        let input = case("pm.response.to.have.status(404);");
        let output = run_script(&input);
        // Uncaught assertion error -> script error, not a failed test.
        assert_eq!(output.outcome, Outcome::Error);
        assert!(!output.transcript.errors.is_empty());
    });
}

#[test]
fn test_response_aliases_and_json() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.test("response", function () {
            pm.expect(pm.response.code).to.equal(200);
            pm.expect(pm.response.responseCode).to.equal(200);
            pm.expect(pm.response.status).to.equal("OK");
            pm.expect(pm.response.responseTime).to.equal(12);
            pm.expect(pm.response.responseSize).to.equal(42);
            pm.expect(pm.response.text()).to.include("ada");
            pm.expect(pm.response.json().name).to.equal("ada");
            pm.expect(pm.response.toJSON().code).to.equal(200);
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_json_schema_best_effort() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.test("schema", function () {
            pm.response.to.have.jsonSchema({ type: "object", required: ["id"] });
            pm.expect(pm.response).to.have.jsonSchema({ properties: { id: { type: "number" } } });
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_list_api_contract() {
    run_bounded(|| {
        let api = mdok_quickjs::list_api();
        assert_eq!(api.profile, "postman-cli-v1");
        assert!(api.supported.contains(&"pm.test".to_string()));
        assert!(
            api.supported
                .contains(&"pm.response.to.have.status".to_string())
        );
        assert!(api.supported.contains(&"require:lodash".to_string()));
        assert_eq!(
            api.modules,
            vec!["lodash", "moment", "ajv", "uuid", "querystring", "crypto-js"]
        );
        for code in [
            "MDOK-PM-UNSUPPORTED",
            "MDOK-PM-NETWORK-OFFLINE",
            "MDOK-PM-REQUIRE",
            "MDOK-PM-SECRET-DENIED",
            "MDOK-PM-TIMEOUT",
            "MDOK-PM-LIMIT",
            "MDOK-PM-EVAL",
        ] {
            assert!(
                api.diagnostic_codes.contains(&code.to_string()),
                "missing {code}"
            );
        }
    });
}

#[test]
fn test_module_sha256_pinned() {
    run_bounded(|| {
        let digest = mdok_quickjs::modules::module_sha256("lodash").unwrap();
        assert_eq!(digest.len(), 64);
    });
}

// ---------------------------------------------------------------------------
// 11. corpus-driven surface: legacy globals, response chains, extra matchers,
//     additional modules
// ---------------------------------------------------------------------------

#[test]
fn test_legacy_globals() {
    run_bounded(|| {
        let mut input = case(
            r#"
        tests["legacy pass"] = true;
        tests["legacy fail"] = false;
        pm.test("legacy env", function () {
            environment.set("legacy_key", "lv");
            pm.expect(environment.get("legacy_key")).to.equal("lv");
            postman.setEnvironmentVariable("pm_env", "pv");
            pm.expect(postman.getEnvironmentVariable("pm_env")).to.equal("pv");
            pm.expect(responseBody).to.include("ada");
            pm.expect(responseCode.code).to.equal(200);
            pm.expect(responseHeaders["Content-Type"]).to.equal("application/json");
            pm.expect(postman.getResponseHeader("content-type")).to.equal("application/json");
            pm.expect(iteration).to.equal(0);
            pm.expect(request.method).to.equal("GET");
        });
        "#,
        );
        input.response.as_mut().unwrap().headers = vec![mdok_quickjs::Header {
            key: "Content-Type".into(),
            value: "application/json".into(),
        }];
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Failed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert_eq!(output.transcript.tests.len(), 3);
        assert!(output.transcript.tests[0].passed); // legacy pass
        assert!(!output.transcript.tests[1].passed); // legacy fail
        assert!(output.transcript.tests[2].passed); // pm.test legacy env
        assert_eq!(output.transcript.tests[1].name, "legacy fail");
        // scope writes from legacy environment.set
        assert!(
            output
                .transcript
                .scope_writes
                .iter()
                .any(|w| w.scope == "environment" && w.key == "legacy_key")
        );
    });
}

#[test]
fn test_response_to_not_and_be_extras() {
    run_bounded(|| {
        let mut input = case(
            r#"
        pm.test("to.not", function () {
            pm.response.to.not.have.status(404);
            pm.response.to.not.have.status(/^5/);
        });
        pm.test("be extras", function () {
            pm.response.to.be.json;
            pm.response.to.be.withBody;
            pm.response.to.be.success;
        });
        pm.test("payload alias", function () {
            pm.expect(pm.payload.json().name).to.equal("ada");
        });
        "#,
        );
        input.response.as_mut().unwrap().headers = vec![mdok_quickjs::Header {
            key: "Content-Type".into(),
            value: "application/json".into(),
        }];
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert!(
            output
                .used_api
                .contains(&"pm.response.to.not.have.status".to_string())
        );
        assert!(
            output
                .used_api
                .contains(&"pm.response.to.be.json".to_string())
        );
        assert!(output.used_api.contains(&"pm.payload.json".to_string()));
    });
}

#[test]
fn test_expect_aliases_and_extra_matchers() {
    run_bounded(|| {
        let input = case(
            r#"
        pm.test("aliases", function () {
            pm.expect(5).to.equals(5);
            pm.expect(5).to.eq(5);
            pm.expect({ a: 1 }).to.eqls({ a: 1 });
            pm.expect({ a: 1 }).to.haveOwnProperty("a");
            pm.expect({}).to.exist;
            pm.expect(5).valueOf;
            pm.expect(5).to.be.gt(4);
            pm.expect(5).to.be.gte(5);
            pm.expect(5).to.be.lt(6);
            pm.expect(5).to.be.lte(5);
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_additional_modules() {
    run_bounded(|| {
        let input = case(
            r#"
        var moment = require("moment");
        var uuid = require("uuid");
        var qs = require("querystring");
        pm.test("moment", function () {
            pm.expect(moment.utc("2020-01-01").add(3, "months").format("YYYY-MM-DD")).to.equal("2020-04-01");
        });
        pm.test("uuid", function () {
            pm.expect(typeof uuid()).to.equal("string");
            pm.expect(uuid().length).to.equal(36);
            pm.expect(uuid.v4().length).to.equal(36);
        });
        pm.test("querystring", function () {
            pm.expect(qs.parse("a=1&b=2")).to.eql({ a: "1", b: "2" });
            pm.expect(qs.stringify({ x: 1, y: "a b" })).to.equal("x=1&y=a+b");
        });
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        for m in ["require:moment", "require:uuid", "require:querystring"] {
            assert!(output.used_api.contains(&m.to_string()), "missing {m}");
        }
    });
}

#[test]
fn test_ajv_loads_but_compile_is_hardened() {
    run_bounded(|| {
        // ajv's `compile` generates validation code via `new Function`, which
        // the hardened profile removes (MDOK-PM-EVAL). The module itself must
        // still load; the usage fails with the named diagnostic.
        let input = case(
            r#"
        var Ajv = require("ajv");
        pm.test("loads", function () {
            pm.expect(typeof Ajv).to.equal("function");
        });
        var ajv = new Ajv();
        try {
            ajv.compile({ type: "object" });
            pm.test("compile should fail", function () { pm.expect(true).to.be.false; });
        } catch (e) {
            pm.test("compile hardened", function () {
                pm.expect(String(e)).to.include("MDOK-PM-EVAL");
            });
        }
        "#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert!(output.diagnostics.iter().any(|d| d.code == "MDOK-PM-EVAL"));
    });
}

// ---------------------------------------------------------------------------
// 12. timers (setTimeout/setInterval) + xml2Json + `_` legacy global
// ---------------------------------------------------------------------------

#[test]
fn test_timers_set_timeout_fires() {
    run_bounded(|| {
        let input = case(
            "var fired = false;\nsetTimeout(function () { fired = true; pm.environment.set(\"t\", \"1\"); }, 10);\npm.test(\"timer eventually runs\", function () { pm.expect(true).to.equal(true); });",
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert!(
            output
                .transcript
                .scope_writes
                .iter()
                .any(|w| w.scope == "environment" && w.key == "t" && w.value == "1"),
            "timer callback did not run: {:?}",
            output.transcript.scope_writes
        );
    });
}

#[test]
fn test_timers_set_timeout_with_args_and_clear() {
    run_bounded(|| {
        let input = case(
            "var seen = null;\nsetTimeout(function (a, b) { seen = a + b; }, 5, 2, 3);\nvar doomed = setTimeout(function () { seen = \"bad\"; }, 5);\nclearTimeout(doomed);\nsetTimeout(function () { pm.test(\"args\", function () { pm.expect(seen).to.equal(5); }); }, 20);",
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_timers_set_interval_and_clear() {
    run_bounded(|| {
        let input = case(
            "var count = 0;\nvar id = setInterval(function () { count++; if (count >= 2) clearInterval(id); }, 5);\nsetTimeout(function () { pm.test(\"count\", function () { pm.expect(count).to.be.at.least(2); }); }, 40);",
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_timers_callback_exception_is_script_error() {
    run_bounded(|| {
        let input = case("setTimeout(function () { throw new Error(\"boom-timer\"); }, 5);");
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Error);
        assert!(
            output
                .transcript
                .errors
                .iter()
                .any(|e| e.contains("boom-timer")),
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_timers_long_wait_times_out() {
    run_bounded(|| {
        // A 60s no-op timer exceeds the script budget: Postman would kill the
        // script too; the pump must stop at the deadline with a timeout.
        let mut input = case("setTimeout(function () {}, 60000);");
        input.profile.script_timeout_ms = 250;
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Timeout);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == "MDOK-PM-TIMEOUT")
        );
    });
}

#[test]
fn test_timers_can_drive_send_request() {
    run_bounded(|| {
        // A timer callback that issues a child request: offline mode rejects
        // it, and the rejection must be observable (not hang the pump).
        let input = case(
            "setTimeout(function () { pm.sendRequest(\"https://api.example.test/x\").then(function () {}, function () { pm.environment.set(\"sr\", \"rejected\"); }); }, 5);",
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        assert!(
            output
                .transcript
                .scope_writes
                .iter()
                .any(|w| w.key == "sr" && w.value == "rejected")
        );
    });
}

#[test]
fn test_xml2json_basic() {
    run_bounded(|| {
        // Official postman-sandbox test fixture: <food><key>Homestyle
        // Breakfast</key><value>950</value></food>
        let input = case(
            r#"var object = xml2Json("<food><key>Homestyle Breakfast</key><value>950</value></food>").food;
        pm.test("xml2Json", function () {
            pm.expect(object.key).to.equal("Homestyle Breakfast");
            pm.expect(object.value).to.equal("950");
        });"#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_xml2json_soap_namespaces() {
    run_bounded(|| {
        let input = case(
            r#"var j = xml2Json('<soap-env:Envelope xmlns:soap-env="http://schemas.xmlsoap.org/soap/envelope/"><soap-env:Body><n0:SalesOrder xmlns:n0="urn:x"><SalesOrderID>1001</SalesOrderID></n0:SalesOrder></soap-env:Body></soap-env:Envelope>');
        pm.test("soap", function () {
            pm.expect(j["soap-env:Envelope"]["soap-env:Body"]["n0:SalesOrder"].SalesOrderID).to.equal("1001");
        });"#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_xml2json_attributes_arrays_text() {
    run_bounded(|| {
        let input = case(
            r#"var j = xml2Json("<root id=\"1\"><item>a</item><item>b</item><note>hi</note><empty/></root>").root;
        pm.test("attrs", function () { pm.expect(j.$).to.eql({ id: "1" }); });
        pm.test("arrays", function () { pm.expect(j.item).to.eql(["a", "b"]); });
        pm.test("text", function () { pm.expect(j.note).to.equal("hi"); });
        pm.test("empty", function () { pm.expect(j.empty).to.equal(""); });"#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_xml2json_cdata_entities_and_malformed() {
    run_bounded(|| {
        let input = case(
            r#"var j = xml2Json("<a><b><![CDATA[x < y]]></b><c>1 &amp; 2</c></a>").a;
        pm.test("cdata", function () { pm.expect(j.b).to.equal("x < y"); });
        pm.test("entities", function () { pm.expect(j.c).to.equal("1 & 2"); });
        pm.test("malformed", function () { pm.expect(typeof xml2Json("<oops")).to.equal("object"); });"#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}

#[test]
fn test_underscore_global_lodash() {
    run_bounded(|| {
        let input = case(
            r#"pm.test("_", function () {
            pm.expect(_.get({ a: { b: 1 } }, "a.b")).to.equal(1);
            pm.expect(_.isArray([1])).to.equal(true);
        });"#,
        );
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
    });
}
#[test]
fn test_crypto_js_module() {
    run_bounded(|| {
        let input = case(
            r#"var cryptoJS = require("crypto-js");
        pm.test("crypto-js", function () {
            pm.expect(typeof cryptoJS.SHA256).to.equal("function");
            pm.expect(cryptoJS.SHA256("abc").toString().length).to.equal(64);
            var b64 = cryptoJS.enc.Base64.stringify(cryptoJS.enc.Utf8.parse("hello"));
            pm.expect(b64).to.equal("aGVsbG8=");
        });"#,
        );
        let output = run_script(&input);
        assert_eq!(output.outcome, Outcome::Passed, "errors: {:?}", output.transcript.errors);
    });
}

/// F10 regression: logging a long run of a 3-byte UTF-8 character must not
/// panic. The byte-limit truncation (4096, 4096 % 3 == 1) previously cut
/// mid-character and `String::truncate` aborted the process (panic=abort),
/// a one-line DoS of the long-lived MCP server. The char-boundary-safe
/// helper must truncate without panicking.
#[test]
fn test_multibyte_log_truncation_does_not_panic() {
    run_bounded(|| {
        let input = case(
            // '日' is U+65E5 (3 bytes in UTF-8); 20000 reps = 60000 bytes, far
            // over the 4096-byte log limit. The truncate point (4096) is not a
            // char boundary (4096 % 3 == 1).
            "console.log('\u{65e5}'.repeat(20000));",
        );
        // This call must not abort the test process. If the fix regresses, the
        // binary exits 134 (SIGABRT) and this test fails to even report.
        let output = run_script(&input);
        assert_eq!(
            output.outcome,
            Outcome::Passed,
            "errors: {:?}",
            output.transcript.errors
        );
        // The log was truncated to a char boundary under the byte limit.
        let logged = output
            .transcript
            .logs
            .first()
            .map(|l| l.message.len())
            .unwrap_or(0);
        assert!(logged <= 4096, "log must be truncated to <= 4096 bytes, got {logged}");
    });
}
