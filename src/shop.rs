use std::{ffi::CStr, ffi::CString, os::raw::c_char};

use anyhow::Result;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    cache::file_cache_dir, cache::from_file_cache, cache::update_file_cache, log_server_error,
    result::FFIResult,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Shop {
    pub id: Option<i32>,
    pub name: String,
    pub description: String,
}

impl Shop {
    pub fn from_game(name: &str, description: &str) -> Self {
        Self {
            id: None,
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawShop {
    pub id: i32,
    pub name: *const c_char,
    pub description: *const c_char,
}

// Required in order to store results in a thread-safe static cache.
// Rust complains that the raw pointers cannot be Send + Sync. We only ever:
// a) read the values in C++/Papyrus land, and it's okay if multiple threads do that.
// b) from_raw() the pointers back into rust values and then drop them. This could be problematic if another script is still reading at the same time, but I'm pretty sure that won't happen.
// Besides, it's already unsafe to read from a raw pointer
unsafe impl Send for RawShop {}
unsafe impl Sync for RawShop {}

#[no_mangle]
pub extern "C" fn create_shop(
    api_url: *const c_char,
    api_key: *const c_char,
    name: *const c_char,
    description: *const c_char,
) -> FFIResult<RawShop> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let description = unsafe { CStr::from_ptr(description) }.to_string_lossy();
    info!(
        "create_shop api_url: {:?}, api_key: {:?}, name: {:?}, description: {:?}",
        api_url, api_key, name, description
    );

    fn inner(api_url: &str, api_key: &str, name: &str, description: &str) -> Result<Shop> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/shops")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/shops")?;

        let shop = Shop::from_game(name, description);
        info!("created shop from game: {:?}", &shop);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key)
            .json(&shop)
            .send()?;
        info!("create shop response from api: {:?}", &resp);
        let bytes = resp.bytes()?;
        let json: Shop = serde_json::from_slice(&bytes)?;
        if let Some(id) = json.id {
            update_file_cache(
                &file_cache_dir(api_url)?.join(format!("shop_{}.json", id)),
                &bytes,
            )?;
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, &name, &description) {
        Ok(shop) => {
            info!("create_shop successful");
            if let Some(id) = shop.id {
                FFIResult::Ok(RawShop {
                    id,
                    name: CString::new(shop.name).unwrap_or_default().into_raw(),
                    description: CString::new(shop.description)
                        .unwrap_or_default()
                        .into_raw(),
                })
            } else {
                error!("create_shop failed. API did not return a shop with an ID");
                let err_string = CString::new("API did not return a shop with an ID".to_string())
                    .expect("could not create CString")
                    .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("create_shop failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn update_shop(
    api_url: *const c_char,
    api_key: *const c_char,
    id: u32,
    name: *const c_char,
    description: *const c_char,
) -> FFIResult<RawShop> {
    info!("update_shop begin");
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let description = unsafe { CStr::from_ptr(description) }.to_string_lossy();
    info!(
        "update_shop api_url: {:?}, api_key: {:?}, name: {:?}, description: {:?}",
        api_url, api_key, name, description
    );

    fn inner(api_url: &str, api_key: &str, id: u32, name: &str, description: &str) -> Result<Shop> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}", id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join(&format!("v1/shops/{}", id))?;

        let shop = Shop::from_game(name, description);
        info!("created shop from game: {:?}", &shop);
        let client = reqwest::blocking::Client::new();
        let resp = client
            .patch(url)
            .header("Api-Key", api_key)
            .json(&shop)
            .send()?;
        info!("update shop response from api: {:?}", &resp);
        let bytes = resp.bytes()?;
        let json: Shop = serde_json::from_slice(&bytes)?;
        if let Some(id) = json.id {
            update_file_cache(
                &file_cache_dir(api_url)?.join(format!("shop_{}.json", id)),
                &bytes,
            )?;
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, id, &name, &description) {
        Ok(shop) => {
            info!("update_shop successful");
            if let Some(id) = shop.id {
                FFIResult::Ok(RawShop {
                    id,
                    name: CString::new(shop.name).unwrap_or_default().into_raw(),
                    description: CString::new(shop.description)
                        .unwrap_or_default()
                        .into_raw(),
                })
            } else {
                error!("create_shop failed. API did not return a shop with an ID");
                let err_string = CString::new("API did not return a shop with an ID".to_string())
                    .expect("could not create CString")
                    .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("update_shop failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn get_shop(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
) -> FFIResult<RawShop> {
    info!("get_shop begin");
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!(
        "get_shop api_url: {:?}, api_key: {:?}, shop_id: {:?}",
        api_url, api_key, shop_id
    );

    fn inner(api_url: &str, api_key: &str, shop_id: i32) -> Result<Shop> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join(&format!("v1/shops/{}", shop_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_path = file_cache_dir(api_url)?.join(format!("shop_{}.json", shop_id));

        match client.get(url).header("Api-Key", api_key).send() {
            Ok(resp) => {
                info!("get_shop response from api: {:?}", &resp);
                if !resp.status().is_server_error() {
                    let bytes = resp.bytes()?;
                    update_file_cache(&cache_path, &bytes)?;
                    let json = serde_json::from_slice(&bytes)?;
                    Ok(json)
                } else {
                    log_server_error(resp);
                    from_file_cache(&cache_path)
                }
            }
            Err(err) => {
                error!("get_shop api request error: {}", err);
                from_file_cache(&cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, shop_id) {
        Ok(shop) => {
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawShop {
                id: shop_id,
                name: CString::new(shop.name).unwrap_or_default().into_raw(),
                description: CString::new(shop.description)
                    .unwrap_or_default()
                    .into_raw(),
            })
        }
        Err(err) => {
            error!("get_shop_list failed. {}", err);
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            // TODO: also need to drop this CString once C++ is done reading it
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
    fn test_create_shop() {
        let mock = mock("POST", "/v1/shops")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "description": "description", "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let description = CString::new("description").unwrap().into_raw();
        let result = create_shop(api_url, api_key, name, description);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => {
                assert_eq!(raw_shop.id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.name).to_string_lossy() },
                    "name"
                );
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.description).to_string_lossy() },
                    "description"
                );
            }
            FFIResult::Err(error) => panic!("create_shop returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_create_shop_server_error() {
        let mock = mock("POST", "/v1/shops")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let description = CString::new("description").unwrap().into_raw();
        let result = create_shop(api_url, api_key, name, description);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => panic!("create_shop returned Ok result: {:#x?}", raw_shop),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }

    #[test]
    fn test_update_shop() {
        let mock = mock("PATCH", "/v1/shops/1")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "description": "description", "updated_at": "2020-08-19T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let description = CString::new("description").unwrap().into_raw();
        let result = update_shop(api_url, api_key, 1, name, description);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => {
                assert_eq!(raw_shop.id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.name).to_string_lossy() },
                    "name"
                );
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.description).to_string_lossy() },
                    "description"
                );
            }
            FFIResult::Err(error) => panic!("update_shop returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_update_shop_server_error() {
        let mock = mock("PATCH", "/v1/shops/1")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let description = CString::new("description").unwrap().into_raw();
        let result = update_shop(api_url, api_key, 1, name, description);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => panic!("update_shop returned Ok result: {:#x?}", raw_shop),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }

    #[test]
    fn test_get_shop() {
        let mock = mock("GET", "/v1/shops/1")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "description": "description", "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_shop(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => {
                assert_eq!(raw_shop.id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.name).to_string_lossy() },
                    "name"
                );
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_shop.description).to_string_lossy() },
                    "description"
                );
            }
            FFIResult::Err(error) => panic!("get_shop returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_get_shop_server_error() {
        let mock = mock("GET", "/v1/shops/1")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_shop(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => panic!("get_shop returned Ok result: {:#x?}", raw_shop),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "EOF while parsing a value at line 1 column 0" // empty tempfile
                );
            }
        }
    }
}
