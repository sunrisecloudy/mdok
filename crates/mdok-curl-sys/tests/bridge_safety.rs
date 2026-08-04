use mdok_curl_sys::{
    Plan, Session, mdok_curl_argv, mdok_curl_callbacks, mdok_curl_error, mdok_curl_execute,
    mdok_curl_global_init, mdok_curl_parse, mdok_curl_plan, mdok_curl_session_free,
    mdok_curl_session_new, mdok_curl_slice,
};
use std::{
    ffi::{c_int, c_void},
    fs, ptr,
    sync::Once,
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

unsafe extern "C" fn short_write(_data: *const u8, _len: usize, _userdata: *mut c_void) -> usize {
    0
}

unsafe extern "C" fn cancel_immediately(_userdata: *mut c_void) -> c_int {
    1
}

#[test]
fn malformed_argv_is_rejected_without_dereference_or_stale_plan() {
    ensure_curl_init();

    let mut plan = 1usize as *mut mdok_curl_plan;
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
fn null_session_free_is_safe() {
    ensure_curl_init();
    // SAFETY: the C API promises a null-safe release helper.
    unsafe { mdok_curl_session_free(ptr::null_mut()) };
    let session = unsafe { mdok_curl_session_new() };
    assert!(!session.is_null());
    unsafe { mdok_curl_session_free(session) };
}
