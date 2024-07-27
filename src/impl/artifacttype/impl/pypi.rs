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

pub struct PypiArtifactType;

#[async_trait]
impl ArtifactType for PypiArtifactType {
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
        version: &'a str,
        _download_spec: &'a DownloadSpec,
        setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        match setting {
            Some(Value::String(project_name)) => Ok(ArtifactInfo::new_url(
                "Package on PyPi",
                Some("pypi.png".into()),
                format!("https://pypi.org/project/{}/{}/", project_name, version).into(),
            )),
            _ => Err(ArtifactError::MissingSetting),
        }
    }

    async fn get_nightly_artifact_info<'a>(
        &self,
        _product_name: &'a str,
        _download_spec: &'a DownloadSpec,
        _setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        Err(ArtifactError::NotSupported)
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
