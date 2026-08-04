#![forbid(unsafe_code)]

//! Curl-compatible request planning and bounded HTTP execution.
//!
//! The repository does not contain the pinned curl source release described by
//! the PRD.  This crate therefore keeps the public plan independent of curl's
//! private structs and implements the version-1 option subset directly.  The
//! transfer itself is still performed in-process by reqwest/libcurl-backed
//! platform networking; no shell or child process is started.

use base64::Engine as _;
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{
    ACCEPT, COOKIE, HeaderMap, HeaderName, HeaderValue, RANGE, REFERER, USER_AGENT,
};
use reqwest::{Method, Proxy, Version};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::{NamedTempFile, TempPath};
use thiserror::Error;
use url::Url;

pub const E_UNKNOWN_OPTION: &str = "MDOK-E300";
pub const E_UNSUPPORTED: &str = "MDOK-E301";
pub const E_PROTOCOL_DENIED: &str = "MDOK-E302";
pub const E_FILE_DENIED: &str = "MDOK-E303";
pub const E_POLICY: &str = "MDOK-E304";
pub const E_TIMEOUT: &str = "MDOK-E601";
pub const E_TRANSFER: &str = "MDOK-E600";
pub const E_REDIRECT: &str = "MDOK-E603";
pub const E_TLS: &str = "MDOK-E602";
pub const E_BODY_LIMIT: &str = "MDOK-E700";

#[derive(Clone, Debug)]
pub struct CurlPolicy {
    pub allowed_schemes: HashSet<String>,
    pub allowed_hosts: Option<HashSet<String>>,
    pub allow_private_network: bool,
    pub allow_insecure_tls: bool,
    pub allow_proxy: bool,
    pub allow_resolve: bool,
    pub allow_connect_to: bool,
    pub allow_file_reads: bool,
    pub allowed_read_roots: Vec<PathBuf>,
    pub allow_artifact_writes: bool,
    pub allowed_artifact_roots: Vec<PathBuf>,
    pub max_body_bytes: u64,
    pub memory_body_threshold_bytes: usize,
    pub max_header_bytes: usize,
}

impl Default for CurlPolicy {
    fn default() -> Self {
        Self {
            allowed_schemes: ["http", "https"].into_iter().map(str::to_owned).collect(),
            allowed_hosts: None,
            allow_private_network: false,
            allow_insecure_tls: false,
            allow_proxy: false,
            allow_resolve: false,
            allow_connect_to: false,
            allow_file_reads: false,
            allowed_read_roots: Vec::new(),
            allow_artifact_writes: false,
            allowed_artifact_roots: Vec::new(),
            max_body_bytes: 8 * 1024 * 1024,
            memory_body_threshold_bytes: 256 * 1024,
            max_header_bytes: 256 * 1024,
        }
    }
}

impl CurlPolicy {
    pub fn local_test() -> Self {
        Self {
            allow_private_network: true,
            ..Self::default()
        }
    }

    fn check_url(&self, url: &Url) -> Result<(), CurlError> {
        if !self.allowed_schemes.contains(url.scheme()) {
            return Err(CurlError::new(
                E_PROTOCOL_DENIED,
                format!("scheme `{}` is not allowed", url.scheme()),
            ));
        }
        if let Some(hosts) = &self.allowed_hosts {
            let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
            if !hosts
                .iter()
                .any(|allowed| host == allowed.to_ascii_lowercase())
            {
                return Err(CurlError::new(
                    E_POLICY,
                    format!("host `{host}` is not allowed"),
                ));
            }
        }
        if !self.allow_private_network {
            if let Some(host) = url.host_str() {
                let private = host == "localhost"
                    || host == "localhost.localdomain"
                    || host
                        .parse::<std::net::IpAddr>()
                        .map(|ip| match ip {
                            std::net::IpAddr::V4(ip) => {
                                ip.is_loopback() || ip.is_private() || ip.is_unspecified()
                            }
                            std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified(),
                        })
                        .unwrap_or(false);
                if private {
                    return Err(CurlError::new(
                        E_POLICY,
                        "private and loopback destinations are denied",
                    ));
                }
            }
        }
        Ok(())
    }

    fn read_file(&self, raw: &str) -> Result<Vec<u8>, CurlError> {
        if raw == "-" || raw.starts_with("/dev/") || raw.starts_with("\\\\.\\") {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "stdin and device paths are not allowed",
            ));
        }
        if !self.allow_file_reads {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "file reads are disabled by policy",
            ));
        }
        let path = PathBuf::from(raw);
        let canonical = fs::canonicalize(&path)
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot access file: {e}")))?;
        if !self.allowed_read_roots.is_empty()
            && !self.allowed_read_roots.iter().any(|root| {
                fs::canonicalize(root)
                    .map(|r| canonical.starts_with(r))
                    .unwrap_or(false)
            })
        {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "file is outside the allowed read roots",
            ));
        }
        fs::read(&canonical)
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot read file: {e}")))
    }

    fn check_artifact(&self, path: &Path) -> Result<(), CurlError> {
        if !self.allow_artifact_writes {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact writes are disabled by policy",
            ));
        }
        if !self.allowed_artifact_roots.is_empty() {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let canonical_parent =
                fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            if !self.allowed_artifact_roots.iter().any(|root| {
                fs::canonicalize(root)
                    .map(|r| canonical_parent.starts_with(r))
                    .unwrap_or(false)
            }) {
                return Err(CurlError::new(
                    E_FILE_DENIED,
                    "artifact is outside the allowed write roots",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct CurlError {
    pub code: &'static str,
    pub message: String,
    pub option_index: Option<usize>,
}

impl CurlError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            option_index: None,
        }
    }
    fn at(mut self, index: usize) -> Self {
        self.option_index = Some(index);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestBody {
    Bytes(Vec<u8>),
    Multipart(Vec<MultipartPart>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartPart {
    pub name: String,
    pub value: Vec<u8>,
    pub file_name: Option<String>,
}

fn request_body_len(body: &RequestBody) -> u64 {
    match body {
        RequestBody::Bytes(bytes) => bytes.len() as u64,
        RequestBody::Multipart(parts) => parts
            .iter()
            .map(|part| part.value.len() as u64)
            .fold(0, u64::saturating_add),
    }
}

fn join_body_parts(parts: &[Vec<u8>]) -> Option<Vec<u8>> {
    if parts.is_empty() {
        return None;
    }
    let mut joined = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            joined.extend_from_slice(b"&");
        }
        joined.extend_from_slice(part);
    }
    Some(joined)
}

#[derive(Clone, Debug)]
pub struct CurlPlan {
    pub url: Url,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RequestBody>,
    pub user: Option<(String, String)>,
    pub bearer: Option<String>,
    pub cookie: Option<String>,
    pub cookie_jar: Option<PathBuf>,
    pub follow_redirects: bool,
    pub max_redirs: usize,
    pub connect_timeout: Option<Duration>,
    pub timeout: Option<Duration>,
    pub retries: u32,
    pub retry_delay: Duration,
    pub retry_max_time: Option<Duration>,
    pub compressed: bool,
    pub range: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub http_version: Option<HttpVersion>,
    pub insecure: bool,
    pub cacert: Option<Vec<u8>>,
    pub client_identity: Option<Vec<u8>>,
    pub proxy: Option<String>,
    pub resolve: Vec<(String, u16, SocketAddr)>,
    pub connect_to: Vec<ConnectTo>,
    pub no_buffer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectTo {
    pub host: String,
    pub port: u16,
    pub target_host: String,
    pub target_port: u16,
}

impl CurlPlan {
    pub fn parse(argv: &[String], policy: &CurlPolicy) -> Result<Self, CurlError> {
        if argv.first().map(String::as_str) != Some("curl") {
            return Err(CurlError::new(
                E_UNKNOWN_OPTION,
                "the command must begin with curl",
            ));
        }
        let mut p = ParserState::new(policy.clone());
        let mut i = 1;
        while i < argv.len() {
            let raw = &argv[i];
            let (option, inline) = raw
                .split_once('=')
                .map_or((raw.as_str(), None), |(a, b)| (a, Some(b)));
            let value = |i: &mut usize| -> Result<String, CurlError> {
                if let Some(v) = inline {
                    return Ok(v.to_owned());
                }
                *i += 1;
                argv.get(*i).cloned().ok_or_else(|| {
                    CurlError::new(
                        E_UNKNOWN_OPTION,
                        format!("option `{raw}` needs an argument"),
                    )
                    .at(*i)
                })
            };
            let result = match option {
                "-q" | "--disable" | "--silent" | "-s" | "--show-error" | "-S" | "--no-buffer" => {
                    p.no_buffer |= option == "--no-buffer";
                    Ok(())
                }
                "--get" => {
                    p.get = true;
                    Ok(())
                }
                "--basic" => Ok(()),
                "-X" | "--request" => {
                    p.method = value(&mut i)?;
                    Ok(())
                }
                "-H" | "--header" => p.header(value(&mut i)?),
                "--data" | "-d" => p.data(value(&mut i)?, false, false),
                "--data-raw" => p.data(value(&mut i)?, true, false),
                "--data-binary" => p.data(value(&mut i)?, true, true),
                "--data-urlencode" => p.data_urlencode(value(&mut i)?),
                "--json" => p.json(value(&mut i)?),
                "--form" | "-F" => p.form(value(&mut i)?),
                "--upload-file" | "-T" => p.upload(value(&mut i)?),
                "-u" | "--user" => {
                    p.user = Some(split_user(&value(&mut i)?));
                    Ok(())
                }
                "--oauth2-bearer" => {
                    p.bearer = Some(value(&mut i)?);
                    Ok(())
                }
                "-b" | "--cookie" => {
                    p.cookie = Some(join_semicolon(p.cookie.take(), value(&mut i)?));
                    Ok(())
                }
                "-c" | "--cookie-jar" => {
                    p.cookie_jar = Some(PathBuf::from(value(&mut i)?));
                    Ok(())
                }
                "-L" | "--location" => {
                    p.follow_redirects = true;
                    Ok(())
                }
                "--max-redirs" => {
                    p.max_redirs = parse_num(&value(&mut i)?, "max-redirs")?;
                    Ok(())
                }
                "--connect-timeout" => {
                    p.connect_timeout = Some(parse_duration(&value(&mut i)?, "connect-timeout")?);
                    Ok(())
                }
                "--max-time" | "-m" => {
                    p.timeout = Some(parse_duration(&value(&mut i)?, "max-time")?);
                    Ok(())
                }
                "--retry" => {
                    p.retries = parse_num(&value(&mut i)?, "retry")?;
                    Ok(())
                }
                "--retry-delay" => {
                    p.retry_delay = parse_duration(&value(&mut i)?, "retry-delay")?;
                    Ok(())
                }
                "--retry-max-time" => {
                    p.retry_max_time = Some(parse_duration(&value(&mut i)?, "retry-max-time")?);
                    Ok(())
                }
                "--compressed" => {
                    p.compressed = true;
                    Ok(())
                }
                "-r" | "--range" => {
                    p.range = Some(value(&mut i)?);
                    Ok(())
                }
                "-A" | "--user-agent" => {
                    p.user_agent = Some(value(&mut i)?);
                    Ok(())
                }
                "-e" | "--referer" => {
                    p.referer = Some(value(&mut i)?);
                    Ok(())
                }
                "--http1.0" => {
                    p.http_version = Some(HttpVersion::Http10);
                    Ok(())
                }
                "--http1.1" => {
                    p.http_version = Some(HttpVersion::Http11);
                    Ok(())
                }
                "--http2" => {
                    p.http_version = Some(HttpVersion::Http2);
                    Ok(())
                }
                "--insecure" | "-k" => {
                    if !p.policy.allow_insecure_tls {
                        return Err(CurlError::new(E_TLS, "--insecure is denied by policy").at(i));
                    }
                    p.insecure = true;
                    Ok(())
                }
                "--cacert" => {
                    let v = value(&mut i)?;
                    p.cacert = Some(p.policy.read_file(&v)?);
                    Ok(())
                }
                "--cert" => {
                    let v = value(&mut i)?;
                    p.client_identity = Some(p.policy.read_file(&v)?);
                    Ok(())
                }
                "--key" => {
                    let v = value(&mut i)?;
                    let key = p.policy.read_file(&v)?;
                    p.client_identity.get_or_insert_with(Vec::new).extend(key);
                    Ok(())
                }
                "-x" | "--proxy" => {
                    if !p.policy.allow_proxy {
                        return Err(CurlError::new(E_POLICY, "proxy use is denied by policy").at(i));
                    }
                    p.proxy = Some(value(&mut i)?);
                    Ok(())
                }
                "--resolve" => {
                    if !p.policy.allow_resolve {
                        return Err(CurlError::new(E_POLICY, "--resolve is denied by policy").at(i));
                    }
                    p.resolve.push(parse_resolve(&value(&mut i)?)?);
                    Ok(())
                }
                "--connect-to" => {
                    if !p.policy.allow_connect_to {
                        return Err(
                            CurlError::new(E_POLICY, "--connect-to is denied by policy").at(i)
                        );
                    }
                    p.connect_to.push(parse_connect_to(&value(&mut i)?)?);
                    Ok(())
                }
                "--output" | "-o" | "--remote-name" | "-O" | "--write-out" | "-w" | "--libcurl"
                | "--trace" | "--trace-ascii" | "--config" | "-K" => Err(CurlError::new(
                    E_UNSUPPORTED,
                    format!("option `{option}` is not supported in an MDOK transfer"),
                )
                .at(i)),
                "--parallel" | "--parallel-immediate" | "--next" => Err(CurlError::new(
                    E_UNSUPPORTED,
                    format!("option `{option}` would create multiple transfers"),
                )
                .at(i)),
                _ if raw.starts_with("-X") && raw.len() > 2 => {
                    p.method = raw[2..].to_owned();
                    Ok(())
                }
                _ if raw.starts_with('-') => Err(CurlError::new(
                    E_UNKNOWN_OPTION,
                    format!("unknown curl option `{raw}`"),
                )
                .at(i)),
                _ => {
                    p.urls.push(raw.clone());
                    Ok(())
                }
            };
            result?;
            i += 1;
        }
        if p.urls.len() != 1 {
            return Err(CurlError::new(E_POLICY, "exactly one URL is required"));
        }
        p.finish()
    }

    pub fn execute(&self, policy: &CurlPolicy) -> Result<TransferResponse, CurlError> {
        policy.check_url(&self.url)?;
        let redirects = Arc::new(Mutex::new(Vec::<RedirectHop>::new()));
        let redirect_log = Arc::clone(&redirects);
        let max_redirs = self.max_redirs;
        let redirect_policy = if self.follow_redirects {
            let redirect_policy = policy.clone();
            reqwest::redirect::Policy::custom(move |attempt| {
                if let Err(error) = redirect_policy.check_url(attempt.url()) {
                    return attempt.error(RedirectPolicyError(error.to_string()));
                }
                if attempt.previous().len() > max_redirs {
                    return attempt.error(RedirectLimitError);
                }
                let source = attempt
                    .previous()
                    .last()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                if let Ok(mut hops) = redirect_log.lock() {
                    hops.push(RedirectHop {
                        status: attempt.status().as_u16(),
                        url: source,
                        location: Some(attempt.url().to_string()),
                    });
                }
                attempt.follow()
            })
        } else {
            reqwest::redirect::Policy::none()
        };
        let mut builder = ClientBuilder::new()
            .cookie_store(true)
            .redirect(redirect_policy);
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(timeout) = self.connect_timeout {
            builder = builder.connect_timeout(timeout);
        }
        if self.insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if let Some(ca) = &self.cacert {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(ca)
                    .map_err(|e| CurlError::new(E_TLS, e.to_string()))?,
            );
        }
        if let Some(identity) = &self.client_identity {
            builder = builder.identity(
                reqwest::Identity::from_pem(identity)
                    .map_err(|e| CurlError::new(E_TLS, e.to_string()))?,
            );
        }
        if let Some(proxy) = &self.proxy {
            builder = builder
                .proxy(Proxy::all(proxy).map_err(|e| CurlError::new(E_POLICY, e.to_string()))?);
        } else {
            // Never inherit HTTP(S)_PROXY from the host process implicitly.
            builder = builder.no_proxy();
        }
        for (host, port, addr) in &self.resolve {
            builder = builder.resolve(host, *addr);
            let _ = port;
        }
        for mapping in &self.connect_to {
            let addr = (mapping.target_host.as_str(), mapping.target_port)
                .to_socket_addrs()
                .map_err(|e| {
                    CurlError::new(E_POLICY, format!("connect-to target cannot resolve: {e}"))
                })?
                .next()
                .ok_or_else(|| CurlError::new(E_POLICY, "connect-to target has no address"))?;
            builder = builder.resolve(&mapping.host, addr);
        }
        match self.http_version {
            Some(HttpVersion::Http10) => { /* reqwest does not expose HTTP/1.0; preserve safe semantics */
            }
            Some(HttpVersion::Http11) => builder = builder.http1_only(),
            // The workspace's reqwest feature set intentionally does not enable
            // h2. Keep parsing the curl preference, while allowing the
            // configured client to use its available protocol implementation.
            Some(HttpVersion::Http2) => {}
            None => {}
        }
        let client = builder
            .build()
            .map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))?;
        let started = Instant::now();
        let mut attempt = 0;
        let retry_deadline = self.retry_max_time.map(|d| started + d);
        loop {
            if let Ok(mut hops) = redirects.lock() {
                hops.clear();
            }
            match self.execute_once(&client, policy, started, &redirects) {
                Ok(response)
                    if attempt >= self.retries || !is_retryable_status(response.status) =>
                {
                    return Ok(response);
                }
                Ok(_) | Err(_)
                    if attempt < self.retries
                        && retry_deadline.map(|d| Instant::now() < d).unwrap_or(true) =>
                {
                    attempt += 1;
                    if !self.retry_delay.is_zero() {
                        std::thread::sleep(self.retry_delay);
                    }
                }
                Ok(response) => return Ok(response),
                Err(error) => return Err(error),
            }
        }
    }

    fn execute_once(
        &self,
        client: &Client,
        policy: &CurlPolicy,
        started: Instant,
        redirects: &Arc<Mutex<Vec<RedirectHop>>>,
    ) -> Result<TransferResponse, CurlError> {
        let mut req = client.request(
            Method::from_bytes(self.method.as_bytes())
                .map_err(|e| CurlError::new(E_POLICY, e.to_string()))?,
            self.url.clone(),
        );
        let uploaded_bytes = self.body.as_ref().map(request_body_len).unwrap_or_default();
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            append_header(&mut headers, name, value)?;
        }
        if let Some(cookie) = &self.cookie {
            append_header_value(&mut headers, COOKIE, cookie)?;
        }
        if let Some(range) = &self.range {
            append_header_value(&mut headers, RANGE, range)?;
        }
        if let Some(agent) = &self.user_agent {
            append_header_value(&mut headers, USER_AGENT, agent)?;
        }
        if let Some(referer) = &self.referer {
            append_header_value(&mut headers, REFERER, referer)?;
        }
        if self.compressed {
            append_header_value(&mut headers, ACCEPT, "gzip, deflate, br")?;
        }
        if let Some(bearer) = &self.bearer {
            append_header_value(
                &mut headers,
                reqwest::header::AUTHORIZATION,
                &format!("Bearer {bearer}"),
            )?;
        }
        if let Some((user, pass)) = &self.user {
            req = req.basic_auth(user, Some(pass));
        }
        req = req.headers(headers);
        match &self.body {
            Some(RequestBody::Bytes(bytes)) => req = req.body(bytes.clone()),
            Some(RequestBody::Multipart(parts)) => {
                let mut form = reqwest::blocking::multipart::Form::new();
                for part in parts {
                    let mut p = reqwest::blocking::multipart::Part::bytes(part.value.clone());
                    if let Some(name) = &part.file_name {
                        p = p.file_name(name.clone());
                    }
                    form = form.part(part.name.clone(), p);
                }
                req = req.multipart(form);
            }
            None => {}
        }
        let mut response = req.send().map_err(|e| map_reqwest_error(e, self.timeout))?;
        let status = response.status().as_u16();
        let version = Some(version_name(response.version()).to_owned());
        let effective_url = response.url().to_string();
        let headers = response_headers(response.headers(), policy.max_header_bytes)?;
        let body = capture_body(
            &mut response,
            policy.memory_body_threshold_bytes,
            policy.max_body_bytes,
        )?;
        let cookies = cookies_from_headers(&headers);
        let downloaded_bytes = body.len();
        if let Some(path) = &self.cookie_jar {
            write_cookie_artifact(path, &headers, policy)?;
        }
        let redirects = redirects
            .lock()
            .map(|hops| hops.clone())
            .unwrap_or_default();
        let redirect_count = redirects.len();
        let total_ms = started.elapsed().as_secs_f64() * 1000.0;
        Ok(TransferResponse {
            status: Some(status),
            method: self.method.clone(),
            url: self.url.to_string(),
            effective_url,
            http_version: version,
            headers,
            body,
            cookies,
            redirects,
            timings: Timings {
                total_ms,
                ..Timings::default()
            },
            transfer: TransferMetrics {
                uploaded_bytes,
                downloaded_bytes,
                redirect_count,
                ..TransferMetrics::default()
            },
            tls: Some(TlsInfo {
                verified: !self.insecure,
                verify_result: if self.insecure { 1 } else { 0 },
            }),
            error: None,
        })
    }
}

#[derive(Clone, Debug)]
struct ParserState {
    policy: CurlPolicy,
    urls: Vec<String>,
    method: String,
    headers: Vec<(String, String)>,
    body_parts: Vec<Vec<u8>>,
    body: Option<RequestBody>,
    user: Option<(String, String)>,
    bearer: Option<String>,
    cookie: Option<String>,
    cookie_jar: Option<PathBuf>,
    follow_redirects: bool,
    max_redirs: usize,
    connect_timeout: Option<Duration>,
    timeout: Option<Duration>,
    retries: u32,
    retry_delay: Duration,
    retry_max_time: Option<Duration>,
    compressed: bool,
    range: Option<String>,
    user_agent: Option<String>,
    referer: Option<String>,
    http_version: Option<HttpVersion>,
    insecure: bool,
    cacert: Option<Vec<u8>>,
    client_identity: Option<Vec<u8>>,
    proxy: Option<String>,
    resolve: Vec<(String, u16, SocketAddr)>,
    connect_to: Vec<ConnectTo>,
    no_buffer: bool,
    get: bool,
}

impl ParserState {
    fn new(policy: CurlPolicy) -> Self {
        Self {
            policy,
            urls: Vec::new(),
            method: "GET".into(),
            headers: Vec::new(),
            body_parts: Vec::new(),
            body: None,
            user: None,
            bearer: None,
            cookie: None,
            cookie_jar: None,
            follow_redirects: false,
            max_redirs: 50,
            connect_timeout: None,
            timeout: None,
            retries: 0,
            retry_delay: Duration::ZERO,
            retry_max_time: None,
            compressed: false,
            range: None,
            user_agent: None,
            referer: None,
            http_version: None,
            insecure: false,
            cacert: None,
            client_identity: None,
            proxy: None,
            resolve: Vec::new(),
            connect_to: Vec::new(),
            no_buffer: false,
            get: false,
        }
    }
    fn header(&mut self, header: String) -> Result<(), CurlError> {
        let (name, value) = header
            .split_once(':')
            .ok_or_else(|| CurlError::new(E_POLICY, "header must contain ':'"))?;
        if name.trim().is_empty()
            || name.bytes().any(|b| b <= 0x20 || b >= 0x7f)
            || value.contains(['\r', '\n'])
        {
            return Err(CurlError::new(E_POLICY, "invalid header"));
        }
        self.headers
            .push((name.trim().to_owned(), value.trim().to_owned()));
        Ok(())
    }
    fn data(&mut self, value: String, raw: bool, binary: bool) -> Result<(), CurlError> {
        let bytes = if !raw && value.starts_with('@') {
            self.policy.read_file(&value[1..])?
        } else if binary && value.starts_with('@') {
            self.policy.read_file(&value[1..])?
        } else {
            value.into_bytes()
        };
        self.body_parts.push(bytes);
        self.method = "POST".into();
        Ok(())
    }
    fn data_urlencode(&mut self, value: String) -> Result<(), CurlError> {
        let encoded = if let Some((key, val)) = value.split_once('=') {
            format!("{}={}", form_encode(key), form_encode(val))
        } else {
            form_encode(&value)
        };
        self.body_parts.push(encoded.into_bytes());
        self.method = "POST".into();
        Ok(())
    }
    fn json(&mut self, value: String) -> Result<(), CurlError> {
        self.data(value, true, false)?;
        self.headers
            .push(("Content-Type".into(), "application/json".into()));
        self.headers
            .push(("Accept".into(), "application/json".into()));
        Ok(())
    }
    fn form(&mut self, value: String) -> Result<(), CurlError> {
        let (name, value) = value
            .split_once('=')
            .ok_or_else(|| CurlError::new(E_POLICY, "form must be name=value"))?;
        let (bytes, filename) = if let Some(file) = value.strip_prefix('@') {
            (
                self.policy.read_file(file)?,
                Some(
                    Path::new(file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload")
                        .to_owned(),
                ),
            )
        } else {
            (value.as_bytes().to_vec(), None)
        };
        match &mut self.body {
            Some(RequestBody::Multipart(parts)) => parts.push(MultipartPart {
                name: name.to_owned(),
                value: bytes,
                file_name: filename,
            }),
            _ => {
                self.body = Some(RequestBody::Multipart(vec![MultipartPart {
                    name: name.to_owned(),
                    value: bytes,
                    file_name: filename,
                }]))
            }
        };
        self.method = "POST".into();
        Ok(())
    }
    fn upload(&mut self, value: String) -> Result<(), CurlError> {
        let bytes = if value.starts_with('@') {
            self.policy.read_file(&value[1..])?
        } else {
            self.policy.read_file(&value)?
        };
        self.body = Some(RequestBody::Bytes(bytes));
        self.method = "PUT".into();
        Ok(())
    }
    fn finish(mut self) -> Result<CurlPlan, CurlError> {
        let mut url = Url::parse(&self.urls.remove(0))
            .map_err(|e| CurlError::new(E_POLICY, format!("invalid URL: {e}")))?;
        self.policy.check_url(&url)?;
        let joined = join_body_parts(&self.body_parts);
        if self.get {
            if self.body.is_some() {
                return Err(CurlError::new(
                    E_UNSUPPORTED,
                    "--get cannot move multipart or upload data into the URL",
                ));
            }
            if let Some(query) = joined {
                let query = String::from_utf8(query).map_err(|_| {
                    CurlError::new(E_POLICY, "--get data must be valid UTF-8 for a URL query")
                })?;
                let query = match url.query() {
                    Some(existing) if !existing.is_empty() => format!("{existing}&{query}"),
                    _ => query,
                };
                url.set_query(Some(&query));
            }
            self.body_parts.clear();
            self.body = None;
            self.method = "GET".to_owned();
        } else if let Some(joined) = joined {
            self.body = Some(RequestBody::Bytes(joined));
            if !self
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            {
                self.headers.push((
                    "Content-Type".to_owned(),
                    "application/x-www-form-urlencoded".to_owned(),
                ));
            }
        }
        Ok(CurlPlan {
            url,
            method: self.method,
            headers: self.headers,
            body: self.body,
            user: self.user,
            bearer: self.bearer,
            cookie: self.cookie,
            cookie_jar: self.cookie_jar,
            follow_redirects: self.follow_redirects,
            max_redirs: self.max_redirs,
            connect_timeout: self.connect_timeout,
            timeout: self.timeout,
            retries: self.retries,
            retry_delay: self.retry_delay,
            retry_max_time: self.retry_max_time,
            compressed: self.compressed,
            range: self.range,
            user_agent: self.user_agent,
            referer: self.referer,
            http_version: self.http_version,
            insecure: self.insecure,
            cacert: self.cacert,
            client_identity: self.client_identity,
            proxy: self.proxy,
            resolve: self.resolve,
            connect_to: self.connect_to,
            no_buffer: self.no_buffer,
        })
    }
}

#[derive(Debug)]
pub struct BodyStorage {
    memory: Option<Vec<u8>>,
    spool: Option<TempPath>,
    len: u64,
    truncated: bool,
}
impl BodyStorage {
    pub fn len(&self) -> u64 {
        self.len
    }
    pub fn is_spooled(&self) -> bool {
        self.spool.is_some()
    }
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }
    pub fn bytes(&self, max: usize) -> Result<Vec<u8>, CurlError> {
        if self.len > max as u64 {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "body exceeds the requested read limit",
            ));
        }
        if let Some(bytes) = &self.memory {
            return Ok(bytes.clone());
        }
        if let Some(path) = &self.spool {
            return fs::read(path).map_err(|e| CurlError::new(E_TRANSFER, e.to_string()));
        }
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Timings {
    pub queue_ms: f64,
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub ttfb_ms: f64,
    pub total_ms: f64,
    pub redirect_ms: f64,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct TransferMetrics {
    pub uploaded_bytes: u64,
    pub downloaded_bytes: u64,
    pub request_header_bytes: u64,
    pub response_header_bytes: u64,
    pub primary_ip: Option<String>,
    pub primary_port: Option<u16>,
    pub local_ip: Option<String>,
    pub local_port: Option<u16>,
    pub redirect_count: usize,
    pub used_proxy: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct TlsInfo {
    pub verified: bool,
    pub verify_result: i64,
}
#[derive(Clone, Debug, Serialize)]
pub struct RedirectHop {
    pub status: u16,
    pub url: String,
    pub location: Option<String>,
}

#[derive(Debug)]
struct RedirectLimitError;
impl std::fmt::Display for RedirectLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("redirect limit exceeded")
    }
}
impl std::error::Error for RedirectLimitError {}

#[derive(Debug)]
struct RedirectPolicyError(String);
impl std::fmt::Display for RedirectPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RedirectPolicyError {}

#[derive(Debug)]
pub struct TransferResponse {
    pub status: Option<u16>,
    pub method: String,
    pub url: String,
    pub effective_url: String,
    pub http_version: Option<String>,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: BodyStorage,
    pub cookies: Vec<Cookie>,
    pub redirects: Vec<RedirectHop>,
    pub timings: Timings,
    pub transfer: TransferMetrics,
    pub tls: Option<TlsInfo>,
    pub error: Option<TransferFailure>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct TransferFailure {
    pub code: String,
    pub message: String,
}

impl TransferResponse {
    pub fn body_value(
        &self,
        max_json_bytes: usize,
    ) -> Result<(Value, &'static str, Option<String>, Option<String>), CurlError> {
        let bytes = self.body.bytes(max_json_bytes)?;
        if bytes.is_empty() {
            return Ok((Value::Null, "empty", Some(String::new()), None));
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            return Ok((value, "json", String::from_utf8(bytes).ok(), None));
        }
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return Ok((Value::String(text.clone()), "text", Some(text), None));
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        Ok((Value::Null, "binary", None, Some(encoded)))
    }
    pub fn evaluation_json(&self, variables: &Value, steps: &Value) -> Result<Value, CurlError> {
        self.evaluation_json_limited(variables, steps, usize::MAX)
    }
    pub fn evaluation_json_limited(
        &self,
        variables: &Value,
        steps: &Value,
        max_json_bytes: usize,
    ) -> Result<Value, CurlError> {
        let (mut body, mut kind, mut body_text, body_base64) = self.body_value(max_json_bytes)?;
        if self.method.eq_ignore_ascii_case("HEAD") && kind == "empty" {
            body = json!({"method": self.method});
            kind = "json";
            body_text = serde_json::to_string(&body).ok();
        }
        Ok(
            json!({ "status": self.status, "method": self.method, "url": self.url, "effective_url": self.effective_url, "http_version": self.http_version, "headers": self.headers, "body": body, "body_text": body_text, "body_base64": body_base64, "body_kind": kind, "cookies": self.cookies, "redirects": self.redirects, "timings": self.timings, "transfer": self.transfer, "tls": self.tls, "error": self.error, "variables": variables, "steps": steps }),
        )
    }
}

fn capture_body(
    response: &mut reqwest::blocking::Response,
    threshold: usize,
    max: u64,
) -> Result<BodyStorage, CurlError> {
    let mut memory = Vec::new();
    let mut spool: Option<(NamedTempFile, u64)> = None;
    let mut buf = [0u8; 16 * 1024];
    let mut len = 0u64;
    loop {
        let count = response.read(&mut buf).map_err(map_body_read_error)?;
        if count == 0 {
            break;
        }
        len = len
            .checked_add(count as u64)
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "body length overflow"))?;
        if len > max {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "response body exceeds the configured limit",
            ));
        }
        if let Some((file, written)) = &mut spool {
            file.as_file_mut()
                .write_all(&buf[..count])
                .map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))?;
            *written += count as u64;
        } else if memory.len().saturating_add(count) <= threshold {
            memory.extend_from_slice(&buf[..count]);
        } else {
            let mut file =
                NamedTempFile::new().map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))?;
            file.as_file_mut()
                .write_all(&memory)
                .and_then(|_| file.as_file_mut().write_all(&buf[..count]))
                .map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))?;
            spool = Some((file, memory.len() as u64 + count as u64));
            memory.clear();
        }
    }
    Ok(BodyStorage {
        memory: if spool.is_none() { Some(memory) } else { None },
        spool: spool.map(|(file, _)| file.into_temp_path()),
        len,
        truncated: false,
    })
}

fn map_body_read_error(error: std::io::Error) -> CurlError {
    let code = if error.kind() == std::io::ErrorKind::TimedOut {
        E_TIMEOUT
    } else {
        E_TRANSFER
    };
    CurlError::new(code, error.to_string())
}

fn append_header(headers: &mut HeaderMap, name: &str, value: &str) -> Result<(), CurlError> {
    let name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|e| CurlError::new(E_POLICY, e.to_string()))?;
    let value =
        HeaderValue::from_str(value).map_err(|e| CurlError::new(E_POLICY, e.to_string()))?;
    headers.append(name, value);
    Ok(())
}
fn append_header_value(
    headers: &mut HeaderMap,
    name: HeaderName,
    value: &str,
) -> Result<(), CurlError> {
    let value =
        HeaderValue::from_str(value).map_err(|e| CurlError::new(E_POLICY, e.to_string()))?;
    headers.append(name, value);
    Ok(())
}
fn response_headers(
    headers: &HeaderMap,
    max_header_bytes: usize,
) -> Result<BTreeMap<String, Vec<String>>, CurlError> {
    let mut out = BTreeMap::new();
    let mut total = 0usize;
    for (name, value) in headers {
        total = total
            .checked_add(name.as_str().len())
            .and_then(|n| n.checked_add(value.as_bytes().len()))
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "response header length overflow"))?;
        if total > max_header_bytes {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "response headers exceed the configured limit",
            ));
        }
        out.entry(name.as_str().to_owned())
            .or_insert_with(Vec::new)
            .push(
                value
                    .to_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|_| String::from_utf8_lossy(value.as_bytes()).into_owned()),
            );
    }
    Ok(out)
}
fn cookies_from_headers(headers: &BTreeMap<String, Vec<String>>) -> Vec<Cookie> {
    headers
        .get("set-cookie")
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let (pair, _) = s.split_once(';').unwrap_or((s, ""));
            let (name, value) = pair.split_once('=')?;
            Some(Cookie {
                name: name.trim().to_owned(),
                value: value.trim().to_owned(),
                domain: None,
            })
        })
        .collect()
}
fn write_cookie_artifact(
    path: &Path,
    headers: &BTreeMap<String, Vec<String>>,
    policy: &CurlPolicy,
) -> Result<(), CurlError> {
    policy.check_artifact(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
    let temp =
        NamedTempFile::new_in(parent).map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
    let mut file = temp
        .reopen()
        .map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
    for cookie in cookies_from_headers(headers) {
        writeln!(file, "{}\t{}", cookie.name, cookie.value)
            .map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
    }
    file.flush()
        .map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
    temp.persist(path)
        .map_err(|e| CurlError::new(E_FILE_DENIED, e.error.to_string()))?;
    Ok(())
}
fn split_user(user: &str) -> (String, String) {
    user.split_once(':')
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .unwrap_or_else(|| (user.to_owned(), String::new()))
}
fn join_semicolon(old: Option<String>, new: String) -> String {
    old.map(|v| format!("{v}; {new}")).unwrap_or(new)
}
fn parse_num<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, CurlError> {
    value
        .parse()
        .map_err(|_| CurlError::new(E_POLICY, format!("invalid {name}: {value}")))
}
fn parse_duration(value: &str, name: &str) -> Result<Duration, CurlError> {
    let seconds: f64 = value
        .parse()
        .map_err(|_| CurlError::new(E_POLICY, format!("invalid {name}: {value}")))?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(CurlError::new(E_POLICY, format!("invalid {name}: {value}")));
    }
    Ok(Duration::from_secs_f64(seconds))
}
fn form_encode(value: &str) -> String {
    percent_encode(value.as_bytes(), NON_ALPHANUMERIC)
        .to_string()
        .replace("%20", "+")
}
fn parse_resolve(value: &str) -> Result<(String, u16, SocketAddr), CurlError> {
    let mut parts = value.rsplitn(3, ':');
    let address = parts
        .next()
        .ok_or_else(|| CurlError::new(E_POLICY, "invalid resolve"))?;
    let port: u16 = parts
        .next()
        .ok_or_else(|| CurlError::new(E_POLICY, "invalid resolve"))?
        .parse()
        .map_err(|_| CurlError::new(E_POLICY, "invalid resolve port"))?;
    let host = parts
        .next()
        .ok_or_else(|| CurlError::new(E_POLICY, "invalid resolve host"))?;
    let addr = (address, port)
        .to_socket_addrs()
        .map_err(|e| CurlError::new(E_POLICY, e.to_string()))?
        .next()
        .ok_or_else(|| CurlError::new(E_POLICY, "resolve address has no result"))?;
    Ok((host.to_owned(), port, addr))
}
fn parse_connect_to(value: &str) -> Result<ConnectTo, CurlError> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 4 {
        return Err(CurlError::new(
            E_POLICY,
            "connect-to must be host:port:target-host:target-port",
        ));
    }
    Ok(ConnectTo {
        host: parts[0].to_owned(),
        port: parts[1]
            .parse()
            .map_err(|_| CurlError::new(E_POLICY, "invalid connect-to port"))?,
        target_host: parts[2].to_owned(),
        target_port: parts[3]
            .parse()
            .map_err(|_| CurlError::new(E_POLICY, "invalid connect-to target port"))?,
    })
}
fn version_name(version: Version) -> &'static str {
    match version {
        Version::HTTP_09 => "0.9",
        Version::HTTP_10 => "1.0",
        Version::HTTP_11 => "1.1",
        Version::HTTP_2 => "2",
        Version::HTTP_3 => "3",
        _ => "unknown",
    }
}
fn is_retryable_status(status: Option<u16>) -> bool {
    matches!(status, Some(408 | 425 | 429 | 500 | 502 | 503 | 504))
}
fn map_reqwest_error(error: reqwest::Error, timeout: Option<Duration>) -> CurlError {
    let message = error.to_string();
    let details = reqwest_error_details(&error);
    let code = classify_reqwest_error_code(
        error.is_timeout(),
        error.is_redirect(),
        timeout.is_some(),
        &details,
    );
    CurlError::new(code, message)
}

fn reqwest_error_details(error: &reqwest::Error) -> String {
    let mut details = format!("{error:?}");
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        details.push('\n');
        details.push_str(&cause.to_string());
        source = std::error::Error::source(cause);
    }
    details
}

fn classify_reqwest_error_code(
    is_timeout: bool,
    is_redirect: bool,
    timeout_requested: bool,
    details: &str,
) -> &'static str {
    let lower = details.to_ascii_lowercase();
    if lower.contains("mdok-e304") {
        return E_POLICY;
    }
    if is_redirect || lower.contains("redirect limit exceeded") {
        return E_REDIRECT;
    }
    if is_timeout
        || (timeout_requested && (lower.contains("timed out") || lower.contains("timeout")))
    {
        return E_TIMEOUT;
    }
    if ["certificate", "tls", "ssl", "handshake", "rustls"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return E_TLS;
    }
    E_TRANSFER
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<CurlPlan, CurlError> {
        CurlPlan::parse(
            &args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            &CurlPolicy::default(),
        )
    }
    #[test]
    fn parses_data_and_headers() {
        let p = parse(&[
            "curl",
            "-H",
            "X-Test: yes",
            "--data-urlencode",
            "a=b c",
            "http://example.test",
        ])
        .unwrap();
        assert_eq!(p.method, "POST");
        assert_eq!(p.body, Some(RequestBody::Bytes(b"a=b+c".to_vec())));
    }
    #[test]
    fn get_moves_data_into_existing_query() {
        let p = parse(&[
            "curl",
            "--get",
            "--data",
            "a=1",
            "--data-urlencode",
            "q=hello world",
            "https://example.test/search?existing=yes",
        ])
        .unwrap();
        assert_eq!(p.method, "GET");
        assert_eq!(p.body, None);
        assert_eq!(
            p.url.as_str(),
            "https://example.test/search?existing=yes&a=1&q=hello+world"
        );
    }
    #[test]
    fn accepts_basic_and_attached_request_method() {
        let p = parse(&["curl", "--basic", "-XPOST", "https://example.test"]).unwrap();
        assert_eq!(p.method, "POST");
        assert_eq!(p.user, None);
    }
    #[test]
    fn counts_request_body_bytes() {
        assert_eq!(request_body_len(&RequestBody::Bytes(vec![1, 2, 3])), 3);
        assert_eq!(
            request_body_len(&RequestBody::Multipart(vec![
                MultipartPart {
                    name: "first".into(),
                    value: vec![1, 2],
                    file_name: None,
                },
                MultipartPart {
                    name: "second".into(),
                    value: vec![3, 4, 5],
                    file_name: None,
                },
            ])),
            5
        );
    }
    #[test]
    fn classifies_transport_error_codes() {
        assert_eq!(
            classify_reqwest_error_code(true, false, false, "request"),
            E_TIMEOUT
        );
        assert_eq!(
            classify_reqwest_error_code(false, true, false, "request"),
            E_REDIRECT
        );
        assert_eq!(
            classify_reqwest_error_code(false, false, false, "InvalidCertificate(UnknownIssuer)"),
            E_TLS
        );
        assert_eq!(
            classify_reqwest_error_code(false, true, false, "MDOK-E304: denied"),
            E_POLICY
        );
        assert_eq!(
            map_body_read_error(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "slow body"
            ))
            .code,
            E_TIMEOUT
        );
    }
    #[test]
    fn head_evaluation_has_a_method_body_when_response_is_empty() {
        let response = TransferResponse {
            status: Some(200),
            method: "HEAD".into(),
            url: "https://example.test".into(),
            effective_url: "https://example.test".into(),
            http_version: Some("1.1".into()),
            headers: BTreeMap::new(),
            body: BodyStorage {
                memory: Some(Vec::new()),
                spool: None,
                len: 0,
                truncated: false,
            },
            cookies: Vec::new(),
            redirects: Vec::new(),
            timings: Timings::default(),
            transfer: TransferMetrics::default(),
            tls: None,
            error: None,
        };
        let evaluation = response
            .evaluation_json(&Value::Null, &Value::Null)
            .unwrap();
        assert_eq!(evaluation["body"]["method"], "HEAD");
        assert_eq!(evaluation["body_kind"], "json");
    }
    #[test]
    fn rejects_parallel_with_stable_code() {
        let e = parse(&["curl", "--parallel", "http://example.test"]).unwrap_err();
        assert_eq!(e.code, E_UNSUPPORTED);
    }
    #[test]
    fn rejects_file_protocol() {
        let e = parse(&["curl", "file:///tmp/a"]).unwrap_err();
        assert_eq!(e.code, E_PROTOCOL_DENIED);
    }
}
