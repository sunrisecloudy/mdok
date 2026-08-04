#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

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
