use mdok_curl_sys::{
    Plan, Session, mdok_curl_argv, mdok_curl_callbacks, mdok_curl_error, mdok_curl_execute,
    mdok_curl_global_init, mdok_curl_parse, mdok_curl_plan, mdok_curl_session_free,
    mdok_curl_session_new, mdok_curl_slice,
};
use std::{
    ffi::{c_int, c_void},
    fs,
    io::{Read, Write},
    net::TcpListener,
    ptr, slice,
    sync::Once,
    thread,
    time::Duration,
};

const MDOK_CURL_OK: c_int = 0;
const MDOK_CURL_PARSE_ERROR: c_int = 1;
const MDOK_CURL_TRANSFER_ERROR: c_int = 3;
const MDOK_CURL_CANCELLED: c_int = 4;

static CURL_INIT: Once = Once::new();

fn ensure_curl_init() {
    CURL_INIT.call_once(|| {
        // SAFETY: libcurl global initialization is process-wide and is called
        // exactly once for this integration-test binary.
        assert_eq!(unsafe { mdok_curl_global_init() }, MDOK_CURL_OK);
    });
}

fn empty_error() -> mdok_curl_error {
    mdok_curl_error {
        code: 0,
        argv_index: 0,
        message: mdok_curl_slice {
            ptr: ptr::null(),
            len: 0,
        },
    }
}

fn raw_argv<'a>(args: &'a [&'a [u8]]) -> (Vec<mdok_curl_slice>, mdok_curl_argv) {
    let slices = args
        .iter()
        .map(|arg| mdok_curl_slice {
            ptr: arg.as_ptr(),
            len: arg.len(),
        })
        .collect::<Vec<_>>();
    let argv = mdok_curl_argv {
        argc: slices.len(),
        argv: slices.as_ptr(),
    };
    (slices, argv)
}

fn parse_raw(args: &[&[u8]]) -> (*mut mdok_curl_plan, mdok_curl_error, c_int) {
    let (_slices, argv) = raw_argv(args);
    let mut plan = ptr::null_mut();
    let mut error = empty_error();
    // SAFETY: `_slices` and `args` stay alive for the duration of the call;
    // the bridge copies every accepted argument into the returned plan.
    let status = unsafe { mdok_curl_parse(&argv, &mut plan, &mut error) };
    (plan, error, status)
}

fn write_file_fixture(body: &[u8], suffix: &str) -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "mdok-curl-sys-{}-{}-{}",
        std::process::id(),
        suffix,
        body.len()
    ));
    fs::write(&path, body).expect("write file transfer fixture");
    (format!("file://{}", path.display()), path)
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set request timeout");
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = stream.read(&mut chunk).expect("read request");
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn start_cookie_server() -> (String, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind cookie fixture");
    let address = listener.local_addr().expect("cookie fixture address");
    let handle = thread::spawn(move || {
        let mut saw_cookie = false;
        for request_number in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept cookie request");
            let request = read_request(&mut stream);
            if request_number == 1 {
                saw_cookie = request.to_ascii_lowercase().contains("cookie: session=abc");
            }
            let response = if request_number == 0 {
                b"HTTP/1.1 200 OK\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
            } else if saw_cookie {
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
            } else {
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno"
                    .as_slice()
            };
            stream.write_all(response).expect("write cookie response");
        }
        saw_cookie
    });
    (format!("http://127.0.0.1:{}/", address.port()), handle)
}

fn start_keep_alive_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind keep-alive fixture");
    let address = listener.local_addr().expect("keep-alive fixture address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept keep-alive connection");
        for request_number in 0..2 {
            let request = read_request(&mut stream);
            assert!(!request.is_empty(), "both requests must use one connection");
            let response = if request_number == 0 {
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok"
                    .as_slice()
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
            };
            stream
                .write_all(response)
                .expect("write keep-alive response");
        }
    });
    (format!("http://127.0.0.1:{}/", address.port()), handle)
}

fn start_redirect_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind redirect fixture");
    let address = listener.local_addr().expect("redirect fixture address");
    let handle = thread::spawn(move || {
        for request_number in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept redirect request");
            let _ = read_request(&mut stream);
            let response = if request_number == 0 {
                b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice()
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
            };
            stream.write_all(response).expect("write redirect response");
        }
    });
    (format!("http://127.0.0.1:{}/start", address.port()), handle)
}

fn start_single_response_server(body: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind single-response fixture");
    let address = listener
        .local_addr()
        .expect("single-response fixture address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept single-response request");
        let _ = read_request(&mut stream);
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(header.as_bytes())
            .expect("write single-response header");
        stream.write_all(body).expect("write single-response body");
    });
    (format!("http://127.0.0.1:{}/", address.port()), handle)
}

fn start_reset_server() -> (String, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind reset fixture");
    let address = listener.local_addr().expect("reset fixture address");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept reset request");
            requests.push(read_request(&mut stream));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .expect("write reset response");
        }
        let first = requests.first().map(String::as_str).unwrap_or_default();
        let second = requests.get(1).map(String::as_str).unwrap_or_default();
        first.starts_with("POST ")
            && first.to_ascii_lowercase().contains("x-mdok-stale: yes")
            && second.starts_with("GET ")
            && !second.to_ascii_lowercase().contains("x-mdok-stale: yes")
    });
    (format!("http://127.0.0.1:{}/", address.port()), handle)
}

unsafe extern "C" fn short_write(_data: *const u8, _len: usize, _userdata: *mut c_void) -> usize {
    0
}

unsafe extern "C" fn append_test_body(
    data: *const u8,
    length: usize,
    userdata: *mut c_void,
) -> usize {
    if userdata.is_null() || (length != 0 && data.is_null()) {
        return 0;
    }
    // SAFETY: the synchronous bridge call supplies a valid Vec pointer and a
    // callback buffer that remains valid for the duration of this callback.
    let target = unsafe { &mut *(userdata as *mut Vec<u8>) };
    if length != 0 {
        // SAFETY: libcurl guarantees that the callback buffer contains length
        // readable bytes for the duration of this callback.
        target.extend_from_slice(unsafe { slice::from_raw_parts(data, length) });
    }
    length
}

unsafe extern "C" fn cancel_immediately(_userdata: *mut c_void) -> c_int {
    1
}

#[test]
fn malformed_argv_is_rejected_without_dereference_or_stale_plan() {
    ensure_curl_init();

    let mut plan = std::ptr::dangling_mut::<mdok_curl_plan>();
    // SAFETY: null input/output pointers are explicitly part of this FFI
    // safety test; the bridge must return a parse error without dereferencing.
    let status = unsafe { mdok_curl_parse(ptr::null(), &mut plan, ptr::null_mut()) };
    assert_eq!(status, MDOK_CURL_PARSE_ERROR);
    assert!(plan.is_null());

    let null_array = mdok_curl_argv {
        argc: 1,
        argv: ptr::null(),
    };
    let (plan, _error, status) = {
        let mut plan = ptr::null_mut();
        let mut error = empty_error();
        // SAFETY: the argv object intentionally has a null slice array.
        let status = unsafe { mdok_curl_parse(&null_array, &mut plan, &mut error) };
        (plan, error, status)
    };
    assert_eq!(status, MDOK_CURL_PARSE_ERROR);
    assert!(plan.is_null());

    let malformed_slice = mdok_curl_slice {
        ptr: ptr::null(),
        len: 4,
    };
    let malformed_argv = mdok_curl_argv {
        argc: 1,
        argv: &malformed_slice,
    };
    let mut plan = ptr::null_mut();
    let mut error = empty_error();
    // SAFETY: the slice deliberately violates the non-null pointer contract.
    let status = unsafe { mdok_curl_parse(&malformed_argv, &mut plan, &mut error) };
    assert_eq!(status, MDOK_CURL_PARSE_ERROR);
    assert!(plan.is_null());
    assert_eq!(error.argv_index, 0);

    let curl = b"curl";
    let args = [&curl[..]];
    let (slices, _argv) = raw_argv(&args);
    let mut error = empty_error();
    // SAFETY: an argc larger than the bridge's bounded input contract must be
    // rejected before the one-element slice array is inspected further.
    let status = unsafe {
        mdok_curl_parse(
            &mdok_curl_argv {
                argc: usize::MAX,
                argv: slices.as_ptr(),
            },
            ptr::null_mut(),
            &mut error,
        )
    };
    assert_eq!(status, MDOK_CURL_PARSE_ERROR);
}

#[test]
fn owned_plan_and_session_can_reuse_one_easy_handle() {
    ensure_curl_init();
    let plan =
        Plan::parse(&[b"curl".as_slice(), b"file:///dev/null".as_slice()]).expect("parse file URL");
    let mut session = Session::new().expect("allocate session");
    assert!(!plan.as_ptr().is_null());
    assert!(!session.as_ptr().is_null());

    for _ in 0..2 {
        let mut error = empty_error();
        // SAFETY: both opaque pointers are owned by the wrappers and the
        // session is borrowed mutably for each serialized execution.
        let status = unsafe {
            mdok_curl_execute(
                session.as_ptr(),
                plan.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                &mut error,
            )
        };
        assert_eq!(status, MDOK_CURL_OK, "bridge error code {}", error.code);
    }
}

#[test]
fn session_preserves_cookies_and_copies_transfer_metadata() {
    ensure_curl_init();
    let (base_url, server) = start_cookie_server();
    let first_url = format!("{base_url}set");
    let second_url = format!("{base_url}check");
    let first = Plan::parse(&[b"curl".as_slice(), first_url.as_bytes()]).expect("parse first URL");
    let second =
        Plan::parse(&[b"curl".as_slice(), second_url.as_bytes()]).expect("parse second URL");
    let mut session = Session::new().expect("allocate native session");

    let first_result = session
        .execute_detailed(&first, 1024, 16 * 1024, None)
        .expect("execute first request");
    assert_eq!(first_result.metadata.response_code, Some(200));
    assert_eq!(first_result.metadata.http_version.as_deref(), Some("1.1"));
    assert_eq!(
        first_result.metadata.effective_url.as_deref(),
        Some(first_url.as_str())
    );
    assert_eq!(first_result.metadata.downloaded_bytes, Some(2));
    assert_eq!(first_result.metadata.uploaded_bytes, Some(0));
    assert!(
        first_result
            .metadata
            .response_header_bytes
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        first_result.metadata.primary_ip.as_deref(),
        Some("127.0.0.1")
    );
    assert!(first_result.metadata.primary_port.is_some());
    assert!(first_result.metadata.local_port.is_some());
    assert!(first_result.metadata.total_time_us.is_some());
    assert!(!first_result.metadata.used_proxy);

    let second_result = session
        .execute_detailed(&second, 1024, 16 * 1024, None)
        .expect("execute second request");
    assert_eq!(second_result.metadata.response_code, Some(200));
    assert_eq!(second_result.transfer.body, b"ok");
    assert!(server.join().expect("join cookie fixture"));
}

#[test]
fn session_reuses_one_multi_connection_cache_for_sequential_steps() {
    ensure_curl_init();
    let (base_url, server) = start_keep_alive_server();
    let first_url = format!("{base_url}one");
    let second_url = format!("{base_url}two");
    let first = Plan::parse(&[b"curl".as_slice(), first_url.as_bytes()]).expect("parse first URL");
    let second =
        Plan::parse(&[b"curl".as_slice(), second_url.as_bytes()]).expect("parse second URL");
    let mut session = Session::new().expect("allocate native session");
    assert_eq!(
        session
            .execute_detailed(&first, 1024, 16 * 1024, None)
            .expect("execute first keep-alive request")
            .metadata
            .response_code,
        Some(200)
    );
    assert_eq!(
        session
            .execute_detailed(&second, 1024, 16 * 1024, None)
            .expect("execute second keep-alive request")
            .metadata
            .response_code,
        Some(200)
    );
    server.join().expect("join keep-alive fixture");
}

#[test]
fn native_metadata_reports_effective_url_and_redirect_count() {
    ensure_curl_init();
    let (url, server) = start_redirect_server();
    let plan = Plan::parse(&[b"curl".as_slice(), b"--location".as_slice(), url.as_bytes()])
        .expect("parse redirect plan");
    let mut session = Session::new().expect("allocate redirect session");
    let result = session
        .execute_detailed(&plan, 1024, 16 * 1024, None)
        .expect("execute redirect plan");
    let expected_url = url.replace("/start", "/final");
    assert_eq!(result.metadata.response_code, Some(200));
    assert_eq!(
        result.metadata.effective_url.as_deref(),
        Some(expected_url.as_str())
    );
    assert_eq!(result.metadata.redirect_count, Some(1));
    server.join().expect("join redirect fixture");
}

#[test]
fn short_body_callback_is_not_reported_as_success() {
    ensure_curl_init();
    let (url, path) = write_file_fixture(&vec![b'x'; 64 * 1024], "short-write");
    let (plan, _parse_error, status) = parse_raw(&[b"curl", url.as_bytes()]);
    assert_eq!(status, MDOK_CURL_OK);
    assert!(!plan.is_null());

    let callbacks = mdok_curl_callbacks {
        body: Some(short_write),
        header: None,
        cancelled: None,
    };
    let mut error = empty_error();
    // SAFETY: `plan` came from the bridge and `callbacks` lives through the
    // synchronous execution.
    let status = unsafe {
        mdok_curl_execute(
            ptr::null_mut(),
            plan,
            &callbacks,
            ptr::null_mut(),
            &mut error,
        )
    };
    assert_eq!(status, MDOK_CURL_TRANSFER_ERROR);
    assert_ne!(error.code, 0);
    // SAFETY: plan is no longer used after this point.
    unsafe { mdok_curl_sys::mdok_curl_plan_free(plan) };
    fs::remove_file(path).expect("remove file transfer fixture");
}

#[test]
fn cancellation_callback_maps_to_cancelled_status() {
    ensure_curl_init();
    let (url, path) = write_file_fixture(&vec![b'x'; 1024 * 1024], "cancel");
    let (plan, _parse_error, status) = parse_raw(&[b"curl", url.as_bytes()]);
    assert_eq!(status, MDOK_CURL_OK);
    assert!(!plan.is_null());

    let callbacks = mdok_curl_callbacks {
        body: None,
        header: None,
        cancelled: Some(cancel_immediately),
    };
    let mut error = empty_error();
    // SAFETY: `plan` and `callbacks` remain valid for this synchronous call.
    let status = unsafe {
        mdok_curl_execute(
            ptr::null_mut(),
            plan,
            &callbacks,
            ptr::null_mut(),
            &mut error,
        )
    };
    assert_eq!(
        status, MDOK_CURL_CANCELLED,
        "bridge error code {}",
        error.code
    );
    // SAFETY: plan is no longer used after this point.
    unsafe { mdok_curl_sys::mdok_curl_plan_free(plan) };
    fs::remove_file(path).expect("remove file transfer fixture");
}

#[test]
fn cancelled_session_can_be_reused_without_stale_multi_state() {
    ensure_curl_init();
    let (url, path) = write_file_fixture(&vec![b'x'; 1024 * 1024], "session-cancel");
    let cancelled_plan = Plan::parse(&[b"curl".as_slice(), url.as_bytes()])
        .expect("parse session cancellation plan");
    let clean_plan = Plan::parse(&[b"curl".as_slice(), b"file:///dev/null".as_slice()])
        .expect("parse clean session plan");
    let mut session = Session::new().expect("allocate native session");
    let always_cancel = || true;
    let error = session
        .execute_detailed(
            &cancelled_plan,
            2 * 1024 * 1024,
            16 * 1024,
            Some(&always_cancel),
        )
        .expect_err("cancelled transfer must fail");
    assert_eq!(error.status, MDOK_CURL_CANCELLED);
    session
        .execute_detailed(&clean_plan, 1024, 16 * 1024, None)
        .expect("session remains usable after cancellation");
    fs::remove_file(path).expect("remove session cancellation fixture");
}

#[test]
fn easy_reset_drops_previous_method_and_headers() {
    ensure_curl_init();
    let (url, server) = start_reset_server();
    let first = Plan::parse(&[
        b"curl".as_slice(),
        b"--request".as_slice(),
        b"POST".as_slice(),
        b"--header".as_slice(),
        b"X-Mdok-Stale: yes".as_slice(),
        url.as_bytes(),
    ])
    .expect("parse stateful first plan");
    let second =
        Plan::parse(&[b"curl".as_slice(), url.as_bytes()]).expect("parse stateful second plan");
    let mut session = Session::new().expect("allocate stateful session");
    session
        .execute_detailed(&first, 1024, 16 * 1024, None)
        .expect("execute stateful first plan");
    session
        .execute_detailed(&second, 1024, 16 * 1024, None)
        .expect("execute stateful second plan");
    assert!(server.join().expect("join reset fixture"));
}

#[test]
fn session_reuses_transfer_support_across_multiple_origins() {
    ensure_curl_init();
    let (first_url, first_server) = start_single_response_server(b"one");
    let (second_url, second_server) = start_single_response_server(b"two");
    let first =
        Plan::parse(&[b"curl".as_slice(), first_url.as_bytes()]).expect("parse first origin");
    let second =
        Plan::parse(&[b"curl".as_slice(), second_url.as_bytes()]).expect("parse second origin");
    let mut session = Session::new().expect("allocate multi-origin session");
    let first_result = session
        .execute_detailed(&first, 1024, 16 * 1024, None)
        .expect("execute first origin");
    let second_result = session
        .execute_detailed(&second, 1024, 16 * 1024, None)
        .expect("execute second origin");
    assert_eq!(first_result.transfer.body, b"one");
    assert_eq!(second_result.transfer.body, b"two");
    assert_ne!(
        first_result.metadata.primary_port,
        second_result.metadata.primary_port
    );
    first_server.join().expect("join first-origin fixture");
    second_server.join().expect("join second-origin fixture");
}

#[test]
fn reusable_session_matches_legacy_null_session_body_path() {
    ensure_curl_init();
    let (legacy_url, legacy_server) = start_single_response_server(b"same");
    let (session_url, session_server) = start_single_response_server(b"same");
    let legacy_plan = Plan::parse(&[b"curl".as_slice(), legacy_url.as_bytes()])
        .expect("parse legacy compatibility plan");
    let session_plan = Plan::parse(&[b"curl".as_slice(), session_url.as_bytes()])
        .expect("parse reusable compatibility plan");

    let mut legacy_body = Vec::new();
    let callbacks = mdok_curl_callbacks {
        body: Some(append_test_body),
        header: None,
        cancelled: None,
    };
    let mut legacy_error = empty_error();
    // SAFETY: the plan and callback state remain alive for this synchronous
    // legacy ABI call, and the null session selects the compatibility path.
    let legacy_status = unsafe {
        mdok_curl_execute(
            ptr::null_mut(),
            legacy_plan.as_ptr(),
            &callbacks,
            &mut legacy_body as *mut Vec<u8> as *mut c_void,
            &mut legacy_error,
        )
    };

    let mut session = Session::new().expect("allocate compatibility session");
    let session_body = session
        .execute_detailed(&session_plan, 1024, 16 * 1024, None)
        .expect("execute reusable compatibility path")
        .transfer
        .body;
    assert_eq!(
        legacy_status, MDOK_CURL_OK,
        "legacy error {}",
        legacy_error.code
    );
    assert_eq!(legacy_body, session_body);
    legacy_server.join().expect("join legacy fixture");
    session_server.join().expect("join session fixture");
}

#[test]
fn streaming_body_sink_receives_chunks_without_retained_body_buffer() {
    ensure_curl_init();
    let (url, server) = start_single_response_server(b"streamed");
    let plan =
        Plan::parse(&[b"curl".as_slice(), url.as_bytes()]).expect("parse streaming body plan");
    let mut session = Session::new().expect("allocate streaming session");
    let mut received = Vec::new();
    let mut sink = |chunk: &[u8]| {
        received.extend_from_slice(chunk);
        Ok::<(), (i32, String)>(())
    };
    let result = session
        .execute_detailed_with_body_sink(&plan, 1024, 16 * 1024, None, &mut sink)
        .expect("execute streaming body plan");
    assert!(result.transfer.body.is_empty());
    assert_eq!(received, b"streamed");
    server.join().expect("join streaming fixture");
}

#[test]
fn null_session_free_is_safe() {
    ensure_curl_init();
    // SAFETY: the C API promises a null-safe release helper.
    unsafe { mdok_curl_session_free(ptr::null_mut()) };
    let session = unsafe { mdok_curl_session_new() };
    assert!(!session.is_null());
    unsafe { mdok_curl_session_free(session) };
}

#[test]
fn upstream_tool_parser_normalizes_supported_options_and_rejects_unsafe_shapes() {
    ensure_curl_init();
    let supported = Plan::parse(&[
        b"curl".as_slice(),
        b"--request".as_slice(),
        b"POST".as_slice(),
        b"--header".as_slice(),
        b"X-Mdok: yes".as_slice(),
        b"--data".as_slice(),
        b"payload".as_slice(),
        b"file:///dev/null".as_slice(),
    ]);
    assert!(
        supported.is_ok(),
        "upstream parser rejected supported options"
    );

    let parallel = Plan::parse(&[
        b"curl".as_slice(),
        b"--parallel".as_slice(),
        b"file:///dev/null".as_slice(),
    ])
    .err()
    .expect("parallel execution must not enter the single-transfer bridge");
    assert_eq!(parallel.status, MDOK_CURL_PARSE_ERROR);

    let unknown = Plan::parse(&[
        b"curl".as_slice(),
        b"--mdok-not-a-curl-option".as_slice(),
        b"file:///dev/null".as_slice(),
    ])
    .err()
    .expect("unknown options must be rejected by curl's parser");
    assert_eq!(unknown.status, MDOK_CURL_PARSE_ERROR);
    assert!(unknown.code >= 300);

    let get_with_body = Plan::parse(&[
        b"curl".as_slice(),
        b"--get".as_slice(),
        b"--data".as_slice(),
        b"query=value".as_slice(),
        b"file:///dev/null".as_slice(),
    ])
    .err()
    .expect("unsupported --get body semantics must be rejected");
    assert_eq!(get_with_body.status, MDOK_CURL_PARSE_ERROR);
}
