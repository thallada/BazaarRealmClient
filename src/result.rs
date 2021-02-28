use anyhow::Error;

use std::convert::From;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr::null;

use crate::error::ServerError;

#[derive(Debug, PartialEq)]
#[repr(C)]
pub struct FFIServerError {
    pub status: u16,
    pub title: *const c_char,
    pub detail: *const c_char,
}

impl From<&ServerError> for FFIServerError {
    fn from(server_error: &ServerError) -> Self {
        FFIServerError {
            status: server_error.status.as_u16(),
            // TODO: may need to drop these CStrings once C++ is done reading them
            title: CString::new(server_error.title.clone())
                .expect("could not create CString")
                .into_raw(),
            detail: match &server_error.detail {
                Some(detail) => CString::new(detail.clone())
                    .expect("could not create CString")
                    .into_raw(),
                None => null(),
            },
        }
    }
}

#[derive(Debug, PartialEq)]
#[repr(C, u8)]
pub enum FFIError {
    Server(FFIServerError),
    Network(*const c_char),
}

impl From<Error> for FFIError {
    fn from(error: Error) -> Self {
        if let Some(server_error) = error.downcast_ref::<ServerError>() {
            FFIError::Server(FFIServerError::from(server_error))
        } else {
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(error.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIError::Network(err_string)
        }
    }
}

#[derive(Debug, PartialEq)]
#[repr(C, u8)]
pub enum FFIResult<T> {
    Ok(T),
    Err(FFIError),
}
