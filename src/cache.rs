use std::{fs::create_dir_all, fs::File, io::BufReader, io::Write, path::Path, path::PathBuf};

use anyhow::{Context, Result};
use base64::{encode_config, URL_SAFE_NO_PAD};
use bytes::Bytes;
use serde::Deserialize;
#[cfg(test)]
use tempfile::tempfile;

#[cfg(not(test))]
use log::info;
#[cfg(test)]
use std::println as info;

use super::API_VERSION;

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
