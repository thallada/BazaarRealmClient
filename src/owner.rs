use std::{ffi::CStr, ffi::CString, os::raw::c_char};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{cache::file_cache_dir, cache::update_file_cache, result::FFIResult};

#[derive(Serialize, Deserialize, Debug)]
pub struct Owner {
    pub id: Option<i32>,
    pub name: String,
    pub api_key: Option<String>,
    pub mod_version: u32,
}

impl Owner {
    pub fn from_game(name: &str, api_key: &str, mod_version: u32) -> Self {
        Self {
            id: None,
            name: name.to_string(),
            api_key: Some(api_key.to_string()),
            mod_version,
        }
    }
}

#[derive(Debug, PartialEq)]
#[repr(C)]
pub struct RawOwner {
    pub id: i32,
    pub name: *const c_char,
    pub mod_version: u32,
}

// Required in order to store results in a thread-safe static cache.
// Rust complains that the raw pointers cannot be Send + Sync. We only ever:
// a) read the values in C++/Papyrus land, and it's okay if multiple threads do that.
// b) from_raw() the pointers back into rust values and then drop them. This could be problematic if another script is still reading at the same time, but I'm pretty sure that won't happen.
// Besides, it's already unsafe to read from a raw pointer
unsafe impl Send for RawOwner {}
unsafe impl Sync for RawOwner {}

#[no_mangle]
pub extern "C" fn create_owner(
    api_url: *const c_char,
    api_key: *const c_char,
    name: *const c_char,
    mod_version: u32,
) -> FFIResult<RawOwner> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    info!(
        "create_owner api_url: {:?}, api_key: {:?}, name: {:?}, mod_version: {:?}",
        api_url, api_key, name, mod_version
    );

    fn inner(api_url: &str, api_key: &str, name: &str, mod_version: u32) -> Result<Owner> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/owners")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/owners")?;

        let owner = Owner::from_game(name, api_key, mod_version);
        info!("created owner from game: {:?}", &owner);
        if let Some(api_key) = &owner.api_key {
            let client = reqwest::blocking::Client::new();
            let resp = client
                .post(url)
                .header("Api-Key", api_key.clone())
                .json(&owner)
                .send()?;
            info!("create owner response from api: {:?}", &resp);
            let bytes = resp.bytes()?;
            let json: Owner = serde_json::from_slice(&bytes)?;
            if let Some(id) = json.id {
                update_file_cache(
                    &file_cache_dir(api_url)?.join(format!("owner_{}.json", id)),
                    &bytes,
                )?;
            }
            Ok(json)
        } else {
            Err(anyhow!("api-key not defined"))
        }
    }

    match inner(&api_url, &api_key, &name, mod_version) {
        Ok(owner) => {
            info!("create_owner successful");
            if let Some(id) = owner.id {
                FFIResult::Ok(RawOwner {
                    id,
                    name: CString::new(owner.name).unwrap_or_default().into_raw(),
                    mod_version: owner.mod_version,
                })
            } else {
                error!("create_owner failed. API did not return an owner with an ID");
                let err_string = CString::new("API did not return an owner with an ID".to_string())
                    .expect("could not create CString")
                    .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("create_owner failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn update_owner(
    api_url: *const c_char,
    api_key: *const c_char,
    id: u32,
    name: *const c_char,
    mod_version: u32,
) -> FFIResult<RawOwner> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    info!(
        "update_owner api_url: {:?}, api_key: {:?}, name: {:?}, mod_version: {:?}",
        api_url, api_key, name, mod_version
    );

    fn inner(api_url: &str, api_key: &str, id: u32, name: &str, mod_version: u32) -> Result<Owner> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/owners/{}", id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join(&format!("v1/owners/{}", id))?;

        let owner = Owner::from_game(name, api_key, mod_version);
        info!("created owner from game: {:?}", &owner);
        if let Some(api_key) = &owner.api_key {
            let client = reqwest::blocking::Client::new();
            let resp = client
                .patch(url)
                .header("Api-Key", api_key.clone())
                .json(&owner)
                .send()?;
            info!("update owner response from api: {:?}", &resp);
            let bytes = resp.bytes()?;
            let json: Owner = serde_json::from_slice(&bytes)?;
            if let Some(id) = json.id {
                update_file_cache(
                    &file_cache_dir(api_url)?.join(format!("owner_{}.json", id)),
                    &bytes,
                )?;
            }
            Ok(json)
        } else {
            Err(anyhow!("api-key not defined"))
        }
    }

    match inner(&api_url, &api_key, id, &name, mod_version) {
        Ok(owner) => {
            info!("update_owner successful");
            if let Some(id) = owner.id {
                FFIResult::Ok(RawOwner {
                    id,
                    name: CString::new(owner.name).unwrap_or_default().into_raw(),
                    mod_version: owner.mod_version,
                })
            } else {
                error!("update_owner failed. API did not return an owner with an ID");
                let err_string = CString::new("API did not return an owner with an ID".to_string())
                    .expect("could not create CString")
                    .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("update_owner failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;
    use mockito::mock;

    #[test]
    fn test_create_owner() {
        let mock = mock("POST", "/v1/owners")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "mod_version": 1, "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        let result = create_owner(api_url, api_key, name, mod_version);
        mock.assert();
        match result {
            FFIResult::Ok(raw_owner) => {
                assert_eq!(raw_owner.id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_owner.name).to_string_lossy() },
                    "name"
                );
                assert_eq!(raw_owner.mod_version, 1);
            }
            FFIResult::Err(error) => panic!("create_owner returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_create_owner_server_error() {
        let mock = mock("POST", "/v1/owners")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        let result = create_owner(api_url, api_key, name, mod_version);
        mock.assert();
        match result {
            FFIResult::Ok(raw_owner) => {
                panic!("create_owner returned Ok result: {:#x?}", raw_owner)
            }
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }

    #[test]
    fn test_update_owner() {
        let mock = mock("PATCH", "/v1/owners/1")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "mod_version": 1, "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        let result = update_owner(api_url, api_key, 1, name, mod_version);
        mock.assert();
        match result {
            FFIResult::Ok(raw_owner) => {
                assert_eq!(raw_owner.id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_owner.name).to_string_lossy() },
                    "name"
                );
                assert_eq!(raw_owner.mod_version, 1);
            }
            FFIResult::Err(error) => panic!("update_owner returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_update_owner_server_error() {
        let mock = mock("PATCH", "/v1/owners/1")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        let result = update_owner(api_url, api_key, 1, name, mod_version);
        mock.assert();
        match result {
            FFIResult::Ok(raw_owner) => {
                panic!("update_owner returned Ok result: {:#x?}", raw_owner)
            }
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }
}
