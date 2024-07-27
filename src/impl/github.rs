use std::sync::OnceLock;

use cached::proc_macro::cached;
use octocrab::models::repos::Release;
use octocrab::models::workflows::Run;
use octocrab::models::RunId;
use octocrab::params::actions::ArchiveFormat;
use octocrab::Octocrab;

static GITHUB_TOKEN: OnceLock<String> = OnceLock::new();
static GITHUB_INSTANCE: OnceLock<GithubClient> = OnceLock::new();

pub struct GithubClient {
    client: Octocrab,
}

impl GithubClient {
    pub fn init_token(token: String) {
        // We need to delay the init of the actual client until Tokio is up...
        // So we just init the token.
        GITHUB_TOKEN.get_or_init(|| token);
    }
    pub fn get_instance() -> &'static Self {
        GITHUB_INSTANCE.get_or_init(|| Self {
            client: Octocrab::builder()
                .personal_token(
                    GITHUB_TOKEN
                        .get()
                        .cloned()
                        .expect("GitHub client was not properly initialized."),
                )
                .build()
                .expect("Failed to init GitHub client"),
        })
    }

    pub async fn fetch_release(
        &self,
        org: &str,
        repo: &str,
        version_name: String,
    ) -> Result<Release, String> {
        cached_fetch_github_release(&self.client, org, repo, version_name).await
    }

    pub async fn fetch_latest_successful_workflow_run(
        &self,
        org: &str,
        repo: &str,
        workflow: &str,
        branch: &str,
    ) -> octocrab::Result<Option<Run>> {
        cached_fetch_latest_successful_github_workflow_run(
            &self.client,
            org,
            repo,
            workflow,
            branch,
        )
        .await
    }

    pub async fn fetch_workflow_run_artifact(
        &self,
        org: &str,
        repo: &str,
        run_id: RunId,
        artifact_name: &str,
    ) -> octocrab::Result<Option<bytes::Bytes>> {
        let artifacts = self
            .client
            .actions()
            .list_workflow_run_artifacts(org, repo, run_id)
            .send()
            .await?;

        if let Some(artifacts) = artifacts.value {
            let mut artifact_id = None;
            for artifact in artifacts {
                if artifact.name == artifact_name {
                    artifact_id = Some(artifact.id);
                    break;
                }
            }
            if let Some(artifact_id) = artifact_id {
                self.client
                    .actions()
                    .download_artifact(org, repo, artifact_id, ArchiveFormat::Zip)
                    .await
                    .map(Some)
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[cached(
    time = 7200,
    key = "String",
    convert = r#"{ format!("{}/{}::{}", org, repo, version_name) }"#,
    time_refresh = false,
    sync_writes = true,
    result = true
)]
async fn cached_fetch_github_release(
    client: &Octocrab,
    org: &str,
    repo: &str,
    version_name: String,
) -> Result<Release, String> {
    client
        .repos(org, repo)
        .releases()
        .get_by_tag(&version_name)
        .await
        .map_err(|err| format!("{:?}", err))
}

#[cached(
    time = 7200,
    key = "String",
    convert = r#"{ format!("{}/{}::{}@{}", org, repo, workflow, branch) }"#,
    time_refresh = false,
    sync_writes = true,
    result = true
)]
async fn cached_fetch_latest_successful_github_workflow_run(
    client: &Octocrab,
    org: &str,
    repo: &str,
    workflow: &str,
    branch: &str,
) -> octocrab::Result<Option<Run>> {
    let workflow_runs = client
        .workflows(org, repo)
        .list_runs(workflow)
        .branch(branch)
        .status("success")
        .send()
        .await?;

    Ok(workflow_runs.into_iter().next())
}
