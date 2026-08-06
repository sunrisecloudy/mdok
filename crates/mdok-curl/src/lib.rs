#![forbid(unsafe_code)]

//! Curl-compatible request planning and bounded HTTP execution.
//!
//! A pinned curl release is linked into the native bridge. The public plan
//! remains independent of curl's private structs. Transfer options that the
//! bridge can represent use the vendored parser/libcurl path; the broader
//! compatibility subset uses the in-process Rust adapter. No shell or child
//! process is started.

use base64::Engine as _;
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{
    ACCEPT, COOKIE, HeaderMap, HeaderName, HeaderValue, RANGE, REFERER, USER_AGENT,
};
use reqwest::{Method, Proxy, Version};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::future;
use std::io::{Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
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
pub const E_CONNECT_POLICY: &str = "MDOK-E604";
pub const E_REDIRECT: &str = "MDOK-E603";
pub const E_TLS: &str = "MDOK-E602";
pub const E_CANCELLED: &str = "MDOK-E605";
pub const E_BODY_LIMIT: &str = "MDOK-E700";

const ARTIFACT_COPY_BUFFER_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub struct CurlPolicy {
    pub allowed_schemes: HashSet<String>,
    pub allowed_hosts: Option<HashSet<String>>,
    pub allowed_host_patterns: Vec<String>,
    pub denied_host_patterns: Vec<String>,
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
            allowed_host_patterns: Vec::new(),
            denied_host_patterns: Vec::new(),
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
        if !self.denied_host_patterns.is_empty()
            && self
                .denied_host_patterns
                .iter()
                .any(|pattern| host_matches_pattern(url.host_str().unwrap_or_default(), pattern))
        {
            return Err(CurlError::new(
                E_POLICY,
                format!("host `{}` is denied", url.host_str().unwrap_or_default()),
            ));
        }
        if !self.allowed_host_patterns.is_empty()
            && !self
                .allowed_host_patterns
                .iter()
                .any(|pattern| host_matches_pattern(url.host_str().unwrap_or_default(), pattern))
        {
            return Err(CurlError::new(
                E_POLICY,
                format!(
                    "host `{}` is not allowed",
                    url.host_str().unwrap_or_default()
                ),
            ));
        }
        if !self.allow_private_network
            && let Some(host) = url.host_str()
        {
            let private = host == "localhost"
                || host == "localhost.localdomain"
                || host
                    .parse::<std::net::IpAddr>()
                    .map(denied_address)
                    .unwrap_or(false);
            if private {
                return Err(CurlError::new(
                    E_POLICY,
                    "private and loopback destinations are denied",
                ));
            }
        }
        Ok(())
    }

    fn check_socket_address(&self, address: SocketAddr) -> Result<(), CurlError> {
        if !self.allow_private_network && denied_address(address.ip()) {
            return Err(CurlError::new(
                E_CONNECT_POLICY,
                format!("resolved destination `{address}` is private or link-local"),
            ));
        }
        Ok(())
    }

    fn check_resolved_url(&self, url: &Url) -> Result<(), CurlError> {
        if self.allow_private_network {
            return Ok(());
        }
        let Some(host) = url.host_str() else {
            return Ok(());
        };
        let port = url
            .port_or_known_default()
            .ok_or_else(|| CurlError::new(E_POLICY, "URL has no known port"))?;
        self.resolve_host(host, port).map(|_| ())
    }

    fn resolve_host(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, CurlError> {
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|error| {
                CurlError::new(
                    E_CONNECT_POLICY,
                    format!("destination cannot resolve: {error}"),
                )
            })?
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(CurlError::new(
                E_CONNECT_POLICY,
                "destination has no resolved addresses",
            ));
        }
        if !self.allow_private_network {
            for address in &addresses {
                self.check_socket_address(*address)?;
            }
        }
        Ok(addresses)
    }

    fn read_file(&self, raw: &str) -> Result<Vec<u8>, CurlError> {
        let canonical = self.canonical_read_path(raw)?;
        fs::read(&canonical)
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot read file: {e}")))
    }

    fn canonical_read_path(&self, raw: &str) -> Result<PathBuf, CurlError> {
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
        Ok(canonical)
    }

    fn file_len(&self, raw: &str) -> Result<u64, CurlError> {
        let canonical = self.canonical_read_path(raw)?;
        fs::metadata(&canonical)
            .map(|metadata| metadata.len())
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot access file: {e}")))
    }

    fn read_file_limited(&self, raw: &str, max_bytes: u64) -> Result<Vec<u8>, CurlError> {
        let canonical = self.canonical_read_path(raw)?;
        let mut file = fs::File::open(&canonical)
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot read file: {e}")))?;
        let length = file
            .metadata()
            .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot access file: {e}")))?
            .len();
        if length > max_bytes {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "request body exceeds the configured limit",
            ));
        }
        let capacity = usize::try_from(length).map_err(|_| {
            CurlError::new(
                E_BODY_LIMIT,
                "request body cannot fit in the configured memory model",
            )
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|_| {
            CurlError::new(
                E_BODY_LIMIT,
                "request body allocation exceeds the configured limit",
            )
        })?;
        let mut total = 0u64;
        let mut buffer = [0u8; 16 * 1024];
        loop {
            let remaining = max_bytes.saturating_sub(total);
            if remaining == 0 {
                let mut extra = [0u8; 1];
                let count = file
                    .read(&mut extra)
                    .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot read file: {e}")))?;
                if count != 0 {
                    return Err(CurlError::new(
                        E_BODY_LIMIT,
                        "request body exceeds the configured limit",
                    ));
                }
                break;
            }
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded body read fits usize");
            let count = file
                .read(&mut buffer[..requested])
                .map_err(|e| CurlError::new(E_FILE_DENIED, format!("cannot read file: {e}")))?;
            if count == 0 {
                break;
            }
            bytes.try_reserve(count).map_err(|_| {
                CurlError::new(
                    E_BODY_LIMIT,
                    "request body allocation exceeds the configured limit",
                )
            })?;
            bytes.extend_from_slice(&buffer[..count]);
            total = total
                .checked_add(count as u64)
                .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
        }
        Ok(bytes)
    }

    fn check_artifact(&self, path: &Path) -> Result<(), CurlError> {
        if !self.allow_artifact_writes {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact writes are disabled by policy",
            ));
        }
        validate_artifact_path(path)?;
        if self.allowed_artifact_roots.is_empty() {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact writes require an allowed write root",
            ));
        }

        let canonical_parent = canonical_artifact_parent(path)?;
        let canonical_roots = self
            .allowed_artifact_roots
            .iter()
            .filter_map(|root| fs::canonicalize(root).ok())
            .collect::<Vec<_>>();
        let confined = canonical_roots
            .iter()
            .any(|root| canonical_parent.starts_with(root));
        if !confined {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact is outside the allowed write roots",
            ));
        }
        reject_unsafe_artifact_symlink_parents(path, &self.allowed_artifact_roots)?;
        Ok(())
    }

    fn checked_artifact_parent(&self, path: &Path) -> Result<PathBuf, CurlError> {
        self.check_artifact(path)?;
        canonical_artifact_parent(path)
    }
}

fn validate_artifact_path(path: &Path) -> Result<(), CurlError> {
    let raw = path.to_string_lossy();
    if raw.is_empty()
        || raw.ends_with('/')
        || raw.ends_with('\\')
        || raw.contains('\0')
        || path.file_name().is_none()
    {
        return Err(CurlError::new(
            E_FILE_DENIED,
            "artifact destination must name a file",
        ));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || raw
            .split(['/', '\\'])
            .any(|component| matches!(component, "." | ".."))
        || looks_like_windows_absolute_path(&raw)
    {
        return Err(CurlError::new(
            E_FILE_DENIED,
            "artifact destination contains an unsafe relative path",
        ));
    }
    Ok(())
}

fn looks_like_windows_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with("\\\\")
        || path.starts_with("//")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn absolute_artifact_parent(path: &Path) -> Result<PathBuf, CurlError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if parent.is_absolute() {
        return Ok(parent.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(parent))
        .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))
}

fn absolute_artifact_path(path: &Path) -> Result<PathBuf, CurlError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))
}

fn canonical_artifact_parent(path: &Path) -> Result<PathBuf, CurlError> {
    let absolute_parent = absolute_artifact_parent(path)?;
    let mut cursor = PathBuf::new();
    for component in absolute_parent.components() {
        cursor.push(component.as_os_str());
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = fs::metadata(&cursor)
                    .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
                if !target.is_dir() {
                    return Err(CurlError::new(
                        E_FILE_DENIED,
                        "artifact destination parent is not a directory",
                    ));
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(CurlError::new(
                    E_FILE_DENIED,
                    "artifact destination parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(CurlError::new(E_FILE_DENIED, error.to_string()));
            }
        }
    }

    let mut existing = absolute_parent.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            CurlError::new(
                E_FILE_DENIED,
                "artifact destination parent cannot be resolved",
            )
        })?;
        missing.push(name.to_owned());
        if !existing.pop() {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact destination parent cannot be resolved",
            ));
        }
    }
    let mut canonical = fs::canonicalize(&existing)
        .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
    for name in missing.iter().rev() {
        canonical.push(name);
    }
    Ok(canonical)
}

fn reject_unsafe_artifact_symlink_parents(
    path: &Path,
    configured_roots: &[PathBuf],
) -> Result<(), CurlError> {
    let absolute_parent = absolute_artifact_parent(path)?;
    let absolute_roots = configured_roots
        .iter()
        .filter_map(|root| absolute_artifact_path(root).ok())
        .collect::<Vec<_>>();
    let mut cursor = PathBuf::new();
    for component in absolute_parent.components() {
        cursor.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&cursor) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(CurlError::new(E_FILE_DENIED, error.to_string())),
        };
        if !metadata.file_type().is_symlink() {
            continue;
        }
        let trusted_boundary = absolute_roots
            .iter()
            .any(|root| cursor == *root || root.starts_with(&cursor));
        if !trusted_boundary {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact destination has a symlink parent",
            ));
        }
    }
    Ok(())
}

#[derive(Clone)]
struct PolicyDnsResolver {
    policy: CurlPolicy,
}

impl Resolve for PolicyDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let result = self
            .policy
            .resolve_host(&host, 0)
            .map(|addresses| Box::new(addresses.into_iter()) as Addrs)
            .map_err(|error| {
                Box::new(DnsPolicyError(error.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            });
        Box::pin(future::ready(result))
    }
}

#[derive(Debug)]
struct DnsPolicyError(String);

impl std::fmt::Display for DnsPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DnsPolicyError {}

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

const REQWEST_MULTIPART_BOUNDARY_LEN: u64 = 67;
const REQWEST_MULTIPART_PATH_SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%');

fn body_limit_error() -> CurlError {
    CurlError::new(E_BODY_LIMIT, "request body exceeds the configured limit")
}

fn ensure_body_limit(length: u64, max: u64) -> Result<(), CurlError> {
    if length > max {
        Err(body_limit_error())
    } else {
        Ok(())
    }
}

fn body_parts_len(parts: &[Vec<u8>]) -> Result<u64, CurlError> {
    let mut length = 0u64;
    for (index, part) in parts.iter().enumerate() {
        if index != 0 {
            length = length
                .checked_add(1)
                .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
        }
        length = length
            .checked_add(
                u64::try_from(part.len())
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
            )
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    }
    Ok(length)
}

fn encoded_length(value: &str, set: &'static percent_encoding::AsciiSet) -> Result<u64, CurlError> {
    percent_encode(value.as_bytes(), set).try_fold(0u64, |length, chunk| {
        length
            .checked_add(
                u64::try_from(chunk.len())
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
            )
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))
    })
}

fn form_encoded_length(value: &str) -> Result<u64, CurlError> {
    percent_encode(value.as_bytes(), NON_ALPHANUMERIC).try_fold(0u64, |length, chunk| {
        let chunk_length = if chunk == "%20" { 1 } else { chunk.len() };
        length
            .checked_add(
                u64::try_from(chunk_length)
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
            )
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))
    })
}

fn multipart_header_len(name: &str, file_name: Option<&str>) -> Result<u64, CurlError> {
    let encoded_name_len = encoded_length(name, REQWEST_MULTIPART_PATH_SEGMENT)?;
    let raw_name_len = u64::try_from(name.len())
        .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    let (name_prefix, name_suffix) = if encoded_name_len == raw_name_len {
        (b"name=\"".as_slice(), 1u64)
    } else {
        (b"name*=utf-8''".as_slice(), 0u64)
    };
    let mut length = u64::try_from(b"Content-Disposition: form-data; ".len())
        .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    length = length
        .checked_add(name_prefix.len() as u64)
        .and_then(|value| value.checked_add(encoded_name_len))
        .and_then(|value| value.checked_add(name_suffix))
        .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    if let Some(file_name) = file_name {
        let file_name_len = file_name.bytes().try_fold(0u64, |length, byte| {
            let byte_len = if matches!(byte, b'\\' | b'"' | b'\r' | b'\n') {
                2
            } else {
                1
            };
            length
                .checked_add(byte_len)
                .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))
        })?;
        length = length
            .checked_add(b"; filename=\"".len() as u64)
            .and_then(|value| value.checked_add(file_name_len))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    }
    Ok(length)
}

fn multipart_part_len(
    boundary_len: u64,
    name: &str,
    value_len: u64,
    file_name: Option<&str>,
) -> Result<u64, CurlError> {
    let header_len = multipart_header_len(name, file_name)?;
    2u64.checked_add(boundary_len)
        .and_then(|length| length.checked_add(2))
        .and_then(|length| length.checked_add(header_len))
        .and_then(|length| length.checked_add(4))
        .and_then(|length| length.checked_add(value_len))
        .and_then(|length| length.checked_add(2))
        .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))
}

fn multipart_wire_len(parts: &[MultipartPart], boundary_len: u64) -> Result<u64, CurlError> {
    let mut length = 0u64;
    for part in parts {
        length = length
            .checked_add(multipart_part_len(
                boundary_len,
                &part.name,
                u64::try_from(part.value.len())
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
                part.file_name.as_deref(),
            )?)
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    }
    if !parts.is_empty() {
        length = length
            .checked_add(2)
            .and_then(|length| length.checked_add(boundary_len))
            .and_then(|length| length.checked_add(4))
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    }
    Ok(length)
}

fn multipart_wire_len_with_part(
    parts: &[MultipartPart],
    boundary_len: u64,
    name: &str,
    value_len: u64,
    file_name: Option<&str>,
) -> Result<u64, CurlError> {
    let mut length = multipart_wire_len_without_closing_boundary(parts, boundary_len)?;
    length = length
        .checked_add(multipart_part_len(
            boundary_len,
            name,
            value_len,
            file_name,
        )?)
        .and_then(|length| length.checked_add(2))
        .and_then(|length| length.checked_add(boundary_len))
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
    Ok(length)
}

fn multipart_wire_len_without_closing_boundary(
    parts: &[MultipartPart],
    boundary_len: u64,
) -> Result<u64, CurlError> {
    parts.iter().try_fold(0u64, |length, part| {
        length
            .checked_add(multipart_part_len(
                boundary_len,
                &part.name,
                u64::try_from(part.value.len())
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
                part.file_name.as_deref(),
            )?)
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))
    })
}

fn validate_request_body(body: Option<&RequestBody>, max: u64) -> Result<(), CurlError> {
    match body {
        Some(RequestBody::Bytes(bytes)) => {
            ensure_body_limit(u64::try_from(bytes.len()).unwrap_or(u64::MAX), max)
        }
        Some(RequestBody::Multipart(parts)) => ensure_body_limit(
            multipart_wire_len(parts, REQWEST_MULTIPART_BOUNDARY_LEN)?,
            max,
        ),
        None => Ok(()),
    }
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

fn join_body_parts(parts: &[Vec<u8>], max: u64) -> Result<Option<Vec<u8>>, CurlError> {
    if parts.is_empty() {
        return Ok(None);
    }
    let length = body_parts_len(parts)?;
    ensure_body_limit(length, max)?;
    let capacity = usize::try_from(length)
        .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body cannot fit in memory"))?;
    let mut joined = Vec::new();
    joined.try_reserve_exact(capacity).map_err(|_| {
        CurlError::new(
            E_BODY_LIMIT,
            "request body allocation exceeds the configured limit",
        )
    })?;
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            joined.extend_from_slice(b"&");
        }
        joined.extend_from_slice(part);
    }
    Ok(Some(joined))
}

fn native_response_with_body(
    plan: &CurlPlan,
    policy: &CurlPolicy,
    transfer: mdok_curl_sys::NativeTransfer,
    metadata: mdok_curl_sys::NativeTransferMetadata,
    body_override: Option<BodyStorage>,
) -> Result<TransferResponse, CurlError> {
    if transfer.headers.len() > policy.max_header_bytes {
        return Err(CurlError::new(
            E_BODY_LIMIT,
            "response headers exceed the configured limit",
        ));
    }
    let mut status = metadata.response_code;
    let mut http_version = metadata.http_version.clone();
    let mut headers = BTreeMap::<String, Vec<String>>::new();
    for line in String::from_utf8_lossy(&transfer.headers).lines() {
        let line = line.trim_end_matches('\r');
        if let Some(version_and_status) = line.strip_prefix("HTTP/") {
            let mut parts = version_and_status.split_whitespace();
            let version = parts.next().map(str::to_owned);
            let code = parts.next().and_then(|value| value.parse::<u16>().ok());
            if http_version.is_none() {
                http_version = version;
            }
            if status.is_none() {
                status = code;
            }
        } else if let Some((name, value)) = line.split_once(':') {
            if name.is_empty() {
                continue;
            }
            headers
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(value.trim().to_owned());
        }
    }
    let Some(status) = status else {
        return Err(CurlError::new(
            E_TRANSFER,
            "native curl response did not contain a status line",
        ));
    };
    let body = if let Some(body) = body_override {
        body
    } else {
        body_storage_from_bytes(
            transfer.body,
            policy.memory_body_threshold_bytes,
            policy.max_body_bytes,
        )?
    };
    let body_len = body.len();
    let redirects = native_redirects(
        &transfer.headers,
        &plan.url,
        metadata.redirect_count.unwrap_or_default(),
    );
    let downloaded_bytes = metadata.downloaded_bytes.unwrap_or(body_len);
    let response_header_bytes = metadata
        .response_header_bytes
        .unwrap_or(transfer.headers.len() as u64);
    let uploaded_bytes = metadata
        .uploaded_bytes
        .or_else(|| plan.body.as_ref().map(request_body_len))
        .unwrap_or_default();
    let total_ms = metadata_ms(metadata.total_time_us);
    let dns_ms = metadata_ms(metadata.name_lookup_time_us);
    let connect_ms = metadata_ms(metadata.connect_time_us);
    let tls_ms = match (metadata.connect_time_us, metadata.appconnect_time_us) {
        (Some(connect), Some(appconnect)) if appconnect >= connect => {
            metadata_ms(Some(appconnect - connect))
        }
        _ => 0.0,
    };
    let ttfb_ms = metadata_ms(metadata.starttransfer_time_us);
    let redirect_ms = metadata_ms(metadata.redirect_time_us);
    let verify_result = metadata.ssl_verify_result.unwrap_or_default();
    Ok(TransferResponse {
        status: Some(status),
        method: plan.method.clone(),
        url: plan.url.to_string(),
        effective_url: metadata
            .effective_url
            .unwrap_or_else(|| plan.url.to_string()),
        http_version,
        cookies: cookies_from_headers(&headers),
        headers,
        body,
        redirects,
        timings: Timings {
            queue_ms: metadata_ms(metadata.pretransfer_time_us),
            dns_ms,
            connect_ms,
            tls_ms,
            ttfb_ms,
            total_ms,
            redirect_ms,
        },
        transfer: TransferMetrics {
            uploaded_bytes,
            downloaded_bytes,
            request_header_bytes: metadata.request_header_bytes.unwrap_or_default(),
            response_header_bytes,
            primary_ip: metadata.primary_ip,
            primary_port: metadata.primary_port,
            local_ip: metadata.local_ip,
            local_port: metadata.local_port,
            redirect_count: metadata.redirect_count.unwrap_or_default(),
            used_proxy: metadata.used_proxy,
        },
        tls: Some(TlsInfo {
            verified: !plan.insecure && verify_result == 0,
            verify_result,
        }),
        error: None,
    })
}

fn metadata_ms(value: Option<u64>) -> f64 {
    value
        .map(|micros| micros as f64 / 1000.0)
        .unwrap_or_default()
}

fn native_redirects(headers: &[u8], start: &Url, expected: usize) -> Vec<RedirectHop> {
    if expected == 0 {
        return Vec::new();
    }
    let mut source = start.clone();
    let mut status = None;
    let mut location = None;
    let mut redirects = Vec::new();
    for line in String::from_utf8_lossy(headers).split("\r\n") {
        if let Some(version_and_status) = line.strip_prefix("HTTP/") {
            push_native_redirect(&mut redirects, &mut source, status.take(), location.take());
            status = version_and_status
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<u16>().ok());
        } else if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("location")
        {
            location = Some(value.trim().to_owned());
        } else if line.is_empty() {
            push_native_redirect(&mut redirects, &mut source, status.take(), location.take());
        }
    }
    push_native_redirect(&mut redirects, &mut source, status.take(), location.take());
    redirects.truncate(expected);
    redirects
}

fn push_native_redirect(
    redirects: &mut Vec<RedirectHop>,
    source: &mut Url,
    status: Option<u16>,
    location: Option<String>,
) {
    let Some(status) = status.filter(|value| (300..400).contains(value)) else {
        return;
    };
    let Some(location) = location else {
        return;
    };
    let Ok(target) = source.join(&location) else {
        return;
    };
    redirects.push(RedirectHop {
        status,
        url: source.to_string(),
        location: Some(target.to_string()),
    });
    *source = target;
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
    native_argv: Vec<String>,
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

/// Per-document native execution state. The opaque bridge session owns one
/// libcurl multi handle, easy handle, cookie share, and connection cache. It
/// is intentionally not global: dropping it ends the document's cookie
/// lifetime and prevents state from crossing documents.
#[must_use]
pub struct ExecutionSession {
    native: Option<mdok_curl_sys::Session>,
}

impl ExecutionSession {
    pub fn new() -> Self {
        Self { native: None }
    }

    fn native_mut(&mut self) -> Result<&mut mdok_curl_sys::Session, CurlError> {
        if self.native.is_none() {
            self.native = Some(mdok_curl_sys::Session::new().map_err(|error| {
                CurlError::new(
                    E_TRANSFER,
                    format!("native curl session failed: {}", error.message),
                )
            })?);
        }
        Ok(self.native.as_mut().expect("native session initialized"))
    }
}

impl Default for ExecutionSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CurlPlan {
    pub fn parse(argv: &[String], policy: &CurlPolicy) -> Result<Self, CurlError> {
        if argv.first().map(String::as_str) != Some("curl") {
            return Err(CurlError::new(
                E_UNKNOWN_OPTION,
                "the command must begin with curl",
            ));
        }
        let mut p = ParserState::new(policy);
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
                "--silent" | "-s" | "--show-error" | "-S" | "--no-buffer" | "-N" => {
                    p.no_buffer |= option == "--no-buffer";
                    Ok(())
                }
                "-q" | "--disable" | "--no-disable" => Ok(()),
                "-G" | "--get" => {
                    p.get = true;
                    Ok(())
                }
                "--basic" | "--no-basic" => Ok(()),
                "--no-compressed" => {
                    p.compressed = false;
                    Ok(())
                }
                "--no-get" => {
                    p.get = false;
                    Ok(())
                }
                "--no-location" => {
                    p.follow_redirects = false;
                    Ok(())
                }
                "--no-show-error" | "--no-silent" => Ok(()),
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
                "--http1.0" => Err(CurlError::new(
                    E_UNSUPPORTED,
                    "option `--http1.0` is not supported in an MDOK transfer",
                )
                .at(i)),
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
                    let resolved = parse_resolve(&value(&mut i)?)?;
                    p.policy.check_socket_address(resolved.2)?;
                    p.resolve.push(resolved);
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
        let mut plan = p.finish()?;
        plan.native_argv = argv.to_owned();
        Ok(plan)
    }

    pub fn execute(&self, policy: &CurlPolicy) -> Result<TransferResponse, CurlError> {
        let mut session = ExecutionSession::new();
        self.execute_in_session_with_cancel(policy, &mut session, None)
    }

    /// Execute a plan using a caller-owned per-document session. The existing
    /// `execute` API remains a one-plan convenience wrapper.
    pub fn execute_in_session(
        &self,
        policy: &CurlPolicy,
        session: &mut ExecutionSession,
    ) -> Result<TransferResponse, CurlError> {
        self.execute_in_session_with_cancel(policy, session, None)
    }

    /// Execute with a synchronous cancellation check. Native transfers poll
    /// this callback from libcurl; fallback transfers check it before and
    /// between blocking attempts.
    pub fn execute_in_session_with_cancel(
        &self,
        policy: &CurlPolicy,
        session: &mut ExecutionSession,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<TransferResponse, CurlError> {
        if cancelled.is_some_and(|callback| callback()) {
            return Err(CurlError::new(E_CANCELLED, "transfer cancelled"));
        }
        validate_request_body(self.body.as_ref(), policy.max_body_bytes)?;
        policy.check_url(&self.url)?;
        policy.check_resolved_url(&self.url)?;
        if self.native_eligible(policy) {
            return self.execute_native(policy, session, cancelled);
        }
        let redirects = Arc::new(Mutex::new(Vec::<RedirectHop>::new()));
        let redirect_log = Arc::clone(&redirects);
        let max_redirs = self.max_redirs;
        let redirect_policy = if self.follow_redirects {
            let redirect_policy = policy.clone();
            reqwest::redirect::Policy::custom(move |attempt| {
                if let Err(error) = redirect_policy.check_url(attempt.url()) {
                    return attempt.error(RedirectPolicyError(error.to_string()));
                }
                if let Err(error) = redirect_policy.check_resolved_url(attempt.url()) {
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
        builder = builder.dns_resolver(Arc::new(PolicyDnsResolver {
            policy: policy.clone(),
        }));
        for (host, port, addr) in &self.resolve {
            builder = builder.resolve(host, *addr);
            let _ = port;
        }
        for mapping in &self.connect_to {
            let addresses = (mapping.target_host.as_str(), mapping.target_port)
                .to_socket_addrs()
                .map_err(|e| {
                    CurlError::new(E_POLICY, format!("connect-to target cannot resolve: {e}"))
                })?
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return Err(CurlError::new(
                    E_CONNECT_POLICY,
                    "connect-to target has no address",
                ));
            }
            for addr in &addresses {
                policy.check_socket_address(*addr)?;
            }
            let addr = addresses[0];
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
            if cancelled.is_some_and(|callback| callback()) {
                return Err(CurlError::new(E_CANCELLED, "transfer cancelled"));
            }
            if let Ok(mut hops) = redirects.lock() {
                hops.clear();
            }
            match self.execute_once(&client, policy, started, &redirects, cancelled) {
                Ok(response)
                    if attempt >= self.retries || !is_retryable_status(response.status) =>
                {
                    if cancelled.is_some_and(|callback| callback()) {
                        return Err(CurlError::new(E_CANCELLED, "transfer cancelled"));
                    }
                    return Ok(response);
                }
                Ok(_) | Err(_)
                    if attempt < self.retries
                        && retry_deadline.map(|d| Instant::now() < d).unwrap_or(true) =>
                {
                    attempt += 1;
                    if !self.retry_delay.is_zero() {
                        let deadline = Instant::now() + self.retry_delay;
                        while Instant::now() < deadline {
                            if cancelled.is_some_and(|callback| callback()) {
                                return Err(CurlError::new(E_CANCELLED, "transfer cancelled"));
                            }
                            std::thread::sleep(
                                Duration::from_millis(10)
                                    .min(deadline.saturating_duration_since(Instant::now())),
                            );
                        }
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
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<TransferResponse, CurlError> {
        validate_request_body(self.body.as_ref(), policy.max_body_bytes)?;
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
                ensure_body_limit(
                    multipart_wire_len(parts, form.boundary().len() as u64)?,
                    policy.max_body_bytes,
                )?;
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
            cancelled,
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

    fn native_eligible(&self, policy: &CurlPolicy) -> bool {
        self.native_argv.len() >= 2
            && self.native_argv.first().map(String::as_str) == Some("curl")
            && !self.native_argv.iter().skip(1).any(|argument| {
                matches!(
                    argument.as_str(),
                    "--get"
                        | "-G"
                        | "--form"
                        | "-F"
                        | "--upload-file"
                        | "-T"
                        | "--user"
                        | "-u"
                        | "--proxy"
                        | "-x"
                        | "--resolve"
                        | "--connect-to"
                        | "--cacert"
                        | "--cert"
                        | "--key"
                ) || argument.starts_with("--form=")
                    || argument.starts_with("--upload-file=")
                    || argument.starts_with("--user=")
                    || argument.starts_with("--proxy=")
                    || argument.starts_with("--resolve=")
                    || argument.starts_with("--connect-to=")
                    || argument.starts_with("--cacert=")
                    || argument.starts_with("--cert=")
                    || argument.starts_with("--key=")
                    || argument.starts_with("-T")
                    || argument.starts_with("-u")
                    || argument.starts_with("-x")
            })
            && !self.native_argv_has_file_body()
            && !self.native_argv_has_empty_header()
            && !matches!(self.body, Some(RequestBody::Multipart(_)))
            && !self.compressed
            && !self.follow_redirects
            && (policy.allow_private_network
                || self
                    .url
                    .host_str()
                    .is_some_and(|host| host.parse::<std::net::IpAddr>().is_ok()))
            && native_timeout_supported(self.timeout)
            && native_timeout_supported(self.connect_timeout)
            && self.user.is_none()
            && self.bearer.is_none()
            && self.cookie.is_none()
            && self.cookie_jar.is_none()
            && self.http_version.is_none()
            && self.cacert.is_none()
            && self.client_identity.is_none()
            && self.proxy.is_none()
            && self.resolve.is_empty()
            && self.connect_to.is_empty()
            && self.retries == 0
    }

    fn native_argv_has_file_body(&self) -> bool {
        self.native_argv
            .iter()
            .enumerate()
            .any(|(index, argument)| {
                let (option, attached) = argument
                    .split_once('=')
                    .map_or((argument.as_str(), None), |(option, value)| {
                        (option, Some(value))
                    });
                let body_option = matches!(
                    option,
                    "--data" | "-d" | "--data-binary" | "--data-urlencode" | "--json"
                ) || (option == "-d" && argument.len() > 2);
                if !body_option {
                    return false;
                }
                attached
                    .or_else(|| self.native_argv.get(index + 1).map(String::as_str))
                    .is_some_and(|value| value.starts_with('@'))
            })
    }

    fn native_argv_has_empty_header(&self) -> bool {
        self.native_argv
            .iter()
            .enumerate()
            .any(|(index, argument)| {
                let value = if argument == "--header" || argument == "-H" {
                    self.native_argv.get(index + 1).map(String::as_str)
                } else if let Some(value) = argument.strip_prefix("--header=") {
                    Some(value)
                } else if argument.starts_with("-H") && argument.len() > 2 {
                    Some(&argument[2..])
                } else {
                    None
                };
                value.is_some_and(|value| value.ends_with(':'))
            })
    }

    fn execute_native(
        &self,
        policy: &CurlPolicy,
        execution: &mut ExecutionSession,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<TransferResponse, CurlError> {
        validate_request_body(self.body.as_ref(), policy.max_body_bytes)?;
        let arguments = self
            .native_argv
            .iter()
            .map(String::as_bytes)
            .collect::<Vec<_>>();
        let parsed = mdok_curl_sys::Plan::parse(&arguments).map_err(|error| {
            CurlError::new(
                E_TRANSFER,
                format!("native curl parser failed: {}", error.message),
            )
        })?;
        let max_body_bytes = usize::try_from(policy.max_body_bytes).unwrap_or(usize::MAX);
        let mut body_sink =
            BodySink::new(policy.memory_body_threshold_bytes, policy.max_body_bytes);
        let result = execution
            .native_mut()?
            .execute_detailed_with_body_sink_and_timeouts(
                &parsed,
                max_body_bytes,
                policy.max_header_bytes,
                native_timeout_ms(self.timeout),
                native_timeout_ms(self.connect_timeout),
                cancelled,
                &mut |chunk| {
                    body_sink.push(chunk).map_err(|error| {
                        let code = if error.code == E_BODY_LIMIT {
                            mdok_curl_sys::BODY_LIMIT_ERROR_CODE
                        } else {
                            mdok_curl_sys::BODY_SINK_ERROR_CODE
                        };
                        (code, error.message)
                    })
                },
            )
            .map_err(|error| {
                if error.status == mdok_curl_sys::CANCELLED_STATUS {
                    CurlError::new(E_CANCELLED, error.message)
                } else if error.code == mdok_curl_sys::TIMEOUT_ERROR_CODE {
                    CurlError::new(E_TIMEOUT, error.message)
                } else if error.code == mdok_curl_sys::TOO_MANY_REDIRECTS_ERROR_CODE {
                    CurlError::new(E_REDIRECT, error.message)
                } else if matches!(
                    error.code,
                    mdok_curl_sys::BODY_LIMIT_ERROR_CODE | mdok_curl_sys::HEADER_LIMIT_ERROR_CODE
                ) {
                    CurlError::new(E_BODY_LIMIT, error.message)
                } else if error.code == mdok_curl_sys::BODY_SINK_ERROR_CODE {
                    CurlError::new(
                        E_TRANSFER,
                        format!("native body sink failed: {}", error.message),
                    )
                } else {
                    CurlError::new(
                        E_TRANSFER,
                        format!("native curl transfer failed: {}", error.message),
                    )
                }
            })?;
        let mdok_curl_sys::NativeTransferResult { transfer, metadata } = result;
        native_response_with_body(self, policy, transfer, metadata, Some(body_sink.finish()?))
    }
}

#[derive(Clone, Debug)]
struct ParserState<'a> {
    policy: &'a CurlPolicy,
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

impl<'a> ParserState<'a> {
    fn new(policy: &'a CurlPolicy) -> Self {
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
        let bytes = if (!raw || binary) && value.starts_with('@') {
            let path = &value[1..];
            let capacity = self.body_part_capacity()?;
            let length = self.policy.file_len(path)?;
            if length > capacity {
                return Err(body_limit_error());
            }
            self.policy.read_file_limited(path, capacity)?
        } else {
            self.ensure_body_part_capacity(
                u64::try_from(value.len())
                    .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?,
            )?;
            value.into_bytes()
        };
        self.body_parts.push(bytes);
        self.method = "POST".into();
        Ok(())
    }
    fn data_urlencode(&mut self, value: String) -> Result<(), CurlError> {
        let encoded_len = if let Some((key, val)) = value.split_once('=') {
            form_encoded_length(key)?
                .checked_add(1)
                .and_then(|length| form_encoded_length(val).ok()?.checked_add(length))
                .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?
        } else {
            form_encoded_length(&value)?
        };
        self.ensure_body_part_capacity(encoded_len)?;
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
        let (file, filename) = if let Some(file) = value.strip_prefix('@') {
            (
                Some(file),
                Some(
                    Path::new(file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload"),
                ),
            )
        } else {
            (None, None)
        };
        let existing = match &self.body {
            Some(RequestBody::Multipart(parts)) => parts.as_slice(),
            _ => &[],
        };
        let value_len = if let Some(file) = file {
            self.policy.file_len(file)?
        } else {
            u64::try_from(value.len())
                .map_err(|_| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?
        };
        let fixed_len = multipart_wire_len_with_part(
            existing,
            REQWEST_MULTIPART_BOUNDARY_LEN,
            name,
            0,
            filename,
        )?;
        ensure_body_limit(fixed_len, self.policy.max_body_bytes)?;
        let value_capacity = self.policy.max_body_bytes - fixed_len;
        if value_len > value_capacity {
            return Err(body_limit_error());
        }
        let bytes = if let Some(file) = file {
            self.policy.read_file_limited(file, value_capacity)?
        } else {
            value.as_bytes().to_vec()
        };
        let filename = filename.map(str::to_owned);
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
        let path = if let Some(path) = value.strip_prefix('@') {
            path
        } else {
            &value
        };
        let length = self.policy.file_len(path)?;
        ensure_body_limit(length, self.policy.max_body_bytes)?;
        let bytes = self
            .policy
            .read_file_limited(path, self.policy.max_body_bytes)?;
        self.body = Some(RequestBody::Bytes(bytes));
        self.method = "PUT".into();
        Ok(())
    }
    fn body_part_capacity(&self) -> Result<u64, CurlError> {
        let current = body_parts_len(&self.body_parts)?;
        let separator = u64::from(!self.body_parts.is_empty());
        let used = current
            .checked_add(separator)
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "request body length overflow"))?;
        if used > self.policy.max_body_bytes {
            return Err(body_limit_error());
        }
        Ok(self.policy.max_body_bytes - used)
    }
    fn ensure_body_part_capacity(&self, part_len: u64) -> Result<(), CurlError> {
        if part_len > self.body_part_capacity()? {
            Err(body_limit_error())
        } else {
            Ok(())
        }
    }
    fn finish(mut self) -> Result<CurlPlan, CurlError> {
        let mut url = Url::parse(&self.urls.remove(0))
            .map_err(|e| CurlError::new(E_POLICY, format!("invalid URL: {e}")))?;
        self.policy.check_url(&url)?;
        for (_, _, address) in &self.resolve {
            self.policy.check_socket_address(*address)?;
        }
        let joined = join_body_parts(&self.body_parts, self.policy.max_body_bytes)?;
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
        validate_request_body(self.body.as_ref(), self.policy.max_body_bytes)?;
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
            native_argv: Vec::new(),
        })
    }
}

#[derive(Debug)]
pub struct BodyStorage {
    memory: Option<Vec<u8>>,
    spool: Option<TempPath>,
    len: u64,
    truncated: bool,
    metadata: BodyMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BodyMetadata {
    pub kind: String,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub storage: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyHash {
    Sha256(String),
}

impl Serialize for BodyHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Sha256(value) => serializer.serialize_str(value),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BodyArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    #[serde(rename = "sha256")]
    pub hash: BodyHash,
}

impl BodyArtifact {
    pub fn sha256(&self) -> &str {
        match &self.hash {
            BodyHash::Sha256(value) => value,
        }
    }
}

impl BodyStorage {
    pub fn len(&self) -> u64 {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn is_spooled(&self) -> bool {
        self.spool.is_some()
    }
    pub fn storage_kind(&self) -> &'static str {
        if self.is_spooled() { "file" } else { "memory" }
    }
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }
    /// Return bounded body metadata computed while the body was captured.
    ///
    /// Spool-backed bodies are scanned through a fixed-size reader during
    /// capture; this accessor never reopens or loads the spool.
    pub fn metadata(&self) -> BodyMetadata {
        self.metadata.clone()
    }
    /// Return body metadata only when the captured body fits the caller's
    /// explicit bound.
    pub fn metadata_limited(&self, max_bytes: u64) -> Result<BodyMetadata, CurlError> {
        if self.len > max_bytes {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "body metadata exceeds the requested limit",
            ));
        }
        Ok(self.metadata())
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

    /// Stream this body into a new owner-only artifact file.
    ///
    /// The destination is created only after the complete body has been
    /// copied and hashed. Existing destinations are never overwritten. The
    /// effective limit is the smaller of `max_bytes` and the policy's body
    /// limit, and the copy uses a fixed-size buffer regardless of body size.
    pub fn promote_to_artifact(
        &self,
        destination: &Path,
        policy: &CurlPolicy,
        max_bytes: u64,
    ) -> Result<BodyArtifact, CurlError> {
        policy.check_artifact(destination)?;
        if destination.file_name().is_none() {
            return Err(CurlError::new(
                E_FILE_DENIED,
                "artifact destination must name a file",
            ));
        }

        let limit = max_bytes.min(policy.max_body_bytes);
        if self.len > limit {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "body artifact exceeds the configured size limit",
            ));
        }

        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
        match fs::symlink_metadata(destination) {
            Ok(_) => {
                return Err(CurlError::new(
                    E_FILE_DENIED,
                    "artifact destination already exists",
                ));
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(CurlError::new(E_FILE_DENIED, error.to_string()));
            }
            Err(_) => {}
        }

        let parent = policy.checked_artifact_parent(destination)?;
        let mut temporary = NamedTempFile::new_in(&parent)
            .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
        let mut source = BodyStorageReader::from_storage(self)?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; ARTIFACT_COPY_BUFFER_BYTES];
        let mut copied = 0u64;
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| CurlError::new(E_TRANSFER, error.to_string()))?;
            if count == 0 {
                break;
            }
            let next = copied
                .checked_add(count as u64)
                .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "body length overflow"))?;
            if next > limit {
                return Err(CurlError::new(
                    E_BODY_LIMIT,
                    "body artifact exceeds the configured size limit",
                ));
            }
            digest.update(&buffer[..count]);
            temporary
                .as_file_mut()
                .write_all(&buffer[..count])
                .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
            copied = next;
        }
        if copied != self.len {
            return Err(CurlError::new(
                E_TRANSFER,
                "body storage length changed during artifact promotion",
            ));
        }
        temporary
            .as_file_mut()
            .flush()
            .map_err(|error| CurlError::new(E_FILE_DENIED, error.to_string()))?;
        temporary
            .persist_noclobber(destination)
            .map_err(|error| CurlError::new(E_FILE_DENIED, error.error.to_string()))?;

        Ok(BodyArtifact {
            path: destination.to_path_buf(),
            bytes: copied,
            hash: BodyHash::Sha256(hex_digest(&digest.finalize())),
        })
    }
}

enum BodyStorageReader<'a> {
    Memory(&'a [u8]),
    Spool(fs::File),
    Empty,
}

impl<'a> BodyStorageReader<'a> {
    fn from_storage(storage: &'a BodyStorage) -> Result<Self, CurlError> {
        if let Some(bytes) = storage.memory.as_deref() {
            return Ok(Self::Memory(bytes));
        }
        if let Some(path) = &storage.spool {
            return fs::File::open(path)
                .map(Self::Spool)
                .map_err(|error| CurlError::new(E_TRANSFER, error.to_string()));
        }
        Ok(Self::Empty)
    }
}

impl Read for BodyStorageReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Memory(bytes) => bytes.read(buffer),
            Self::Spool(file) => file.read(buffer),
            Self::Empty => Ok(0),
        }
    }
}

struct MetadataReader<R> {
    inner: R,
    digest: Sha256,
    bytes: u64,
    utf8: Utf8Validator,
}

impl<R> MetadataReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            bytes: 0,
            utf8: Utf8Validator::default(),
        }
    }
}

impl<R: Read> Read for MetadataReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if count == 0 {
            return Ok(0);
        }
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("body length overflow"))?;
        self.digest.update(&buffer[..count]);
        self.utf8.push(&buffer[..count]);
        Ok(count)
    }
}

struct Utf8Validator {
    valid: bool,
    lead: u8,
    expected: u8,
    seen: u8,
}

impl Default for Utf8Validator {
    fn default() -> Self {
        Self {
            valid: true,
            lead: 0,
            expected: 0,
            seen: 0,
        }
    }
}

impl Utf8Validator {
    fn push(&mut self, bytes: &[u8]) {
        if !self.valid {
            return;
        }
        for &byte in bytes {
            self.push_byte(byte);
            if !self.valid {
                return;
            }
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.expected == 0 {
            match byte {
                0x00..=0x7f => {}
                0xc2..=0xdf => {
                    self.lead = byte;
                    self.expected = 2;
                    self.seen = 1;
                }
                0xe0..=0xef => {
                    self.lead = byte;
                    self.expected = 3;
                    self.seen = 1;
                }
                0xf0..=0xf4 => {
                    self.lead = byte;
                    self.expected = 4;
                    self.seen = 1;
                }
                _ => self.valid = false,
            }
            return;
        }

        let valid_continuation = (0x80..=0xbf).contains(&byte)
            && (self.seen != 1
                || match self.lead {
                    0xe0 => byte >= 0xa0,
                    0xed => byte <= 0x9f,
                    0xf0 => byte >= 0x90,
                    0xf4 => byte <= 0x8f,
                    _ => true,
                });
        if !valid_continuation {
            self.valid = false;
            return;
        }
        self.seen += 1;
        if self.seen == self.expected {
            self.lead = 0;
            self.expected = 0;
            self.seen = 0;
        }
    }

    fn is_valid(&self) -> bool {
        self.valid && self.expected == 0
    }
}

fn compute_body_metadata(storage: &BodyStorage) -> Result<BodyMetadata, CurlError> {
    // A temporary spool is an internal memory-saving representation, not a
    // durable artifact. The response still exposes the bytes inline unless
    // the caller explicitly promotes them to an artifact.
    let storage_name = if storage.len == 0 { "none" } else { "inline" };
    let mut source = BodyStorageReader::from_storage(storage)?;
    let mut reader = MetadataReader::new(&mut source);
    let json = if storage.len == 0 {
        false
    } else {
        let mut deserializer = serde_json::Deserializer::from_reader(&mut reader);
        let parsed = serde::de::IgnoredAny::deserialize(&mut deserializer).is_ok();
        parsed && deserializer.end().is_ok()
    };

    let mut buffer = [0u8; ARTIFACT_COPY_BUFFER_BYTES];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| CurlError::new(E_TRANSFER, error.to_string()))?;
        if count == 0 {
            break;
        }
    }
    if reader.bytes != storage.len {
        return Err(CurlError::new(
            E_TRANSFER,
            "body storage length changed while reading metadata",
        ));
    }

    let kind = if storage.truncated {
        "truncated"
    } else if storage.len == 0 {
        "empty"
    } else if json {
        "json"
    } else if reader.utf8.is_valid() {
        "text"
    } else {
        "binary"
    };
    let sha256 = if storage.truncated {
        None
    } else {
        Some(hex_digest(&reader.digest.finalize()))
    };
    Ok(BodyMetadata {
        kind: kind.to_owned(),
        bytes: storage.len,
        sha256,
        storage: storage_name.to_owned(),
        truncated: storage.truncated,
    })
}

fn body_storage_from_parts(
    memory: Option<Vec<u8>>,
    spool: Option<TempPath>,
    len: u64,
    truncated: bool,
) -> Result<BodyStorage, CurlError> {
    let mut storage = BodyStorage {
        memory,
        spool,
        len,
        truncated,
        metadata: BodyMetadata::default(),
    };
    storage.metadata = compute_body_metadata(&storage)?;
    Ok(storage)
}

fn body_storage_from_bytes(
    body: Vec<u8>,
    threshold: usize,
    max: u64,
) -> Result<BodyStorage, CurlError> {
    let len = u64::try_from(body.len())
        .map_err(|_| CurlError::new(E_BODY_LIMIT, "body length overflow"))?;
    if len > max {
        return Err(CurlError::new(
            E_BODY_LIMIT,
            "response body exceeds the configured limit",
        ));
    }
    if body.len() <= threshold {
        return body_storage_from_parts(Some(body), None, len, false);
    }
    let mut file =
        NamedTempFile::new().map_err(|error| CurlError::new(E_TRANSFER, error.to_string()))?;
    write_body_chunk(file.as_file_mut(), &body)?;
    body_storage_from_parts(None, Some(file.into_temp_path()), len, false)
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
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

type BodyValue = Result<(Value, &'static str, Option<String>, Option<String>), CurlError>;

impl TransferResponse {
    pub fn body_value(&self, max_json_bytes: usize) -> BodyValue {
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
        self.evaluation_json_limited_with_artifact(variables, steps, max_json_bytes, None)
    }

    pub fn evaluation_json_limited_with_artifact(
        &self,
        variables: &Value,
        steps: &Value,
        max_json_bytes: usize,
        artifact: Option<&BodyArtifact>,
    ) -> Result<Value, CurlError> {
        let (mut body, mut kind, mut body_text, body_base64) = if artifact.is_some() {
            let metadata = self.body.metadata();
            (Value::Null, metadata.kind, None, None)
        } else {
            let (body, kind, body_text, body_base64) = self.body_value(max_json_bytes)?;
            (body, kind.to_owned(), body_text, body_base64)
        };
        let mut body_metadata = self.body.metadata();
        if self.method.eq_ignore_ascii_case("HEAD") && kind == "empty" {
            body = json!({"method": self.method});
            kind = "json".to_owned();
            body_text = serde_json::to_string(&body).ok();
            body_metadata = BodyMetadata {
                kind: "json".to_owned(),
                bytes: body_text.as_ref().map_or(0, String::len) as u64,
                sha256: body_text.as_ref().map(|text| sha256_bytes(text.as_bytes())),
                storage: "inline".to_owned(),
                truncated: false,
            };
        }
        if artifact.is_some() {
            body_metadata.storage = "artifact".to_owned();
        }
        let body_metadata_value = json!({
            "kind": body_metadata.kind,
            "bytes": body_metadata.bytes,
            "sha256": body_metadata.sha256,
            "storage": body_metadata.storage,
            "truncated": body_metadata.truncated,
            "artifact": artifact
                .map(|artifact| json!({
                    "kind": "body",
                    "path": artifact.path,
                    "bytes": artifact.bytes,
                    "sha256": artifact.sha256(),
                    "complete": true,
                }))
                .unwrap_or(Value::Null),
        });
        Ok(
            json!({ "status": self.status, "method": self.method, "url": self.url, "effective_url": self.effective_url, "http_version": self.http_version, "headers": self.headers, "body": body, "body_text": body_text, "body_base64": body_base64, "body_metadata": body_metadata_value, "body_kind": kind, "cookies": self.cookies, "redirects": self.redirects, "timings": self.timings, "transfer": self.transfer, "tls": self.tls, "error": self.error, "variables": variables, "steps": steps }),
        )
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_digest(&digest)
}

fn capture_body<R: Read>(
    response: &mut R,
    threshold: usize,
    max: u64,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<BodyStorage, CurlError> {
    let mut sink = BodySink::new(threshold, max);
    let mut buf = [0u8; 16 * 1024];
    loop {
        if cancelled.is_some_and(|callback| callback()) {
            return Err(CurlError::new(E_CANCELLED, "transfer cancelled"));
        }
        let count = response.read(&mut buf).map_err(map_body_read_error)?;
        if count == 0 {
            break;
        }
        sink.push(&buf[..count])?;
    }
    sink.finish()
}

#[derive(Debug)]
struct BodySink {
    memory: Vec<u8>,
    spool: Option<NamedTempFile>,
    threshold: usize,
    max: u64,
    len: u64,
}

impl BodySink {
    fn new(threshold: usize, max: u64) -> Self {
        Self {
            memory: Vec::new(),
            spool: None,
            threshold,
            max,
            len: 0,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<(), CurlError> {
        let chunk_len = u64::try_from(chunk.len())
            .map_err(|_| CurlError::new(E_BODY_LIMIT, "body length overflow"))?;
        let new_len = self
            .len
            .checked_add(chunk_len)
            .ok_or_else(|| CurlError::new(E_BODY_LIMIT, "body length overflow"))?;
        if new_len > self.max {
            return Err(CurlError::new(
                E_BODY_LIMIT,
                "response body exceeds the configured limit",
            ));
        }

        if let Some(file) = &mut self.spool {
            write_body_chunk(file.as_file_mut(), chunk)?;
        } else if chunk.len() <= self.threshold.saturating_sub(self.memory.len()) {
            self.memory.extend_from_slice(chunk);
        } else {
            let mut file =
                NamedTempFile::new().map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))?;
            let buffered = std::mem::take(&mut self.memory);
            write_body_chunk(file.as_file_mut(), &buffered)?;
            write_body_chunk(file.as_file_mut(), chunk)?;
            self.spool = Some(file);
        }
        self.len = new_len;
        Ok(())
    }

    fn finish(self) -> Result<BodyStorage, CurlError> {
        body_storage_from_parts(
            (self.spool.is_none()).then_some(self.memory),
            self.spool.map(NamedTempFile::into_temp_path),
            self.len,
            false,
        )
    }
}

fn write_body_chunk<W: Write>(writer: &mut W, chunk: &[u8]) -> Result<(), CurlError> {
    writer
        .write_all(chunk)
        .map_err(|e| CurlError::new(E_TRANSFER, e.to_string()))
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
    let parent = policy.checked_artifact_parent(path)?;
    let temp =
        NamedTempFile::new_in(&parent).map_err(|e| CurlError::new(E_FILE_DENIED, e.to_string()))?;
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
    Duration::try_from_secs_f64(seconds)
        .map_err(|_| CurlError::new(E_POLICY, format!("invalid {name}: {value}")))
}
fn native_timeout_ms(timeout: Option<Duration>) -> Option<u64> {
    timeout.and_then(|duration| {
        let millis = duration.as_millis();
        let rounded = millis.checked_add(u128::from(
            !duration.subsec_nanos().is_multiple_of(1_000_000),
        ))?;
        u64::try_from(rounded).ok()
    })
}
fn native_timeout_supported(timeout: Option<Duration>) -> bool {
    timeout.map_or(true, |duration| {
        native_timeout_ms(Some(duration))
            .is_some_and(|millis| millis <= mdok_curl_sys::MAX_NATIVE_TIMEOUT_MS)
    })
}
fn form_encode(value: &str) -> String {
    percent_encode(value.as_bytes(), NON_ALPHANUMERIC)
        .to_string()
        .replace("%20", "+")
}

fn denied_address(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_loopback()
                || ip.is_private()
                || ip.is_unspecified()
                || ip.is_link_local()
                || ip.is_broadcast()
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                || address == std::net::IpAddr::V4(std::net::Ipv4Addr::new(169, 254, 169, 254))
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    pattern == "*"
        || pattern == host
        || pattern
            .strip_prefix("*.")
            .is_some_and(|suffix| host.ends_with(&format!(".{suffix}")) || host == suffix)
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
    if lower.contains("mdok-e604") {
        return E_CONNECT_POLICY;
    }
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

    struct ChunkReader {
        bytes: Vec<u8>,
        offset: usize,
        chunk_size: usize,
    }

    impl ChunkReader {
        fn new(bytes: &[u8], chunk_size: usize) -> Self {
            assert!(chunk_size > 0);
            Self {
                bytes: bytes.to_vec(),
                offset: 0,
                chunk_size,
            }
        }
    }

    impl Read for ChunkReader {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if self.offset == self.bytes.len() {
                return Ok(0);
            }
            let count = self
                .chunk_size
                .min(output.len())
                .min(self.bytes.len() - self.offset);
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    struct PartialWriter {
        bytes: Vec<u8>,
        max_write: usize,
    }

    impl Write for PartialWriter {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            let count = self.max_write.min(input.len());
            self.bytes.extend_from_slice(&input[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn parse(args: &[&str]) -> Result<CurlPlan, CurlError> {
        CurlPlan::parse(
            &args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            &CurlPolicy::default(),
        )
    }

    fn artifact_policy(root: &Path) -> CurlPolicy {
        CurlPolicy {
            allow_artifact_writes: true,
            allowed_artifact_roots: vec![root.to_path_buf()],
            ..CurlPolicy::default()
        }
    }

    #[test]
    fn body_sink_keeps_exact_threshold_in_memory_and_spills_after_it() {
        let payload = b"0123456789";
        let mut exact_reader = ChunkReader::new(payload, 3);
        let exact = capture_body(&mut exact_reader, payload.len(), 100, None).unwrap();
        assert_eq!(exact.len(), payload.len() as u64);
        assert!(!exact.is_spooled());
        assert_eq!(exact.storage_kind(), "memory");
        assert_eq!(exact.bytes(100).unwrap(), payload);

        let mut spill_reader = ChunkReader::new(payload, 3);
        let spill = capture_body(&mut spill_reader, payload.len() - 1, 100, None).unwrap();
        assert_eq!(spill.len(), payload.len() as u64);
        assert!(spill.is_spooled());
        assert_eq!(spill.storage_kind(), "file");
        assert_eq!(spill.bytes(100).unwrap(), payload);
    }

    #[test]
    fn body_sink_handles_empty_and_zero_threshold_bodies() {
        let mut empty_reader = ChunkReader::new(&[], 1);
        let empty = capture_body(&mut empty_reader, 0, 0, None).unwrap();
        assert!(empty.is_empty());
        assert!(!empty.is_spooled());

        let mut reader = ChunkReader::new(b"x", 1);
        let one_byte_spill = capture_body(&mut reader, 0, 1, None).unwrap();
        assert!(one_byte_spill.is_spooled());
        assert_eq!(one_byte_spill.bytes(1).unwrap(), b"x");
    }

    #[test]
    fn body_sink_rejects_max_overflow_before_writing_and_cleans_spool() {
        let mut sink = BodySink::new(2, 3);
        sink.push(b"abc").unwrap();
        let path = sink.spool.as_ref().unwrap().path().to_path_buf();
        assert!(path.exists());
        let error = sink.push(b"d").unwrap_err();
        assert_eq!(error.code, E_BODY_LIMIT);
        drop(sink);
        assert!(!path.exists());
    }

    #[test]
    fn body_sink_rejects_length_counter_overflow() {
        let mut sink = BodySink::new(8, u64::MAX);
        sink.len = u64::MAX - 1;
        let error = sink.push(b"xx").unwrap_err();
        assert_eq!(error.code, E_BODY_LIMIT);
        assert_eq!(error.message, "body length overflow");
    }

    #[test]
    fn body_sink_retries_partial_writes_until_the_chunk_is_complete() {
        let mut writer = PartialWriter {
            bytes: Vec::new(),
            max_write: 2,
        };
        write_body_chunk(&mut writer, b"partial writes").unwrap();
        assert_eq!(writer.bytes, b"partial writes");
    }

    #[cfg(unix)]
    #[test]
    fn body_sink_spool_has_private_temp_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let mut sink = BodySink::new(0, 1);
        sink.push(b"x").unwrap();
        let path = sink.spool.as_ref().unwrap().path();
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn body_storage_promotes_memory_body_with_hash_and_private_permissions() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let mut reader = ChunkReader::new(b"hello world", 2);
        let body = capture_body(&mut reader, 1024, 1024, None).unwrap();
        assert!(!body.is_spooled());

        let destination = root.path().join("memory.bin");
        let artifact = body
            .promote_to_artifact(&destination, &policy, 1024)
            .unwrap();
        assert_eq!(artifact.path, destination);
        assert_eq!(artifact.bytes, 11);
        assert_eq!(fs::read(&destination).unwrap(), b"hello world");
        assert_eq!(
            artifact.hash,
            BodyHash::Sha256(
                "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".to_owned()
            )
        );
        let serialized = serde_json::to_value(&artifact).unwrap();
        assert_eq!(
            serialized["sha256"],
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert!(serialized.get("hash").is_none());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&destination).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn body_storage_promotes_spooled_body_with_fixed_memory_copy() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let payload = vec![b'x'; ARTIFACT_COPY_BUFFER_BYTES * 2 + 17];
        let mut reader = ChunkReader::new(&payload, 7);
        let body = capture_body(&mut reader, 0, payload.len() as u64, None).unwrap();
        assert!(body.is_spooled());

        let destination = root.path().join("spooled.bin");
        let artifact = body
            .promote_to_artifact(&destination, &policy, payload.len() as u64)
            .unwrap();
        assert_eq!(artifact.bytes, payload.len() as u64);
        assert_eq!(fs::read(&destination).unwrap(), payload);
        assert!(matches!(artifact.hash, BodyHash::Sha256(ref digest) if digest.len() == 64));
    }

    #[test]
    fn body_storage_rejects_artifact_size_limit_before_creating_destination() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let mut reader = ChunkReader::new(b"too large", 2);
        let body = capture_body(&mut reader, 1024, 1024, None).unwrap();
        let destination = root.path().join("limited.bin");

        let error = body
            .promote_to_artifact(&destination, &policy, 3)
            .unwrap_err();
        assert_eq!(error.code, E_BODY_LIMIT);
        assert!(!destination.exists());
    }

    #[test]
    fn body_storage_does_not_overwrite_existing_artifact() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let destination = root.path().join("existing.bin");
        fs::write(&destination, b"keep me").unwrap();
        let mut reader = ChunkReader::new(b"replace me", 2);
        let body = capture_body(&mut reader, 1024, 1024, None).unwrap();

        let error = body
            .promote_to_artifact(&destination, &policy, 1024)
            .unwrap_err();
        assert_eq!(error.code, E_FILE_DENIED);
        assert_eq!(fs::read(&destination).unwrap(), b"keep me");
    }

    #[test]
    fn body_storage_metadata_is_typed_bounded_and_cached_for_spools() {
        let mut json_reader = ChunkReader::new(br#"{"answer":42}"#, 1);
        let json_body = capture_body(&mut json_reader, 1024, 1024, None).unwrap();
        let json_metadata = json_body.metadata();
        assert_eq!(json_metadata.kind, "json");
        assert_eq!(json_metadata.bytes, 13);
        assert_eq!(json_metadata.storage, "inline");
        assert!(!json_metadata.truncated);
        let json_sha256 = hex_digest(&Sha256::digest(br#"{"answer":42}"#));
        assert_eq!(json_metadata.sha256.as_deref(), Some(json_sha256.as_str()));
        assert_eq!(json_body.metadata_limited(13).unwrap(), json_metadata);
        assert_eq!(
            json_body.metadata_limited(12).unwrap_err().code,
            E_BODY_LIMIT
        );

        let mut binary_reader = ChunkReader::new(&[0xff, 0x00, 0xfe], 1);
        let binary_body = capture_body(&mut binary_reader, 1024, 1024, None).unwrap();
        assert_eq!(binary_body.metadata().kind, "binary");

        let mut empty_reader = ChunkReader::new(&[], 1);
        let empty_body = capture_body(&mut empty_reader, 1024, 1024, None).unwrap();
        let empty_metadata = empty_body.metadata();
        assert_eq!(empty_metadata.kind, "empty");
        assert_eq!(empty_metadata.storage, "none");
        assert_eq!(
            empty_metadata.sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        let payload = vec![b'x'; ARTIFACT_COPY_BUFFER_BYTES + 1];
        let mut spool_reader = ChunkReader::new(&payload, 3);
        let spool_body = capture_body(&mut spool_reader, 0, payload.len() as u64, None).unwrap();
        assert_eq!(spool_body.metadata().kind, "text");
        assert_eq!(spool_body.metadata().storage, "inline");
        let cached_metadata = spool_body.metadata();
        let spool_path = spool_body.spool.as_ref().unwrap().to_path_buf();
        fs::remove_file(&spool_path).unwrap();
        assert_eq!(spool_body.metadata(), cached_metadata);
    }

    #[test]
    fn artifact_destination_rejects_unsafe_relative_components() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let mut reader = ChunkReader::new(b"body", 1);
        let body = capture_body(&mut reader, 1024, 1024, None).unwrap();
        let destination = root.path().join("new-parent").join("..").join("escape.bin");

        let error = body
            .promote_to_artifact(&destination, &policy, 1024)
            .unwrap_err();
        assert_eq!(error.code, E_FILE_DENIED);
        assert!(!root.path().join("new-parent").exists());
    }

    #[cfg(unix)]
    #[test]
    fn artifact_destination_rejects_symlink_parents() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let link = root.path().join("linked-parent");
        symlink(root.path(), &link).unwrap();
        let mut reader = ChunkReader::new(b"body", 1);
        let body = capture_body(&mut reader, 1024, 1024, None).unwrap();

        let error = body
            .promote_to_artifact(&link.join("artifact.bin"), &policy, 1024)
            .unwrap_err();
        assert_eq!(error.code, E_FILE_DENIED);
    }

    #[test]
    fn body_capture_checks_cancellation_between_reader_chunks() {
        let mut reader = ChunkReader::new(b"abcdef", 1);
        let checks = std::cell::Cell::new(0);
        let cancelled = || {
            let next = checks.get() + 1;
            checks.set(next);
            next >= 3
        };
        let error = capture_body(&mut reader, 10, 10, Some(&cancelled)).unwrap_err();
        assert_eq!(error.code, E_CANCELLED);
        assert!(reader.offset < reader.bytes.len());
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
            body: body_storage_from_parts(Some(Vec::new()), None, 0, false).unwrap(),
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
    fn artifact_evaluation_omits_inline_body_and_emits_complete_reference() {
        let root = tempfile::tempdir().unwrap();
        let policy = artifact_policy(root.path());
        let body = body_storage_from_parts(Some(b"download".to_vec()), None, 8, false).unwrap();
        let destination = root.path().join("download.bin");
        let artifact = body
            .promote_to_artifact(&destination, &policy, 1024)
            .unwrap();
        let reference = BodyArtifact {
            path: PathBuf::from(".mdok/artifacts/download.bin"),
            ..artifact
        };
        let response = TransferResponse {
            status: Some(200),
            method: "GET".into(),
            url: "https://example.test/download".into(),
            effective_url: "https://example.test/download".into(),
            http_version: Some("1.1".into()),
            headers: BTreeMap::new(),
            body,
            cookies: Vec::new(),
            redirects: Vec::new(),
            timings: Timings::default(),
            transfer: TransferMetrics::default(),
            tls: None,
            error: None,
        };
        let evaluation = response
            .evaluation_json_limited_with_artifact(
                &Value::Null,
                &Value::Null,
                1024,
                Some(&reference),
            )
            .unwrap();
        assert!(evaluation["body"].is_null());
        assert_eq!(evaluation["body_metadata"]["storage"], "artifact");
        assert_eq!(evaluation["body_metadata"]["artifact"]["complete"], true);
        assert_eq!(
            evaluation["body_metadata"]["artifact"]["path"],
            ".mdok/artifacts/download.bin"
        );
        assert_eq!(evaluation["body_kind"], "text");
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

    #[test]
    fn native_metadata_maps_redirects_and_transfer_details() {
        let plan = parse(&["curl", "https://example.test/start"]).unwrap();
        let headers = b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec();
        let transfer = mdok_curl_sys::NativeTransfer {
            body: b"ok".to_vec(),
            headers,
        };
        let metadata = mdok_curl_sys::NativeTransferMetadata {
            response_code: Some(200),
            http_version: Some("1.1".into()),
            effective_url: Some("https://example.test/final".into()),
            total_time_us: Some(12_000),
            name_lookup_time_us: Some(1_000),
            connect_time_us: Some(3_000),
            appconnect_time_us: Some(4_000),
            pretransfer_time_us: Some(5_000),
            starttransfer_time_us: Some(7_000),
            redirect_time_us: Some(2_000),
            uploaded_bytes: Some(9),
            downloaded_bytes: Some(2),
            request_header_bytes: Some(80),
            response_header_bytes: Some(123),
            redirect_count: Some(1),
            num_connects: Some(1),
            ssl_verify_result: Some(0),
            used_proxy: false,
            primary_ip: Some("192.0.2.10".into()),
            primary_port: Some(443),
            local_ip: Some("192.0.2.20".into()),
            local_port: Some(50_000),
        };
        let response =
            native_response_with_body(&plan, &CurlPolicy::default(), transfer, metadata, None)
                .unwrap();
        assert_eq!(response.status, Some(200));
        assert_eq!(response.effective_url, "https://example.test/final");
        assert_eq!(response.redirects.len(), 1);
        assert_eq!(response.transfer.uploaded_bytes, 9);
        assert_eq!(response.transfer.primary_port, Some(443));
        assert_eq!(response.transfer.local_port, Some(50_000));
        assert!(!response.transfer.used_proxy);
        assert_eq!(response.timings.total_ms, 12.0);
    }

    #[test]
    fn host_patterns_are_case_insensitive() {
        assert!(host_matches_pattern("API.EXAMPLE.COM", "*.Example.Com"));
        assert!(host_matches_pattern("Example.Com", "*.example.com"));
        assert!(!host_matches_pattern("api.example.net", "*.Example.Com"));
    }

    #[test]
    fn resolved_private_addresses_require_local_test_policy() {
        let policy = CurlPolicy::default();
        let error = policy
            .check_socket_address("127.0.0.1:80".parse().unwrap())
            .unwrap_err();
        assert_eq!(error.code, E_CONNECT_POLICY);
        assert!(
            CurlPolicy::local_test()
                .check_socket_address("127.0.0.1:80".parse().unwrap())
                .is_ok()
        );
        assert!(denied_address(std::net::IpAddr::V4(
            std::net::Ipv4Addr::new(169, 254, 169, 254)
        )));
    }

    #[test]
    fn parser_uses_caller_policy_for_gated_options() {
        let policy = CurlPolicy::default();
        let error = CurlPlan::parse(
            &[
                "curl".into(),
                "--insecure".into(),
                "https://example.test".into(),
            ],
            &policy,
        )
        .unwrap_err();
        assert_eq!(error.code, E_TLS);
    }

    #[test]
    fn native_fast_path_requires_safe_redirect_and_hostname_policy() {
        let plan = parse(&["curl", "https://example.test"]).unwrap();
        assert!(!plan.native_eligible(&CurlPolicy::default()));
        assert!(plan.native_eligible(&CurlPolicy::local_test()));

        let mut redirects = plan.clone();
        redirects.follow_redirects = true;
        assert!(!redirects.native_eligible(&CurlPolicy::local_test()));
    }

    #[test]
    fn oversized_durations_are_normal_parse_errors() {
        let result = std::panic::catch_unwind(|| parse_duration("1e300", "max-time"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap_err().code, E_POLICY);
        assert!(!native_timeout_supported(Some(Duration::from_secs(301))));
        assert!(native_timeout_supported(Some(Duration::from_secs(300))));
    }

    #[test]
    fn request_body_files_are_rejected_before_reading_over_limit() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"too large").unwrap();
        let parent = file.path().parent().unwrap().to_path_buf();
        let mut policy = CurlPolicy::local_test();
        policy.allow_file_reads = true;
        policy.allowed_read_roots = vec![parent];
        policy.max_body_bytes = 2;
        let error = CurlPlan::parse(
            &[
                "curl".into(),
                "--data-binary".into(),
                format!("@{}", file.path().display()),
                "https://example.test".into(),
            ],
            &policy,
        )
        .unwrap_err();
        assert_eq!(error.code, E_BODY_LIMIT);

        let error = CurlPlan::parse(
            &[
                "curl".into(),
                "--data".into(),
                "too large".into(),
                "https://example.test".into(),
            ],
            &policy,
        )
        .unwrap_err();
        assert_eq!(error.code, E_BODY_LIMIT);
    }

    #[test]
    fn multipart_body_preflight_accounts_for_overhead_without_rejecting_valid_values() {
        let arguments = vec![
            "curl".to_owned(),
            "--form".to_owned(),
            "field=value".to_owned(),
            "https://example.test".to_owned(),
        ];
        let unrestricted = CurlPolicy::default();
        let plan = CurlPlan::parse(&arguments, &unrestricted).unwrap();
        let Some(RequestBody::Multipart(parts)) = plan.body else {
            panic!("expected multipart body");
        };
        let required = multipart_wire_len(&parts, REQWEST_MULTIPART_BOUNDARY_LEN).unwrap();

        let mut limited = unrestricted.clone();
        limited.max_body_bytes = required - 1;
        assert_eq!(
            CurlPlan::parse(&arguments, &limited).unwrap_err().code,
            E_BODY_LIMIT
        );

        limited.max_body_bytes = required;
        assert!(CurlPlan::parse(&arguments, &limited).is_ok());
    }
}
