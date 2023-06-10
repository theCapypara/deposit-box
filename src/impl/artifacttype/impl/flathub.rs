use crate::r#impl::artifacttype::{ArtifactError, ArtifactInfo, ArtifactType};
use crate::r#impl::config::Config;
use crate::r#impl::release_map::NamedVersion;
use crate::r#impl::routes::{get_storage_config, is_release_info};
use async_trait::async_trait;
use indexmap::IndexMap;
use rocket::get;
use rocket::http::uri::Host;
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::{Request, Response, State};
use serde_yaml::Value;
use std::borrow::Cow;
use std::io::Cursor;
use std::marker::PhantomData;

pub trait FlathubBranch: Send + Sync {
    const ARTIFACT_KEY: &'static str;
    const BRANCH: &'static str;
}
pub struct FlathubStable;
impl FlathubBranch for FlathubStable {
    const ARTIFACT_KEY: &'static str = "flathub";
    const BRANCH: &'static str = "stable";
}
pub struct FlathubBeta;
impl FlathubBranch for FlathubBeta {
    const ARTIFACT_KEY: &'static str = "flathub_beta";
    const BRANCH: &'static str = "beta";
}

pub struct FlathubArtifactType<T: FlathubBranch>(pub(crate) PhantomData<T>);
const FLATHUB_URL: &str = "https://dl.flathub.org/repo/";
const FLATHUB_RUNTIME_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

#[async_trait]
impl<T: FlathubBranch> ArtifactType for FlathubArtifactType<T> {
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
        download_value: &'a str,
        setting: Option<&'a Value>,
    ) -> Result<ArtifactInfo<'a>, ArtifactError> {
        match setting {
            Some(Value::String(_)) => {
                let mut info = ArtifactInfo::new_url(
                    "Linux Flatpak",
                    Some("flatpak.png".into()),
                    format!("/{}/{}/{}", T::ARTIFACT_KEY, product_name, version).into(),
                );
                info.set_extra_info_markdown(
                    format!(
                        "See [Quick Setup](https://flatpak.org/setup/) to get started.\n\n*Link is to latest stable. To \
                        [downgrade](https://docs.flatpak.org/en/latest/tips-and-tricks.html#downgrading) \
                        to this release use commit \
                        **{}**.*",
                        download_value
                    )
                    .into(),
                );
                Ok(info)
            }
            _ => Err(ArtifactError::MissingSetting),
        }
    }
}

#[get("/flathub/<product>/<release>")]
pub async fn get_flatpakref<'a>(
    host: &'a Host<'a>,
    config: &'a State<Config>,
    product: &'a str,
    #[allow(unused)] release: &'a str,
) -> Result<Flatpakref<'a>, Status> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        get_flatpakref_impl(
            config,
            product,
            FlathubStable::ARTIFACT_KEY,
            FlathubStable::BRANCH,
        )
        .await
    }
}

#[get("/flathub_beta/<product>/<release>")]
pub async fn get_flatpakref_beta<'a>(
    host: &'a Host<'a>,
    config: &'a State<Config>,
    product: &'a str,
    #[allow(unused)] release: &'a str,
) -> Result<Flatpakref<'a>, Status> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        get_flatpakref_impl(
            config,
            product,
            FlathubBeta::ARTIFACT_KEY,
            FlathubBeta::BRANCH,
        )
        .await
    }
}

async fn get_flatpakref_impl<'a>(
    config: &'a State<Config>,
    product: &'a str,
    key: &'static str,
    branch: &'static str,
) -> Result<Flatpakref<'a>, Status> {
    let mut products = get_storage_config(config).await?.products;
    if let Some(product_data) = products.remove(product) {
        let setting = product_data.settings.get(key);
        // TODO: Flatpakref doesn't really allow specifying a special commit, so we always serve latest for now.
        if let Some(Value::String(flatpak_id)) = setting {
            Ok(Flatpakref {
                name: flatpak_id.to_string(),
                branch,
                title: product_data.name,
                url: FLATHUB_URL,
                runtime_repo: FLATHUB_RUNTIME_REPO,
            })
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

pub struct Flatpakref<'a> {
    name: String,
    branch: &'a str,
    title: String,
    url: &'a str,
    runtime_repo: &'a str,
}

impl<'r, 'o: 'r> Responder<'r, 'r> for Flatpakref<'r> {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'r> {
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

impl<'a> From<Flatpakref<'a>> for String {
    fn from(fr: Flatpakref<'a>) -> Self {
        format!(
            "[Flatpak Ref]\n\
            Version=1\n\
            Name={}\n\
            Branch={}\n\
            Title={}\n\
            Url={}\n\
            RuntimeRepo={}\n",
            fr.name, fr.branch, fr.title, fr.url, fr.runtime_repo
        )
    }
}
