use std::os::raw::c_char;

#[repr(C, u8)]
pub enum FFIResult<T> {
    Ok(T),
    Err(*const c_char),
}