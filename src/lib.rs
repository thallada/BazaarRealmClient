#![allow(non_snake_case)]
#![feature(vec_into_raw_parts)]
#![feature(unwind_attributes)]
#[macro_use]
extern crate lazy_static;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::slice;
use std::fs::{create_dir_all, File};
use std::io::BufReader;
use std::io::prelude::*;
use std::sync::RwLock;

use anyhow::{anyhow, Result};
use base64::{encode_config, URL_SAFE_NO_PAD};
use log::LevelFilter;
use reqwest::blocking::Response;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use url::Url;

#[cfg(test)]
use mockito;

#[cfg(not(test))]
use log::{error, info};

#[cfg(test)]
use std::{println as info, println as error};

mod result;
mod cache;

use cache::Cache;

const API_VERSION: &'static str = "v1";

fn file_cache_dir(api_url: &str) -> Result<PathBuf> {
    let encoded_url = encode_config(api_url, URL_SAFE_NO_PAD);
    let path = Path::new("Data/SKSE/Plugins/BazaarRealmCache").join(encoded_url).join(API_VERSION);
    create_dir_all(&path)?;
    Ok(path)
}

#[derive(Serialize, Deserialize, Debug)]
struct Owner {
    id: Option<i32>,
    name: String,
    api_key: Option<String>,
    mod_version: u32,
}

impl Owner {
    fn from_game(name: &str, api_key: &str, mod_version: u32) -> Self {
        Self {
            id: None,
            name: name.to_string(),
            api_key: Some(api_key.to_string()),
            mod_version,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Shop {
    id: Option<i32>,
    name: String,
    description: String,
}

impl Shop {
    fn from_game(name: &str, description: &str) -> Self {
        Self {
            id: None,
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct InteriorRef {
    base_mod_name: String,
    base_local_form_id: i32,
    ref_mod_name: Option<String>,
    ref_local_form_id: i32,
    position_x: f32,
    position_y: f32,
    position_z: f32,
    angle_x: f32,
    angle_y: f32,
    angle_z: f32,
    scale: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct InteriorRefList {
    id: Option<i32>,
    shop_id: i32,
    ref_list: Vec<InteriorRef>,
}

impl InteriorRefList {
    fn from_game(shop_id: i32, ref_records: &[RefRecord]) -> Self {
        Self {
            id: None,
            shop_id,
            ref_list: ref_records
                .iter()
                .map(|rec| InteriorRef {
                    base_mod_name: unsafe { CStr::from_ptr(rec.base_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    base_local_form_id: rec.base_local_form_id as i32,
                    ref_mod_name: match rec.ref_mod_name.is_null() {
                        true => None,
                        false => Some(
                            unsafe { CStr::from_ptr(rec.ref_mod_name) }
                                .to_string_lossy()
                                .to_string(),
                        ),
                    },
                    ref_local_form_id: rec.ref_local_form_id as i32,
                    position_x: rec.position_x,
                    position_y: rec.position_y,
                    position_z: rec.position_z,
                    angle_x: rec.angle_x,
                    angle_y: rec.angle_y,
                    angle_z: rec.angle_z,
                    scale: rec.scale,
                })
                .collect(),
        }
    }
}

#[repr(C, u8)]
pub enum FFIResult<T> {
    Ok(T),
    Err(*const c_char),
}

#[derive(Debug)]
#[repr(C)]
pub struct RefRecord {
    base_mod_name: *const c_char,
    base_local_form_id: u32,
    ref_mod_name: *const c_char,
    ref_local_form_id: u32,
    position_x: f32,
    position_y: f32,
    position_z: f32,
    angle_x: f32,
    angle_y: f32,
    angle_z: f32,
    scale: u16,
}

#[derive(Debug)]
#[repr(C)]
pub struct RefRecordVec {
    ptr: *mut RefRecord,
    len: usize,
    cap: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct Merchandise {
    mod_name: String,
    local_form_id: u32,
    name: String,
    quantity: u32,
    form_type: u32,
    is_food: bool,
    price: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct MerchandiseList {
    id: Option<i32>,
    shop_id: i32,
    form_list: Vec<Merchandise>,
}

impl MerchandiseList {
    fn from_game(shop_id: i32, merch_records: &[MerchRecord]) -> Self {
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
pub struct MerchRecord {
    mod_name: *const c_char,
    local_form_id: u32,
    name: *const c_char,
    quantity: u32,
    form_type: u32,
    is_food: u8,
    price: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct MerchRecordVec {
    ptr: *mut MerchRecord,
    len: usize,
    cap: usize,
}

// Required in order to store results in a thread-safe static cache.
// Rust complains that the raw pointers cannot be Send + Sync. We only ever:
// a) read the values in C++/Papyrus land, and it's okay if multiple threads do that.
// b) from_raw() the pointers back into rust values and then drop them. This could be problematic if another script is still reading at the same time, but I'm pretty sure that won't happen.
// Besides, it's already unsafe to read from a raw pointer
unsafe impl<T> Send for FFIResult<T> {}
unsafe impl Send for RefRecordVec {}
unsafe impl Send for RefRecord {}
unsafe impl Send for MerchRecordVec {}
unsafe impl Send for MerchRecord {}
unsafe impl<T> Sync for FFIResult<T> {}
unsafe impl Sync for RefRecordVec {}
unsafe impl Sync for RefRecord {}
unsafe impl Sync for MerchRecordVec {}
unsafe impl Sync for MerchRecord {}


#[no_mangle]
pub extern "C" fn init() {
    info!("init called");
    let mut log_dir = dirs::document_dir().expect("could not get Documents directory");
    log_dir.push(Path::new(
        r#"My Games\Skyrim Special Edition\SKSE\BazaarRealmClient.log"#,
    ));
    simple_logging::log_to_file(log_dir, LevelFilter::Info).unwrap();
}

#[no_mangle]
pub extern "C" fn status_check(api_url: *const c_char) -> bool {
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_url = api_url.to_string_lossy();

    match status_check_inner(&api_url) {
        Ok(resp) if resp.status() == 200 => {
            info!("status_check ok");
            true
        }
        Ok(resp) => {
            error!("status_check failed. Server error");
            log_server_error(resp);
            false
        }
        Err(err) => {
            error!("status_check failed. {}", err);
            false
        }
    }
}

fn status_check_inner(api_url: &str) -> Result<Response> {
    #[cfg(not(test))]
    let api_url = Url::parse(api_url)?.join("status")?;
    #[cfg(test)]
    let api_url = &mockito::server_url();

    Ok(reqwest::blocking::get(api_url)?)
}

#[no_mangle]
pub unsafe extern "C" fn generate_api_key() -> *mut c_char {
    info!("generate_api_key begin");
    // TODO: is leaking this CString bad?
    let uuid = CString::new(format!("{}", Uuid::new_v4()))
        .expect("could not create CString")
        .into_raw();
    info!("generate_api_key successful");
    uuid
}

// Because C++ does not have Result, -1 means that the request was unsuccessful
#[no_mangle]
pub extern "C" fn create_owner(
    api_url: *const c_char,
    api_key: *const c_char,
    name: *const c_char,
    mod_version: u32,
) -> i32 {
    info!("create_owner begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let name = unsafe { CStr::from_ptr(name) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    let name = name.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);
    info!("name: {:?}", name);
    info!("mod_version: {:?}", mod_version);
    match create_owner_inner(&api_url, &api_key, &name, mod_version) {
        Ok(owner) => {
            info!("create_owner successful");
            if let Some(id) = owner.id {
                id
            } else {
                -1
            }
        }
        Err(err) => {
            error!("create_owner failed. {}", err);
            -1
        }
    }
}

fn create_owner_inner(api_url: &str, api_key: &str, name: &str, mod_version: u32) -> Result<Owner> {
    #[cfg(not(test))]
    let url = Url::parse(api_url)?.join("v1/owners")?;
    #[cfg(test)]
    let url = &mockito::server_url();

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
            let mut file = File::create(file_cache_dir(api_url)?.join(format!("owner_{}.json", id)))?;
            file.write_all(&bytes.as_ref())?;
        }
        Ok(json)
    } else {
        Err(anyhow!("api-key not defined"))
    }
}

// Because C++ does not have Result, -1 means that the request was unsuccessful
#[no_mangle]
pub extern "C" fn create_shop(
    api_url: *const c_char,
    api_key: *const c_char,
    name: *const c_char,
    description: *const c_char,
) -> i32 {
    info!("create_shop begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let name = unsafe { CStr::from_ptr(name) };
    let description = unsafe { CStr::from_ptr(description) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    let name = name.to_string_lossy();
    let description = description.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);
    info!("name: {:?}", name);
    info!("description: {:?}", description);
    match create_shop_inner(&api_url, &api_key, &name, &description) {
        Ok(shop) => {
            info!("create_shop successful");
            if let Some(id) = shop.id {
                id
            } else {
                -1
            }
        }
        Err(err) => {
            error!("create_shop failed. {}", err);
            -1
        }
    }
}

fn create_shop_inner(api_url: &str, api_key: &str, name: &str, description: &str) -> Result<Shop> {
    #[cfg(not(test))]
    let url = Url::parse(api_url)?.join("v1/shops")?;
    #[cfg(test)]
    let url = &mockito::server_url();

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
        let mut file = File::create(file_cache_dir(api_url)?.join(format!("shop_{}.json", id)))?;
        file.write_all(&bytes.as_ref())?;
    }
    Ok(json)
}

// Because C++ does not have Result, -1 means that the request was unsuccessful
#[no_mangle]
pub extern "C" fn create_interior_ref_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    ref_records: *const RefRecord,
    ref_records_len: usize,
) -> i32 {
    info!("create_interior_ref_list begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);
    let ref_records_slice = unsafe {
        assert!(!ref_records.is_null());
        slice::from_raw_parts(ref_records, ref_records_len)
    };
    match create_interior_ref_list_inner(&api_url, &api_key, shop_id, ref_records_slice) {
        Ok(interior_ref_list) => {
            if let Some(id) = interior_ref_list.id {
                id
            } else {
                -1
            }
        }
        Err(err) => {
            error!("interior_ref_list failed. {}", err);
            -1
        }
    }
}

fn create_interior_ref_list_inner(
    api_url: &str,
    api_key: &str,
    shop_id: i32,
    ref_records: &[RefRecord],
) -> Result<InteriorRefList> {
    #[cfg(not(test))]
    let url = Url::parse(api_url)?.join("v1/interior_ref_lists")?;
    #[cfg(test)]
    let url = &mockito::server_url();

    let interior_ref_list = InteriorRefList::from_game(shop_id, ref_records);
    info!(
        "created interior_ref_list from game: shop_id: {}",
        &interior_ref_list.shop_id
    );
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(url)
        .header("Api-Key", api_key)
        .json(&interior_ref_list)
        .send()?;
    info!("create interior_ref_list response from api: {:?}", &resp);
    let bytes = resp.bytes()?;
    let json: InteriorRefList = serde_json::from_slice(&bytes)?;
    if let Some(id) = json.id {
        let mut file = File::create(file_cache_dir(api_url)?.join(format!("interior_ref_list_{}.json", id)))?;
        file.write_all(&bytes.as_ref())?;
    }
    Ok(json)
}

lazy_static! {
    // lazy_static! requires the static values to be thread-safe, so the caches need to be wrapped in a RwLock
    // I'm not sure if multiple C++ threads would be calling into these functions, but at least it should be safe if there are.
    // Note: not using this. Trying to avoid too many external function calls in Papyrus (which is really slow)
    static ref INTERIOR_REF_LIST_RESULT_CACHE: RwLock<Cache<FFIResult<RefRecordVec>>> = RwLock::new(Cache::new());
}

// TODO: fetch by shop_id
#[no_mangle]
#[unwind(allowed)]
pub extern "C" fn get_interior_ref_list(
    api_url: *const c_char,
    api_key: *const c_char,
    interior_ref_list_id: i32,
) -> FFIResult<RefRecordVec> {
    info!("get_interior_ref_list begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);

    #[unwind(allowed)]
    fn inner(
        api_url: &str,
        api_key: &str,
        interior_ref_list_id: i32,
    ) -> Result<InteriorRefList> {
        #[cfg(not(test))]
        let url =
            Url::parse(api_url)?.join(&format!("v1/interior_ref_lists/{}", interior_ref_list_id))?;
        #[cfg(test)]
        let url = &mockito::server_url();
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_path = file_cache_dir(api_url)?.join(format!("interior_ref_list_{}.json", interior_ref_list_id));

        fn from_file_cache(cache_path: &Path) -> Result<InteriorRefList> {
            let file = File::open(cache_path)?;
            let reader = BufReader::new(file);
            info!("get_interior_ref_list returning value from cache: {:?}", cache_path);
            Ok(serde_json::from_reader(reader)?)
        }

        match client.get(url).header("Api-Key", api_key).send() {
            Ok(resp) => {
                info!("get_interior_ref_list response from api: {:?}", &resp);
                if !resp.status().is_server_error() {
                    let mut file = File::create(&cache_path)?;
                    let bytes = resp.bytes()?;
                    file.write_all(&bytes.as_ref())?;
                    let json = serde_json::from_slice(&bytes)?;
                    Ok(json)
                } else {
                    from_file_cache(&cache_path)
                }
            }
            Err(err) => {
                error!("get_interior_ref_list api request error: {}", err);
                from_file_cache(&cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, interior_ref_list_id) {
        Ok(interior_ref_list) => {
            let (ptr, len, cap) = interior_ref_list
                .ref_list
                .into_iter()
                .map(|interior_ref| RefRecord {
                    base_mod_name: CString::new(interior_ref.base_mod_name)
                        .unwrap_or_default()
                        .into_raw(),
                    base_local_form_id: interior_ref.base_local_form_id as u32,
                    ref_mod_name: match interior_ref.ref_mod_name {
                        None => std::ptr::null(),
                        Some(ref_mod_name) => {
                            CString::new(ref_mod_name).unwrap_or_default().into_raw()
                        }
                    },
                    ref_local_form_id: interior_ref.ref_local_form_id as u32,
                    position_x: interior_ref.position_x,
                    position_y: interior_ref.position_y,
                    position_z: interior_ref.position_z,
                    angle_x: interior_ref.angle_x,
                    angle_y: interior_ref.angle_y,
                    angle_z: interior_ref.angle_z,
                    scale: interior_ref.scale,
                })
                .collect::<Vec<RefRecord>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RefRecordVec { ptr, len, cap })
        }
        Err(err) => {
            error!("interior_ref_list failed. {}", err);
            // TODO: how to do error handling?
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            // TODO: also need to drop this CString once C++ is done reading it
            FFIResult::Err(err_string)
        }
    }
}

// Because C++ does not have Result, -1 means that the request was unsuccessful
#[no_mangle]
pub extern "C" fn create_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    merch_records: *const MerchRecord,
    merch_records_len: usize,
) -> i32 {
    info!("create_merchandise_list begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);
    let merch_records_slice = unsafe {
        assert!(!merch_records.is_null());
        slice::from_raw_parts(merch_records, merch_records_len)
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        merch_records: &[MerchRecord],
    ) -> Result<MerchandiseList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/merchandise_lists")?;
        #[cfg(test)]
        let url = &mockito::server_url();

        let merchandise_list = MerchandiseList::from_game(shop_id, merch_records);
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
            let mut file = File::create(file_cache_dir(api_url)?.join(format!("merchandise_list_{}.json", id)))?;
            file.write_all(&bytes.as_ref())?;
        }
        Ok(json)
    }

    match inner(&api_url, &api_key, shop_id, merch_records_slice) {
        Ok(merchandise_list) => {
            if let Some(id) = merchandise_list.id {
                id
            } else {
                -1
            }
        }
        Err(err) => {
            error!("merchandise_list failed. {}", err);
            -1
        }
    }
}

// TODO: fetch by shop_id
#[no_mangle]
#[unwind(allowed)]
pub extern "C" fn get_merchandise_list(
    api_url: *const c_char,
    api_key: *const c_char,
    merchandise_list_id: i32,
) -> FFIResult<MerchRecordVec> {
    info!("get_merchandise_list begin");
    let api_url = unsafe { CStr::from_ptr(api_url) };
    let api_key = unsafe { CStr::from_ptr(api_key) };
    let api_url = api_url.to_string_lossy();
    let api_key = api_key.to_string_lossy();
    info!("api_url: {:?}", api_url);
    info!("api_key: {:?}", api_key);

    #[unwind(allowed)]
    fn inner(
        api_url: &str,
        api_key: &str,
        merchandise_list_id: i32,
    ) -> Result<MerchandiseList> {
        #[cfg(not(test))]
        let url =
            Url::parse(api_url)?.join(&format!("v1/merchandise_lists/{}", merchandise_list_id))?;
        #[cfg(test)]
        let url = &mockito::server_url();
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_path = file_cache_dir(api_url)?.join(format!("merchandise_list_{}.json", merchandise_list_id));

        fn from_file_cache(cache_path: &Path) -> Result<MerchandiseList> {
            let file = File::open(cache_path)?;
            let reader = BufReader::new(file);
            info!("get_merchandise_list returning value from cache: {:?}", cache_path);
            Ok(serde_json::from_reader(reader)?)
        }

        match client.get(url).header("Api-Key", api_key).send() {
            Ok(resp) => {
                info!("get_merchandise_list response from api: {:?}", &resp);
                if !resp.status().is_server_error() {
                    let mut file = File::create(&cache_path)?;
                    let bytes = resp.bytes()?;
                    file.write_all(&bytes.as_ref())?;
                    let json = serde_json::from_slice(&bytes)?;
                    Ok(json)
                } else {
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
                .map(|merchandise| MerchRecord {
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
                .collect::<Vec<MerchRecord>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(MerchRecordVec { ptr, len, cap })
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

fn log_server_error(resp: Response) {
    let status = resp.status();
    if let Ok(text) = resp.text() {
        error!("Server error: {} {}", status, text);
    }
    error!("Server error: {}", status);
}

#[no_mangle]
pub extern "C" fn free_string(ptr: *mut c_char) {
    unsafe { drop(CString::from_raw(ptr)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[test]
    fn test_status_check() {
        let _m = mock("GET", "/").with_status(200).create();

        let api_url = CString::new("url").unwrap().into_raw();
        assert_eq!(status_check(api_url), true);
    }

    #[test]
    fn test_status_check_server_error() {
        let _m = mock("GET", "/")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        assert_eq!(status_check(api_url), false);
    }

    #[test]
    fn test_create_owner() {
        let _m = mock("POST", "/")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "created_at": "2020-08-18T00:00:00.000", "id": 1, "name": "name", "mod_version": 1, "updated_at": "2020-08-18T00:00:00.000" }"#)
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        assert_eq!(create_owner(api_url, api_key, name, mod_version), 1);
    }

    #[test]
    fn test_create_owner_server_error() {
        let _m = mock("POST", "/")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let name = CString::new("name").unwrap().into_raw();
        let mod_version = 1;
        assert_eq!(create_owner(api_url, api_key, name, mod_version), -1);
    }
}
