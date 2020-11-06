use std::{fs::create_dir_all, fs::File, io::BufReader, io::Write, path::Path, path::PathBuf};

use anyhow::{Context, Result};
use base64::{encode_config, URL_SAFE_NO_PAD};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::{blocking::Response, header::HeaderMap};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use tempfile::tempfile;

#[cfg(not(test))]
use log::info;
#[cfg(test)]
use std::println as info;

use super::API_VERSION;

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub etag: Option<String>,
    pub date: Option<DateTime<Utc>>,
}

pub fn file_cache_dir(api_url: &str) -> Result<PathBuf> {
    let encoded_url = encode_config(api_url, URL_SAFE_NO_PAD);
    let path = Path::new("Data/SKSE/Plugins/BazaarRealmCache")
        .join(encoded_url)
        .join(API_VERSION);
    #[cfg(not(test))]
    create_dir_all(&path)?;
    Ok(path)
}

pub fn update_file_cache(cache_path: &Path, bytes: &Bytes) -> Result<()> {
    #[cfg(not(test))]
    let mut file = File::create(cache_path)?;
    #[cfg(test)]
    let mut file = tempfile()?;

    file.write_all(&bytes.as_ref())?;
    Ok(())
}

pub fn update_metadata_file_cache(cache_path: &Path, headers: &HeaderMap) -> Result<()> {
    #[cfg(not(test))]
    let mut file = File::create(cache_path)?;
    #[cfg(test)]
    let mut file = tempfile()?;

    let etag = headers
        .get("etag")
        .map(|val| val.to_str().unwrap_or("").to_string());
    let date = headers
        .get("date")
        .map(|val| val.to_str().unwrap_or("").parse().unwrap_or(Utc::now()));
    let metadata = Metadata { etag, date };
    serde_json::to_writer(file, &metadata)?;
    Ok(())
}

pub fn update_file_caches(
    body_cache_path: &Path,
    metadata_cache_path: &Path,
    response: Response,
) -> Result<Bytes> {
    update_metadata_file_cache(metadata_cache_path, &response.headers())?;
    let bytes = response.bytes()?;
    update_file_cache(body_cache_path, &bytes)?;
    Ok(bytes)
}

pub fn from_file_cache<T: for<'de> Deserialize<'de>>(cache_path: &Path) -> Result<T> {
    #[cfg(not(test))]
    let file = File::open(cache_path).context(format!(
        "Object not found in API or in cache: {}",
        cache_path.file_name().unwrap_or_default().to_string_lossy()
    ))?;
    #[cfg(test)]
    let file = tempfile()?; // cache always reads from an empty temp file in cfg(test)

    let reader = BufReader::new(file);
    info!("returning value from cache: {:?}", cache_path);
    Ok(serde_json::from_reader(reader)?)
}

pub fn load_metadata_from_file_cache(cache_path: &Path) -> Result<Metadata> {
    #[cfg(not(test))]
    let file = File::open(cache_path).context(format!(
        "Object not found in API or in cache: {}",
        cache_path.file_name().unwrap_or_default().to_string_lossy()
    ))?;
    #[cfg(test)]
    let file = tempfile()?; // cache always reads from an empty temp file in cfg(test)

    let reader = BufReader::new(file);
    info!("returning value from cache: {:?}", cache_path);
    let metadata: Metadata = serde_json::from_reader(reader)?;
    Ok(metadata)
}
