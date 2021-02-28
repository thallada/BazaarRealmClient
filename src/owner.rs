use std::{ffi::CStr, ffi::CString, os::raw::c_char};

use anyhow::Result;
use chrono::NaiveDateTime;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    cache::file_cache_dir,
    cache::update_file_caches,
    error::extract_error_from_response,
    result::{FFIError, FFIResult},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Owner {
    pub name: String,
    pub mod_version: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavedOwner {
    pub id: i32,
    pub name: String,
    pub mod_version: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Owner {
    pub fn from_game(name: &str, mod_version: i32) -> Self {
        Self {
            name: name.to_string(),
            mod_version: mod_version,
        }
    }
}

#[derive(Debug, PartialEq)]
#[repr(C)]
pub struct RawOwner {
    pub id: i32,
    pub name: *const c_char,
    pub mod_version: i32,
}

impl From<SavedOwner> for RawOwner {
    fn from(raw_owner: SavedOwner) -> Self {
        Self {
            id: raw_owner.id,
            name: CString::new(raw_owner.name).unwrap_or_default().into_raw(),
            mod_version: raw_owner.mod_version,
        }
    }
}

#[no_mangle]
pub extern "C" fn create_owner(
    api_url: *const c_char,
    api_key: *const c_char,
    name: *const c_char,
    mod_version: i32,
) -> FFIResult<RawOwner> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    info!(
        "create_owner api_url: {:?}, api_key: {:?}, name: {:?}, mod_version: {:?}",
        api_url, api_key, name, mod_version
    );

    fn inner(api_url: &str, api_key: &str, name: &str, mod_version: i32) -> Result<SavedOwner> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/owners")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/owners")?;

        let owner = Owner::from_game(name, mod_version);
        info!("created owner from game: {:?}", &owner);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key.clone())
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&owner)?)
            .send()?;
        info!("create owner response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_owner: SavedOwner = bincode::deserialize(&bytes)?;
            let body_cache_path = cache_dir.join(format!("owner_{}.bin", saved_owner.id));
            let metadata_cache_path =
                cache_dir.join(format!("owner_{}_metadata.json", saved_owner.id));
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_owner)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url, &api_key, &name, mod_version) {
        Ok(owner) => {
            info!("create_owner successful");
            FFIResult::Ok(RawOwner::from(owner))
        }
        Err(err) => {
            error!("create_owner failed. {}", err);
            FFIResult::Err(FFIError::from(err))
        }
    }
}

#[no_mangle]
pub extern "C" fn update_owner(
    api_url: *const c_char,
    api_key: *const c_char,
    id: i32,
    name: *const c_char,
    mod_version: i32,
) -> FFIResult<RawOwner> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    info!(
        "update_owner api_url: {:?}, api_key: {:?}, name: {:?}, mod_version: {:?}",
        api_url, api_key, name, mod_version
    );

    fn inner(
        api_url: &str,
        api_key: &str,
        id: i32,
        name: &str,
        mod_version: i32,
    ) -> Result<SavedOwner> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/owners/{}", id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join(&format!("v1/owners/{}", id))?;

        let owner = Owner::from_game(name, mod_version);
        info!("created owner from game: {:?}", &owner);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .patch(url)
            .header("Api-Key", api_key.clone())
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&owner)?)
            .send()?;
        info!("update owner response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("owner_{}.bin", id));
        let metadata_cache_path = cache_dir.join(format!("owner_{}_metadata.json", id));
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_owner: SavedOwner = bincode::deserialize(&bytes)?;
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_owner)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url, &api_key, id, &name, mod_version) {
        Ok(owner) => {
            info!("update_owner successful");
            FFIResult::Ok(RawOwner::from(owner))
        }
        Err(err) => {
            error!("update_owner failed. {}", err);
            FFIResult::Err(FFIError::from(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;
    use chrono::Utc;
    use mockito::mock;

    #[test]
    fn test_create_owner() {
        let example = SavedOwner {
            id: 1,
            name: "name".to_string(),
            mod_version: 1,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("POST", "/v1/owners")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
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
            FFIResult::Err(error) => panic!(
                "create_owner returned error: {:?}",
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
            FFIResult::Err(error) => match error {
                FFIError::Server(server_error) => {
                    assert_eq!(server_error.status, 500);
                    assert_eq!(
                        unsafe { CStr::from_ptr(server_error.title).to_string_lossy() },
                        "Internal Server Error"
                    );
                }
                _ => panic!("create_owner did not return a server error"),
            },
        }
    }

    #[test]
    fn test_update_owner() {
        let example = SavedOwner {
            id: 1,
            name: "name".to_string(),
            mod_version: 1,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("PATCH", "/v1/owners/1")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
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
            FFIResult::Err(error) => panic!(
                "update_owner returned error: {:?}",
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
            FFIResult::Err(error) => match error {
                FFIError::Server(server_error) => {
                    assert_eq!(server_error.status, 500);
                    assert_eq!(
                        unsafe { CStr::from_ptr(server_error.title).to_string_lossy() },
                        "Internal Server Error"
                    );
                }
                _ => panic!("update_owner did not return a server error"),
            },
        }
    }
}
