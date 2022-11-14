use crate::r#impl::artifacttype::{
    ArtifactDisplayTitle, ArtifactError, ArtifactInfo, ArtifactType,
};
use crate::r#impl::release_map::NamedVersion;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde_yaml::Value;
use std::borrow::Cow;

pub struct Win32ArtifactType;

#[async_trait]
impl ArtifactType for Win32ArtifactType {
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
        download_value: &'a str,
        _setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        Ok(ArtifactInfo::new_file(
            ArtifactDisplayTitle::Descriptive {
                file_name: download_value.into(),
                descriptive_title: "Windows (32-bit)".into(),
            },
            Some("win32.png".into()),
            download_value.into(),
        ))
    }
}
