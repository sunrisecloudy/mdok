use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use mdok_curl::{CurlPlan, CurlPolicy, ExecutionSession};
use mdok_report::{CheckReport, DocumentReport, Event, EventMetadata, Report, Status, StepReport};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const BENCH_PATH: &str = "<bench>/document.md";
const NORMAL_STEPS: usize = 10;
const INTENSE_STEPS: usize = 48;

#[derive(Clone, Copy)]
struct WorkloadSpec {
    label: &'static str,
    steps: usize,
    prose_bytes: usize,
}

const NORMAL: WorkloadSpec = WorkloadSpec {
    label: "normal",
    steps: NORMAL_STEPS,
    prose_bytes: 8 * 1024,
};

const INTENSE: WorkloadSpec = WorkloadSpec {
    label: "intense",
    steps: INTENSE_STEPS,
    prose_bytes: 128 * 1024,
};

fn markdown_source(spec: WorkloadSpec, endpoint: &str) -> String {
    let filler =
        "The benchmark document contains ordinary API notes, examples, and response guidance.\n";
    let mut source = String::with_capacity(spec.prose_bytes + spec.steps * 320);
    source.push_str("# MDOK benchmark document\n\n");
    source.push_str("```toml mdok vars\nrequest_id = \"bench-request\"\n\n[metadata]\nowner = \"performance\"\n```\n\n");
    for index in 0..spec.steps {
        source.push_str(&format!(
            "## API step {index}\n\n```curl mdok name=step_{index}\ncurl --request POST --header 'X-Mdok-Request: {{{{request_id|header}}}}' --header 'X-Mdok-Step: {index}' --data '{{\"step\":{index},\"workload\":\"{}\"}}' {endpoint}/api/{index}\n```\n\n",
            spec.label
        ));
        source.push_str(&format!(
            "```jmespath mdok check=step_{index}\nstatus == `200`\n```\n\n"
        ));
        source.push_str(&format!(
            "```jmespath mdok capture=step_{index}\n{{response_id_{index}: body.id}}\n```\n\n"
        ));
    }
    while source.len() < spec.prose_bytes {
        source.push_str(filler);
    }
    source
}

fn shell_source(template_count: usize, payload_bytes: usize) -> String {
    let payload = "v".repeat(payload_bytes);
    let mut source = format!(
        "curl --request POST https://example.test/api --data '{{\"payload\":\"{payload}\"}}'"
    );
    for index in 0..template_count {
        source.push_str(&format!(
            " --header 'X-Mdok-{index}: {{{{value_{index}|header}}}}'"
        ));
    }
    source
}

fn argv_for(endpoint: &str, index: usize) -> Vec<String> {
    vec![
        "curl".to_owned(),
        "--request".to_owned(),
        "POST".to_owned(),
        "--header".to_owned(),
        format!("X-Mdok-Step: {index}"),
        "--data".to_owned(),
        format!("{{\"step\":{index}}}"),
        format!("{endpoint}/api/{index}"),
    ]
}

fn markdown_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_extract");
    for bytes in [256usize, 4 * 1024, 32 * 1024] {
        let source = markdown_source(
            WorkloadSpec {
                label: "size",
                steps: 2,
                prose_bytes: bytes,
            },
            "https://example.test",
        );
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::new("size", bytes), &source, |bench, source| {
            bench.iter(|| black_box(mdok_markdown::parse(black_box(source), BENCH_PATH).unwrap()))
        });
    }
    for blocks in [1usize, 10, 50] {
        let source = markdown_source(
            WorkloadSpec {
                label: "blocks",
                steps: blocks,
                prose_bytes: 0,
            },
            "https://example.test",
        );
        group.bench_with_input(
            BenchmarkId::new("blocks", blocks),
            &source,
            |bench, source| {
                bench.iter(|| {
                    black_box(mdok_markdown::parse(black_box(source), BENCH_PATH).unwrap())
                })
            },
        );
    }
    for spec in [NORMAL, INTENSE] {
        let source = markdown_source(spec, "https://example.test");
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("workload", spec.label),
            &source,
            |bench, source| {
                bench.iter(|| {
                    let document = mdok_markdown::parse(black_box(source), BENCH_PATH).unwrap();
                    black_box(mdok_markdown::plan_document(&document).unwrap())
                })
            },
        );
    }
    group.finish();
}

fn shell_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("shell_parse");
    for bytes in [64usize, 1024, 16 * 1024] {
        let source = shell_source(2, bytes);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("argv_bytes", bytes),
            &source,
            |bench, source| bench.iter(|| black_box(mdok_shell::parse(black_box(source)).unwrap())),
        );
    }
    for templates in [0usize, 2, 10, 50] {
        let source = shell_source(templates, 32);
        group.bench_with_input(
            BenchmarkId::new("templates", templates),
            &source,
            |bench, source| bench.iter(|| black_box(mdok_shell::parse(black_box(source)).unwrap())),
        );
    }
    for (label, templates, payload) in [(NORMAL.label, 8, 64), (INTENSE.label, 64, 512)] {
        let source = shell_source(templates, payload);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("workload", label),
            &source,
            |bench, source| bench.iter(|| black_box(mdok_shell::parse(black_box(source)).unwrap())),
        );
    }
    group.finish();
}

fn curl_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("curl_parse");
    let policy = CurlPolicy::local_test();
    for option_count in [0usize, 2, 8, 24] {
        let mut argv = vec!["curl".to_owned(), "https://example.test/items".to_owned()];
        for index in 0..option_count {
            argv.push("--header".to_owned());
            argv.push(format!("X-Mdok-{index}: value-{index}"));
        }
        group.bench_with_input(
            BenchmarkId::new("options", option_count),
            &argv,
            |bench, argv| {
                bench.iter(|| black_box(CurlPlan::parse(black_box(argv), &policy).unwrap()))
            },
        );
    }
    for (label, count) in [(NORMAL.label, 10), (INTENSE.label, 48)] {
        let argv = argv_for("https://example.test", count);
        let mut argv = argv;
        for index in 0..count {
            argv.push("--header".to_owned());
            argv.push(format!("X-Mdok-Meta-{index}: value-{index}"));
        }
        group.bench_with_input(BenchmarkId::new("workload", label), &argv, |bench, argv| {
            bench.iter(|| black_box(CurlPlan::parse(black_box(argv), &policy).unwrap()))
        });
    }
    group.finish();
}

fn jmespath_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("jmespath_compile");
    let expressions = [
        ("simple", "status"),
        ("nested", "body.items[].metadata.identifiers[].value"),
        (
            "complex",
            "items[?active == `true` && contains(name, 'mdok')].metadata.id",
        ),
    ];
    for (complexity, expression) in expressions {
        group.bench_with_input(
            BenchmarkId::new("complexity", complexity),
            &expression,
            |bench, expression| {
                bench.iter(|| black_box(mdok_jmespath::compile(black_box(expression)).unwrap()))
            },
        );
    }
    group.finish();
}

fn evaluation_value(items: usize) -> Value {
    Value::Object(
        [(
            "items".to_owned(),
            Value::Array(
                (0..items)
                    .map(|index| {
                        json!({
                            "id": index,
                            "active": index % 2 == 0,
                            "metadata": {"name": format!("mdok-{index}")}
                        })
                    })
                    .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    )
}

fn jmespath_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("jmespath_eval");
    for (label, items) in [("small", 4usize), ("normal", 64), ("intense", 2048)] {
        let value = evaluation_value(items);
        group.throughput(Throughput::Elements(items as u64));
        group.bench_with_input(
            BenchmarkId::new("json_size", label),
            &value,
            |bench, value| {
                let expression = mdok_jmespath::compile("items[].metadata.name").unwrap();
                bench.iter(|| black_box(expression.evaluate(black_box(value)).unwrap()))
            },
        );
    }
    let value = evaluation_value(128);
    for (label, source) in [
        ("projection", "items[].id"),
        ("filter", "items[?active == `true`].id"),
        ("nested", "items[].metadata.name"),
    ] {
        let expression = mdok_jmespath::compile(source).unwrap();
        group.bench_with_input(
            BenchmarkId::new("expression", label),
            &value,
            |bench, value| bench.iter(|| black_box(expression.evaluate(black_box(value)).unwrap())),
        );
    }
    group.finish();
}

struct BodyServer {
    address: String,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl BodyServer {
    fn start(body: Vec<u8>, keep_alive: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!(
            "http://127.0.0.1:{}/body",
            listener.local_addr().unwrap().port()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_body = Arc::new(body);
        let join = thread::spawn(move || {
            let mut connections = Vec::new();
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let body = Arc::clone(&thread_body);
                        connections.push(thread::spawn(move || {
                            serve_body(&mut stream, &body, keep_alive)
                        }));
                    }
                    Err(_) => break,
                }
            }
            for connection in connections {
                let _ = connection.join();
            }
        });
        Self {
            address,
            stop,
            join: Some(join),
        }
    }
}

impl Drop for BodyServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(
            self.address
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap(),
        );
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn serve_body(stream: &mut TcpStream, body: &[u8], keep_alive: bool) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    loop {
        if !read_request(stream) {
            return;
        }
        let connection = if keep_alive { "keep-alive" } else { "close" };
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: {connection}\r\n\r\n",
            body.len()
        );
        if stream.write_all(headers.as_bytes()).is_err() || stream.write_all(body).is_err() {
            return;
        }
        if !keep_alive {
            return;
        }
    }
}

fn read_request(stream: &mut TcpStream) -> bool {
    let mut request = Vec::with_capacity(4096);
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let bytes = match stream.read(&mut chunk) {
            Ok(0) => return false,
            Ok(bytes) => bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return false;
            }
            Err(_) => return false,
        };
        request.extend_from_slice(&chunk[..bytes]);
        if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        if request.len() > 64 * 1024 {
            return false;
        }
    };
    let content_length = request[..header_end]
        .split(|byte| *byte == b'\n')
        .find_map(|line| {
            let line = std::str::from_utf8(line).ok()?.trim();
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while request.len().saturating_sub(header_end) < content_length {
        let mut chunk = [0u8; 4096];
        let bytes = match stream.read(&mut chunk) {
            Ok(0) => return false,
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        request.extend_from_slice(&chunk[..bytes]);
    }
    true
}

fn body_capture(c: &mut Criterion) {
    let mut group = c.benchmark_group("body_capture");
    let cases = [
        ("memory", vec![b'm'; 4 * 1024], 256 * 1024),
        ("spill", vec![b's'; 64 * 1024], 32 * 1024),
        (
            "binary",
            (0..16 * 1024).map(|index| (index % 256) as u8).collect(),
            256 * 1024,
        ),
        ("intense", vec![b'i'; 1024 * 1024], 256 * 1024),
    ];
    for (kind, body, threshold) in cases {
        let body_len = body.len();
        let server = BodyServer::start(body, false);
        let mut policy = CurlPolicy::local_test();
        policy.memory_body_threshold_bytes = threshold;
        let argv = vec!["curl".to_owned(), server.address.clone()];
        let plan = CurlPlan::parse(&argv, &policy).unwrap();
        group.throughput(Throughput::Bytes(body_len as u64));
        group.bench_function(kind, |bench| {
            let mut session = ExecutionSession::new();
            bench.iter(|| {
                let response = plan.execute_in_session(&policy, &mut session).unwrap();
                black_box(response.body_value(2 * 1024 * 1024).unwrap())
            })
        });
    }
    group.finish();
}

fn report_with_events(events: usize) -> Report {
    let mut report = Report::new("bench");
    for index in 0..events {
        report.push_event(
            Event {
                sequence: index as u64,
                kind: "step_completed".to_owned(),
                document: Some(format!("document-{index}")),
                step: Some(format!("step-{index}")),
                status: Some(Status::Passed),
                message: None,
            },
            Some(EventMetadata {
                document_ordinal: Some(index),
                step_ordinal: Some(0),
                duration_ms: Some(1),
                ..EventMetadata::default()
            }),
        );
    }
    report
}

fn report(c: &mut Criterion) {
    let mut group = c.benchmark_group("report");
    for events in [1usize, 16, 256, 2048] {
        group.bench_function(BenchmarkId::new("events", events), |bench| {
            bench.iter(|| {
                let report = report_with_events(events);
                black_box((report.json().unwrap(), report.json_lines().unwrap()))
            })
        });
    }
    group.finish();
}

fn planned_requests(endpoint: &str, spec: WorkloadSpec) -> Vec<Vec<String>> {
    (0..spec.steps)
        .map(|index| argv_for(endpoint, index))
        .collect()
}

fn end_to_end_iteration(
    source: &str,
    policy: &CurlPolicy,
    session: &mut ExecutionSession,
) -> String {
    let document = mdok_markdown::parse(source, BENCH_PATH).unwrap();
    let plan = mdok_markdown::plan_document(&document).unwrap();
    let mut report = Report::new("bench");
    let mut step_reports = Vec::with_capacity(plan.steps.len());
    for (index, step) in plan.steps.iter().enumerate() {
        let shell = mdok_shell::parse(step.curl.source.trim()).unwrap();
        let argv = shell.evaluate(&plan.variables).unwrap();
        let curl_plan = CurlPlan::parse(&argv, policy).unwrap();
        let response = curl_plan.execute_in_session(policy, session).unwrap();
        black_box(response.body_value(2 * 1024 * 1024).unwrap());
        step_reports.push(StepReport {
            name: step.name.to_string(),
            status: Status::Passed,
            command: argv,
            checks: vec![CheckReport {
                expression: "status == `200`".to_owned(),
                status: Status::Passed,
                result: Some(json!(true)),
            }],
            captures: vec![format!("response_id=step-{index}")],
            diagnostics: Vec::new(),
            duration_ms: 0,
        });
    }
    report.add_document(DocumentReport {
        path: BENCH_PATH.to_owned(),
        status: Status::Passed,
        duration_ms: 0,
        steps: step_reports,
        diagnostics: Vec::new(),
    });
    report.json().unwrap()
}

fn end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    let workload_server = BodyServer::start(br#"{"id":"bench-response","ok":true}"#.to_vec(), true);
    let policy = CurlPolicy::local_test();
    for spec in [NORMAL, INTENSE] {
        let source = markdown_source(spec, &workload_server.address);
        group.bench_function(BenchmarkId::new("workload", spec.label), |bench| {
            bench.iter(|| {
                let mut session = ExecutionSession::new();
                black_box(end_to_end_iteration(
                    black_box(&source),
                    &policy,
                    &mut session,
                ))
            })
        });
    }

    let one_shot_server =
        BodyServer::start(br#"{"id":"bench-response","ok":true}"#.to_vec(), false);
    let reused_server = BodyServer::start(br#"{"id":"bench-response","ok":true}"#.to_vec(), true);
    let one_shot_requests = planned_requests(&one_shot_server.address, NORMAL);
    group.bench_function("keepalive/one_shot", |bench| {
        bench.iter(|| {
            for argv in &one_shot_requests {
                let plan = CurlPlan::parse(argv, &policy).unwrap();
                black_box(plan.execute(&policy).unwrap());
            }
        })
    });
    let reused_requests = planned_requests(&reused_server.address, NORMAL);
    group.bench_function("keepalive/reused_session", |bench| {
        let mut session = ExecutionSession::new();
        bench.iter(|| {
            for argv in &reused_requests {
                let plan = CurlPlan::parse(argv, &policy).unwrap();
                black_box(plan.execute_in_session(&policy, &mut session).unwrap());
            }
        })
    });
    group.finish();
}

fn criterion_config() -> Criterion {
    Criterion::default()
        .configure_from_args()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
}

criterion_group! {
    name = prd;
    config = criterion_config();
    targets = markdown_extract, shell_parse, curl_parse, jmespath_compile, jmespath_eval,
        body_capture, report, end_to_end
}
criterion_main!(prd);
