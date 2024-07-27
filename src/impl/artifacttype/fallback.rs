use std::borrow::Cow;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_yaml::Value;

use crate::r#impl::artifacttype::{
    ArtifactError, ArtifactInfo, ArtifactType, NightlyArtifactResponder,
};
use crate::r#impl::nightly::NightlyConfig;
use crate::r#impl::release_map::NamedVersion;
use crate::r#impl::storage::DownloadSpec;

pub struct FallbackArtifactType;

#[async_trait]
impl ArtifactType for FallbackArtifactType {
    async fn describe<'a>(
        &self,
        _description_map: &mut IndexMap<Cow<'a, str>, Cow<'a, str>>,
        _setting: Option<&Value>,
        _version: &NamedVersion<'_>,
    ) {
    }

    async fn get_artifact<'a>(
        &self,
        _product_name: &'a str,
        _version: &'a str,
        download_spec: &'a DownloadSpec,
        _setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        Ok(ArtifactInfo::new_file(
            download_spec.url(),
            None,
            download_spec.url().into(),
        ))
    }

    async fn get_nightly_artifact_info<'a>(
        &self,
        _product_name: &'a str,
        download_spec: &'a DownloadSpec,
        _setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        Ok(ArtifactInfo::new_empty(download_spec.url(), None))
    }

    async fn get_nightly_artifact_download<'a>(
        &self,
        _product_name: &'a str,
        _download_spec: &'a DownloadSpec,
        _setting: Option<&'a Value>,
        _nightly_config: &'a NightlyConfig,
    ) -> Result<NightlyArtifactResponder, ArtifactError> {
        Err(ArtifactError::NotSupported)
    }
}
