use std::borrow::Cow;

use async_trait::async_trait;
use indexmap::IndexMap;
use log::debug;
use rocket::response::Redirect;
use serde_yaml::Value;

use crate::r#impl::artifacttype::{
    ArtifactDisplayTitle, ArtifactError, ArtifactInfo, ArtifactType, NightlyArtifactResponder,
};
use crate::r#impl::github::GithubClient;
use crate::r#impl::nightly::NightlyConfig;
use crate::r#impl::release_map::NamedVersion;
use crate::r#impl::storage::DownloadSpec;

pub struct GithubArtifactType;

#[async_trait]
impl ArtifactType for GithubArtifactType {
    async fn describe<'a>(
        &self,
        description_map: &mut IndexMap<Cow<'a, str>, Cow<'a, str>>,
        setting: Option<&Value>,
        version: &NamedVersion<'_>,
    ) {
        match setting {
            Some(Value::String(project_name)) => {
                if let Some((org, repo)) = project_name.split_once('/') {
                    match GithubClient::get_instance()
                        .fetch_release(
                            org,
                            repo,
                            version
                                .info()
                                .changelog
                                .as_deref()
                                .unwrap_or_else(|| version.name())
                                .to_string(),
                        )
                        .await
                    {
                        Ok(release) => {
                            if let Some(release_description) = release.body {
                                if let Some(changelog_section) = version.info().changelog_section {
                                    let split_changelog = release_description
                                        .split("---\r\n")
                                        .nth(changelog_section as usize);
                                    if let Some(split_changelog) = split_changelog {
                                        description_map.insert(
                                            "Changelog".into(),
                                            split_changelog.to_string().into(),
                                        );
                                    }
                                } else {
                                    description_map
                                        .insert("Changelog".into(), release_description.into());
                                }
                            }
                        }
                        Err(err) => debug!(
                            "Failed to read github release '{}' from '{}': {:?}",
                            version.name(),
                            project_name,
                            err
                        ),
                    }
                } else {
                    debug!(
                        "Could not extract org and repo for github artifact type for '{}'.",
                        project_name
                    );
                }
            }
            _ => {
                debug!("No setting for github artifact type, could not fetch changelog.");
            }
        }
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
                "Source on GitHub",
                Some("github.png".into()),
                format!("https://github.com/{}/tree/{}", project_name, version).into(),
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
        Ok(ArtifactInfo::new_empty(
            ArtifactDisplayTitle::Simple("Source on GitHub".into()),
            Some("github.png".into()),
        ))
    }

    async fn get_nightly_artifact_download<'a>(
        &self,
        _product_name: &'a str,
        download_spec: &'a DownloadSpec,
        setting: Option<&'a Value>,
        _nightly_config: &'a NightlyConfig,
    ) -> Result<NightlyArtifactResponder, ArtifactError> {
        match setting {
            Some(Value::String(project_name)) => Ok(NightlyArtifactResponder::Redirect(
                Redirect::permanent(format!(
                    "https://github.com/{}/tree/{}",
                    project_name,
                    download_spec.url()
                )),
            )),
            _ => Err(ArtifactError::MissingSetting),
        }
    }
}
