pub mod fallback;
pub mod r#impl;

use crate::r#impl::artifacttype::fallback::FallbackArtifactType;
#[cfg(feature = "flatpak")]
use crate::r#impl::artifacttype::r#impl::flathub::FLATHUB_KEY;
use crate::r#impl::artifacttype::r#impl::*;
use crate::r#impl::config::Endpoints;
use crate::r#impl::release_map::NamedVersion;
use askama::filters::filesizeformat;
use async_trait::async_trait;
use indexmap::IndexMap;
use log::warn;
use serde_yaml::Value;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

/// This key in ArtifactTypes is used as the fallback implementation for unknown
/// artifact types.
pub const FALLBACK_KEY: &str = "__fallback";

pub type ArtifactKey = String;
pub struct ArtifactTypes(IndexMap<String, Box<dyn ArtifactType>>);

impl From<IndexMap<String, Box<dyn ArtifactType>>> for ArtifactTypes {
    fn from(v: IndexMap<String, Box<dyn ArtifactType>>) -> Self {
        Self(v)
    }
}

impl Default for ArtifactTypes {
    fn default() -> Self {
        let mut m: IndexMap<String, Box<dyn ArtifactType>> = IndexMap::new();
        m.insert(FALLBACK_KEY.into(), Box::new(FallbackArtifactType));
        #[cfg(feature = "flatpak")]
        m.insert(FLATHUB_KEY.into(), Box::new(FlathubArtifactType));
        #[cfg(feature = "github")]
        m.insert("github".into(), Box::new(GithubArtifactType));
        m.insert("mac64".into(), Box::new(Mac64ArtifactType));
        #[cfg(feature = "pypi")]
        m.insert("pypi".into(), Box::new(PypiArtifactType));
        m.insert("win32".into(), Box::new(Win32ArtifactType));
        m.insert("win64".into(), Box::new(Win64ArtifactType));
        m.into()
    }
}

#[async_trait]
pub trait ArtifactType: Send + Sync {
    async fn describe<'a>(
        &self,
        description_map: &mut IndexMap<Cow<'a, str>, Cow<'a, str>>,
        setting: Option<&Value>,
        version: &NamedVersion<'_>,
    );

    async fn get_artifact<'a>(
        &self,
        product_name: &'a str,
        version: &'a str,
        download_value: &'a str,
        setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError>;
}

#[derive(Debug)]
pub struct RenderableArtifact<'a> {
    pub icon_path: Option<Cow<'a, str>>,
    pub display_name: ArtifactDisplayTitle<'a>,
    pub modified_date: Option<Cow<'a, str>>,
    pub file_size: Option<Cow<'a, str>>,
    pub urls: BTreeMap<Cow<'a, str>, Cow<'a, str>>,
    pub extra_info_markdown: Option<Cow<'a, str>>,
}

impl<'a> RenderableArtifact<'a> {
    pub fn display_title(&'a self) -> &'a str {
        self.display_name.title()
    }

    pub fn display_subtitle(&'a self) -> &'a str {
        self.display_name.subtitle()
    }
}

pub async fn artifacts_describe<'a>(
    settings: &HashMap<ArtifactKey, Value>,
    version: &NamedVersion<'_>,
    ats: &'a ArtifactTypes,
) -> IndexMap<Cow<'a, str>, Cow<'a, str>> {
    let mut out = IndexMap::new();
    for (key, at) in &ats.0 {
        // TODO: Async could be improved here.
        at.describe(&mut out, settings.get(key), version).await;
    }
    out
}

pub async fn artifacts_collect(
    product_name: &str,
    settings: &HashMap<ArtifactKey, Value>,
    version: &NamedVersion<'_>,
    ats: &ArtifactTypes,
    endpoints: &Endpoints,
    #[cfg(feature = "s3_bucket_list")] bucket_list: Option<Vec<s3::serde_types::ListBucketResult>>,
) -> Vec<RenderableArtifact<'static>> {
    let mut renderables = Vec::new();
    for (key, download) in &version.info().downloads {
        // TODO: Async could be improved here.
        match get_artifact_info(
            ats,
            key,
            product_name,
            version.name(),
            download,
            settings.get(key),
        )
        .await
        {
            Ok(artifact_info) => {
                let mut modified_date = None;
                let mut file_size: Option<u64> = None;
                #[cfg(feature = "s3_bucket_list")]
                if let Some(file_path) = artifact_info.file_path(product_name, version.name()) {
                    if let Some((out_modified_date, out_file_size)) =
                        get_file_metadata(&bucket_list, &file_path)
                    {
                        modified_date = Some(out_modified_date);
                        file_size = Some(out_file_size);
                    }
                }
                renderables.push(RenderableArtifact {
                    icon_path: artifact_info
                        .icon()
                        .map(ToString::to_string)
                        .map(Into::into),
                    display_name: artifact_info.display_name().clone_owned(),
                    modified_date: modified_date
                        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string().into()),
                    file_size: file_size
                        .and_then(|file_size| filesizeformat(&file_size).ok())
                        .map(Into::into),
                    urls: artifact_info.urls(product_name, version.name(), endpoints),
                    extra_info_markdown: artifact_info
                        .extra_info_markdown
                        .map(|s| s.to_string())
                        .map(Into::into),
                })
            }
            Err(err) => warn!(
                "Was unable to serve artifact '{}' for version '{}' of '{}': {}",
                key,
                version.name(),
                product_name,
                err
            ),
        }
    }
    renderables
}

async fn get_artifact_info<'a>(
    ats: &ArtifactTypes,
    key: &str,
    product_name: &'a str,
    version_name: &'a str,
    download_value: &'a str,
    setting: Option<&'a Value>,
) -> Result<ArtifactInfo<'a>, ArtifactError> {
    if let Some(at_info) = ats.0.get(key) {
        at_info
            .get_artifact(product_name, version_name, download_value, setting)
            .await
    } else if let Some(at_fallback) = ats.0.get(FALLBACK_KEY) {
        at_fallback
            .get_artifact(product_name, version_name, download_value, setting)
            .await
    } else {
        Err(ArtifactError::NoFallback)
    }
}

#[cfg(feature = "s3_bucket_list")]
fn get_file_metadata<'a>(
    bucket_list: &'a Option<Vec<s3::serde_types::ListBucketResult>>,
    file_path: &str,
) -> Option<(&'a str, u64)> {
    if let Some(bucket_list) = bucket_list {
        for page in bucket_list {
            for object in &page.contents {
                if object.key == file_path {
                    return Some((&object.last_modified, object.size));
                }
            }
        }
    }
    None
}

#[derive(Error, Debug)]
pub enum ArtifactError {
    #[error(
        "The server did not know how to handle the artifact type and no fallback was provided."
    )]
    NoFallback,
    #[error(
        "The product did not have the configuration for this artifact type in the correct format."
    )]
    MissingSetting,
}

enum ArtifactPath<'a> {
    File(Cow<'a, str>),
    RemoteUrl(Cow<'a, str>),
}

#[derive(Debug)]
pub enum ArtifactDisplayTitle<'a> {
    Simple(Cow<'a, str>),
    Descriptive {
        file_name: Cow<'a, str>,
        descriptive_title: Cow<'a, str>,
    },
}

impl<'a> ArtifactDisplayTitle<'a> {
    pub fn clone_owned(&self) -> ArtifactDisplayTitle<'static> {
        match self {
            ArtifactDisplayTitle::Simple(s) => ArtifactDisplayTitle::Simple(s.to_string().into()),
            ArtifactDisplayTitle::Descriptive {
                file_name,
                descriptive_title,
            } => ArtifactDisplayTitle::Descriptive {
                file_name: file_name.to_string().into(),
                descriptive_title: descriptive_title.to_string().into(),
            },
        }
    }

    pub fn title(&'a self) -> &'a str {
        match self {
            ArtifactDisplayTitle::Simple(s) => s,
            ArtifactDisplayTitle::Descriptive {
                descriptive_title, ..
            } => descriptive_title,
        }
    }

    pub fn subtitle(&'a self) -> &'a str {
        match self {
            ArtifactDisplayTitle::Simple(_s) => "",
            ArtifactDisplayTitle::Descriptive { file_name, .. } => file_name,
        }
    }
}

impl<'a> From<String> for ArtifactDisplayTitle<'a> {
    fn from(v: String) -> Self {
        ArtifactDisplayTitle::Simple(v.into())
    }
}

impl<'a> From<&'a str> for ArtifactDisplayTitle<'a> {
    fn from(v: &'a str) -> Self {
        ArtifactDisplayTitle::Simple(v.into())
    }
}

pub struct ArtifactInfo<'a> {
    display_name: ArtifactDisplayTitle<'a>,
    extra_info_markdown: Option<Cow<'a, str>>,
    icon: Option<Cow<'a, str>>,
    path: ArtifactPath<'a>,
}

impl<'a> ArtifactInfo<'a> {
    /// Create a new artifact info for a file stored on the endpoint. The file path is relative
    /// to the release's directory on the endpoint.
    pub fn new_file<T: Into<ArtifactDisplayTitle<'a>>>(
        display_name: T,
        icon: Option<Cow<'a, str>>,
        file_path: Cow<'a, str>,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            extra_info_markdown: None,
            icon,
            path: ArtifactPath::File(file_path),
        }
    }

    /// Create a new artifact info that works via a remote URL.
    pub fn new_url<T: Into<ArtifactDisplayTitle<'a>>>(
        display_name: T,
        icon: Option<Cow<'a, str>>,
        remote_url: Cow<'a, str>,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            extra_info_markdown: None,
            icon,
            path: ArtifactPath::RemoteUrl(remote_url),
        }
    }

    pub fn set_extra_info_markdown(&mut self, value: Cow<'a, str>) {
        self.extra_info_markdown = Some(value);
    }

    pub fn display_name(&self) -> &ArtifactDisplayTitle {
        &self.display_name
    }

    pub fn icon(&self) -> Option<&Cow<'a, str>> {
        self.icon.as_ref()
    }

    pub fn file_path(&self, product_name: &str, version_name: &str) -> Option<String> {
        match &self.path {
            ArtifactPath::File(file_name) => {
                Some(format!("{}/{}/{}", product_name, version_name, file_name))
            }
            _ => None,
        }
    }

    pub fn urls(
        &self,
        product_name: &str,
        version_name: &str,
        endpoints: &Endpoints,
    ) -> BTreeMap<Cow<'static, str>, Cow<'static, str>> {
        match &self.path {
            ArtifactPath::File(_) => endpoints
                .get_all()
                .iter()
                .map(|e| {
                    (
                        e.display_name.clone().into(),
                        format!(
                            "{}/{}",
                            &e.url,
                            self.file_path(product_name, version_name).unwrap()
                        )
                        .into(),
                    )
                })
                .collect(),
            ArtifactPath::RemoteUrl(remote_url) => endpoints
                .get_all()
                .iter()
                .map(|e| (e.display_name.clone().into(), remote_url.to_string().into()))
                .collect(),
        }
    }
}
