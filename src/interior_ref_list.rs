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

#[derive(Serialize, Deserialize, Debug)]
pub struct InteriorRefList {
    pub shop_id: i32,
    pub owner_id: Option<i32>,
    pub ref_list: Vec<InteriorRef>,
    pub shelves: Vec<Shelf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InteriorRef {
    pub base_mod_name: String,
    pub base_local_form_id: u32,
    pub ref_mod_name: Option<String>,
    pub ref_local_form_id: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub angle_z: f32,
    pub scale: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Shelf {
    pub shelf_type: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub angle_z: f32,
    pub scale: u16,
    pub page: u32,
    pub filter_form_type: Option<u32>,
    pub filter_is_food: bool,
    pub search: Option<String>,
    pub sort_on: Option<String>,
    pub sort_asc: bool,
}

impl InteriorRefList {
    pub fn from_game(
        shop_id: i32,
        raw_interior_ref_slice: &[RawInteriorRef],
        raw_shelves_slice: &[RawShelf],
    ) -> Self {
        Self {
            shop_id,
            owner_id: None,
            ref_list: raw_interior_ref_slice
                .iter()
                .map(|rec| InteriorRef {
                    base_mod_name: unsafe { CStr::from_ptr(rec.base_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    base_local_form_id: rec.base_local_form_id,
                    ref_mod_name: match rec.ref_mod_name.is_null() {
                        true => None,
                        false => Some(
                            unsafe { CStr::from_ptr(rec.ref_mod_name) }
                                .to_string_lossy()
                                .to_string(),
                        ),
                    },
                    ref_local_form_id: rec.ref_local_form_id,
                    position_x: rec.position_x,
                    position_y: rec.position_y,
                    position_z: rec.position_z,
                    angle_x: rec.angle_x,
                    angle_y: rec.angle_y,
                    angle_z: rec.angle_z,
                    scale: rec.scale,
                })
                .collect(),
            shelves: raw_shelves_slice
                .iter()
                .map(|rec| Shelf {
                    shelf_type: rec.shelf_type,
                    position_x: rec.position_x,
                    position_y: rec.position_y,
                    position_z: rec.position_z,
                    angle_x: rec.angle_x,
                    angle_y: rec.angle_y,
                    angle_z: rec.angle_z,
                    scale: rec.scale,
                    page: rec.page,
                    filter_form_type: match rec.filter_form_type {
                        0 => None,
                        _ => Some(rec.filter_form_type),
                    },
                    filter_is_food: rec.filter_is_food,
                    search: match rec.search.is_null() {
                        true => None,
                        false => Some(
                            unsafe { CStr::from_ptr(rec.search) }
                                .to_string_lossy()
                                .to_string(),
                        ),
                    },
                    sort_on: match rec.sort_on.is_null() {
                        true => None,
                        false => Some(
                            unsafe { CStr::from_ptr(rec.sort_on) }
                                .to_string_lossy()
                                .to_string(),
                        ),
                    },
                    sort_asc: rec.sort_asc,
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavedInteriorRefList {
    pub id: i32,
    pub shop_id: i32,
    pub owner_id: i32,
    pub ref_list: Vec<InteriorRef>,
    pub shelves: Vec<Shelf>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawInteriorRef {
    pub base_mod_name: *const c_char,
    pub base_local_form_id: u32,
    pub ref_mod_name: *const c_char,
    pub ref_local_form_id: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub angle_z: f32,
    pub scale: u16,
}

impl From<InteriorRef> for RawInteriorRef {
    fn from(interior_ref: InteriorRef) -> Self {
        Self {
            base_mod_name: CString::new(interior_ref.base_mod_name)
                .unwrap_or_default()
                .into_raw(),
            base_local_form_id: interior_ref.base_local_form_id,
            ref_mod_name: match interior_ref.ref_mod_name {
                None => std::ptr::null(),
                Some(ref_mod_name) => CString::new(ref_mod_name).unwrap_or_default().into_raw(),
            },
            ref_local_form_id: interior_ref.ref_local_form_id,
            position_x: interior_ref.position_x,
            position_y: interior_ref.position_y,
            position_z: interior_ref.position_z,
            angle_x: interior_ref.angle_x,
            angle_y: interior_ref.angle_y,
            angle_z: interior_ref.angle_z,
            scale: interior_ref.scale,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawShelf {
    pub shelf_type: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub angle_z: f32,
    pub scale: u16,
    pub page: u32,
    pub filter_form_type: u32,
    pub filter_is_food: bool,
    pub search: *const c_char,
    pub sort_on: *const c_char,
    pub sort_asc: bool,
}

impl From<Shelf> for RawShelf {
    fn from(shelf: Shelf) -> Self {
        Self {
            shelf_type: shelf.shelf_type,
            position_x: shelf.position_x,
            position_y: shelf.position_y,
            position_z: shelf.position_z,
            angle_x: shelf.angle_x,
            angle_y: shelf.angle_y,
            angle_z: shelf.angle_z,
            scale: shelf.scale,
            page: shelf.page,
            filter_form_type: match shelf.filter_form_type {
                None => 0,
                Some(form_type) => form_type,
            },
            filter_is_food: shelf.filter_is_food,
            search: match shelf.search {
                None => std::ptr::null(),
                Some(search) => CString::new(search).unwrap_or_default().into_raw(),
            },
            sort_on: match shelf.sort_on {
                None => std::ptr::null(),
                Some(sort_on) => CString::new(sort_on).unwrap_or_default().into_raw(),
            },
            sort_asc: shelf.sort_asc,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawInteriorRefVec {
    pub ptr: *mut RawInteriorRef,
    pub len: usize,
    pub cap: usize,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawShelfVec {
    pub ptr: *mut RawShelf,
    pub len: usize,
    pub cap: usize,
}

#[derive(Debug)]
#[repr(C)]
pub struct RawInteriorRefData {
    pub interior_ref_vec: RawInteriorRefVec,
    pub shelf_vec: RawShelfVec,
}

// TODO: delete me if unused
#[no_mangle]
pub extern "C" fn create_interior_ref_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    raw_interior_ref_ptr: *const RawInteriorRef,
    raw_interior_ref_len: usize,
    raw_shelf_ptr: *const RawShelf,
    raw_shelf_len: usize,
) -> FFIResult<i32> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("create_interior_ref_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_interior_ref_len: {:?}, raw_shelf_len: {:?}", api_url, api_key, shop_id, raw_interior_ref_len, raw_shelf_len);
    let raw_interior_ref_slice = match raw_interior_ref_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_interior_ref_ptr, raw_interior_ref_len) },
    };
    let raw_shelf_slice = match raw_shelf_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_shelf_ptr, raw_shelf_len) },
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_interior_ref_slice: &[RawInteriorRef],
        raw_shelf_slice: &[RawShelf],
    ) -> Result<SavedInteriorRefList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/interior_ref_lists")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/interior_ref_lists")?;

        let interior_ref_list =
            InteriorRefList::from_game(shop_id, raw_interior_ref_slice, raw_shelf_slice);
        info!(
            "created interior_ref_list from game: shop_id: {}",
            &interior_ref_list.shop_id
        );
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key)
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&interior_ref_list)?)
            .send()?;
        info!("create interior_ref_list response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_interior_ref_list: SavedInteriorRefList = bincode::deserialize(&bytes)?;
            let body_cache_path = cache_dir.join(format!(
                "interior_ref_list_{}.bin",
                saved_interior_ref_list.id
            ));
            let metadata_cache_path = cache_dir.join(format!(
                "interior_ref_list_{}_metadata.json",
                saved_interior_ref_list.id
            ));
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_interior_ref_list)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(
        &api_url,
        &api_key,
        shop_id,
        raw_interior_ref_slice,
        raw_shelf_slice,
    ) {
        Ok(interior_ref_list) => FFIResult::Ok(interior_ref_list.id),
        Err(err) => {
            error!("create_interior_ref_list failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

#[no_mangle]
pub extern "C" fn update_interior_ref_list(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
    raw_interior_ref_ptr: *const RawInteriorRef,
    raw_interior_ref_len: usize,
    raw_shelf_ptr: *const RawShelf,
    raw_shelf_len: usize,
) -> FFIResult<i32> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!("update_interior_ref_list api_url: {:?}, api_key: {:?}, shop_id: {:?}, raw_interior_ref_len: {:?}, raw_shelf_len: {:?}", api_url, api_key, shop_id, raw_interior_ref_len, raw_shelf_len);
    let raw_interior_ref_slice = match raw_interior_ref_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_interior_ref_ptr, raw_interior_ref_len) },
    };
    let raw_shelf_slice = match raw_shelf_ptr.is_null() {
        true => &[],
        false => unsafe { slice::from_raw_parts(raw_shelf_ptr, raw_shelf_len) },
    };

    fn inner(
        api_url: &str,
        api_key: &str,
        shop_id: i32,
        raw_interior_ref_slice: &[RawInteriorRef],
        raw_shelf_slice: &[RawShelf],
    ) -> Result<SavedInteriorRefList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}/interior_ref_list", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/shops/{}/interior_ref_list", shop_id))?;

        let interior_ref_list =
            InteriorRefList::from_game(shop_id, raw_interior_ref_slice, raw_shelf_slice);
        info!(
            "created interior_ref_list from game: shop_id: {}",
            &interior_ref_list.shop_id
        );
        let client = reqwest::blocking::Client::new();
        let resp = client
            .patch(url)
            .header("Api-Key", api_key)
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&interior_ref_list)?)
            .send()?;
        info!("update interior_ref_list response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}_interior_ref_list.bin", shop_id));
        let metadata_cache_path =
            cache_dir.join(format!("shop_{}_interior_ref_list_metadata.json", shop_id));
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_interior_ref_list: SavedInteriorRefList = bincode::deserialize(&bytes)?;
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_interior_ref_list)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(
        &api_url,
        &api_key,
        shop_id,
        raw_interior_ref_slice,
        raw_shelf_slice,
    ) {
        Ok(interior_ref_list) => FFIResult::Ok(interior_ref_list.id),
        Err(err) => {
            error!("update_interior_ref_list failed. {}", err);
            // TODO: also need to drop this CString once C++ is done reading it
            let err_string = CString::new(err.to_string())
                .expect("could not create CString")
                .into_raw();
            FFIResult::Err(err_string)
        }
    }
}

// TODO: delete me if unused
#[no_mangle]
pub extern "C" fn get_interior_ref_list(
    api_url: *const c_char,
    api_key: *const c_char,
    interior_ref_list_id: i32,
) -> FFIResult<RawInteriorRefData> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!(
        "get_interior_ref_list api_url: {:?}, api_key: {:?}, interior_ref_list_id: {:?}",
        api_url, api_key, interior_ref_list_id
    );

    fn inner(
        api_url: &str,
        api_key: &str,
        interior_ref_list_id: i32,
    ) -> Result<SavedInteriorRefList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?
            .join(&format!("v1/interior_ref_lists/{}", interior_ref_list_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/interior_ref_lists/{}", interior_ref_list_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path =
            cache_dir.join(format!("interior_ref_list_{}.bin", interior_ref_list_id));
        let metadata_cache_path = cache_dir.join(format!(
            "interior_ref_list_{}_metadata.json",
            interior_ref_list_id
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
                info!("get_interior_ref_list response from api: {:?}", &resp);
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let saved_interior_ref_list = bincode::deserialize(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(saved_interior_ref_list)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!("get_interior_ref_list api request error: {}", err);
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, interior_ref_list_id) {
        Ok(interior_ref_list) => {
            let (interior_ref_ptr, interior_ref_len, interior_ref_cap) = interior_ref_list
                .ref_list
                .into_iter()
                .map(RawInteriorRef::from)
                .collect::<Vec<RawInteriorRef>>()
                .into_raw_parts();
            let (shelf_ptr, shelf_len, shelf_cap) = interior_ref_list
                .shelves
                .into_iter()
                .map(RawShelf::from)
                .collect::<Vec<RawShelf>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawInteriorRefData {
                interior_ref_vec: RawInteriorRefVec {
                    ptr: interior_ref_ptr,
                    len: interior_ref_len,
                    cap: interior_ref_cap,
                },
                shelf_vec: RawShelfVec {
                    ptr: shelf_ptr,
                    len: shelf_len,
                    cap: shelf_cap,
                },
            })
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

#[no_mangle]
pub extern "C" fn get_interior_ref_list_by_shop_id(
    api_url: *const c_char,
    api_key: *const c_char,
    shop_id: i32,
) -> FFIResult<RawInteriorRefData> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    info!(
        "get_interior_ref_list_by_shop_id api_url: {:?}, api_key: {:?}, shop_id: {:?}",
        api_url, api_key, shop_id
    );

    fn inner(api_url: &str, api_key: &str, shop_id: i32) -> Result<SavedInteriorRefList> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join(&format!("v1/shops/{}/interior_ref_list", shop_id))?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?
            .join(&format!("v1/shops/{}/interior_ref_list", shop_id))?;
        info!("api_url: {:?}", url);

        let client = reqwest::blocking::Client::new();
        let cache_dir = file_cache_dir(api_url)?;
        let body_cache_path = cache_dir.join(format!("shop_{}_interior_ref_list.bin", shop_id));
        let metadata_cache_path =
            cache_dir.join(format!("shop_{}_interior_ref_list_metadata.json", shop_id));
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
                    "get_interior_ref_list_by_shop_id response from api: {:?}",
                    &resp
                );
                if resp.status().is_success() {
                    let headers = resp.headers().clone();
                    let bytes = resp.bytes()?;
                    let saved_interior_ref_list = bincode::deserialize(&bytes)?;
                    update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
                    Ok(saved_interior_ref_list)
                } else if resp.status() == StatusCode::NOT_MODIFIED {
                    from_file_cache(&body_cache_path)
                } else {
                    log_server_error(resp);
                    from_file_cache(&body_cache_path)
                }
            }
            Err(err) => {
                error!(
                    "get_interior_ref_list_by_shop_id api request error: {}",
                    err
                );
                from_file_cache(&body_cache_path)
            }
        }
    }

    match inner(&api_url, &api_key, shop_id) {
        Ok(interior_ref_list) => {
            let (interior_ref_ptr, interior_ref_len, interior_ref_cap) = interior_ref_list
                .ref_list
                .into_iter()
                .map(RawInteriorRef::from)
                .collect::<Vec<RawInteriorRef>>()
                .into_raw_parts();
            let (shelf_ptr, shelf_len, shelf_cap) = interior_ref_list
                .shelves
                .into_iter()
                .map(RawShelf::from)
                .collect::<Vec<RawShelf>>()
                .into_raw_parts();
            // TODO: need to pass this back into Rust once C++ is done with it so it can be manually dropped and the CStrings dropped from raw pointers.
            FFIResult::Ok(RawInteriorRefData {
                interior_ref_vec: RawInteriorRefVec {
                    ptr: interior_ref_ptr,
                    len: interior_ref_len,
                    cap: interior_ref_cap,
                },
                shelf_vec: RawShelfVec {
                    ptr: shelf_ptr,
                    len: shelf_len,
                    cap: shelf_cap,
                },
            })
        }
        Err(err) => {
            error!("get_interior_ref_list_by_shop_id failed. {}", err);
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
    fn test_create_interior_ref_list() {
        let example = SavedInteriorRefList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            ref_list: vec![InteriorRef {
                base_mod_name: "Skyrim.esm".to_string(),
                base_local_form_id: 1,
                ref_mod_name: Some("BazaarRealm.esp".to_string()),
                ref_local_form_id: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
            }],
            shelves: vec![Shelf {
                shelf_type: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
                page: 1,
                filter_form_type: None,
                filter_is_food: false,
                search: None,
                sort_on: None,
                sort_asc: true,
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("POST", "/v1/interior_ref_lists")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (interior_ref_ptr, interior_ref_len, _cap) = vec![RawInteriorRef {
            base_mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            base_local_form_id: 1,
            ref_mod_name: CString::new("BazaarRealm.esp").unwrap().into_raw(),
            ref_local_form_id: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
        }]
        .into_raw_parts();
        let (shelf_ptr, shelf_len, _cap) = vec![RawShelf {
            shelf_type: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
            page: 1,
            filter_form_type: 0,
            filter_is_food: false,
            search: std::ptr::null(),
            sort_on: std::ptr::null(),
            sort_asc: true,
        }]
        .into_raw_parts();
        let result = create_interior_ref_list(
            api_url,
            api_key,
            1,
            interior_ref_ptr,
            interior_ref_len,
            shelf_ptr,
            shelf_len,
        );
        mock.assert();
        match result {
            FFIResult::Ok(interior_ref_list_id) => {
                assert_eq!(interior_ref_list_id, 1);
            }
            FFIResult::Err(error) => {
                panic!("create_interior_ref_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_create_interior_ref_list_server_error() {
        let mock = mock("POST", "/v1/interior_ref_lists")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (interior_ref_ptr, interior_ref_len, _cap) = vec![RawInteriorRef {
            base_mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            base_local_form_id: 1,
            ref_mod_name: CString::new("BazaarRealm.esp").unwrap().into_raw(),
            ref_local_form_id: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
        }]
        .into_raw_parts();
        let (shelf_ptr, shelf_len, _cap) = vec![RawShelf {
            shelf_type: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
            page: 1,
            filter_form_type: 0,
            filter_is_food: false,
            search: std::ptr::null(),
            sort_on: std::ptr::null(),
            sort_asc: true,
        }]
        .into_raw_parts();
        let result = create_interior_ref_list(
            api_url,
            api_key,
            1,
            interior_ref_ptr,
            interior_ref_len,
            shelf_ptr,
            shelf_len,
        );
        mock.assert();
        match result {
            FFIResult::Ok(interior_ref_list_id) => panic!(
                "create_interior_ref_list returned Ok result: {:?}",
                interior_ref_list_id
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
    fn test_update_interior_ref_list() {
        let example = SavedInteriorRefList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            ref_list: vec![InteriorRef {
                base_mod_name: "Skyrim.esm".to_string(),
                base_local_form_id: 1,
                ref_mod_name: Some("BazaarRealm.esp".to_string()),
                ref_local_form_id: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
            }],
            shelves: vec![Shelf {
                shelf_type: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
                page: 1,
                filter_form_type: None,
                filter_is_food: false,
                search: None,
                sort_on: None,
                sort_asc: true,
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("PATCH", "/v1/shops/1/interior_ref_list")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (interior_ref_ptr, interior_ref_len, _cap) = vec![RawInteriorRef {
            base_mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            base_local_form_id: 1,
            ref_mod_name: CString::new("BazaarRealm.esp").unwrap().into_raw(),
            ref_local_form_id: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
        }]
        .into_raw_parts();
        let (shelf_ptr, shelf_len, _cap) = vec![RawShelf {
            shelf_type: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
            page: 1,
            filter_form_type: 0,
            filter_is_food: false,
            search: std::ptr::null(),
            sort_on: std::ptr::null(),
            sort_asc: true,
        }]
        .into_raw_parts();
        let result = update_interior_ref_list(
            api_url,
            api_key,
            1,
            interior_ref_ptr,
            interior_ref_len,
            shelf_ptr,
            shelf_len,
        );
        mock.assert();
        match result {
            FFIResult::Ok(interior_ref_list_id) => {
                assert_eq!(interior_ref_list_id, 1);
            }
            FFIResult::Err(error) => {
                panic!("update_interior_ref_list returned error: {:?}", unsafe {
                    CStr::from_ptr(error).to_string_lossy()
                })
            }
        }
    }

    #[test]
    fn test_update_interior_ref_list_server_error() {
        let mock = mock("PATCH", "/v1/shops/1/interior_ref_list")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let (interior_ref_ptr, interior_ref_len, _cap) = vec![RawInteriorRef {
            base_mod_name: CString::new("Skyrim.esm").unwrap().into_raw(),
            base_local_form_id: 1,
            ref_mod_name: CString::new("BazaarRealm.esp").unwrap().into_raw(),
            ref_local_form_id: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
        }]
        .into_raw_parts();
        let (shelf_ptr, shelf_len, _cap) = vec![RawShelf {
            shelf_type: 1,
            position_x: 100.,
            position_y: 0.,
            position_z: 100.,
            angle_x: 0.,
            angle_y: 0.,
            angle_z: 0.,
            scale: 1,
            page: 1,
            filter_form_type: 0,
            filter_is_food: false,
            search: std::ptr::null(),
            sort_on: std::ptr::null(),
            sort_asc: true,
        }]
        .into_raw_parts();
        let result = update_interior_ref_list(
            api_url,
            api_key,
            1,
            interior_ref_ptr,
            interior_ref_len,
            shelf_ptr,
            shelf_len,
        );
        mock.assert();
        match result {
            FFIResult::Ok(interior_ref_list_id) => panic!(
                "update_interior_ref_list returned Ok result: {:?}",
                interior_ref_list_id
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
    fn test_get_interior_ref_list() {
        let example = SavedInteriorRefList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            ref_list: vec![InteriorRef {
                base_mod_name: "Skyrim.esm".to_string(),
                base_local_form_id: 1,
                ref_mod_name: Some("BazaarRealm.esp".to_string()),
                ref_local_form_id: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
            }],
            shelves: vec![Shelf {
                shelf_type: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
                page: 1,
                filter_form_type: None,
                filter_is_food: false,
                search: None,
                sort_on: None,
                sort_asc: true,
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("GET", "/v1/interior_ref_lists/1")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_interior_ref_list(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_interior_ref_data) => {
                assert_eq!(raw_interior_ref_data.interior_ref_vec.len, 1);
                assert_eq!(raw_interior_ref_data.shelf_vec.len, 1);
                assert!(!raw_interior_ref_data.interior_ref_vec.ptr.is_null());
                let raw_interior_ref_slice = unsafe {
                    slice::from_raw_parts(
                        raw_interior_ref_data.interior_ref_vec.ptr,
                        raw_interior_ref_data.interior_ref_vec.len,
                    )
                };
                let raw_interior_ref = &raw_interior_ref_slice[0];
                assert!(!raw_interior_ref_data.shelf_vec.ptr.is_null());
                let raw_shelf_slice = unsafe {
                    slice::from_raw_parts(
                        raw_interior_ref_data.shelf_vec.ptr,
                        raw_interior_ref_data.shelf_vec.len,
                    )
                };
                let raw_shelf = &raw_shelf_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_interior_ref.base_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_interior_ref.base_local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_interior_ref.ref_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "BazaarRealm.esp".to_string(),
                );
                assert_eq!(raw_interior_ref.ref_local_form_id, 1);
                assert_eq!(raw_interior_ref.position_x, 100.);
                assert_eq!(raw_interior_ref.position_y, 0.);
                assert_eq!(raw_interior_ref.position_z, 100.);
                assert_eq!(raw_interior_ref.angle_x, 0.);
                assert_eq!(raw_interior_ref.angle_y, 0.);
                assert_eq!(raw_interior_ref.angle_z, 0.);
                assert_eq!(raw_interior_ref.scale, 1);
                assert_eq!(raw_shelf.shelf_type, 1);
                assert_eq!(raw_shelf.position_x, 100.);
                assert_eq!(raw_shelf.position_y, 0.);
                assert_eq!(raw_shelf.position_z, 100.);
                assert_eq!(raw_shelf.angle_x, 0.);
                assert_eq!(raw_shelf.angle_y, 0.);
                assert_eq!(raw_shelf.angle_z, 0.);
                assert_eq!(raw_shelf.scale, 1);
                assert_eq!(raw_shelf.filter_form_type, 0);
                assert_eq!(raw_shelf.filter_is_food, false);
                assert_eq!(raw_shelf.search, std::ptr::null());
                assert_eq!(raw_shelf.sort_on, std::ptr::null());
                assert_eq!(raw_shelf.sort_asc, true);
            }
            FFIResult::Err(error) => panic!("get_interior_ref_list returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_get_interior_ref_list_server_error() {
        let mock = mock("GET", "/v1/interior_ref_lists/1")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_interior_ref_list(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_interior_ref_vec) => panic!(
                "get_interior_ref_list returned Ok result: {:#x?}",
                raw_interior_ref_vec
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
    fn test_get_interior_ref_list_by_shop_id() {
        let example = SavedInteriorRefList {
            id: 1,
            owner_id: 1,
            shop_id: 1,
            ref_list: vec![InteriorRef {
                base_mod_name: "Skyrim.esm".to_string(),
                base_local_form_id: 1,
                ref_mod_name: Some("BazaarRealm.esp".to_string()),
                ref_local_form_id: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
            }],
            shelves: vec![Shelf {
                shelf_type: 1,
                position_x: 100.,
                position_y: 0.,
                position_z: 100.,
                angle_x: 0.,
                angle_y: 0.,
                angle_z: 0.,
                scale: 1,
                page: 1,
                filter_form_type: None,
                filter_is_food: false,
                search: None,
                sort_on: None,
                sort_asc: true,
            }],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("GET", "/v1/shops/1/interior_ref_list")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_interior_ref_list_by_shop_id(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_interior_ref_data) => {
                assert_eq!(raw_interior_ref_data.interior_ref_vec.len, 1);
                assert_eq!(raw_interior_ref_data.shelf_vec.len, 1);
                assert!(!raw_interior_ref_data.interior_ref_vec.ptr.is_null());
                let raw_interior_ref_slice = unsafe {
                    slice::from_raw_parts(
                        raw_interior_ref_data.interior_ref_vec.ptr,
                        raw_interior_ref_data.interior_ref_vec.len,
                    )
                };
                let raw_interior_ref = &raw_interior_ref_slice[0];
                assert!(!raw_interior_ref_data.shelf_vec.ptr.is_null());
                let raw_shelf_slice = unsafe {
                    slice::from_raw_parts(
                        raw_interior_ref_data.shelf_vec.ptr,
                        raw_interior_ref_data.shelf_vec.len,
                    )
                };
                let raw_shelf = &raw_shelf_slice[0];
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_interior_ref.base_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "Skyrim.esm".to_string(),
                );
                assert_eq!(raw_interior_ref.base_local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_interior_ref.ref_mod_name) }
                        .to_string_lossy()
                        .to_string(),
                    "BazaarRealm.esp".to_string(),
                );
                assert_eq!(raw_interior_ref.ref_local_form_id, 1);
                assert_eq!(raw_interior_ref.position_x, 100.);
                assert_eq!(raw_interior_ref.position_y, 0.);
                assert_eq!(raw_interior_ref.position_z, 100.);
                assert_eq!(raw_interior_ref.angle_x, 0.);
                assert_eq!(raw_interior_ref.angle_y, 0.);
                assert_eq!(raw_interior_ref.angle_z, 0.);
                assert_eq!(raw_interior_ref.scale, 1);
                assert_eq!(raw_shelf.shelf_type, 1);
                assert_eq!(raw_shelf.position_x, 100.);
                assert_eq!(raw_shelf.position_y, 0.);
                assert_eq!(raw_shelf.position_z, 100.);
                assert_eq!(raw_shelf.angle_x, 0.);
                assert_eq!(raw_shelf.angle_y, 0.);
                assert_eq!(raw_shelf.angle_z, 0.);
                assert_eq!(raw_shelf.scale, 1);
                assert_eq!(raw_shelf.filter_form_type, 0);
                assert_eq!(raw_shelf.filter_is_food, false);
                assert_eq!(raw_shelf.search, std::ptr::null());
                assert_eq!(raw_shelf.sort_on, std::ptr::null());
                assert_eq!(raw_shelf.sort_asc, true);
            }
            FFIResult::Err(error) => panic!(
                "get_interior_ref_list_by_shop_id returned error: {:?}",
                unsafe { CStr::from_ptr(error).to_string_lossy() }
            ),
        }
    }

    #[test]
    fn test_get_interior_ref_list_by_shop_id_server_error() {
        let mock = mock("GET", "/v1/shops/1/interior_ref_list")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let result = get_interior_ref_list_by_shop_id(api_url, api_key, 1);
        mock.assert();
        match result {
            FFIResult::Ok(raw_interior_ref_vec) => panic!(
                "get_interior_ref_list_by_shop_id returned Ok result: {:#x?}",
                raw_interior_ref_vec
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
