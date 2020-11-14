#![allow(non_snake_case)]
#![feature(vec_into_raw_parts)]

use std::ffi::CString;
use std::os::raw::c_char;

use reqwest::blocking::Response;

#[cfg(not(test))]
use log::error;

#[cfg(test)]
use std::println as error;

mod cache;
mod client;
mod error;
mod interior_ref_list;
mod merchandise_list;
mod owner;
mod result;
mod shop;
mod transaction;

pub const API_VERSION: &'static str = "v1";

pub fn log_server_error(resp: Response) {
    let status = resp.status();
    if let Ok(text) = resp.text() {
        error!("Server error: {} {}", status, text);
    }
    error!("Server error: {}", status);
}

#[no_mangle]
pub extern "C" fn free_string(ptr: *mut c_char) {
    unsafe { drop(CString::from_raw(ptr)) }
}
