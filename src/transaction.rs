use std::{ffi::CStr, ffi::CString, os::raw::c_char, slice};

use anyhow::Result;
use chrono::NaiveDateTime;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use log::{error, info};
#[cfg(test)]
use std::{println as info, println as error};

use crate::{
    cache::file_cache_dir, cache::update_file_caches, error::extract_error_from_response,
    result::FFIResult,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub shop_id: i32,
    pub owner_id: Option<i32>,
    pub mod_name: String,
    pub local_form_id: i32,
    pub name: String,
    pub form_type: i32,
    pub is_food: bool,
    pub price: i32,
    pub is_sell: bool,
    pub quantity: i32,
    pub amount: i32,
    pub keywords: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavedTransaction {
    pub id: i32,
    pub owner_id: i32,
    pub shop_id: i32,
    pub mod_name: String,
    pub local_form_id: i32,
    pub name: String,
    pub form_type: i32,
    pub is_food: bool,
    pub price: i32,
    pub is_sell: bool,
    pub quantity: i32,
    pub amount: i32,
    pub keywords: Vec<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<RawTransaction> for Transaction {
    fn from(raw_transaction: RawTransaction) -> Self {
        Self {
            shop_id: raw_transaction.shop_id,
            owner_id: None,
            mod_name: unsafe { CStr::from_ptr(raw_transaction.mod_name) }
                .to_string_lossy()
                .to_string(),
            local_form_id: raw_transaction.local_form_id,
            name: unsafe { CStr::from_ptr(raw_transaction.name) }
                .to_string_lossy()
                .to_string(),
            form_type: raw_transaction.form_type,
            is_food: raw_transaction.is_food,
            price: raw_transaction.price,
            is_sell: raw_transaction.is_sell,
            quantity: raw_transaction.quantity,
            amount: raw_transaction.amount,
            keywords: match raw_transaction.keywords.is_null() {
                true => vec![],
                false => unsafe {
                    slice::from_raw_parts(raw_transaction.keywords, raw_transaction.keywords_len)
                }
                .iter()
                .map(|&keyword| {
                    unsafe { CStr::from_ptr(keyword) }
                        .to_string_lossy()
                        .to_string()
                })
                .collect(),
            },
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawTransaction {
    pub id: i32,
    pub shop_id: i32,
    pub mod_name: *const c_char,
    pub local_form_id: i32,
    pub name: *const c_char,
    pub form_type: i32,
    pub is_food: bool,
    pub price: i32,
    pub is_sell: bool,
    pub quantity: i32,
    pub amount: i32,
    pub keywords: *mut *const c_char,
    pub keywords_len: usize,
}

impl From<SavedTransaction> for RawTransaction {
    fn from(transaction: SavedTransaction) -> Self {
        let (keywords_ptr, keywords_len, _) = transaction
            .keywords
            .into_iter()
            .map(|keyword| CString::new(keyword).unwrap_or_default().into_raw() as *const c_char)
            .collect::<Vec<*const c_char>>()
            .into_raw_parts();
        Self {
            id: transaction.id,
            shop_id: transaction.shop_id,
            mod_name: CString::new(transaction.mod_name)
                .unwrap_or_default()
                .into_raw(),
            local_form_id: transaction.local_form_id,
            name: CString::new(transaction.name)
                .unwrap_or_default()
                .into_raw(),
            form_type: transaction.form_type,
            is_food: transaction.is_food,
            price: transaction.price,
            is_sell: transaction.is_sell,
            quantity: transaction.quantity,
            amount: transaction.amount,
            keywords: keywords_ptr,
            keywords_len,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawTransactionVec {
    pub ptr: *mut RawTransaction,
    pub len: usize,
    pub cap: usize,
}

#[no_mangle]
pub extern "C" fn create_transaction(
    api_url: *const c_char,
    api_key: *const c_char,
    raw_transaction: RawTransaction,
) -> FFIResult<RawTransaction> {
    let api_url = unsafe { CStr::from_ptr(api_url) }.to_string_lossy();
    let api_key = unsafe { CStr::from_ptr(api_key) }.to_string_lossy();
    let transaction = Transaction::from(raw_transaction);
    info!(
        "create_transaction api_url: {:?}, api_key: {:?}, transaction: {:?}",
        api_url, api_key, transaction
    );

    fn inner(api_url: &str, api_key: &str, transaction: Transaction) -> Result<SavedTransaction> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/transactions")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/transactions")?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key)
            .header("Content-Type", "application/octet-stream")
            .body(bincode::serialize(&transaction)?)
            .send()?;
        info!("create transaction response from api: {:?}", &resp);

        let cache_dir = file_cache_dir(api_url)?;
        let headers = resp.headers().clone();
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let saved_transaction: SavedTransaction = bincode::deserialize(&bytes)?;
            let body_cache_path =
                cache_dir.join(format!("transaction_{}.bin", saved_transaction.id));
            let metadata_cache_path = cache_dir.join(format!(
                "transaction_{}_metadata.json",
                saved_transaction.id
            ));
            update_file_caches(body_cache_path, metadata_cache_path, bytes, headers);
            Ok(saved_transaction)
        } else {
            Err(extract_error_from_response(status, &bytes))
        }
    }

    match inner(&api_url, &api_key, transaction) {
        Ok(transaction) => FFIResult::Ok(RawTransaction::from(transaction)),
        Err(err) => {
            error!("create_transaction failed. {}", err);
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
    use chrono::Utc;
    use mockito::mock;

    #[test]
    fn test_create_transaction() {
        let example = SavedTransaction {
            id: 1,
            shop_id: 1,
            owner_id: 1,
            mod_name: "Skyrim.esm".to_string(),
            local_form_id: 1,
            name: "Item".to_string(),
            form_type: 41,
            is_food: false,
            is_sell: false,
            price: 100,
            quantity: 1,
            amount: 100,
            keywords: vec!["VendorItemMisc".to_string()],
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let mock = mock("POST", "/v1/transactions")
            .with_status(201)
            .with_header("content-type", "application/octet-stream")
            .with_body(bincode::serialize(&example).unwrap())
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let mod_name = CString::new("Skyrim.esm").unwrap().into_raw();
        let name = CString::new("Item").unwrap().into_raw();
        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemsMisc").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let raw_transaction = RawTransaction {
            id: 0,
            shop_id: 1,
            mod_name,
            local_form_id: 1,
            name,
            form_type: 41,
            is_food: false,
            price: 100,
            is_sell: false,
            amount: 100,
            quantity: 1,
            keywords,
            keywords_len,
        };
        let result = create_transaction(api_url, api_key, raw_transaction);
        mock.assert();
        match result {
            FFIResult::Ok(raw_transaction) => {
                assert_eq!(raw_transaction.id, 1);
                assert_eq!(raw_transaction.shop_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_transaction.mod_name).to_string_lossy() },
                    "Skyrim.esm"
                );
                assert_eq!(raw_transaction.local_form_id, 1);
                assert_eq!(
                    unsafe { CStr::from_ptr(raw_transaction.name).to_string_lossy() },
                    "Item"
                );
                assert_eq!(raw_transaction.form_type, 41);
                assert_eq!(raw_transaction.is_food, false);
                assert_eq!(raw_transaction.price, 100);
                assert_eq!(raw_transaction.is_sell, false);
                assert_eq!(raw_transaction.quantity, 1);
                assert_eq!(raw_transaction.amount, 100);
                assert_eq!(raw_transaction.keywords_len, 1);
                assert_eq!(
                    unsafe {
                        slice::from_raw_parts(
                            raw_transaction.keywords,
                            raw_transaction.keywords_len,
                        )
                    }
                    .iter()
                    .map(|&keyword| {
                        unsafe { CStr::from_ptr(keyword).to_string_lossy().to_string() }
                    })
                    .collect::<Vec<String>>(),
                    vec!["VendorItemMisc".to_string()]
                );
            }
            FFIResult::Err(error) => panic!("create_transaction returned error: {:?}", unsafe {
                CStr::from_ptr(error).to_string_lossy()
            }),
        }
    }

    #[test]
    fn test_create_transaction_server_error() {
        let mock = mock("POST", "/v1/transactions")
            .with_status(500)
            .with_header("content-type", "application/problem+json")
            .with_body(
                r#"{
                "detail": "Some error detail",
                "instance": "https://httpstatuses.com/500",
                "status": 500,
                "title": "Internal Server Error"
            }"#,
            )
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let mod_name = CString::new("Skyrim.esm").unwrap().into_raw();
        let name = CString::new("Item").unwrap().into_raw();
        let (keywords, keywords_len, _) =
            vec![CString::new("VendorItemsMisc").unwrap().into_raw() as *const c_char]
                .into_raw_parts();
        let raw_transaction = RawTransaction {
            id: 0,
            shop_id: 1,
            mod_name,
            local_form_id: 1,
            name,
            form_type: 41,
            is_food: false,
            price: 100,
            is_sell: false,
            amount: 100,
            quantity: 1,
            keywords,
            keywords_len,
        };
        let result = create_transaction(api_url, api_key, raw_transaction);
        mock.assert();
        match result {
            FFIResult::Ok(raw_transaction) => panic!(
                "create_transaction returned Ok result: {:#?}",
                raw_transaction
            ),
            FFIResult::Err(error) => {
                assert_eq!(
                    unsafe { CStr::from_ptr(error).to_string_lossy() },
                    "Server 500: Internal Server Error. Some error detail"
                );
            }
        }
    }
}
