use crate::r#impl::artifacttype::ArtifactKey;
use crate::r#impl::config::Endpoint;
use crate::r#impl::release_map::ReleaseMap;
use cached::proc_macro::cached;
use indexmap::IndexMap;
use log::{debug, warn};
use regex::Regex;
#[cfg(feature = "s3_bucket_list")]
use s3::error::S3Error;
#[cfg(feature = "s3_bucket_list")]
use s3::serde_types::ListBucketResult;
#[cfg(feature = "s3_bucket_list")]
use s3::{Bucket, Region};
use serde::Deserialize;
use serde_yaml::Value;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::sync::Mutex;
use thiserror::Error;

pub const PRODUCTS_YML: &str = "products.yml";

/// Struct to interact with an endpoint.
pub struct Storage {
    endpoint: Endpoint,
    bucket_list_error_logged: Mutex<RefCell<bool>>,
}

impl Storage {
    pub fn new(endpoint: Endpoint) -> Result<Self, String> {
        Ok(Self {
            endpoint,
            bucket_list_error_logged: Mutex::new(RefCell::new(false)),
        })
    }

    pub fn endpoint_url(&self) -> &str {
        &self.endpoint.url
    }

    /// Returns the product configuration, or an error on error. The result may be cached.
    pub async fn get_config(&self) -> Result<ProductsConfig, StorageError> {
        _impl_get_config(self.endpoint.url.clone()).await
    }

    #[cfg(feature = "s3_bucket_list")]
    /// Returns the S3-compatible bucket listing or None, if the endpoint does not provide a listing.
    /// The result may be cached. If no listing can be retrieved a warning will be logged on the
    /// first call to this function.
    pub async fn get_bucket_list(&self) -> Option<Vec<ListBucketResult>> {
        _impl_get_bucket_list(
            self.endpoint
                .url
                .replace("http://", "")
                .replace("https://", ""),
        )
        .await
        .map_err(|err| {
            let guard = self.bucket_list_error_logged.lock().unwrap();
            let mut bucket_list_error_logged = guard.borrow_mut();
            if !*bucket_list_error_logged {
                warn!(
                    "The endpoint '{}' does not provide an S3-compatible bucket listing. \
                         Some information, like file sizes and modification dates, will not be \
                         available: {}.",
                    &self.endpoint.url, err
                );
                *bucket_list_error_logged = true;
            }
            err
        })
        .ok()
    }
}

#[cached(time = 900, time_refresh = false, sync_writes = true, result = true)]
async fn _impl_get_config(endpoint_url: String) -> Result<ProductsConfig, StorageError> {
    debug!("Loading products.yml for {}", endpoint_url);
    Ok(serde_yaml::from_str(
        &reqwest::get(format!("{}/{}", endpoint_url, PRODUCTS_YML))
            .await?
            .text()
            .await?,
    )?)
}

#[cfg(feature = "s3_bucket_list")]
#[cached(time = 900, time_refresh = false, sync_writes = true, result = true)]
async fn _impl_get_bucket_list(
    endpoint_url: String,
) -> Result<Vec<ListBucketResult>, StorageError> {
    debug!("Loading bucket listing for {}", endpoint_url);
    let (bucket_name, endpoint) = endpoint_url
        .split_once('.')
        .ok_or(StorageError::NoBucketFound)?;
    let bucket = Bucket::new_public(
        bucket_name,
        Region::Custom {
            region: "n/a".to_string(),
            endpoint: endpoint.to_string(),
        },
    )?;
    Ok(bucket.list("".to_string(), None).await?)
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Error during request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error while reading YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    IOError(#[from] io::Error),
    #[cfg(feature = "s3_bucket_list")]
    #[error("S3Error: {0}")]
    S3Error(#[from] S3Error),
    #[cfg(feature = "s3_bucket_list")]
    #[error("The bucket name could not be extracted from the URL.")]
    NoBucketFound,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductsConfig {
    pub products: IndexMap<String, Product>,
    #[serde(default)]
    pub banner: Option<Banner>,
    #[serde(default)]
    pub pre_release_patterns: Vec<PreReleasePatternEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Banner {
    pub url_file: String,
    pub image_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreReleasePatternEntry {
    #[serde(with = "serde_regex")]
    pub pattern: Regex,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Product {
    pub name: String,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub settings: HashMap<ArtifactKey, Value>,
    pub versions: ReleaseMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionInfo {
    pub date: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub changelog_section: Option<u64>,
    #[serde(default)]
    pub downloads: IndexMap<ArtifactKey, DownloadSpec>,
}

const DOWNLOAD_ATTRIBUTE_UNSUPPORTED: &str = "unsupported";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum DownloadSpec {
    Url(String),
    Complex {
        url: String,
        #[serde(flatten)]
        attributes: IndexMap<String, Value>,
    },
    Null,
}

impl DownloadSpec {
    pub fn url(&self) -> &str {
        match self {
            DownloadSpec::Url(url) => url,
            DownloadSpec::Null => "",
            DownloadSpec::Complex { url, .. } => url,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        match self {
            DownloadSpec::Complex { attributes, .. } => {
                if let Some(v) = attributes.get(DOWNLOAD_ATTRIBUTE_UNSUPPORTED) {
                    matches!(v, Value::Bool(true))
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl<'a> From<&'a VersionInfo> for Cow<'a, VersionInfo> {
    fn from(v: &'a VersionInfo) -> Self {
        Cow::Borrowed(v)
    }
}
