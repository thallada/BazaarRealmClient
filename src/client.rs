use std::{ffi::CStr, ffi::CString, os::raw::c_char, path::Path};

use anyhow::Result;
use log::LevelFilter;
use reqwest::{blocking::Response, Url};
use uuid::Uuid;

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{log_server_error, result::FFIResult};

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

    fn inner(api_url: &str) -> Result<Response> {
        #[cfg(not(test))]
        let api_url = Url::parse(api_url)?.join("status")?;
        #[cfg(test)]
        let api_url = Url::parse(&mockito::server_url())?.join("status")?;

        Ok(reqwest::blocking::get(api_url)?)
    }

    match inner(&api_url) {
        Ok(resp) if resp.status() == 200 => {
            info!("status_check ok");
            FFIResult::Ok(true)
        }
        Ok(resp) => {
            error!("status_check failed. Server error");
            log_server_error(resp);
            let err_string = CString::new("API returned a non-200 status code".to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
        Err(err) => {
            error!("status_check failed. {}", err);
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
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
        let mock = mock("GET", "/status").with_status(200).create();

        let api_url = CString::new("url").unwrap().into_raw();
        let result = status_check(api_url);
        mock.assert();
        match result {
            FFIResult::Ok(success) => {
                assert_eq!(success, true);
            }
            FFIResult::Err(error) => panic!("status_check returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_status_check_server_error() {
        let mock = mock("GET", "/status")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let result = status_check(api_url);
        mock.assert();
        match result {
            FFIResult::Ok(success) => panic!("status_check returned Ok result: {:?}", success),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "API returned a non-200 status code"
                );
            }
        }
    }
}
