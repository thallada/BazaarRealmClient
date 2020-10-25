use std::{ffi::CStr, ffi::CString, os::raw::c_char, slice};

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
pub struct MerchandiseList {
    pub id: Option<i32>,
    pub shop_id: i32,
    pub form_list: Vec<Merchandise>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Merchandise {
    pub mod_name: String,
    pub local_form_id: u32,
    pub name: String,
    pub quantity: u32,
    pub form_type: u32,
    pub is_food: bool,
    pub price: u32,
}

impl MerchandiseList {
    pub fn from_game(shop_id: i32, merch_records: &[RawMerchandise]) -> Self {
        Self {
            id: None,
            shop_id,
            form_list: merch_records
                .iter()
                .map(|rec| Merchandise {
                    mod_name: unsafe { CStr::from_ptr(rec.mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    local_form_id: rec.local_form_id,
                    name: unsafe { CStr::from_ptr(rec.name) }
                        .to_string_lossy()
                        .to_string(),
                    quantity: rec.quantity,
                    form_type: rec.form_type,
                    is_food: rec.is_food == 1,
                    price: rec.price,
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawMerchandise {
    pub mod_name: *const c_char,
    pub local_form_id: u32,
    pub name: *const c_char,
    pub quantity: u32,
    pub form_type: u32,
    pub is_food: u8,
    pub price: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawMerchandiseVec {
    pub ptr: *mut RawMerchandise,
    pub len: usize,
    pub cap: usize,
}

#[no_mangle]
pub extern "C" fn create_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    raw_merchandise_ptr: *const RawMerchandise,
    raw_merchandise_len: usize,
) -> FFIResult<i32> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("create_merchandise_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_merchandise_len: {:?}", api_url, api_key, shop_id, raw_merchandise_len);
    let raw_merchandise_slice = unsafe {
        assert!(!raw_merchandise_ptr.is_null());
        slice::from_raw_parts(raw_merchandise_ptr, raw_merchandise_len)
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_merchandise_slice: &[RawMerchandise],
    ) -> Result<MerchandiseList> {
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
            .json(&merchandise_list)
            .send()?;
        info!("create merchandise_list response from api: {:?}", &resp);
        let bytes = resp.bytes()?;
        let json: MerchandiseList = serde_json::from_slice(&bytes)?;
        if let Some(id) = json.id {
            update_file_cache(
                &file_cache_dir(api_url)?.join(format!("merchandise_list_{}.json", id)),
                &bytes,
            )?;
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, shop_id, raw_merchandise_slice) {
        Ok(merchandise_list) => {
            if let Some(id) = merchandise_list.id {
                FFIResult::Ok(id)
            } else {
                error!(
                    "create_merchandise failed. API did not return an interior ref list with an ID"
                );
                let err_string =
                    CString::new("API did not return an interior ref list with an ID".to_string())
                        .expect("could not create CString")
                        .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("create_merchandise_list failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
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
) -> FFIResult<i32> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("create_merchandise_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_merchandise_len: {:?}", api_url, api_key, shop_id, raw_merchandise_len);
    let raw_merchandise_slice = unsafe {
        assert!(!raw_merchandise_ptr.is_null());
        slice::from_raw_parts(raw_merchandise_ptr, raw_merchandise_len)
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_merchandise_slice: &[RawMerchandise],
    ) -> Result<MerchandiseList> {
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
            .json(&merchandise_list)
            .send()?;
        info!("update merchandise_list response from api: {:?}", &resp);
        let bytes = resp.bytes()?;
        let json: MerchandiseList = serde_json::from_slice(&bytes)?;
        if let Some(id) = json.id {
            update_file_cache(
                &file_cache_dir(api_url)?.join(format!("shops_{}_merchandise_list.json", id)),
                &bytes,
            )?;
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, shop_id, raw_merchandise_slice) {
        Ok(merchandise_list) => {
            if let Some(id) = merchandise_list.id {
                FFIResult::Ok(id)
            } else {
                error!(
                    "update_merchandise failed. API did not return an interior ref list with an ID"
                );
                let err_string =
                    CString::new("API did not return an interior ref list with an ID".to_string())
                        .expect("could not create CString")
                        .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
        Err(err) => {
            error!("update_merchandise_list failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

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

    fn inner(api_url: &str, api_key: &str, merchandise_list_id: i32) -> Result<MerchandiseList> {
        #[cfg(not(test))]
        let url =
            Url::parse(api_url)?.join(&format!("v1/merchandise_lists/{}", merchandise_list_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/merchandise_lists/{}", merchandise_list_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_path =
            file_cache_dir(api_url)?.join(format!("merchandise_list_{}.json", merchandise_list_id));

        match client.get(url).header("Api-Key", api_key).send() {
            Ok(resp) => {
                info!("get_merchandise_list response from api: {:?}", &resp);
                if resp.status().is_success() {
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
                error!("get_merchandise_list api request error: {}", err);
                from_file_cache(&cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, merchandise_list_id) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| RawMerchandise {
                    mod_name: CString::new(merchandise.mod_name)
                        .unwrap_or_default()
                        .into_raw(),
                    local_form_id: merchandise.local_form_id,
                    name: CString::new(merchandise.name)
                        .unwrap_or_default()
                        .into_raw(),
                    quantity: merchandise.quantity,
                    form_type: merchandise.form_type,
                    is_food: merchandise.is_food as u8,
                    price: merchandise.price,
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

    fn inner(api_url: &str, api_key: &str, shop_id: i32) -> Result<MerchandiseList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}/merchandise_list", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/shops/{}/merchandise_list", shop_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_path =
            file_cache_dir(api_url)?.join(format!("shops_{}_merchandise_list.json", shop_id));

        match client.get(url).header("Api-Key", api_key).send() {
            Ok(resp) => {
                info!(
                    "get_merchandise_list_by_shop_id response from api: {:?}",
                    &resp
                );
                if resp.status().is_success() {
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
                error!("get_merchandise_list_by_shop_id api request error: {}", err);
                from_file_cache(&cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, shop_id) {
        Ok(merchandise_list) => {
            let (ptr, len, cap) = merchandise_list
                .form_list
                .into_iter()
                .map(|merchandise| RawMerchandise {
                    mod_name: CString::new(merchandise.mod_name)
                        .unwrap_or_default()
                        .into_raw(),
                    local_form_id: merchandise.local_form_id,
                    name: CString::new(merchandise.name)
                        .unwrap_or_default()
                        .into_raw(),
                    quantity: merchandise.quantity,
                    form_type: merchandise.form_type,
                    is_food: merchandise.is_food as u8,
                    price: merchandise.price,
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
    use mockito::mock;

    #[test]
    fn test_create_merchandise_list() {
        let mock = mock("POST", "/v1/merchandise_lists")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "shop_id": 1, "form_list": [], "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: 0,
            price: 100,
        }]
        .into_raw_parts();
        let result = create_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(merchandise_list_id) => {
                assert_eq!(merchandise_list_id, 1);
            }
            FFIResult::Err(error) => {
                panic!("create_merchandise_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_create_interior_ref_list_server_error() {
        let mock = mock("POST", "/v1/merchandise_lists")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: 0,
            price: 100,
        }]
        .into_raw_parts();
        let result = create_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(merchandise_list_id) => panic!(
                "create_merchandise_list returned Ok result: {:?}",
                merchandise_list_id
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }

    #[test]
    fn test_update_merchandise_list() {
        let mock = mock("PATCH", "/v1/shops/1/merchandise_list")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "shop_id": 1, "form_list": [], "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: 0,
            price: 100,
        }]
        .into_raw_parts();
        let result = update_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(merchandise_list_id) => {
                assert_eq!(merchandise_list_id, 1);
            }
            FFIResult::Err(error) => {
                panic!("update_merchandise_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_update_interior_ref_list_server_error() {
        let mock = mock("PATCH", "/v1/shops/1/merchandise_list")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (ptr, len, _cap) = vec![RawMerchandise {
            mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            local_form_id: 1,
            name: CString::new("Iron Sword").unwrap().into_raw(),
            quantity: 1,
            form_type: 1,
            is_food: 0,
            price: 100,
        }]
        .into_raw_parts();
        let result = update_merchandise_list(api_url, api_key, 1, ptr, len);
        mock.assert();
        match result {
            FFIResult::Ok(merchandise_list_id) => panic!(
                "update_merchandise_list returned Ok result: {:?}",
                merchandise_list_id
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "expected value at line 1 column 1"
                );
            }
        }
    }
    #[test]
    fn test_get_merchandise_list() {
        let mock = mock("GET", "/v1/merchandise_lists/1")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "created_at": "2020-08-18T00:00:00.000",
                "id": 1,
                "shop_id": 1,
                "form_list": [
                    {
                        "mod_name": "Skyrim.esm",
                        "local_form_id": 1,
                        "name": "Iron Sword",
                        "quantity": 1,
                        "form_type": 1,
                        "is_food": false,
                        "price": 100
                    }
                ],
                "updated_at": "2020-08-18T00:00:00.000"
            }"#,
            )
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                let raw_merchandise_slice = unsafe {
                    assert!(!raw_merchandise_vec.ptr.is_null());
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
                assert_eq!(raw_merchandise.is_food, 0);
                assert_eq!(raw_merchandise.price, 100);
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
                    "EOF while parsing a value at line 1 column 0" // empty tempfile
                );
            }
        }
    }

    #[test]
    fn test_get_merchandise_list_by_shop_id() {
        let mock = mock("GET", "/v1/shops/1/merchandise_list")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "created_at": "2020-08-18T00:00:00.000",
                "id": 1,
                "shop_id": 1,
                "form_list": [
                    {
                        "mod_name": "Skyrim.esm",
                        "local_form_id": 1,
                        "name": "Iron Sword",
                        "quantity": 1,
                        "form_type": 1,
                        "is_food": false,
                        "price": 100
                    }
                ],
                "updated_at": "2020-08-18T00:00:00.000"
            }"#,
            )
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_merchandise_list_by_shop_id(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_merchandise_vec) => {
                assert_eq!(raw_merchandise_vec.len, 1);
                let raw_merchandise_slice = unsafe {
                    assert!(!raw_merchandise_vec.ptr.is_null());
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
                assert_eq!(raw_merchandise.is_food, 0);
                assert_eq!(raw_merchandise.price, 100);
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
                    "EOF while parsing a value at line 1 column 0" // empty tempfile
                );
            }
        }
    }
}
