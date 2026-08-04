#![allow(non_camel_case_types)]

use std::{
    ffi::{c_char, c_int, c_void},
    ptr::NonNull,
    slice,
};

#[repr(C)]
pub struct mdok_curl_plan {
    _private: [u8; 0],
}
#[repr(C)]
pub struct mdok_curl_session {
    _private: [u8; 0],
}

#[repr(C)]
pub struct mdok_curl_slice {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
pub struct mdok_curl_argv {
    pub argc: usize,
    pub argv: *const mdok_curl_slice,
}

#[repr(C)]
pub struct mdok_curl_error {
    pub code: i32,
    pub argv_index: usize,
    pub message: mdok_curl_slice,
}

pub type mdok_curl_write_cb = unsafe extern "C" fn(*const u8, usize, *mut c_void) -> usize;
pub type mdok_curl_cancel_cb = unsafe extern "C" fn(*mut c_void) -> c_int;

#[repr(C)]
pub struct mdok_curl_callbacks {
    pub body: Option<mdok_curl_write_cb>,
    pub header: Option<mdok_curl_write_cb>,
    pub cancelled: Option<mdok_curl_cancel_cb>,
}

unsafe extern "C" {
    pub fn mdok_curl_global_init() -> c_int;
    pub fn mdok_curl_global_cleanup();
    pub fn mdok_curl_session_new() -> *mut mdok_curl_session;
    pub fn mdok_curl_session_free(session: *mut mdok_curl_session);
    pub fn mdok_curl_parse(
        argv: *const mdok_curl_argv,
        out_plan: *mut *mut mdok_curl_plan,
        out_error: *mut mdok_curl_error,
    ) -> c_int;
    pub fn mdok_curl_execute(
        session: *mut mdok_curl_session,
        plan: *const mdok_curl_plan,
        callbacks: *const mdok_curl_callbacks,
        userdata: *mut c_void,
        out_error: *mut mdok_curl_error,
    ) -> c_int;
    pub fn mdok_curl_plan_free(plan: *mut mdok_curl_plan);
    pub fn mdok_curl_last_error_message() -> *const c_char;
    pub fn mdok_curl_reserved(userdata: *mut c_void);
}

/// Owned error information copied out of the bridge before another bridge
/// call can overwrite its thread-local diagnostic buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeError {
    pub status: c_int,
    pub code: i32,
    pub message: String,
}

/// An owned curl plan. The raw plan remains opaque and is released on drop.
#[must_use]
pub struct Plan(NonNull<mdok_curl_plan>);

impl Plan {
    /// Parse argv without exposing raw pointers to callers.
    pub fn parse(args: &[&[u8]]) -> Result<Self, BridgeError> {
        let slices: Vec<_> = args
            .iter()
            .map(|arg| mdok_curl_slice {
                ptr: arg.as_ptr(),
                len: arg.len(),
            })
            .collect();
        let argv = mdok_curl_argv {
            argc: slices.len(),
            argv: slices.as_ptr(),
        };
        let mut raw_plan = std::ptr::null_mut();
        let mut error = mdok_curl_error {
            code: 0,
            argv_index: 0,
            message: mdok_curl_slice {
                ptr: std::ptr::null(),
                len: 0,
            },
        };
        // SAFETY: `slices` and all borrowed argument bytes remain alive for
        // the duration of the FFI call; the bridge copies them immediately.
        let status = unsafe { mdok_curl_parse(&argv, &mut raw_plan, &mut error) };
        if status == 0 {
            return NonNull::new(raw_plan).map(Self).ok_or_else(|| BridgeError {
                status: 5,
                code: 0,
                message: "bridge returned a null plan".to_owned(),
            });
        }
        Err(copy_error(status, &error))
    }

    pub fn as_ptr(&self) -> *const mdok_curl_plan {
        self.0.as_ptr()
    }
}

impl Drop for Plan {
    fn drop(&mut self) {
        // SAFETY: the pointer was returned by mdok_curl_parse and is owned by
        // this value until this destructor runs.
        unsafe { mdok_curl_plan_free(self.0.as_ptr()) };
    }
}

/// Owned per-session handle. A session serializes its own executions in the C
/// bridge; callers must still keep the `Session` alive for every raw call.
#[must_use]
pub struct Session(NonNull<mdok_curl_session>);

impl Session {
    pub fn new() -> Result<Self, BridgeError> {
        // SAFETY: the constructor takes no borrowed pointers and returns an
        // owned opaque handle or null.
        let raw = unsafe { mdok_curl_session_new() };
        NonNull::new(raw).map(Self).ok_or_else(|| BridgeError {
            status: 5,
            code: 0,
            message: last_error_message(),
        })
    }

    pub fn as_ptr(&mut self) -> *mut mdok_curl_session {
        self.0.as_ptr()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: the pointer was returned by mdok_curl_session_new and is
        // owned by this value until this destructor runs.
        unsafe { mdok_curl_session_free(self.0.as_ptr()) };
    }
}

fn copy_error(status: c_int, error: &mdok_curl_error) -> BridgeError {
    let message = if error.message.len == 0 {
        String::new()
    } else if error.message.ptr.is_null() {
        "bridge returned an invalid error message".to_owned()
    } else {
        // SAFETY: the bridge owns the diagnostic buffer and guarantees that it
        // remains valid until the next bridge call on this thread.
        unsafe {
            String::from_utf8_lossy(slice::from_raw_parts(error.message.ptr, error.message.len))
        }
        .into_owned()
    };
    BridgeError {
        status,
        code: error.code,
        message,
    }
}

fn last_error_message() -> String {
    // SAFETY: the bridge returns a NUL-terminated thread-local diagnostic
    // string. A null pointer is handled defensively for failed initialization.
    let ptr = unsafe { mdok_curl_last_error_message() };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}
