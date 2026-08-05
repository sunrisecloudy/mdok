//! A small, deterministic HTTP fixture service used by mdok integration tests.
//!
//! This intentionally uses the standard library for the HTTP loop.  It has no
//! process or public-network dependencies, and every piece of mutable state is
//! keyed by `X-Mdok-Test-Key` so parallel tests cannot affect one another.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use clap::Parser;
use flate2::Compression;
use flate2::write::GzEncoder;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const MAX_HEADER_BYTES: usize = 128 * 1024;
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
const MAX_GENERATED_BYTES: usize = 32 * 1024 * 1024;

// These are test-only certificates. The CA is separate from the loopback
// server leaf so strict TLS implementations (including rustls) validate the
// same chain as production clients. The CA file contains only CA_CERT_PEM.
const CA_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIBljCCATygAwIBAgIUK3GpPK08MQR44eiiKiPCQlrkCDswCgYIKoZIzj0EAwIw
FzEVMBMGA1UEAwwMbWRvay10ZXN0LWNhMB4XDTI2MDgwNTAzMzM0OVoXDTM2MDgw
MjAzMzM0OVowFzEVMBMGA1UEAwwMbWRvay10ZXN0LWNhMFkwEwYHKoZIzj0CAQYI
KoZIzj0DAQcDQgAEfRmVk2Eiol6Hjp7cOZuTInb+ZEkSeQLFCSRGCVtTzaKQek4a
Iv2ONoPcjW36WKPWZ+OotWu1AXYLHY7QYWQFmKNmMGQwHQYDVR0OBBYEFGp3uy+x
7qjkcMT5TmJVKW8lEtE5MB8GA1UdIwQYMBaAFGp3uy+x7qjkcMT5TmJVKW8lEtE5
MBIGA1UdEwEB/wQIMAYBAf8CAQEwDgYDVR0PAQH/BAQDAgEGMAoGCCqGSM49BAMC
A0gAMEUCIQCjiWipHtE87Ngu6I0qXVan4dARj+bxxDXmufsrgC+xBAIgclgz/s+l
oz/cjF/Roug5st5agvQn2f/sk3sJqcgdzhk=
-----END CERTIFICATE-----
"#;

const SERVER_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIByDCCAW2gAwIBAgIUOfpki100z5fksfpVJtf4Ln+EGdMwCgYIKoZIzj0EAwIw
FzEVMBMGA1UEAwwMbWRvay10ZXN0LWNhMB4XDTI2MDgwNTAzMzM0OVoXDTM2MDgw
MjAzMzM0OVowGzEZMBcGA1UEAwwQbWRvay10ZXN0LXNlcnZlcjBZMBMGByqGSM49
AgEGCCqGSM49AwEHA0IABGZ2jz2zwCkGinTeMUr8RqJfmSXphZt9Do/MAHSOm3qi
yHZjyjtFKDIVno8kmyzQdSS+CvIC7ZDWzx+WNshAYnujgZIwgY8wDAYDVR0TAQH/
BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwGgYDVR0R
BBMwEYcEfwAAAYIJbG9jYWxob3N0MB0GA1UdDgQWBBQiUKoHSIxtnHTQALa5JxsI
Eq3aajAfBgNVHSMEGDAWgBRqd7svse6o5HDE+U5iVSlvJRLROTAKBggqhkjOPQQD
AgNJADBGAiEA6h5wfjEj4TY7Ap9kJeT5jw/vwUzwkvZ9AChflf8AgwgCIQCv8zq1
azfr9mLg7DlSRnlpMAknyl1NfjTN6Eal6gDatw==
-----END CERTIFICATE-----
"#;

const KEY_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIDtFPgdVfvBVqMV6GymXwKTmnqdCEmL8R5+KyzKYGn8loAoGCCqGSM49
AwEHoUQDQgAEZnaPPbPAKQaKdN4xSvxGol+ZJemFm30Oj8wAdI6beqLIdmPKO0Uo
MhWejySbLNB1JL4K8gLtkNbPH5Y2yEBiew==
-----END EC PRIVATE KEY-----
"#;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: String,
    #[arg(long, default_value = "127.0.0.1:0")]
    tls_listen: String,
    #[arg(long)]
    json_ready: bool,
}

#[derive(Serialize)]
struct Ready {
    http_base_url: String,
    https_base_url: String,
    proxy_url: String,
    ca_file: String,
}

#[derive(Default)]
struct ServerState {
    users: HashMap<String, BTreeMap<String, Value>>,
    retries: HashMap<(String, String), u32>,
}

#[derive(Debug)]
struct Request {
    method: String,
    target: String,
    path: String,
    query: BTreeMap<String, Vec<String>>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .rev()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn test_key(&self) -> String {
        [
            "x-mdok-test-key",
            "x-mdok-fixture-key",
            "x-fixture-test-key",
            "x-test-key",
        ]
        .iter()
        .find_map(|name| self.header(name))
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string()
    }

    fn query_one(&self, name: &str) -> Option<&str> {
        self.query
            .get(name)
            .and_then(|values| values.first().map(String::as_str))
    }
}

enum ResponseBody {
    Fixed(Vec<u8>),
    Chunks(Vec<Vec<u8>>),
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: ResponseBody,
    close_after: bool,
    chunk_delay_ms: u64,
}

impl Response {
    fn fixed(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: ResponseBody::Fixed(body),
            close_after: false,
            chunk_delay_ms: 0,
        }
    }

    fn json(status: u16, value: Value) -> Self {
        let body = serde_json::to_vec(&value).expect("JSON values in fixtures are serializable");
        let mut response = Self::fixed(status, body);
        response
            .headers
            .push(("Content-Type".into(), "application/json".into()));
        response
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let http = bind_loopback(&args.listen).context("binding HTTP fixture listener")?;
    let tls = bind_loopback(&args.tls_listen).context("binding HTTPS fixture listener")?;
    let proxy = TcpListener::bind("127.0.0.1:0").context("binding loopback proxy listener")?;
    let state = Arc::new(Mutex::new(ServerState::default()));
    let tls_config = Arc::new(tls_config()?);
    let ca_file = write_ca_file()?;

    let ready = Ready {
        http_base_url: format!("http://{}", http.local_addr()?),
        https_base_url: format!("https://{}", tls.local_addr()?),
        proxy_url: format!("http://{}", proxy.local_addr()?),
        ca_file: ca_file.display().to_string(),
    };
    if args.json_ready {
        println!("{}", serde_json::to_string(&ready)?);
        io::stdout().flush()?;
    }
    eprintln!(
        "{}",
        json!({"event":"ready", "http":ready.http_base_url, "https":ready.https_base_url})
    );

    spawn_http(http, Arc::clone(&state));
    spawn_tls(tls, Arc::clone(&state), tls_config);
    spawn_proxy(proxy);
    loop {
        thread::park();
    }
}

fn bind_loopback(spec: &str) -> Result<TcpListener> {
    let addresses: Vec<SocketAddr> = spec.to_socket_addrs()?.collect();
    if addresses.is_empty() {
        bail!("listen address resolved to no addresses: {spec}");
    }
    if addresses.iter().any(|address| !address.ip().is_loopback()) {
        bail!("fixture server only accepts loopback listen addresses");
    }
    for address in addresses {
        if let Ok(listener) = TcpListener::bind(address) {
            return Ok(listener);
        }
    }
    bail!("could not bind loopback address {spec}")
}

fn write_ca_file() -> Result<PathBuf> {
    let path = std::env::temp_dir().join("mdok-ca.pem");
    fs::write(&path, CA_CERT_PEM).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn tls_config() -> Result<ServerConfig> {
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut Cursor::new(SERVER_CERT_PEM))
            .collect::<std::result::Result<_, _>>()?;
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut Cursor::new(KEY_PEM))?
        .ok_or_else(|| anyhow!("embedded fixture key is missing"))?;
    Ok(ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?)
}

fn spawn_http(listener: TcpListener, state: Arc<Mutex<ServerState>>) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = Arc::clone(&state);
                    thread::spawn(move || serve_connection(stream, state));
                }
                Err(error) => eprintln!(
                    "{}",
                    json!({"event":"accept_error", "error":error.to_string()})
                ),
            }
        }
    });
}

fn spawn_tls(listener: TcpListener, state: Arc<Mutex<ServerState>>, config: Arc<ServerConfig>) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = Arc::clone(&state);
                    let config = Arc::clone(&config);
                    thread::spawn(move || match rustls::ServerConnection::new(config) {
                        Ok(connection) => {
                            serve_connection(rustls::StreamOwned::new(connection, stream), state)
                        }
                        Err(error) => eprintln!(
                            "{}",
                            json!({"event":"tls_error", "error":error.to_string()})
                        ),
                    });
                }
                Err(error) => eprintln!(
                    "{}",
                    json!({"event":"accept_error", "error":error.to_string()})
                ),
            }
        }
    });
}

fn serve_connection<S: Read + Write>(mut stream: S, state: Arc<Mutex<ServerState>>) {
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_response(
                &mut stream,
                Response::json(400, json!({"error":error.to_string()})),
                false,
            );
            return;
        }
    };
    let response = route(&request, &state);
    let _ = write_response(
        &mut stream,
        response,
        request.method.eq_ignore_ascii_case("HEAD"),
    );
}

fn read_request<S: Read>(stream: &mut S) -> Result<Request> {
    let mut bytes = Vec::with_capacity(4096);
    let header_end = loop {
        let mut buffer = [0u8; 4096];
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            bail!("connection closed before request headers");
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() > MAX_HEADER_BYTES {
            bail!("request headers exceed {} bytes", MAX_HEADER_BYTES);
        }
    };
    let header_text =
        std::str::from_utf8(&bytes[..header_end - 4]).context("request headers are not UTF-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request method"))?
        .to_string();
    let target = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request target"))?
        .to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| anyhow!("invalid request header"))?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.parse::<usize>())
        .transpose()
        .context("invalid Content-Length")?
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        bail!("request body exceeds {} bytes", MAX_BODY_BYTES);
    }
    let mut body = bytes[header_end..].to_vec();
    while body.len() < content_length {
        let needed = content_length - body.len();
        let mut buffer = vec![0u8; needed.min(8192)];
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            bail!("connection closed before request body");
        }
        body.extend_from_slice(&buffer[..count]);
    }
    body.truncate(content_length);
    let (path, query) = split_target(&target);
    Ok(Request {
        method,
        target,
        path,
        query,
        headers,
        body,
    })
}

fn split_target(target: &str) -> (String, BTreeMap<String, Vec<String>>) {
    let target = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
        .and_then(|rest| rest.find('/').map(|index| &rest[index..]))
        .unwrap_or(target);
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let path = if raw_path.is_empty() { "/" } else { raw_path }.to_string();
    let mut query = BTreeMap::new();
    for item in raw_query.split('&').filter(|item| !item.is_empty()) {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        query
            .entry(percent_decode(key))
            .or_insert_with(Vec::new)
            .push(percent_decode(value));
    }
    (path, query)
}

fn write_response<S: Write>(stream: &mut S, mut response: Response, head: bool) -> io::Result<()> {
    let reason = reason_phrase(response.status);
    let chunked = matches!(response.body, ResponseBody::Chunks(_));
    let length = match &response.body {
        ResponseBody::Fixed(body) => body.len(),
        ResponseBody::Chunks(_) => 0,
    };
    if !chunked {
        response
            .headers
            .push(("Content-Length".into(), length.to_string()));
    } else {
        response
            .headers
            .push(("Transfer-Encoding".into(), "chunked".into()));
    }
    response.headers.push(("Connection".into(), "close".into()));
    write!(stream, "HTTP/1.1 {} {}\r\n", response.status, reason)?;
    for (name, value) in response.headers {
        write!(stream, "{}: {}\r\n", name, value)?;
    }
    stream.write_all(b"\r\n")?;
    if head {
        return Ok(());
    }
    match response.body {
        ResponseBody::Fixed(body) => {
            if response.close_after {
                stream.write_all(&body[..body.len() / 2])?;
                stream.flush()?;
                return Ok(());
            }
            stream.write_all(&body)?;
        }
        ResponseBody::Chunks(chunks) => {
            for chunk in chunks {
                if response.chunk_delay_ms > 0 {
                    thread::sleep(Duration::from_millis(response.chunk_delay_ms));
                }
                write!(stream, "{:x}\r\n", chunk.len())?;
                stream.write_all(&chunk)?;
                stream.write_all(b"\r\n")?;
                stream.flush()?;
            }
            stream.write_all(b"0\r\n\r\n")?;
        }
    }
    stream.flush()
}

fn route(request: &Request, state: &Arc<Mutex<ServerState>>) -> Response {
    let path = request.path.as_str();
    match (request.method.as_str(), path) {
        (_, "/health") => Response::json(200, json!({"ok":true})),
        (_, "/echo") => echo(request),
        (_, "/headers") => headers_endpoint(),
        (_, "/gzip") => gzip_response(request),
        (_, "/upload") => upload(request),
        (_, "/multipart") => multipart(request),
        (_, "/cookies/set") => cookies_set(request),
        (_, "/cookies/echo") => cookies_echo(request),
        (_, "/auth/basic") => basic_auth(request),
        (_, "/auth/bearer") => bearer_auth(request),
        ("POST", "/auth/login") => login(request),
        (_, "/close/early") | (_, "/early") => close_early(),
        _ if path.starts_with("/status/") => status_endpoint(path),
        _ if path.starts_with("/json/") => json_case(path),
        _ if path.starts_with("/redirect/") => redirect(request, path),
        _ if path.starts_with("/delay/") => delay(path),
        _ if path.starts_with("/stream/") => stream_response(path),
        _ if path.starts_with("/binary/") => binary_response(path),
        _ if path.starts_with("/retry/") => retry(request, path, state),
        _ if path.starts_with("/large/") => large_response(path),
        _ if path == "/users" => users_collection(request, state),
        _ if path.starts_with("/users/") => user_endpoint(request, path, state),
        _ => Response::json(404, json!({"error":"not_found", "path":path})),
    }
}

fn echo(request: &Request) -> Response {
    let mut headers = Map::new();
    for (name, value) in &request.headers {
        let key = name.to_ascii_lowercase();
        match headers.get_mut(&key) {
            Some(Value::Array(values)) => values.push(Value::String(value.clone())),
            Some(existing) => {
                let old = existing.take();
                *existing = json!([old, value]);
            }
            None => {
                headers.insert(key, Value::Array(vec![Value::String(value.clone())]));
            }
        }
    }
    let query = request
        .query
        .iter()
        .map(|(key, values)| {
            let value = if values.len() == 1 {
                Value::String(values[0].clone())
            } else {
                Value::Array(values.iter().cloned().map(Value::String).collect())
            };
            (key.clone(), value)
        })
        .collect::<Map<String, Value>>();
    let json_body = serde_json::from_slice::<Value>(&request.body).ok();
    let body = json_body
        .clone()
        .unwrap_or_else(|| Value::String(String::from_utf8_lossy(&request.body).into_owned()));
    let raw_body = String::from_utf8(request.body.clone()).ok();
    let form = request
        .header("content-type")
        .and_then(|value| value.split(';').next())
        .filter(|value| {
            value
                .trim()
                .eq_ignore_ascii_case("application/x-www-form-urlencoded")
        })
        .map(|_| parse_form(&request.body));
    let mut result = json!({
        "method": request.method,
        "path": request.path,
        "target": request.target,
        "query": query,
        "headers": headers,
        "cookies": parse_cookies(request.header("cookie")),
        "body": body,
        "json": json_body,
        "form": form,
        "text": raw_body,
        "body_size": request.body.len(),
        "raw_body_base64": base64::engine::general_purpose::STANDARD.encode(&request.body),
    });
    if let Some(raw_body) = raw_body {
        result["raw_body"] = Value::String(raw_body);
    }
    Response::json(200, result)
}

fn parse_form(body: &[u8]) -> Value {
    let mut fields = Map::new();
    for item in String::from_utf8_lossy(body)
        .split('&')
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        let key = percent_decode(key);
        let value = Value::String(percent_decode(value));
        match fields.get_mut(&key) {
            Some(Value::Array(values)) => values.push(value),
            Some(existing) => {
                let old = existing.take();
                *existing = json!([old, value]);
            }
            None => {
                fields.insert(key, value);
            }
        }
    }
    Value::Object(fields)
}

fn status_endpoint(path: &str) -> Response {
    let code = path
        .strip_prefix("/status/")
        .and_then(|value| value.parse::<u16>().ok());
    let Some(code) = code.filter(|code| (100..=599).contains(code)) else {
        return Response::json(400, json!({"error":"invalid_status"}));
    };
    Response::json(
        code,
        json!({"status":code, "ok":code < 400, "value":code, "message":format!("status {code}")}),
    )
}

fn json_case(path: &str) -> Response {
    let case = path.strip_prefix("/json/").unwrap_or("standard");
    let standard = json!({
        "ok": true,
        "items": [
            {"id":"a", "name":"Alpha", "value":1},
            {"id":"b", "name":"Beta", "value":2},
            {"id":"c", "name":"Gamma", "value":3}
        ],
        "tags": ["red", "blue", "green"],
        "nested": {"value": 42, "array": [1, 2, 3]},
        "object": {"answer": 42, "enabled": true},
        "null_value": null,
        "number": 123.5,
        "unicode": "こんにちは, fixture 🌱"
    });
    let value = match case {
        "standard" => standard,
        "empty" => json!({}),
        "null" => Value::Null,
        "array" => json!([null, false, 0, "text", {"key":"value"}]),
        "numbers" => json!({"integer":42, "negative":-7, "decimal":3.25, "zero":0}),
        "unicode" => json!({"text":"Grüße — こんにちは — 🌍"}),
        "nested" => json!({"a":{"b":{"c":[{"value":1},{"value":2}]}}}),
        "booleans" => json!({"true":true, "false":false}),
        _ => json!({"case":case, "value":standard}),
    };
    Response::json(200, value)
}

fn headers_endpoint() -> Response {
    let mut response = Response::json(200, json!({"ok":true, "headers":"deterministic"}));
    response.headers.extend([
        ("X-Duplicate".into(), "one".into()),
        ("X-Duplicate".into(), "two".into()),
        ("X-Mixed-Case".into(), "Value".into()),
        ("X-Empty".into(), String::new()),
        ("X-Long".into(), "x".repeat(4096)),
    ]);
    response
}

fn basic_auth(request: &Request) -> Response {
    let valid = request
        .header("authorization")
        .map(|value| {
            value.trim() == "Basic bWRvazpzZWNyZXQ=" || value.trim() == "Basic mdok:secret"
        })
        .unwrap_or(false);
    auth_response(valid, "Basic realm=mdok")
}

fn bearer_auth(request: &Request) -> Response {
    let valid = request
        .header("authorization")
        .map(|value| value.trim() == "Bearer test-token")
        .unwrap_or(false);
    auth_response(valid, "Bearer")
}

fn auth_response(valid: bool, challenge: &str) -> Response {
    let status = if valid { 200 } else { 401 };
    let mut response = Response::json(status, json!({"authenticated":valid, "ok":valid}));
    if !valid {
        response
            .headers
            .push(("WWW-Authenticate".into(), challenge.into()));
    }
    response
}

fn login(request: &Request) -> Response {
    let input = serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null);
    let email = input.get("email").and_then(Value::as_str).unwrap_or("");
    let password = input.get("password").and_then(Value::as_str).unwrap_or("");
    if email.is_empty() || password != "test-password" {
        return Response::json(
            401,
            json!({"authenticated":false, "error":"invalid_credentials"}),
        );
    }
    let user_id = format!("user-{}", &digest_hex(email.as_bytes())[..12]);
    Response::json(
        200,
        json!({
            "access_token":"test-token",
            "token_type":"Bearer",
            "user":{"id":user_id, "email":email, "name":email.split('@').next().unwrap_or(email)}
        }),
    )
}

fn users_collection(request: &Request, state: &Arc<Mutex<ServerState>>) -> Response {
    if request.method == "GET" {
        let prefix = format!("{}\0", request.test_key());
        let users = state
            .lock()
            .unwrap()
            .users
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, user)| Value::Object(user.clone().into_iter().collect()))
            .collect::<Vec<_>>();
        let count = users.len();
        return Response::json(200, json!({"users":users, "count":count}));
    }
    if request.method != "POST" {
        return Response::json(405, json!({"error":"method_not_allowed"}));
    }
    let input = match serde_json::from_slice::<Value>(&request.body) {
        Ok(Value::Object(object)) => object,
        _ => return Response::json(400, json!({"error":"expected_json_object"})),
    };
    let id = input
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("user-{}", &digest_hex(&request.body)[..12]));
    let user = user_map(&id, input.into_iter().collect());
    state
        .lock()
        .unwrap()
        .users
        .insert(namespaced(request, &id), user.clone());
    Response::json(201, Value::Object(user.into_iter().collect()))
}

fn user_endpoint(request: &Request, path: &str, state: &Arc<Mutex<ServerState>>) -> Response {
    let id = path.strip_prefix("/users/").unwrap_or("");
    if id.is_empty() {
        return Response::json(400, json!({"error":"missing_user_id"}));
    }
    let key = namespaced(request, id);
    match request.method.as_str() {
        "GET" => {
            let user = state
                .lock()
                .unwrap()
                .users
                .get(&key)
                .cloned()
                .unwrap_or_else(|| user_map(id, BTreeMap::new()));
            Response::json(200, Value::Object(user.into_iter().collect()))
        }
        "PUT" | "PATCH" => {
            let input = match serde_json::from_slice::<Value>(&request.body) {
                Ok(Value::Object(object)) => object,
                _ => return Response::json(400, json!({"error":"expected_json_object"})),
            };
            let mut users = state.lock().unwrap();
            let mut user = users
                .users
                .get(&key)
                .cloned()
                .unwrap_or_else(|| user_map(id, BTreeMap::new()));
            for (field, value) in input {
                user.insert(field, value);
            }
            user.insert("id".into(), Value::String(id.into()));
            users.users.insert(key, user.clone());
            Response::json(200, Value::Object(user.into_iter().collect()))
        }
        "DELETE" => {
            let removed = state.lock().unwrap().users.remove(&key).is_some();
            Response::json(200, json!({"deleted":removed, "id":id}))
        }
        _ => Response::json(405, json!({"error":"method_not_allowed"})),
    }
}

fn user_map(id: &str, mut fields: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    fields.insert("id".into(), Value::String(id.into()));
    fields
        .entry("name".into())
        .or_insert_with(|| Value::String(format!("User {id}")));
    fields
        .entry("email".into())
        .or_insert_with(|| Value::String(format!("{id}@example.com")));
    fields
}

fn namespaced(request: &Request, id: &str) -> String {
    format!("{}\0{id}", request.test_key())
}

fn cookies_set(request: &Request) -> Response {
    let mut response = Response::json(200, json!({"ok":true, "set":request.query}));
    let mut cookies = Vec::new();
    if let (Some(name), Some(value)) = (request.query_one("name"), request.query_one("value")) {
        cookies.push(format!("{name}={value}; Path=/"));
    } else {
        for (name, values) in &request.query {
            if ["path", "domain", "max_age", "secure", "http_only"].contains(&name.as_str()) {
                continue;
            }
            for value in values {
                cookies.push(format!("{name}={value}; Path=/"));
            }
        }
    }
    if request.query_one("secure") == Some("true") {
        for cookie in &mut cookies {
            cookie.push_str("; Secure");
        }
    }
    if cookies.is_empty() {
        cookies.push("fixture=ok; Path=/".into());
    }
    response.headers.extend(
        cookies
            .into_iter()
            .map(|cookie| ("Set-Cookie".into(), cookie)),
    );
    response
}

fn cookies_echo(request: &Request) -> Response {
    Response::json(
        200,
        json!({"cookies":parse_cookies(request.header("cookie")), "raw":request.header("cookie").unwrap_or("")}),
    )
}

fn parse_cookies(header: Option<&str>) -> BTreeMap<String, String> {
    let mut cookies = BTreeMap::new();
    for item in header.unwrap_or("").split(';') {
        if let Some((name, value)) = item.trim().split_once('=') {
            cookies.insert(name.trim().into(), value.trim().into());
        }
    }
    cookies
}

fn redirect(request: &Request, path: &str) -> Response {
    let count = path
        .strip_prefix("/redirect/")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let mut response = if count == 0 {
        match request.query_one("final") {
            Some("/cookies/echo") => cookies_echo(request),
            Some("/health") => Response::json(200, json!({"ok":true})),
            _ => echo(request),
        }
    } else {
        let target = if request.query_one("external") == Some("true") {
            format!("http://example.invalid/redirect/{}", count - 1)
        } else if let Some(host) = request.query_one("host") {
            let scheme = request.query_one("scheme").unwrap_or("http");
            format!("{scheme}://{host}/redirect/{}", count - 1)
        } else {
            format!("/redirect/{}", count - 1)
        };
        let location = request
            .query_one("final")
            .map(|final_path| format!("{target}?final={final_path}"))
            .unwrap_or(target);
        let mut response = Response::fixed(302, Vec::new());
        response.headers.push(("Location".into(), location));
        response
    };
    if count == 0 && request.query_one("redirect_status").is_some() {
        response.status = request
            .query_one("redirect_status")
            .and_then(|value| value.parse().ok())
            .unwrap_or(200);
    }
    response
}

fn delay(path: &str) -> Response {
    let ms = path
        .strip_prefix("/delay/")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .min(60_000);
    thread::sleep(Duration::from_millis(ms));
    Response::json(200, json!({"ok":true, "delay_ms":ms}))
}

fn stream_response(path: &str) -> Response {
    let values: Vec<&str> = path
        .strip_prefix("/stream/")
        .unwrap_or("0/0")
        .split('/')
        .collect();
    let chunks = values
        .first()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .min(1024);
    let delay = values
        .get(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .min(60_000);
    let chunks = (0..chunks)
        .map(|index| format!("chunk-{index}\n").into_bytes())
        .collect();
    let mut response = Response {
        status: 200,
        headers: vec![
            ("Content-Type".into(), "text/plain; charset=utf-8".into()),
            ("X-Chunk-Delay-Ms".into(), delay.to_string()),
        ],
        body: ResponseBody::Chunks(chunks),
        close_after: false,
        chunk_delay_ms: delay,
    };
    if delay > 0 {
        // The delay is applied while writing chunks, in write_response.
        response
            .headers
            .push(("X-Stream-Delay-Ms".into(), delay.to_string()));
    }
    response
}

fn binary_response(path: &str) -> Response {
    let size = bounded_size(path.strip_prefix("/binary/"));
    let body = (0..size)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(7))
        .collect();
    let mut response = Response::fixed(200, body);
    response
        .headers
        .push(("Content-Type".into(), "application/octet-stream".into()));
    response
}

fn gzip_response(request: &Request) -> Response {
    let value = json!({"ok":true, "encoding":"gzip", "case":request.query_one("case").unwrap_or("standard"), "payload":"deterministic"});
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&serde_json::to_vec(&value).unwrap())
        .expect("gzip writes to memory");
    let body = encoder.finish().expect("gzip finishes in memory");
    let mut response = Response::fixed(200, body);
    response.headers.extend([
        ("Content-Type".into(), "application/json".into()),
        ("Content-Encoding".into(), "gzip".into()),
    ]);
    response
}

fn upload(request: &Request) -> Response {
    Response::json(
        200,
        json!({"size":request.body.len(), "sha256":digest_hex(&request.body)}),
    )
}

fn multipart(request: &Request) -> Response {
    let content_type = request.header("content-type").unwrap_or("");
    let boundary = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("boundary="))
        .map(|value| value.trim_matches('"'));
    let Some(boundary) = boundary else {
        return Response::json(400, json!({"error":"missing_multipart_boundary"}));
    };
    let marker = format!("--{boundary}").into_bytes();
    let mut fields = BTreeMap::new();
    let mut files = Vec::new();
    for part in split_bytes(&request.body, &marker).into_iter().skip(1) {
        let part = part.strip_prefix(b"\r\n").unwrap_or(part);
        if part.starts_with(b"--") {
            continue;
        }
        let Some(header_end) = part.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_text = String::from_utf8_lossy(&part[..header_end]);
        let content = part[header_end + 4..]
            .strip_suffix(b"\r\n")
            .unwrap_or(&part[header_end + 4..]);
        let disposition = header_text.lines().find(|line| {
            line.to_ascii_lowercase()
                .starts_with("content-disposition:")
        });
        let Some(disposition) = disposition else {
            continue;
        };
        let name = disposition_parameter(disposition, "name").unwrap_or_default();
        if let Some(filename) = disposition_parameter(disposition, "filename") {
            files.push(json!({"name":name, "filename":filename, "size":content.len(), "sha256":digest_hex(content)}));
        } else {
            fields.insert(name, String::from_utf8_lossy(content).into_owned());
        }
    }
    Response::json(
        200,
        json!({
            "fields": fields.clone(),
            "files": files.clone(),
            "multipart": {"fields": fields, "files": files},
        }),
    )
}

fn split_bytes<'a>(input: &'a [u8], marker: &[u8]) -> Vec<&'a [u8]> {
    let mut result = Vec::new();
    let mut start = 0;
    while let Some(relative) = input[start..]
        .windows(marker.len())
        .position(|window| window == marker)
    {
        let index = start + relative;
        result.push(&input[start..index]);
        start = index + marker.len();
    }
    result.push(&input[start..]);
    result
}

fn disposition_parameter(line: &str, parameter: &str) -> Option<String> {
    line.split(';').skip(1).find_map(|item| {
        let (key, value) = item.trim().split_once('=')?;
        (key == parameter).then(|| value.trim_matches('"').to_string())
    })
}

fn close_early() -> Response {
    let mut response = Response::fixed(200, b"this body is intentionally truncated".to_vec());
    response.close_after = true;
    response
}

fn retry(request: &Request, path: &str, state: &Arc<Mutex<ServerState>>) -> Response {
    let failures = path
        .strip_prefix("/retry/")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
        .min(1000);
    let key = (request.test_key(), path.to_string());
    let mut retries = state.lock().unwrap();
    let attempt = retries.retries.entry(key).or_insert(0);
    *attempt += 1;
    if *attempt <= failures {
        Response::json(
            503,
            json!({"ok":false, "attempt":*attempt, "remaining":failures + 1 - *attempt}),
        )
    } else {
        Response::json(
            200,
            json!({"ok":true, "attempt":*attempt, "failures":failures}),
        )
    }
}

fn large_response(path: &str) -> Response {
    let size = bounded_size(path.strip_prefix("/large/"));
    let pattern = b"mdok-large-fixture\n";
    let mut body = Vec::with_capacity(size);
    while body.len() < size {
        let remaining = size - body.len();
        body.extend_from_slice(&pattern[..remaining.min(pattern.len())]);
    }
    let mut response = Response::fixed(200, body);
    response
        .headers
        .push(("Content-Type".into(), "application/octet-stream".into()));
    response
}

fn bounded_size(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_GENERATED_BYTES)
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'+' {
            output.push(b' ');
            index += 1;
        } else if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
            output.push(bytes[index]);
            index += 1;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ if (100..200).contains(&status) => "Informational",
        _ if (200..300).contains(&status) => "Success",
        _ if (300..400).contains(&status) => "Redirect",
        _ if (400..500).contains(&status) => "Client Error",
        _ => "Server Error",
    }
}

fn spawn_proxy(listener: TcpListener) {
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || proxy_connection(stream));
        }
    });
}

fn proxy_connection(mut client: TcpStream) {
    let request = match read_request(&mut client) {
        Ok(request) => request,
        Err(_) => return,
    };
    if request.method.eq_ignore_ascii_case("CONNECT") {
        let Some(address) = loopback_destination(&request.target) else {
            return;
        };
        let Ok(upstream) = TcpStream::connect(address) else {
            return;
        };
        if client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .is_err()
        {
            return;
        }
        let mut client_read = client.try_clone().ok();
        let mut upstream_read = upstream.try_clone().ok();
        let first = thread::spawn(move || {
            if let (Some(mut from), Some(mut to)) = (client_read.take(), Some(upstream)) {
                let _ = io::copy(&mut from, &mut to);
                let _ = to.shutdown(Shutdown::Write);
            }
        });
        if let Some(mut from) = upstream_read.take() {
            let _ = io::copy(&mut from, &mut client);
        }
        let _ = first.join();
        return;
    }
    let Some((address, origin_target)) = proxy_target(&request) else {
        return;
    };
    let Ok(mut upstream) = TcpStream::connect(address) else {
        return;
    };
    let _ = write!(
        upstream,
        "{} {} HTTP/1.1\r\n",
        request.method, origin_target
    );
    for (name, value) in &request.headers {
        if !name.eq_ignore_ascii_case("proxy-connection") {
            let _ = write!(upstream, "{name}: {value}\r\n");
        }
    }
    let _ = write!(upstream, "\r\n");
    let _ = upstream.write_all(&request.body);
    let _ = io::copy(&mut upstream, &mut client);
}

fn proxy_target(request: &Request) -> Option<(SocketAddr, String)> {
    let target = request.target.strip_prefix("http://")?;
    let slash = target.find('/').unwrap_or(target.len());
    let authority = &target[..slash];
    let address = loopback_destination(authority)?;
    let origin = if slash == target.len() {
        "/"
    } else {
        &target[slash..]
    };
    Some((address, origin.to_string()))
}

fn loopback_destination(authority: &str) -> Option<SocketAddr> {
    let authority = authority
        .rsplit_once('@')
        .map(|(_, value)| value)
        .unwrap_or(authority);
    let addresses: Vec<SocketAddr> = authority.to_socket_addrs().ok()?.collect();
    addresses
        .into_iter()
        .find(|address| address.ip().is_loopback())
}
