use std::{convert::TryFrom, ffi::CStr, ffi::CString, os::raw::c_char};

use anyhow::{anyhow, Result};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    cache::file_cache_dir, cache::from_file_cache, cache::load_metadata_from_file_cache,
    cache::update_file_caches, log_server_error, result::FFIResult,
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

impl From<RawShop> for Shop {
    fn from(raw_shop: RawShop) -> Self {
        Self {
            id: Some(raw_shop.id),
            name: unsafe { CStr::from_ptr(raw_shop.name) }
                .to_string_lossy()
                .to_string(),
            description: unsafe { CStr::from_ptr(raw_shop.description) }
                .to_string_lossy()
                .to_string(),
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

impl TryFrom<Shop> for RawShop {
    type Error = anyhow::Error;

    fn try_from(shop: Shop) -> Result<Self> {
        if let Some(id) = shop.id {
            Ok(Self {
                id,
                name: CString::new(shop.name).unwrap_or_default().into_raw(),
                description: CString::new(shop.description)
                    .unwrap_or_default()
                    .into_raw(),
            })
        } else {
            Err(anyhow!("shop.id is None"))
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawShopVec {
    pub ptr: *mut RawShop,
    pub len: usize,
    pub cap: usize,
}

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

        let cache_dir = file_cache_dir(api_url)?;
        let headers = resp.headers().clone();
        let bytes = resp.bytes()?;
        let json: Shop = serde_json::from_slice(&bytes)?;
        if let Some(id) = json.id {
            let body_cache_path = cache_dir.join(format!("shop_{}.json", id));
            let metadata_cache_path = cache_dir.join(format!("shop_{}_metadata.json", id));
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, &name, &description) {
        Ok(shop) => {
            info!("create_shop successful");
            if let Ok(raw_shop) = RawShop::try_from(shop) {
                FFIResult::Ok(raw_shop)
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

        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}.json", id));
        let metadata_cache_path = cache_dir.join(format!("shop_{}_metadata.json", id));
        let headers = resp.headers().clone();
        let bytes = resp.bytes()?;
        let json: Shop = serde_json::from_slice(&bytes)?;
        update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
        Ok(json)
    }

    match inner(&api_url, &api_key, id, &name, &description) {
        Ok(shop) => {
            info!("update_shop successful");
            if let Ok(raw_shop) = RawShop::try_from(shop) {
                FFIResult::Ok(raw_shop)
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
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}.json", shop_id));
        let metadata_cache_path = cache_dir.join(format!("shop_{}_metadata.json", shop_id));
        let mut request = client.get(url).header("Api-Key", api_key);
        // TODO: load metadata from in-memory LRU cache first before trying to load from file
        if let Ok(metadata) = load_metadata_from_file_cache(&metadata_cache_path) {
            if let Some(etag) = metadata.etag {
                request = request.header("If-None-Match", etag);
            }
        }

        match request.send() {
            Ok(resp) => {
                info!("get_shop response from api: {:?}", &resp);
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let json = serde_json::from_slice(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(json)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!("get_shop api request error: {}", err);
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, shop_id) {
        Ok(shop) => {
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            if let Ok(raw_shop) = RawShop::try_from(shop) {
                FFIResult::Ok(raw_shop)
            } else {
                error!("get_shop failed. API did not return a shop with an ID");
                let err_string = CString::new("API did not return a shop with an ID".to_string())
                    .expect("could not create CString")
                    .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
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

#[no_mangle]
pub extern "C" fn list_shops(
    api_url: *const c_char,
    api_key: *const c_char,
) -> FFIResult<RawShopVec> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("list_shops api_url: {:?}, api_key: {:?}", api_url, api_key);

    fn inner(api_url: &str, api_key: &str) -> Result<Vec<Shop>> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/shops?limit=128")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/shops?limit=128")?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join("shops.json");
        let metadata_cache_path = cache_dir.join("shops_metadata.json");
        let mut request = client.get(url).header("Api-Key", api_key);
        // TODO: load metadata from in-memory LRU cache first before trying to load from file
        if let Ok(metadata) = load_metadata_from_file_cache(&metadata_cache_path) {
            if let Some(etag) = metadata.etag {
                request = request.header("If-None-Match", etag);
            }
        }

        match request.send() {
            Ok(resp) => {
                info!("list_shops response from api: {:?}", &resp);
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let json = serde_json::from_slice(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(json)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!("list_shops api request error: {}", err);
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key) {
        Ok(shops) => {
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            let raw_shops: Result<Vec<RawShop>> =
                shops.into_iter().map(RawShop::try_from).collect();
            if let Ok(raw_shops) = raw_shops {
                let (ptr, len, cap) = raw_shops.into_raw_parts();
                FFIResult::Ok(RawShopVec { ptr, len, cap })
            } else {
                error!("list_shops failed. API returned one or more shops with no ID");
                let err_string =
                    CString::new("API returned one or more shops with no ID".to_string())
                        .expect("could not create CString")
                        .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("list_shops failed. {}", err);
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
    use std::{ffi::CString, slice};

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

    #[test]
    fn test_list_shops() {
        let mock = mock("GET", "/v1/shops?limit=128")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"[{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "description": "description", "updated_at": "2020-08-18T00:00:00.000" }]"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = list_shops(api_url, api_key);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shops_vec) => {
                assert_eq!(raw_shops_vec.len, 1);
                let raw_shops_slice = unsafe {
                    assert!(!raw_shops_vec.ptr.is_null());
                    slice::from_raw_parts(raw_shops_vec.ptr, raw_shops_vec.len)
                };
                let raw_shop = &raw_shops_slice[0];
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
            FFIResult::Err(error) => panic!("list_shops returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_list_shops_server_error() {
        let mock = mock("GET", "/v1/shops?limit=128")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = list_shops(api_url, api_key);
        mock.assert();
        match result {
            FFIResult::Ok(raw_shop) => panic!("list_shops returned Ok result: {:#x?}", raw_shop),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "EOF while parsing a value at line 1 column 0" // empty tempfile
                );
            }
        }
    }
}
