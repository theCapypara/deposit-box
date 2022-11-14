use crate::r#impl::artifacttype::{ArtifactError, ArtifactInfo, ArtifactType};
use crate::r#impl::release_map::NamedVersion;
use async_trait::async_trait;
use cached::proc_macro::cached;
use indexmap::IndexMap;
use log::debug;
use octocrab::models::repos::Release;
use serde_yaml::Value;
use std::borrow::Cow;

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
                    match fetch_github_release(
                        org.to_string(),
                        repo.to_string(),
                        version
                            .info()
                            .changelog
                            .as_ref()
                            .map(String::as_str)
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
                                        .skip(changelog_section as usize)
                                        .next();
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
        _download_value: &'a str,
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
}

#[cached(time = 7200, time_refresh = false, sync_writes = true, result = false)]
async fn fetch_github_release(
    org: String,
    repo: String,
    version_name: String,
) -> Result<Release, String> {
    octocrab::instance()
        .repos(org, repo)
        .releases()
        .get_by_tag(&version_name)
        .await
        .map_err(|err| format!("{:?}", err))
}
