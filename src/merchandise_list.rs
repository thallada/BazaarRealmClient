use std::{ffi::CStr, ffi::CString, os::raw::c_char, slice};

use anyhow::Result;
use chrono::NaiveDateTime;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    cache::file_cache_dir, cache::from_file_cache, cache::load_metadata_from_file_cache,
    cache::update_file_caches, error::extract_error_from_response, log_server_error,
    result::FFIResult,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MerchandiseList {
    pub shop_id: i32,
    pub owner_id: Option<i32>,
    pub form_list: Vec<Merchandise>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Merchandise {
    pub mod_name: String,
    pub local_form_id: u32,
    pub name: String,
    pub quantity: u32,
    pub form_type: u32,
    pub is_food: bool,
    pub price: u32,
    pub keywords: Vec<String>,
}

impl MerchandiseList {
    pub fn from_game(shop_id: i32, merch_records: &[RawMerchandise]) -> Self {
        info!("MerchandiseList::from_game shop_id: {:?}", shop_id);
        Self {
            shop_id,
            owner_id: None,
            form_list: merch_records
                .iter()
                .map(|rec| {
                    info!("MerchandiseList::from_game local_form_id: {:?} keywords_len: {:?} keywords.is_null(): {:?}", rec.local_form_id, rec.keywords_len, rec.keywords.is_null());
                    Merchandise {
                        mod_name: unsafe { CStr::from_ptr(rec.mod_name) }
                            .to_string_lossy()
                            .to_string(),
                        local_form_id: rec.local_form_id,
                        name: unsafe { CStr::from_ptr(rec.name) }
                            .to_string_lossy()
                            .to_string(),
                        quantity: rec.quantity,
                        form_type: rec.form_type,
                        is_food: rec.is_food,
                        price: rec.price,
                        keywords: match rec.keywords.is_null() {
                            true => vec![],
                            false => unsafe { slice::from_raw_parts(rec.keywords, rec.keywords_len) }
                                .iter()
                                .map(|&keyword| {
                                    unsafe { CStr::from_ptr(keyword) }
                                        .to_string_lossy()
                                        .to_string()
                                })
                                .collect(),
                        }
                    }
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedMerchandiseList {
    pub id: i32,
    pub shop_id: i32,
    pub owner_id: i32,
    pub form_list: Vec<Merchandise>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawMerchandise {
    pub mod_name: *const c_char,
    pub local_form_id: u32,
    pub name: *const c_char,
    pub quantity: u32,
    pub form_type: u32,
    pub is_food: bool,
    pub price: u32,
    pub keywords: *mut *const c_char,
    pub keywords_len: usize,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawMerchandiseVec {
    pub ptr: *mut RawMerchandise,
    pub len: usize,
    pub cap: usize,
}

// TODO: delete me if unused
#[no_mangle]
pub extern "C" fn create_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    raw_merchandise_ptr: *const RawMerchandise,
    raw_merchandise_len: usize,
) -> FFIResult<RawMerchandiseVec> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("create_merchandise_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_merchandise_len: {:?}, raw_merchandise_ptr: {:?}", api_url, api_key, shop_id, raw_merchandise_len, raw_merchandise_ptr);
    let raw_merchandise_slice = match raw_merchandise_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_merchandise_ptr, raw_merchandise_len) },
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_merchandise_slice: &[RawMerchandise],
    ) -> Result<SavedMerchandiseList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/merchandise_lists")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/merchandise_lists")?;

        let merchandise_list = MerchandiseList::from_game(shop_id, raw_merchandise_slice);
        info!(
            "created merchandise_list from game: shop_id: {}",
            &merchandise_list.shop_id
        );
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key)
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&merchandise_list)?)
            .send()?;
        info!("create merchandise_list response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_merchandise_list: SavedMerchandiseList = bincode::deserialize(&bytes)?;
            let body_cache_path = cache_dir.join(format!(
                "merchandise_list_{}.bin",
                saved_merchandise_list.id
            ));
            let metadata_cache_path = cache_dir.join(format!(
                "merchandise_list_{}_metadata.json",
                saved_merchandise_list.id
            ));
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_merchandise_list)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url, &api_key, shop_id, raw_merchandise_slice) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| {
                    let (keywords_ptr, keywords_len, _) = merchandise
                        .keywords
                        .into_iter()
                        .map(|keyword| {
                            CString::new(keyword).unwrap_or_default().into_raw() as *const c_char
                        })
                        .collect::<Vec<*const c_char>>()
                        .into_raw_parts();
                    RawMerchandise {
                        mod_name: CString::new(merchandise.mod_name)
                            .unwrap_or_default()
                            .into_raw(),
                        local_form_id: merchandise.local_form_id,
                        name: CString::new(merchandise.name)
                            .unwrap_or_default()
                            .into_raw(),
                        quantity: merchandise.quantity,
                        form_type: merchandise.form_type,
                        is_food: merchandise.is_food,
                        price: merchandise.price,
                        keywords: keywords_ptr,
                        keywords_len: keywords_len,
                    }
                })
                .collect::<Vec<RawMerchandise>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawMerchandiseVec { ptr, len, cap })
        }
        Err(err) => {
            error!("create_merchandise_list failed. {}", err);
            // TODO: how to do error handling?
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            // TODO: also need to drop this CString once C++ is done reading it
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn update_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    raw_merchandise_ptr: *const RawMerchandise,
    raw_merchandise_len: usize,
) -> FFIResult<RawMerchandiseVec> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("update_merchandise_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_merchandise_len: {:?}, raw_merchandise_ptr: {:?}", api_url, api_key, shop_id, raw_merchandise_len, raw_merchandise_ptr);
    let raw_merchandise_slice = match raw_merchandise_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_merchandise_ptr, raw_merchandise_len) },
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_merchandise_slice: &[RawMerchandise],
    ) -> Result<SavedMerchandiseList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}/merchandise_list", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/shops/{}/merchandise_list", shop_id))?;

        let merchandise_list = MerchandiseList::from_game(shop_id, raw_merchandise_slice);
        info!(
            "created merchandise_list from game: shop_id: {}",
            &merchandise_list.shop_id
        );
        let client = reqwest::blocking::Client::new();
        let resp = client
            .patch(url)
            .header("Api-Key", api_key)
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&merchandise_list)?)
            .send()?;
        info!("update merchandise_list response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}_merchandise_list.bin", shop_id));
        let metadata_cache_path =
            cache_dir.join(format!("shop_{}_merchandise_list_metadata.json", shop_id));
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_merchandise_list = bincode::deserialize(&bytes)?;
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_merchandise_list)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url, &api_key, shop_id, raw_merchandise_slice) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| {
                    let (keywords_ptr, keywords_len, _) = merchandise
                        .keywords
                        .into_iter()
                        .map(|keyword| {
                            CString::new(keyword).unwrap_or_default().into_raw() as *const c_char
                        })
                        .collect::<Vec<*const c_char>>()
                        .into_raw_parts();
                    RawMerchandise {
                        mod_name: CString::new(merchandise.mod_name)
                            .unwrap_or_default()
                            .into_raw(),
                        local_form_id: merchandise.local_form_id,
                        name: CString::new(merchandise.name)
                            .unwrap_or_default()
                            .into_raw(),
                        quantity: merchandise.quantity,
                        form_type: merchandise.form_type,
                        is_food: merchandise.is_food,
                        price: merchandise.price,
                        keywords: keywords_ptr,
                        keywords_len: keywords_len,
                    }
                })
                .collect::<Vec<RawMerchandise>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawMerchandiseVec { ptr, len, cap })
        }
        Err(err) => {
            error!("update_merchandise_list failed. {}", err);
            // TODO: how to do error handling?
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            // TODO: also need to drop this CString once C++ is done reading it
            FFIResult::Err(err_string)
        }
    }
}

// TODO: delete me if unused
#[no_mangle]
pub extern "C" fn get_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    merchandise_list_id: i32,
) -> FFIResult<RawMerchandiseVec> {
    info!("get_merchandise_list begin");
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!(
        "get_merchandise_list api_url: {:?}, api_key: {:?}, merchandise_list_id: {:?}",
        api_url, api_key, merchandise_list_id
    );

    fn inner(
        api_url: &str,
        api_key: &str,
        merchandise_list_id: i32,
    ) -> Result<SavedMerchandiseList> {
        #[cfg(not(test))]
        let url =
            Url::parse(api_url)?.join(&format!("v1/merchandise_lists/{}", merchandise_list_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/merchandise_lists/{}", merchandise_list_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path =
            cache_dir.join(format!("merchandise_list_{}.bin", merchandise_list_id));
        let metadata_cache_path = cache_dir.join(format!(
            "merchandise_list_{}_metadata.json",
            merchandise_list_id
        ));
        let mut request = client
            .get(url)
            .header("Api-Key", api_key)
            .header("Accept", "application/octet-stream");
        // TODO: load metadata from in-memory LRU cache first before trying to load from file
        if let Ok(metadata) = load_metadata_from_file_cache(&metadata_cache_path) {
            if let Some(etag) = metadata.etag {
                request = request.header("If-None-Match", etag);
            }
        }

        match request.send() {
            Ok(resp) => {
                info!("get_merchandise_list response from api: {:?}", &resp);
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let saved_merchandise_list = bincode::deserialize(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(saved_merchandise_list)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!("get_merchandise_list api request error: {}", err);
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, merchandise_list_id) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| {
                    let (keywords_ptr, keywords_len, _) = merchandise
                        .keywords
                        .into_iter()
                        .map(|keyword| {
                            CString::new(keyword).unwrap_or_default().into_raw() as *const c_char
                        })
                        .collect::<Vec<*const c_char>>()
                        .into_raw_parts();
                    RawMerchandise {
                        mod_name: CString::new(merchandise.mod_name)
                            .unwrap_or_default()
                            .into_raw(),
                        local_form_id: merchandise.local_form_id,
                        name: CString::new(merchandise.name)
                            .unwrap_or_default()
                            .into_raw(),
                        quantity: merchandise.quantity,
                        form_type: merchandise.form_type,
                        is_food: merchandise.is_food,
                        price: merchandise.price,
                        keywords: keywords_ptr,
                        keywords_len: keywords_len,
                    }
                })
                .collect::<Vec<RawMerchandise>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawMerchandiseVec { ptr, len, cap })
        }
        Err(err) => {
            error!("merchandise_list failed. {}", err);
            // TODO: how to do error handling?
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            // TODO: also need to drop this CString once C++ is done reading it
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn get_merchandise_list_by_shop_id(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
) -> FFIResult<RawMerchandiseVec> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!(
        "get_merchandise_list_by_shop_id api_url: {:?}, api_key: {:?}, shop_id: {:?}",
        api_url, api_key, shop_id
    );

    fn inner(api_url: &str, api_key: &str, shop_id: i32) -> Result<SavedMerchandiseList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}/merchandise_list", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/shops/{}/merchandise_list", shop_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}_merchandise_list.bin", shop_id));
        let metadata_cache_path =
            cache_dir.join(format!("shop_{}_merchandise_list_metadata.json", shop_id));
        let mut request = client
            .get(url)
            .header("Api-Key", api_key)
            .header("Accept", "application/octet-stream");
        // TODO: load metadata from in-memory LRU cache first before trying to load from file
        if let Ok(metadata) = load_metadata_from_file_cache(&metadata_cache_path) {
            if let Some(etag) = metadata.etag {
                request = request.header("If-None-Match", etag);
            }
        }

        match request.send() {
            Ok(resp) => {
                info!(
                    "get_merchandise_list_by_shop_id response from api: {:?}",
                    &resp
                );
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let saved_merchandise_list = bincode::deserialize(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(saved_merchandise_list)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!("get_merchandise_list_by_shop_id api request error: {}", err);
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, shop_id) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| {
                    let (keywords_ptr, keywords_len, _) = merchandise
                        .keywords
                        .into_iter()
                        .map(|keyword| {
                            CString::new(keyword).unwrap_or_default().into_raw() as *const c_char
                        })
                        .collect::<Vec<*const c_char>>()
                        .into_raw_parts();
                    RawMerchandise {
                        mod_name: CString::new(merchandise.mod_name)
                            .unwrap_or_default()
                            .into_raw(),
                        local_form_id: merchandise.local_form_id,
                        name: CString::new(merchandise.name)
                            .unwrap_or_default()
                            .into_raw(),
                        quantity: merchandise.quantity,
                        form_type: merchandise.form_type,
                        is_food: merchandise.is_food,
                        price: merchandise.price,
                        keywords: keywords_ptr,
                        keywords_len: keywords_len,
                    }
                })
                .collect::<Vec<RawMerchandise>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawMerchandiseVec { ptr, len, cap })
        }
        Err(err) => {
            error!("get_merchandise_list_by_shop_id failed. {}", err);
            // TODO: how to do error handling?
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
    use chrono::Utc;
    use mockito::mock;

    #[test]
    fn test_create_merchandise_list() {
        let example = SavedMerchandiseList {
            id: 1,
            shop_id: 1,
            owner_id: 1,
            form_list: vec![Merchandise {
                mod_name: "Skyrim.esm".to_string(),
                local_form_id: 1,
                name: "Iron Sword".to_string(),
                quantity: 1,
                form_type: 1,
                is_food: false,
                price: 100,
                keywords: vec!["VendorItemWeapon".to_string()],
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("POST", "/v1/merchandise_lists")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: false,
            price: 100,
            keywords,
            keywords_len,
        }]
        .into_raw_parts();
        let result = create_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                assert!(!raw_merchandise_vec.ptr.is_null());
                let raw_merchandise_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise_vec.ptr, raw_merchandise_vec.len)
                };
                let raw_merchandise = &raw_merchandise_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_merchandise.local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.name) }
                        .to_string_lossy()
                        .to_string(),
                    "Iron Sword".to_string(),
                );
                assert_eq!(raw_merchandise.quantity, 1);
                assert_eq!(raw_merchandise.form_type, 1);
                assert_eq!(raw_merchandise.is_food, false);
                assert_eq!(raw_merchandise.price, 100);
            }
            FFIResult::Err(error) => {
                panic!("create_merchandise_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_create_merchandise_list_server_error() {
        let mock = mock("POST", "/v1/merchandise_lists")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: false,
            price: 100,
            keywords,
            keywords_len,
        }]
        .into_raw_parts();
        let result = create_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => panic!(
                "create_merchandise_list returned Ok result: {:#x?}",
                raw_merchandise_vec
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "Server 500: Internal Server Error"
                );
            }
        }
    }

    #[test]
    fn test_update_merchandise_list() {
        let example = SavedMerchandiseList {
            id: 1,
            shop_id: 1,
            owner_id: 1,
            form_list: vec![Merchandise {
                mod_name: "Skyrim.esm".to_string(),
                local_form_id: 1,
                name: "Iron Sword".to_string(),
                quantity: 1,
                form_type: 1,
                is_food: false,
                price: 100,
                keywords: vec!["VendorItemWeapon".to_string()],
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("PATCH", "/v1/shops/1/merchandise_list")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: false,
            price: 100,
            keywords,
            keywords_len,
        }]
        .into_raw_parts();
        let result = update_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                assert!(!raw_merchandise_vec.ptr.is_null());
                let raw_merchandise_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise_vec.ptr, raw_merchandise_vec.len)
                };
                let raw_merchandise = &raw_merchandise_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_merchandise.local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.name) }
                        .to_string_lossy()
                        .to_string(),
                    "Iron Sword".to_string(),
                );
                assert_eq!(raw_merchandise.quantity, 1);
                assert_eq!(raw_merchandise.form_type, 1);
                assert_eq!(raw_merchandise.is_food, false);
                assert_eq!(raw_merchandise.price, 100);
            }
            FFIResult::Err(error) => {
                panic!("update_merchandise_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_update_merchandise_list_server_error() {
        let mock = mock("PATCH", "/v1/shops/1/merchandise_list")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: false,
            price: 100,
            keywords,
            keywords_len,
        }]
        .into_raw_parts();
        let result = update_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => panic!(
                "update_merchandise_list returned Ok result: {:#x?}",
                raw_merchandise_vec
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "Server 500: Internal Server Error"
                );
            }
        }
    }
    #[test]
    fn test_get_merchandise_list() {
        let example = SavedMerchandiseList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            form_list: vec![Merchandise {
                mod_name: "Skyrim.esm".to_string(),
                local_form_id: 1,
                name: "Iron Sword".to_string(),
                quantity: 1,
                form_type: 1,
                is_food: false,
                price: 100,
                keywords: vec!["VendorItemWeapon".to_string()],
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("GET", "/v1/merchandise_lists/1")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                assert!(!raw_merchandise_vec.ptr.is_null());
                let raw_merchandise_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise_vec.ptr, raw_merchandise_vec.len)
                };
                let raw_merchandise = &raw_merchandise_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_merchandise.local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.name) }
                        .to_string_lossy()
                        .to_string(),
                    "Iron Sword".to_string(),
                );
                assert_eq!(raw_merchandise.quantity, 1);
                assert_eq!(raw_merchandise.form_type, 1);
                assert_eq!(raw_merchandise.is_food, false);
                assert_eq!(raw_merchandise.price, 100);
                assert!(!raw_merchandise.keywords.is_null());
                let keywords_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise.keywords, raw_merchandise.keywords_len)
                };
                assert_eq!(
                    unsafe { CStr::from_ptr(keywords_slice[0]) }
                        .to_string_lossy()
                        .to_string(),
                    "VendorItemWeapon".to_string(),
                );
            }
            FFIResult::Err(error) => panic!("get_merchandise_list returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_get_merchandise_list_server_error() {
        let mock = mock("GET", "/v1/merchandise_lists/1")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => panic!(
                "get_merchandise_list returned Ok result: {:#x?}",
                raw_merchandise_vec
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "io error: failed to fill whole buffer" // empty tempfile
                );
            }
        }
    }

    #[test]
    fn test_get_merchandise_list_by_shop_id() {
        let example = SavedMerchandiseList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            form_list: vec![Merchandise {
                mod_name: "Skyrim.esm".to_string(),
                local_form_id: 1,
                name: "Iron Sword".to_string(),
                quantity: 1,
                form_type: 1,
                is_food: false,
                price: 100,
                keywords: vec!["VendorItemWeapon".to_string()],
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("GET", "/v1/shops/1/merchandise_list")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemWeapon").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list_by_shop_id(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                assert!(!raw_merchandise_vec.ptr.is_null());
                let raw_merchandise_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise_vec.ptr, raw_merchandise_vec.len)
                };
                let raw_merchandise = &raw_merchandise_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_merchandise.local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_merchandise.name) }
                        .to_string_lossy()
                        .to_string(),
                    "Iron Sword".to_string(),
                );
                assert_eq!(raw_merchandise.quantity, 1);
                assert_eq!(raw_merchandise.form_type, 1);
                assert_eq!(raw_merchandise.is_food, false);
                assert_eq!(raw_merchandise.price, 100);
                assert!(!raw_merchandise.keywords.is_null());
                let keywords_slice = unsafe {
                    slice::from_raw_parts(raw_merchandise.keywords, raw_merchandise.keywords_len)
                };
                assert_eq!(
                    unsafe { CStr::from_ptr(keywords_slice[0]) }
                        .to_string_lossy()
                        .to_string(),
                    "VendorItemWeapon".to_string(),
                );
            }
            FFIResult::Err(error) => panic!(
                "get_merchandise_list_by_shop_id returned error: {:?}",
                unsafe { CStr::from_ptr(error).to_string_lossy() }
            ),
        }
    }

    #[test]
    fn test_get_merchandise_list_server_error_by_shop_id() {
        let mock = mock("GET", "/v1/shops/1/merchandise_list")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list_by_shop_id(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => panic!(
                "get_merchandise_list_by_shop_id returned Ok result: {:#x?}",
                raw_merchandise_vec
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "io error: failed to fill whole buffer" // empty tempfile
                );
            }
        }
    }
}
