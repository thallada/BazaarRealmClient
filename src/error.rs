use std::fmt;
use std::str;

use anyhow::{anyhow, Error};
use bytes::Bytes;
use http_api_problem::HttpApiProblem;
use reqwest::StatusCode;

#[cfg(not(test))]
use log::error;
#[cfg(test)]
use std::println as error;

#[derive(Debug)]
pub struct ServerError {
    pub status: StatusCode,
    pub title: String,
    pub detail: Option<String>,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(detail) = &self.detail {
            write!(
                f,
                "Server {} {}: {}",
                self.status.as_u16(),
                self.title,
                detail
            )
        } else {
            write!(f, "Server {} {}", self.status.as_u16(), self.title)
        }
    }
}

pub fn extract_error_from_response(status: StatusCode, bytes: &Bytes) -> Error {
    match serde_json::from_slice::<HttpApiProblem>(bytes) {
        Ok(api_problem) => {
            let server_error = ServerError {
                status,
                title: api_problem.title,
                detail: api_problem.detail,
            };
            error!("{}", server_error);
            anyhow!(server_error)
        }
        Err(_) => {
            let title = str::from_utf8(bytes)
                .unwrap_or_else(|_| &status.canonical_reason().unwrap_or("unknown"))
                .to_string();
            let server_error = ServerError {
                status,
                title,
                detail: None,
            };
            error!("{}", server_error);
            anyhow!(server_error)
        }
    }
}
