use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use mdok_core::ValueMap;
use mdok_curl::{CurlPlan, CurlPolicy};
use mdok_report::{CheckReport, DocumentReport, Event, EventMetadata, Report, Status, StepReport};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const BENCH_PATH: &str = "<bench>/document.md";

fn markdown_source(blocks: usize, payload_bytes: usize) -> String {
    let payload = "x".repeat(payload_bytes);
    let mut source = String::from("# benchmark\n\n");
    for index in 0..blocks {
        source.push_str(&format!(
            "## step {index}\n\n```curl mdok name=step_{index}\ncurl https://example.test/{index}?payload={payload}\n```\n\n"
        ));
    }
    source
}

fn markdown_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_extract");
    for bytes in [256usize, 4 * 1024, 32 * 1024] {
        let source = markdown_source(2, bytes);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::new("size", bytes), &source, |bench, source| {
            bench.iter(|| black_box(mdok_markdown::parse(black_box(source), BENCH_PATH).unwrap()))
        });
    }
    for blocks in [1usize, 10, 50] {
        let source = markdown_source(blocks, 32);
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
    group.finish();
}

fn shell_source(template_count: usize, payload_bytes: usize) -> String {
    let payload = "v".repeat(payload_bytes);
    let mut source = format!("curl --request POST https://example.test/{payload}");
    for index in 0..template_count {
        source.push_str(&format!(
            " --header 'X-Mdok-{index}: {{{{value_{index}|header}}}}'"
        ));
    }
    source.push_str(" --data '{{body|json}}'");
    source
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
    for items in [4usize, 64, 512] {
        let value = evaluation_value(items);
        group.bench_with_input(
            BenchmarkId::new("json_size", items),
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
    fn start(body: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!(
            "http://127.0.0.1:{}/body",
            listener.local_addr().unwrap().port()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => serve_body(&mut stream, &body),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
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

fn serve_body(stream: &mut TcpStream, body: &[u8]) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    let mut request = [0u8; 4096];
    let _ = stream.read(&mut request);
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.write_all(body);
}

fn body_capture(c: &mut Criterion) {
    let mut group = c.benchmark_group("body_capture");
    let cases = [
        ("memory", vec![b'm'; 4 * 1024]),
        ("spill", vec![b's'; 64 * 1024]),
        (
            "binary",
            (0..16 * 1024).map(|index| (index % 256) as u8).collect(),
        ),
    ];
    for (kind, body) in cases {
        let server = BodyServer::start(body);
        let mut policy = CurlPolicy::local_test();
        if kind == "spill" {
            policy.memory_body_threshold_bytes = 32 * 1024;
        }
        // Keep this benchmark on the public Rust transfer path. The native
        // fast path intentionally handles the simpler transfer subset;
        // explicitly selecting HTTP/1.1 measures the existing bounded
        // body-capture logic without reaching into private constructors.
        let argv = vec![
            "curl".to_owned(),
            "--http1.1".to_owned(),
            "--header".to_owned(),
            "X-Mdok-Benchmark: body".to_owned(),
            server.address.clone(),
        ];
        let plan = CurlPlan::parse(&argv, &policy).unwrap();
        group.bench_function(kind, |bench| {
            bench.iter(|| {
                let response = plan.execute(&policy).unwrap();
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
    for events in [1usize, 16, 256] {
        group.bench_function(BenchmarkId::new("events", events), |bench| {
            bench.iter(|| {
                let report = report_with_events(events);
                black_box((report.json().unwrap(), report.json_lines().unwrap()))
            })
        });
    }
    group.finish();
}

fn end_to_end_report(steps: usize) -> Report {
    let source = markdown_source(steps, 16).replace("?payload=", "/payload-");
    let document = mdok_markdown::parse(&source, BENCH_PATH).unwrap();
    let plan = mdok_markdown::plan_document(&document).unwrap();
    let policy = CurlPolicy::local_test();
    let values = ValueMap::new();
    let mut report = Report::new("bench");
    let mut step_reports = Vec::with_capacity(plan.steps.len());
    for step in &plan.steps {
        let shell = mdok_shell::parse(step.curl.source.trim()).unwrap();
        let argv = shell.evaluate(&values).unwrap();
        let _curl_plan = CurlPlan::parse(&argv, &policy).unwrap();
        step_reports.push(StepReport {
            name: step.name.to_string(),
            status: Status::Planned,
            command: argv,
            checks: vec![CheckReport {
                expression: "status == `200`".to_owned(),
                status: Status::Planned,
                result: None,
            }],
            captures: Vec::new(),
            diagnostics: Vec::new(),
            duration_ms: 0,
        });
    }
    report.add_document(DocumentReport {
        path: BENCH_PATH.to_owned(),
        status: Status::Planned,
        duration_ms: 0,
        steps: step_reports,
        diagnostics: Vec::new(),
    });
    report
}

fn end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    for steps in [1usize, 10, 50] {
        group.bench_function(BenchmarkId::new("steps", steps), |bench| {
            bench.iter(|| black_box(end_to_end_report(steps).json().unwrap()))
        });
    }
    // The current public APIs do not expose a reusable worker/session handle;
    // this benchmark records the connectionless planning/report path under
    // the required keepalive dimension without reaching into private state.
    group.bench_function("keepalive", |bench| {
        bench.iter(|| black_box(end_to_end_report(10).json_lines().unwrap()))
    });
    group.finish();
}

criterion_group!(
    prd,
    markdown_extract,
    shell_parse,
    curl_parse,
    jmespath_compile,
    jmespath_eval,
    body_capture,
    report,
    end_to_end
);
criterion_main!(prd);
