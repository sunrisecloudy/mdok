#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct mdok_curl_plan { _private: [u8; 0] }
#[repr(C)]
pub struct mdok_curl_session { _private: [u8; 0] }

unsafe extern "C" {
    pub fn mdok_curl_global_init() -> c_int;
    pub fn mdok_curl_global_cleanup();
    pub fn mdok_curl_plan_free(plan: *mut mdok_curl_plan);
    pub fn mdok_curl_last_error_message() -> *const c_char;
    pub fn mdok_curl_reserved(userdata: *mut c_void);
}
