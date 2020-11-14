use std::str;

use anyhow::{anyhow, Error};
use bytes::Bytes;
use http_api_problem::HttpApiProblem;
use reqwest::StatusCode;

#[cfg(not(test))]
use log::error;
#[cfg(test)]
use std::println as error;

pub fn extract_error_from_response(status: StatusCode, bytes: &Bytes) -> Error {
    match serde_json::from_slice::<HttpApiProblem>(bytes) {
        Ok(api_problem) => {
            let detail = api_problem.detail.unwrap_or("".to_string());
            error!(
                "Server {}: {}. {}",
                status.as_u16(),
                api_problem.title,
                detail
            );
            anyhow!(format!(
                "Server {}: {}. {}",
                status.as_u16(),
                api_problem.title,
                detail
            ))
        }
        Err(_) => {
            let detail = str::from_utf8(bytes).unwrap_or("unknown");
            error!("Server {}: {}", status.as_u16(), detail);
            anyhow!(format!("Server {}: {}", status.as_u16(), detail))
        }
    }
}
