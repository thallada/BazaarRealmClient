use std::{ffi::CStr, ffi::CString, os::raw::c_char, path::Path};

use anyhow::Result;
use log::LevelFilter;
use reqwest::{blocking::Response, Url};
use uuid::Uuid;

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    error::extract_error_from_response,
    log_server_error,
    result::{FFIError, FFIResult},
};

#[no_mangle]
pub extern "C" fn init() -> bool {
    match dirs::document_dir() {
        Some(mut log_dir) => {
            log_dir.push(Path::new(
                r#"My Games\Skyrim Special Edition\SKSE\BazaarRealmClient.log"#,
            ));
            match simple_logging::log_to_file(log_dir, LevelFilter::Info) {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn status_check(api_url: *const c_char) -> FFIResult<bool> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    info!("status_check api_url: {:?}", api_url);

    fn inner(api_url: &str) -> Result<()> {
        #[cfg(not(test))]
        let api_url = Url::parse(api_url)?.join("v1/status")?;
        #[cfg(test)]
        let api_url = Url::parse(&mockito::server_url())?.join("v1/status")?;

        let resp = reqwest::blocking::get(api_url)?;
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            Ok(())
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url) {
        Ok(()) => {
            info!("status_check ok");
            FFIResult::Ok(true)
        }
        Err(err) => {
            error!("status_check failed. {}", err);
            FFIResult::Err(FFIError::from(err))
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn generate_api_key() -> *mut c_char {
    // TODO: is leaking this CString bad?
    let uuid = CString::new(format!("{}", Uuid::new_v4()))
        .expect("could not create CString")
        .into_raw();
    info!("generate_api_key successful");
    uuid
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[test]
    fn test_status_check() {
        let mock = mock("GET", "/v1/status").with_status(200).create();

        let api_url = CString::new("url").unwrap().into_raw();
        let result = status_check(api_url);
        mock.assert();
        match result {
            FFIResult::Ok(success) => {
                assert_eq!(success, true);
            }
            FFIResult::Err(error) => panic!(
                "status_check returned error: {:?}",
                match error {
                    FFIError::Server(server_error) =>
                        format!("{} {}", server_error.status, unsafe {
                            CStr::from_ptr(server_error.title).to_string_lossy()
                        }),
                    FFIError::Network(network_error) =>
                        unsafe { CStr::from_ptr(network_error).to_string_lossy() }.to_string(),
                }
            ),
        }
    }

    #[test]
    fn test_status_check_server_error() {
        let mock = mock("GET", "/v1/status")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let result = status_check(api_url);
        mock.assert();
        match result {
            FFIResult::Ok(success) => panic!("status_check returned Ok result: {:?}", success),
            FFIResult::Err(error) => match error {
                FFIError::Server(server_error) => {
                    assert_eq!(server_error.status, 500);
                    assert_eq!(
                        unsafe { CStr::from_ptr(server_error.title).to_string_lossy() },
                        "Internal Server Error"
                    );
                }
                _ => panic!("status_check did not return a server error"),
            },
        }
    }
}
