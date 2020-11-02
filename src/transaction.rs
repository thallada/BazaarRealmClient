use std::{convert::TryFrom, ffi::CStr, ffi::CString, os::raw::c_char, slice, str};

use anyhow::{anyhow, Result};
use http_api_problem::HttpApiProblem;
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
pub struct Transaction {
    pub id: Option<u32>,
    pub shop_id: u32,
    pub mod_name: String,
    pub local_form_id: u32,
    pub name: String,
    pub form_type: u32,
    pub is_food: bool,
    pub price: u32,
    pub is_sell: bool,
    pub quantity: u32,
    pub amount: u32,
}

impl Transaction {
    pub fn from_game(
        shop_id: u32,
        mod_name: &str,
        local_form_id: u32,
        name: &str,
        form_type: u32,
        is_food: bool,
        price: u32,
        is_sell: bool,
        quantity: u32,
        amount: u32,
    ) -> Self {
        Self {
            id: None,
            shop_id,
            mod_name: mod_name.to_string(),
            local_form_id,
            name: name.to_string(),
            form_type,
            is_food,
            price,
            is_sell,
            quantity,
            amount,
        }
    }
}

impl From<RawTransaction> for Transaction {
    fn from(raw_transaction: RawTransaction) -> Self {
        Self {
            id: match raw_transaction.id {
                0 => None,
                _ => Some(raw_transaction.id),
            },
            shop_id: raw_transaction.shop_id,
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
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct RawTransaction {
    pub id: u32,
    pub shop_id: u32,
    pub mod_name: *const c_char,
    pub local_form_id: u32,
    pub name: *const c_char,
    pub form_type: u32,
    pub is_food: bool,
    pub price: u32,
    pub is_sell: bool,
    pub quantity: u32,
    pub amount: u32,
}

impl From<Transaction> for RawTransaction {
    fn from(transaction: Transaction) -> Self {
        Self {
            id: transaction.id.unwrap_or(0),
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

    fn inner(api_url: &str, api_key: &str, transaction: Transaction) -> Result<Transaction> {
        #[cfg(not(test))]
        let url = Url::parse(api_url)?.join("v1/transactions")?;
        #[cfg(test)]
        let url = Url::parse(&mockito::server_url())?.join("v1/transactions")?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(url)
            .header("Api-Key", api_key)
            .json(&transaction)
            .send()?;
        info!("create transaction response from api: {:?}", &resp);
        let status = resp.status();
        let bytes = resp.bytes()?;
        if status.is_success() {
            let json: Transaction = serde_json::from_slice(&bytes)?;
            if let Some(id) = json.id {
                update_file_cache(
                    &file_cache_dir(api_url)?.join(format!("transaction_{}.json", id)),
                    &bytes,
                )?;
            }
            Ok(json)
        } else {
            match serde_json::from_slice::<HttpApiProblem>(&bytes) {
                Ok(api_problem) => {
                    let detail = api_problem.detail.unwrap_or("".to_string());
                    error!("Server {} error: {}. {}", status, api_problem.title, detail);
                    Err(anyhow!(format!(
                        "Server {} error: {}. {}",
                        status, api_problem.title, detail
                    )))
                }
                Err(_) => {
                    let detail = str::from_utf8(&bytes).unwrap_or("unknown");
                    error!("Server {} error: {}", status, detail);
                    Err(anyhow!(format!("Server {} error: {}", status, detail)))
                }
            }
        }
    }

    match inner(&api_url, &api_key, transaction) {
        Ok(transaction) => {
            if let Ok(raw_transaction) = RawTransaction::try_from(transaction) {
                FFIResult::Ok(raw_transaction)
            } else {
                error!("create_transaction failed. API did not return a transaction with an ID");
                let err_string =
                    CString::new("API did not return a transaction with an ID".to_string())
                        .expect("could not create CString")
                        .into_raw();
                // TODO: also need to drop this CString once C++ is done reading it
                FFIResult::Err(err_string)
            }
        }
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
    use mockito::mock;

    #[test]
    fn test_create_transaction() {
        let mock = mock("POST", "/v1/transactions")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "amount": 100,
                "created_at": "2020-08-18T00:00:00.000",
                "form_type": 41,
                "id": 1,
                "is_food": false,
                "is_sell": false,
                "local_form_id": 1,
                "mod_name": "Skyrim.esm",
                "name": "Item",
                "owner_id": 1,
                "price": 100,
                "quantity": 1,
                "shop_id": 1,
                "updated_at": "2020-08-18T00:00:00.000"
            }"#,
            )
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let mod_name = CString::new("Skyrim.esm").unwrap().into_raw();
        let name = CString::new("Item").unwrap().into_raw();
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
            .with_body("Internal Server Error")
            .create();

        let api_url = CString::new("url").unwrap().into_raw();
        let api_key = CString::new("api-key").unwrap().into_raw();
        let mod_name = CString::new("Skyrim.esm").unwrap().into_raw();
        let name = CString::new("Item").unwrap().into_raw();
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
                    "expected value at line 1 column 1"
                );
            }
        }
    }
}
