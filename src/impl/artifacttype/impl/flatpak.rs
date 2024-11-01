use std::borrow::Cow;
use std::io::Cursor;

use async_trait::async_trait;
use indexmap::IndexMap;
use rocket::get;
use rocket::http::uri::Host;
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::{Request, Response, State};
use serde_yaml::{Mapping, Value};

use crate::r#impl::artifacttype::{
    ArtifactError, ArtifactInfo, ArtifactType, NightlyArtifactResponder,
};
use crate::r#impl::config::Config;
use crate::r#impl::nightly::NightlyConfig;
use crate::r#impl::release_map::NamedVersion;
use crate::r#impl::routes::{get_storage_config, is_release_info};
use crate::r#impl::storage::DownloadSpec;

pub const FLATPAK_CUSTOM_ARTIFACT_KEY: &str = "flatpak_custom";
pub const FLATHUB_STABLE_ARTIFACT_KEY: &str = "flathub";
pub const FLATHUB_BETA_ARTIFACT_KEY: &str = "flathub_beta";
const FLATHUB_URL: &str = "https://dl.flathub.org/repo/";
const FLATHUB_RUNTIME_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

#[derive(Debug, Clone)]
pub struct FlatpakRepo {
    url: Cow<'static, str>,
    suggested_name: Cow<'static, str>,
    gpg_verify: bool,
    branch: Cow<'static, str>,
}

impl FlatpakRepo {
    #[allow(unused)]
    pub fn flatpak_custom(
        url: String,
        suggested_name: String,
        gpg_verify: bool,
        branch: String,
    ) -> Self {
        Self {
            url: url.into(),
            suggested_name: suggested_name.into(),
            gpg_verify,
            branch: branch.into(),
        }
    }

    pub const fn flathub_stable() -> Self {
        Self {
            url: Cow::Borrowed(FLATHUB_URL),
            suggested_name: Cow::Borrowed("Flathub"),
            gpg_verify: true,
            branch: Cow::Borrowed("stable"),
        }
    }

    pub const fn flathub_beta() -> Self {
        Self {
            url: Cow::Borrowed(FLATHUB_URL),
            suggested_name: Cow::Borrowed("Flathub Beta"),
            gpg_verify: true,
            branch: Cow::Borrowed("beta"),
        }
    }
}

pub struct FlatpakArtifactType {
    artifact_key: &'static str,
    repo: Option<FlatpakRepo>,
}

#[async_trait]
impl ArtifactType for FlatpakArtifactType {
    async fn describe<'a>(
        &self,
        _description_map: &mut IndexMap<Cow<'a, str>, Cow<'a, str>>,
        _setting: Option<&Value>,
        _version: &NamedVersion<'_>,
    ) {
    }

    async fn get_artifact<'a>(
        &self,
        product_name: &'a str,
        version: &'a str,
        _download_spec: &'a DownloadSpec,
        setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        self.get_artifact_info(
            setting,
            Some(format!(
                "/{}/{}/{}",
                self.artifact_key, product_name, version
            )),
        )
        .await
    }

    async fn get_nightly_artifact_info<'a>(
        &self,
        _product_name: &'a str,
        _download_spec: &'a DownloadSpec,
        setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        self.get_artifact_info(setting, None).await
    }

    async fn get_nightly_artifact_download<'a>(
        &self,
        product_name: &'a str,
        _download_spec: &'a DownloadSpec,
        setting: Option<&'a Value>,
        _nightly_config: &'a NightlyConfig,
    ) -> Result<NightlyArtifactResponder, ArtifactError> {
        let (repo_info, package_id) = self.get_infos(setting)?;
        Ok(NightlyArtifactResponder::Flatpakref(Flatpakref {
            name: Cow::Owned(package_id.to_string()),
            branch: repo_info.branch.clone(),
            title: Cow::Owned(product_name.to_string()),
            url: repo_info.url.clone(),
            gpg_verify: repo_info.gpg_verify,
            runtime_repo: Cow::Borrowed(FLATHUB_RUNTIME_REPO),
        }))
    }
}

impl FlatpakArtifactType {
    pub fn new(artifact_key: &'static str, repo: Option<FlatpakRepo>) -> Self {
        Self { artifact_key, repo }
    }

    async fn get_artifact_info<'a>(
        &self,
        setting: Option<&'a Value>,
        url: Option<String>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        let (repo_info, _package_id) = self.get_infos(setting)?;

        let mut info = match url {
            None => ArtifactInfo::new_empty("Linux Flatpak", Some("flatpak.png".into())),
            Some(url) => {
                ArtifactInfo::new_url("Linux Flatpak", Some("flatpak.png".into()), url.into())
            }
        };
        // TODO: We don't show the downgrade text anymore, it's too cluttered.
        /*let commit = download_spec.url();
        let downgrade_text = if !commit.is_empty() && commit != "pending" {
            Cow::Owned(
                format!(" *To [downgrade](https://docs.flatpak.org/en/latest/tips-and-tricks.html#downgrading) \
                        to this release use commit \
                        **{}**.*", download_spec.url())
            )
        } else {
            Cow::Borrowed("")
        };*/
        let setup_text = if self.artifact_key == FLATPAK_CUSTOM_ARTIFACT_KEY {
            Cow::Owned(format!(
                "You may need to set up our custom Flatpak repo first using\n\n```{}```",
                flatpak_remote_add(&repo_info)
            ))
        } else {
            Cow::Borrowed("See [Quick Setup](https://flatpak.org/setup/) to get started.")
        };
        info.set_extra_info_markdown(setup_text);
        Ok(info)
    }

    async fn get_flatpakref_impl(
        &self,
        config: &State<Config>,
        product: &str,
    ) -> Result<Flatpakref, Status> {
        let mut products = get_storage_config(config).await?.products;

        if let Some(product_data) = products.swap_remove(product) {
            let setting = product_data.settings.get(self.artifact_key);
            // TODO: Flatpakref doesn't really allow specifying a special commit, so we always serve latest for now.
            let (repo_info, package_id) = self.get_infos(setting).map_err(|_| Status::NotFound)?;
            Ok(Flatpakref {
                name: Cow::Owned(package_id.to_string()),
                branch: repo_info.branch.clone(),
                title: Cow::Owned(product_data.name.clone()),
                url: repo_info.url.clone(),
                gpg_verify: repo_info.gpg_verify,
                runtime_repo: Cow::Borrowed(FLATHUB_RUNTIME_REPO),
            })
        } else {
            Err(Status::NotFound)
        }
    }

    fn get_infos<'a>(
        &'a self,
        setting: Option<&'a Value>,
    ) -> Result<(Cow<'a, FlatpakRepo>, Cow<'a, str>), ArtifactError> {
        // If we have repo settings set, then we only need the package id from the settings.
        // Otherwise we need all from the settings.
        if self.repo.is_some() {
            match setting {
                Some(Value::String(package_id)) => Ok((
                    Cow::Borrowed(self.repo.as_ref().unwrap()),
                    package_id.into(),
                )),
                _ => Err(ArtifactError::MissingSetting),
            }
        } else {
            match setting {
                Some(Value::Mapping(mapping)) => {
                    let repo = FlatpakRepo {
                        url: get_string(mapping, "repo")?.into(),
                        suggested_name: get_string(mapping, "repo_suggested_name")?.into(),
                        gpg_verify: get_bool(mapping, "repo_gpg_verify")?,
                        branch: get_string(mapping, "repo_branch")?.into(),
                    };
                    Ok((Cow::Owned(repo), get_string(mapping, "package")?.into()))
                }
                _ => Err(ArtifactError::MissingSetting),
            }
        }
    }
}

fn flatpak_remote_add(repo_info: &FlatpakRepo) -> String {
    let no_gpg_verify = if repo_info.gpg_verify {
        ""
    } else {
        " --no-gpg-verify"
    };
    format!(
        "flatpak --user remote-add{} {} {}",
        no_gpg_verify, repo_info.suggested_name, repo_info.url
    )
}

fn get_string(mapping: &Mapping, key: &'static str) -> Result<String, ArtifactError> {
    match mapping.get(key) {
        Some(Value::String(v)) => Ok(v.to_string()),
        _ => Err(ArtifactError::MissingSetting),
    }
}

fn get_bool(mapping: &Mapping, key: &'static str) -> Result<bool, ArtifactError> {
    match mapping.get(key) {
        Some(Value::Bool(v)) => Ok(*v),
        _ => Err(ArtifactError::MissingSetting),
    }
}

#[get("/flathub/<product>/<release>")]
pub async fn get_flatpakref(
    host: &Host<'_>,
    config: &State<Config>,
    product: &str,
    #[allow(unused)] release: &str,
) -> Result<Flatpakref, Status> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        FlatpakArtifactType::new(
            FLATHUB_STABLE_ARTIFACT_KEY,
            Some(FlatpakRepo::flathub_stable()),
        )
        .get_flatpakref_impl(config, product)
        .await
    }
}

#[get("/flathub_beta/<product>/<release>")]
pub async fn get_flatpakref_beta(
    host: &Host<'_>,
    config: &State<Config>,
    product: &str,
    #[allow(unused)] release: &str,
) -> Result<Flatpakref, Status> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        FlatpakArtifactType::new(FLATHUB_BETA_ARTIFACT_KEY, Some(FlatpakRepo::flathub_beta()))
            .get_flatpakref_impl(config, product)
            .await
    }
}

#[get("/flatpak_custom/<product>/<release>")]
pub async fn get_flatpakref_custom(
    host: &Host<'_>,
    config: &State<Config>,
    product: &str,
    #[allow(unused)] release: &str,
) -> Result<Flatpakref, Status> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        FlatpakArtifactType::new(FLATPAK_CUSTOM_ARTIFACT_KEY, None)
            .get_flatpakref_impl(config, product)
            .await
    }
}

pub struct Flatpakref {
    name: Cow<'static, str>,
    branch: Cow<'static, str>,
    title: Cow<'static, str>,
    url: Cow<'static, str>,
    runtime_repo: Cow<'static, str>,
    gpg_verify: bool,
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Flatpakref {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'o> {
        let title = self.name.clone();
        let slf: String = self.into();
        Ok(Response::build()
            .header(ContentType::new("application", "vnd.flatpak.ref"))
            .raw_header(
                "Content-Disposition",
                format!("attachment; filename=\"{}.flatpakref\"", title),
            )
            .sized_body(slf.len(), Cursor::new(slf))
            .finalize())
    }
}

impl From<Flatpakref> for String {
    fn from(fr: Flatpakref) -> Self {
        format!(
            "[Flatpak Ref]\n\
            Version=1\n\
            Name={}\n\
            Branch={}\n\
            Title={}\n\
            Url={}\n\
            RuntimeRepo={}\n\
            gpg-verify={}\n\
            gpg-verify-summary={}\n",
            fr.name, fr.branch, fr.title, fr.url, fr.runtime_repo, fr.gpg_verify, fr.gpg_verify,
        )
    }
}
