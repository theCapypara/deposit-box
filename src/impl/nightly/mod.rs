use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;

use indexmap::IndexMap;
use log::error;
use rocket::http::Status;
use rocket::serde::Deserialize;
use rocket::State;
use serde_yaml::Value;

use crate::r#impl::artifacttype::{
    get_artifact_nightly_download, get_artifact_nightly_info, ArtifactError, ArtifactKey,
    ArtifactTypes, NightlyArtifactResponder, RenderableArtifact,
};
use crate::r#impl::config::Config;
#[cfg(feature = "github")]
use crate::r#impl::github::GithubClient;
use crate::r#impl::markdown::markdown;
#[cfg(feature = "github")]
use crate::r#impl::nightly::github_cache::{
    get_cached_artifact, get_cached_artifact_run_id, store_cached_artifact_run,
};
use crate::r#impl::storage::{DownloadSpec, Product};
use crate::r#impl::templates::{DownloadGridTemplate, TemplateNightly};

#[cfg(feature = "github")]
mod github_cache;

const PSEUDO_ENDPOINT_NAME: &str = "nightly-download";

#[cfg(feature = "github")]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NightlyGitHubConfig {
    org: String,
    repo: String,
    workflow: String,
    branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NightlyConfig {
    #[cfg(feature = "github")]
    github: NightlyGitHubConfig,
    downloads: IndexMap<ArtifactKey, DownloadSpec>,
}

async fn nightly_artifacts_collect(
    product_name: &str,
    nightly_config: &NightlyConfig,
    settings: &HashMap<ArtifactKey, Value>,
    ats: &ArtifactTypes,
) -> Vec<RenderableArtifact<'static>> {
    let mut artifacts = Vec::new();
    for (key, download_spec) in &nightly_config.downloads {
        match get_artifact_nightly_info(ats, key, product_name, download_spec, settings.get(key))
            .await
        {
            Ok(artifact_info) => {
                let mut urls = BTreeMap::new();
                urls.insert(
                    Cow::Borrowed(PSEUDO_ENDPOINT_NAME),
                    format!("/nightly-download/{}/{}", product_name, key).into(),
                );
                artifacts.push(RenderableArtifact {
                    icon_path: artifact_info
                        .icon()
                        .map(ToString::to_string)
                        .map(Into::into),
                    display_name: artifact_info.display_name().clone_owned(),
                    modified_date: None,
                    file_size: None,
                    urls,
                    extra_info_markdown: artifact_info
                        .extra_info_markdown()
                        .map(|s| s.to_string())
                        .map(Into::into),
                })
            }
            Err(err) => log::warn!(
                "Was unable to serve nightly artifact info '{}' of '{}': {}",
                key,
                product_name,
                err
            ),
        }
    }
    artifacts
}

#[cfg(feature = "github")]
async fn get_github_nightly_info(
    config: &NightlyGitHubConfig,
) -> Result<(Option<i64>, Cow<'static, str>), Status> {
    match GithubClient::get_instance()
        .fetch_latest_successful_workflow_run(
            &config.org,
            &config.repo,
            &config.workflow,
            &config.branch,
        )
        .await
    {
        Ok(Some(run)) => {
            let time = run.updated_at.timestamp();
            let head_commit_url = format!(
                "https://github.com/{}/{}/commit/{}",
                &config.org, &config.repo, &run.head_commit.id,
            );
            let comment = markdown(&format!(
                "Latest commit: [{}]({}) by {}: *{}*.\n\n\
                 Based on latest successful run: [#{}]({}).",
                &run.head_commit.id[..7],
                head_commit_url,
                &run.head_commit.author.name,
                &run.head_commit.message.split("\n").next().unwrap_or(""),
                run.run_number,
                &run.html_url
            ));
            Ok((Some(time), Cow::Owned(comment)))
        }
        Ok(None) => Ok((None, Cow::Borrowed(""))),
        Err(e) => {
            error!(
                "Failed to fetch latest successful workflow run from GitHub: {}: {:?}",
                e,
                e.source()
            );
            Err(Status::InternalServerError)
        }
    }
}

pub async fn do_get_nightly<'a>(
    config: &'a State<Config>,
    product_key: &'a str,
    product_data: &Product,
) -> Result<TemplateNightly<'a>, Status> {
    if let Some(nightly_config) = product_data.nightly.as_ref() {
        let artifacts = nightly_artifacts_collect(
            product_key,
            nightly_config,
            &product_data.settings,
            config.artifact_types(),
        )
        .await;

        #[cfg(feature = "github")]
        let (last_built_time, description) =
            get_github_nightly_info(&nightly_config.github).await?;
        #[cfg(not(feature = "github"))]
        let (last_built_time, description) = (None, Cow::Borrowed(""));

        let downloads = DownloadGridTemplate {
            theme_name: config.theme().into(),
            auto_endpoint: Cow::Borrowed(PSEUDO_ENDPOINT_NAME),
            show_file_size_and_date: false,
            artifacts,
        };
        Ok(TemplateNightly {
            self_name: config.self_name().into(),
            theme_name: config.theme().into(),
            home_url: config.home_url().into(),
            product_key: product_key.into(),
            product_title: product_data.name.to_string().into(),
            product_icon: product_data.icon_path.clone().map(Into::into),
            default_endpoint_url: config.default_endpoint_url().into(),
            last_built_time,
            description,
            downloads,
        })
    } else {
        Err(Status::NotFound)
    }
}

pub async fn do_get_nightly_artifact<'a>(
    config: &'a State<Config>,
    product_key: &'a str,
    product_data: &Product,
    artifacttype: &'a str,
) -> Result<NightlyArtifactResponder, Status> {
    if let Some(nightly_config) = product_data.nightly.as_ref() {
        if let Some(download_spec) = nightly_config.downloads.get(artifacttype) {
            get_artifact_nightly_download(
                config.artifact_types(),
                artifacttype,
                product_key,
                download_spec,
                product_data.settings.get(artifacttype),
                nightly_config,
            )
            .await
            .map_err(|err| match err {
                ArtifactError::NoFallback | ArtifactError::NotSupported => Status::NotFound,
                e => {
                    error!("Artifact download error for nightly: {:?}", e);
                    Status::InternalServerError
                }
            })
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

pub async fn get_generic_nightly_artifact_download(
    product_key: &str,
    download_spec: &DownloadSpec,
    nightly_config: &NightlyConfig,
) -> Result<NightlyArtifactResponder, ArtifactError> {
    #[cfg(feature = "github")]
    {
        let ghartifact_name = download_spec.url();
        let ghclient = GithubClient::get_instance();
        let ghconfig = &nightly_config.github;
        let last_run = ghclient
            .fetch_latest_successful_workflow_run(
                &ghconfig.org,
                &ghconfig.repo,
                &ghconfig.workflow,
                &ghconfig.branch,
            )
            .await
            .map_err(|e| ArtifactError::Custom(Box::new(e)))?
            .ok_or_else(|| ArtifactError::NotSupported)?;

        let last_run_id = last_run.id.to_string();
        if get_cached_artifact_run_id(product_key, ghartifact_name).await != last_run_id {
            let binartifact = ghclient
                .fetch_workflow_run_artifact(
                    &ghconfig.org,
                    &ghconfig.repo,
                    last_run.id,
                    ghartifact_name,
                )
                .await
                .map_err(|e| ArtifactError::Custom(Box::new(e)))?
                .ok_or_else(|| ArtifactError::NotSupported)?;
            store_cached_artifact_run(product_key, ghartifact_name, last_run_id, binartifact)
                .await
                .map_err(|e| ArtifactError::Custom(Box::new(e)))?;
        }

        Ok(NightlyArtifactResponder::File(
            get_cached_artifact(product_key, ghartifact_name)
                .await
                .map_err(|e| ArtifactError::Custom(Box::new(e)))?,
        ))
    }
    #[cfg(not(feature = "github"))]
    {
        error!("Generic artifact downloads are currently only implemented for when Github support is enabled");
        Err(ArtifactError::NotSupported)
    }
}
