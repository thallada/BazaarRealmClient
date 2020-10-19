use std::os::raw::c_char;

#[derive(Debug, PartialEq)]
#[repr(C, u8)]
pub enum FFIResult<T> {
    Ok(T),
    Err(*const c_char),
}

// Required in order to store results in a thread-safe static cache.
// Rust complains that the raw pointers cannot be Send + Sync. We only ever:
// a) read the values in C++/Papyrus land, and it's okay if multiple threads do that.
// b) from_raw() the pointers back into rust values and then drop them. This could be problematic if another script is still reading at the same time, but I'm pretty sure that won't happen.
// Besides, it's already unsafe to read from a raw pointer
unsafe impl<T> Send for FFIResult<T> {}
unsafe impl<T> Sync for FFIResult<T> {}
