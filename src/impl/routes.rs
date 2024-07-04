use crate::r#impl::artifacttype::{artifacts_collect, artifacts_describe};
use crate::r#impl::config::Config;
use crate::r#impl::markdown::markdown;
use crate::r#impl::pre_release::parse_pre_release;
use crate::r#impl::release_map::NamedVersion;
use crate::r#impl::storage::ProductsConfig;
use crate::r#impl::templates::*;
#[cfg(feature = "amazon_translate")]
use crate::r#impl::translate::*;
use async_trait::async_trait;
use cached::proc_macro::cached;
use log::{error, warn};
use rocket::http::uri::Host;
use rocket::http::{ContentType, Header, Status};
use rocket::outcome::Outcome::{Forward, Success};
use rocket::request::{FromRequest, Outcome};
use rocket::response::{Redirect, Responder};
use rocket::{catch, get, Request, State};
use rocket_accept_language::AcceptLanguage;
use std::borrow::Cow;
use std::io::Cursor;
use std::net::IpAddr;

type Response<T> = Result<T, Status>;
const LATEST: &str = "latest";

#[get("/")]
pub async fn get_root<'a>(
    host: &'a Host<'a>,
    config: &'a State<Config>,
) -> Response<TemplateProducts<'a>> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        let products = get_storage_config(config).await?.products;
        Ok(TemplateProducts {
            self_name: config.self_name().into(),
            theme_name: config.theme().into(),
            home_url: config.home_url().into(),
            default_endpoint_url: config.default_endpoint_url().into(),
            products,
        })
    }
}

#[get("/<product>")]
pub async fn get_product<'a>(
    host: &'a Host<'a>,
    config: &'a State<Config>,
    product: &'a str,
) -> Response<GetProductResponder<'a>> {
    let storage_config = get_storage_config(config).await?;
    let mut products = storage_config.products;
    let pre_release_patterns = storage_config.pre_release_patterns;
    if let Some(product_data) = products.swap_remove(product) {
        if is_release_info(config, host) {
            Ok(GetProductResponder::LatestRelease(
                product_data
                    .versions
                    .latest(&pre_release_patterns)
                    .unwrap()
                    .name()
                    .to_string(),
            ))
        } else {
            Ok(GetProductResponder::Template(Box::new(TemplateReleases {
                self_name: config.self_name().into(),
                theme_name: config.theme().into(),
                home_url: config.home_url().into(),
                default_endpoint_url: config.default_endpoint_url().into(),
                product_key: product.into(),
                product: product_data,
                pre_release_patterns,
            })))
        }
    } else {
        Err(Status::NotFound)
    }
}

#[get("/<product>/<release>/en", rank = 1)]
pub async fn get_release_en<'a>(
    host: &'a Host<'a>,
    client_addr: ForwardedIpAddr,
    config: &'a State<Config>,
    product: &'a str,
    release: &'a str,
) -> Response<TemplateRelease<'a>> {
    do_get_release(host, client_addr, config, product, release, None).await
}

#[get("/<product>/<release>")]
pub async fn get_release<'a>(
    host: &'a Host<'a>,
    accept_language: &AcceptLanguage,
    client_addr: ForwardedIpAddr,
    config: &'a State<Config>,
    product: &'a str,
    release: &'a str,
) -> Response<TemplateRelease<'a>> {
    do_get_release(
        host,
        client_addr,
        config,
        product,
        release,
        Some(accept_language),
    )
    .await
}

async fn do_get_release<'a>(
    host: &'a Host<'a>,
    client_addr: ForwardedIpAddr,
    config: &'a State<Config>,
    product: &'a str,
    release: &'a str,
    #[allow(unused)] accept_language: Option<&AcceptLanguage>,
) -> Response<TemplateRelease<'a>> {
    if is_release_info(config, host) {
        Err(Status::NotFound)
    } else {
        let storage_config = get_storage_config(config).await?;
        let products = &storage_config.products;
        if let Some(product_data) = products.get(product) {
            let latest = product_data
                .versions
                .latest(&storage_config.pre_release_patterns);

            let named_version: NamedVersion = if release == LATEST {
                latest.as_ref().unwrap().clone()
            } else if let Some(named_version) = product_data.versions.get(release) {
                named_version
            } else {
                return Err(Status::NotFound);
            };

            let mut iter_versions = product_data.versions.map().keys();
            let mut product_version_prev = None;
            let product_version_next;

            loop {
                if let Some(this_version) = iter_versions.next() {
                    if this_version == named_version.name() {
                        product_version_next = iter_versions.next();
                        break;
                    } else {
                        product_version_prev = Some(this_version);
                    }
                } else {
                    panic!("Logic error trying to find prev and next version.");
                }
            }

            let mut description: Option<Cow<str>> = named_version
                .info()
                .description
                .as_ref()
                .map(AsRef::as_ref)
                .map(markdown)
                .map(Into::into);
            let mut extra_description = artifacts_describe(
                &product_data.settings,
                &named_version,
                config.artifact_types(),
            )
            .await;
            for v in extra_description.values_mut() {
                *v = Cow::Owned(markdown(v))
            }
            let mut translate_note_text_en = None;
            let mut translate_note_text = None;

            #[cfg(feature = "amazon_translate")]
            if let Some(accept_language) = accept_language {
                if let Some(client) = config.get_translate_client() {
                    let lang = accept_language.get_first_language();
                    if let Some(lang) = lang {
                        if let Err(e) = translate_artifact_release(
                            lang.as_str(),
                            client,
                            &mut description,
                            &mut extra_description,
                            &mut translate_note_text_en,
                            &mut translate_note_text,
                        )
                        .await
                        {
                            warn!("Failed translating artifact release to {lang}: {e:?}")
                        }
                    }
                }
            }

            Ok(TemplateRelease {
                self_name: config.self_name().into(),
                theme_name: config.theme().into(),
                home_url: config.home_url().into(),
                default_endpoint_url: config.default_endpoint_url().into(),
                product_key: product.into(),
                release_key: release.into(),
                product_title: product_data.name.to_string().into(),
                product_version: named_version.name().to_string().into(),
                product_version_prev: product_version_prev.cloned().map(Into::into),
                product_version_next: product_version_next.cloned().map(Into::into),
                release_date: named_version.info().date.clone().into(),
                product_icon: product_data.icon_path.clone().map(Into::into),
                description,
                extra_description,
                pre_release: parse_pre_release(
                    named_version.name(),
                    &storage_config.pre_release_patterns,
                )
                .as_ref()
                .map(ToString::to_string)
                .map(Into::into),
                artifacts: artifacts_collect(
                    product,
                    &product_data.settings,
                    &named_version,
                    config.artifact_types(),
                    config.endpoints(),
                    #[cfg(feature = "s3_bucket_list")]
                    config.get_bucket_list().await,
                )
                .await,
                endpoints: config
                    .endpoints()
                    .get_all()
                    .iter()
                    .map(|s| (s.key.as_str().into(), s.display_name.as_str().into()))
                    .collect(),
                auto_endpoint: config.find_best_location(client_addr.0).key.clone().into(),
                translate_note_text_en,
                translate_note_text,
            })
        } else {
            Err(Status::NotFound)
        }
    }
}

#[get("/banner")]
pub async fn get_banner(config: &State<Config>) -> Response<BodyAndHeaders> {
    if config.provide_banner() {
        if let Some(url) = config.get_banner_url_url().await {
            cached_relayed_reqwest(url)
                .await
                .map_err(|_| Status::NotFound)
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

#[get("/banner.png")]
pub async fn get_banner_png(config: &State<Config>) -> Response<BodyAndHeaders> {
    if config.provide_banner() {
        if let Some(url) = config.get_banner_png_url().await {
            cached_relayed_reqwest(url)
                .await
                .map_err(|_| Status::NotFound)
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

#[get("/favicon.ico")]
pub fn favicon(config: &State<Config>) -> Redirect {
    Redirect::permanent(format!("/static/theme/{}/favicon.ico", config.theme()))
}

#[catch(404)]
pub fn not_found<'a>(req: &'a Request) -> Template404<'a> {
    let config = req.rocket().state::<Config>().unwrap();
    Template404 {
        self_name: config.self_name().into(),
        theme_name: config.theme().into(),
        home_url: config.home_url().into(),
    }
}

#[catch(500)]
pub fn internal_server_error<'a>(req: &'a Request) -> Template500<'a> {
    let config = req.rocket().state::<Config>().unwrap();
    Template500 {
        self_name: config.self_name().into(),
        theme_name: config.theme().into(),
        home_url: config.home_url().into(),
    }
}

#[catch(default)]
pub fn other_error<'a>(req: &'a Request) -> Template500<'a> {
    let config = req.rocket().state::<Config>().unwrap();
    Template500 {
        self_name: config.self_name().into(),
        theme_name: config.theme().into(),
        home_url: config.home_url().into(),
    }
}

pub(crate) fn is_release_info(config: &Config, host: &Host) -> bool {
    if let Some(release_info_domain) = config.get_release_info() {
        host.domain().eq(release_info_domain)
    } else {
        false
    }
}

pub(crate) async fn get_storage_config(config: &State<Config>) -> Result<ProductsConfig, Status> {
    config.get_config().await.map_err(|err| {
        error!("Failed to get products config: {}", err);
        Status::InternalServerError
    })
}

pub enum GetProductResponder<'a> {
    LatestRelease(String),
    Template(Box<TemplateReleases<'a>>),
}

impl<'r> Responder<'r, 'r> for GetProductResponder<'r> {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'r> {
        match self {
            GetProductResponder::LatestRelease(release) => release.respond_to(request),
            GetProductResponder::Template(tpl) => tpl.respond_to(request),
        }
    }
}

#[cached(time = 1800, time_refresh = false, sync_writes = true, result = false)]
async fn cached_relayed_reqwest(url: String) -> Result<BodyAndHeaders, ()> {
    match reqwest::get(url).await {
        Ok(c) => {
            let mut headers = Vec::with_capacity(1);
            if let Some(value) = c.headers().get(reqwest::header::CONTENT_TYPE) {
                if let Some(parsed) =
                    ContentType::parse_flexible(value.to_str().unwrap_or_default())
                {
                    headers.push(parsed.into())
                }
            }
            Ok(BodyAndHeaders {
                body: c.bytes().await.map_err(|_| ())?.to_vec(),
                headers,
            })
        }
        Err(_) => Err(()),
    }
}

#[derive(Clone)]
pub struct BodyAndHeaders {
    body: Vec<u8>,
    headers: Vec<Header<'static>>,
}

impl<'r> Responder<'r, 'r> for BodyAndHeaders {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'r> {
        let mut builder = rocket::Response::build();
        for header in self.headers {
            builder.header(header);
        }
        Ok(builder
            .sized_body(self.body.len(), Cursor::new(self.body))
            .finalize())
    }
}

pub struct ForwardedIpAddr(IpAddr);

#[async_trait]
impl<'r> FromRequest<'r> for ForwardedIpAddr {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ip = request
            .headers()
            .get_one("X-Forwarded-For")
            .and_then(|ip| {
                ip.parse()
                    .map_err(|_| warn!("'X-Forwarded-For' header is malformed: {}", ip))
                    .ok()
            })
            .or_else(|| request.client_ip());
        match ip {
            Some(addr) => Success(Self(addr)),
            None => Forward(Status::Ok),
        }
    }
}
