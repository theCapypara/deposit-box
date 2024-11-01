use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::net::IpAddr;
use std::path::PathBuf;

use dotenv::dotenv;
#[cfg(feature = "geoip")]
use geoutils::Location;
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use regex::Regex;
#[cfg(feature = "s3_bucket_list")]
use s3::serde_types::ListBucketResult;
use tokio::fs::read_to_string;

use crate::r#impl::artifacttype::ArtifactTypes;
#[cfg(feature = "geoip")]
use crate::r#impl::geoip::{find_best_location, self_server_ip, sort_by_location};
#[cfg(feature = "github")]
use crate::r#impl::github::GithubClient;
use crate::r#impl::storage::{ProductsConfig, Storage, StorageError};
#[cfg(feature = "amazon_translate")]
use crate::r#impl::translate::TranslateConfig;

#[cfg(not(feature = "geoip"))]
/// If the GeoIP feature is not enabled, Location is simply a unit type.
type Location = ();

lazy_static! {
    static ref ENDPOINTS_NAME_PATTERN: Regex =
        Regex::new(r"DEPBOX_S3_ENDPOINT__(\d+)__(.+?)__DISPLAY_NAME").unwrap();
    static ref ENDPOINTS_URL_PATTERN: Regex =
        Regex::new(r"DEPBOX_S3_ENDPOINT__(\d+)__(.+?)__URL").unwrap();
    static ref ENDPOINTS_LOC_PATTERN: Regex =
        Regex::new(r"DEPBOX_S3_ENDPOINT__(\d+)__(.+?)__LOC").unwrap();
}

pub struct Config {
    #[cfg(feature = "geoip")]
    geoipdb: Option<maxminddb::Reader<Vec<u8>>>,
    banner: bool,
    release_info: Option<String>,
    storage: Storage,
    artifacttypes: ArtifactTypes,
    theme: String,
    home_url: String,
    self_name: String,
    #[cfg(feature = "amazon_translate")]
    translate: Option<TranslateConfig>,
    // Overwrite location to products.yml to read, just for development!
    products_yml_path: Option<String>,
}

impl Config {
    #[allow(clippy::result_unit_err)]
    pub fn load(artifacttypes: ArtifactTypes) -> Result<Self, ()> {
        debug!("-- Loading config from environment and .env file... --");

        if let Err(err) = Self::init_env() {
            error!("Failed to load environment: {}.", err);
            return Err(());
        }

        #[cfg(feature = "geoip")]
        let geoipdb = Self::load_geoipdb()
            .map(|r| {
                r.map_err(|e| {
                    warn!("Failed to load Maxmind-compatible GeoIP database: {}", e);
                })
            })
            .transpose()?;

        #[cfg(feature = "geoip")]
        let endpoints = Endpoints::load(
            #[cfg(feature = "geoip")]
            geoipdb.as_ref(),
        );
        #[cfg(not(feature = "geoip"))]
        let endpoints = Endpoints::load();

        #[cfg(feature = "github")]
        GithubClient::init_token(GithubToken::get());

        let storage =
            Storage::new(endpoints).map_err(|e| error!("Failed to initialize storage: {}", e))?;

        let slf = Self {
            storage,
            #[cfg(feature = "geoip")]
            geoipdb,
            banner: BannerEnable::get(),
            release_info: match ReleaseInfoEnable::get() {
                true => Some(ReleaseInfoDomain::get()),
                false => None,
            },
            artifacttypes,
            theme: Theme::get(),
            home_url: HomeUrl::get(),
            self_name: SelfName::get_checked()
                .ok()
                .unwrap_or_else(SelfName::default_value),
            #[cfg(feature = "amazon_translate")]
            translate: TranslateConfig::get(),
            products_yml_path: ProductsYmlPath::get_checked().ok(),
        };

        if !Self::check_env(&slf.endpoints()) {
            return Err(());
        }

        if !PathBuf::from(format!("view/static/theme/{}", Theme::get())).exists() {
            error!(
                "Theme files directory (view/static/theme/{}) does not exist.",
                Theme::get()
            );
            return Err(());
        }

        Ok(slf)
    }

    pub fn provide_banner(&self) -> bool {
        self.banner
    }

    pub fn get_release_info(&self) -> &Option<String> {
        &self.release_info
    }

    fn init_env() -> dotenv::Result<()> {
        dotenv()
            .map(|_| ())
            .or_else(|err| if err.not_found() { Ok(()) } else { Err(err) })
    }

    fn check_env(endpoints: &Endpoints) -> bool {
        if endpoints.get_all().is_empty() {
            error!("No endpoints configured.");
            return false;
        } else {
            info!("Endpoints:");
            for endpoint in endpoints.get_all() {
                info!("-> {:?}", endpoint);
            }
        }

        match SelfName::get_checked() {
            Ok(value) => info!("Self Name (title shown in browser): {}", value),
            Err(_) => warn!(
                "Self Name (title shown in browser) not configured. Using default: {}",
                SelfName::default_value()
            ),
        }

        match HomeUrl::get_checked() {
            Ok(value) => info!("Home URL: {}", value),
            Err(err) => {
                error!("Home URL not configured: {}", err);
                return false;
            }
        }

        match Theme::get_checked() {
            Ok(value) => info!("Theme: {}", value),
            Err(err) => {
                error!("Theme not configured: {}", err);
                return false;
            }
        }

        if ReleaseInfoEnable::get() {
            info!("Release Info: enabled");
            match ReleaseInfoDomain::get_checked() {
                Ok(value) => info!("Release Info Domain: {}", value),
                Err(err) => {
                    error!("Release Info Domain not configured: {}", err);
                    return false;
                }
            }
        } else {
            info!("Release Info: disabled");
        }

        if BannerEnable::get() {
            info!("Serving banner: enabled (/banner for URL, /banner.png for image)");
        } else {
            info!("Serving banner: disabled");
        }

        match MaxmindDbPath::get_checked() {
            Ok(value) => info!("Maxmind DB path: {}", value),
            Err(err) => {
                warn!("Maxmind DB path not configured: {}", err);
                warn!("-> GeoIP mirror selection disabled.");
            }
        }

        true
    }

    #[cfg(feature = "geoip")]
    fn load_geoipdb() -> Option<Result<maxminddb::Reader<Vec<u8>>, maxminddb::MaxMindDBError>> {
        MaxmindDbPath::get_checked()
            .map(maxminddb::Reader::open_readfile)
            .ok()
    }

    pub fn self_name(&self) -> &str {
        self.self_name.as_str()
    }

    pub fn default_endpoint_url(&self) -> &str {
        &self.storage.endpoints().get_all()[0].url
    }

    pub fn theme(&self) -> &str {
        self.theme.as_str()
    }

    pub fn home_url(&self) -> &str {
        self.home_url.as_str()
    }

    pub fn artifact_types(&self) -> &ArtifactTypes {
        &self.artifacttypes
    }

    pub fn endpoints(&self) -> &Endpoints {
        &self.storage.endpoints()
    }

    /// Returns the product configuration, or an error on error. The result may be cached.
    pub async fn get_config(&self) -> Result<ProductsConfig, StorageError> {
        if let Some(overwitten_path) = self.products_yml_path.as_ref() {
            warn!(
                "Loading products.yml from overwritten path {}",
                overwitten_path
            );

            Ok(serde_yaml::from_str(
                &read_to_string(overwitten_path).await?,
            )?)
        } else {
            self.storage.get_config().await
        }
    }

    /// Returns the banner target URL, may be cached.
    pub async fn get_banner_url_url(&self) -> Option<String> {
        self.get_config()
            .await
            .ok()
            .and_then(|p| p.banner.map(|b| b.url_file))
    }

    /// Returns the banner PNG URL, may be cached.
    pub async fn get_banner_png_url(&self) -> Option<String> {
        self.get_config()
            .await
            .ok()
            .and_then(|p| p.banner.map(|b| b.image_file))
    }

    #[cfg(feature = "geoip")]
    pub fn find_best_location(&self, addr: IpAddr) -> &Endpoint {
        match &self.geoipdb {
            None => &self.storage.endpoints().get_all()[0],
            Some(geoipdb) => find_best_location(self.storage.endpoints().get_all(), geoipdb, addr),
        }
    }

    #[cfg(not(feature = "geoip"))]
    pub fn find_best_location(&self, _addr: IpAddr) -> &Endpoint {
        &self.storage.endpoints().get_all()[0]
    }

    #[cfg(feature = "s3_bucket_list")]
    /// Returns the S3-compatible bucket listing or None, if the endpoint does not provide a listing.
    /// The result may be cached. If no listing can be retrieved a warning will be logged on the
    /// first call to this function.
    pub async fn get_bucket_list(&self) -> Option<Vec<ListBucketResult>> {
        self.storage.get_bucket_list().await
    }

    #[cfg(feature = "amazon_translate")]
    /// Returns an Amazon Translate client, if available.
    pub fn get_translate_client(&self) -> Option<&aws_sdk_translate::Client> {
        self.translate.as_ref().map(|v| &v.client)
    }
}

pub(crate) trait SimpleConfig {
    const VAR_NAME: &'static str;

    fn get() -> String {
        Self::get_checked().expect("Expected getting a config variable value.")
    }

    fn get_checked() -> Result<String, env::VarError> {
        env::var(Self::VAR_NAME)
    }
}

trait SimpleConfigBool {
    const VAR_NAME: &'static str;

    fn get() -> bool {
        env::var(Self::VAR_NAME).map_or(false, |x| x.trim() != "0")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub key: String,
    pub display_name: String,
    pub url: String,
    pub location: Location,
}

pub struct Endpoints {
    _loaded: Vec<Endpoint>,
    _loaded_map: HashMap<String, Endpoint>,
}

impl Endpoints {
    pub fn load(#[cfg(feature = "geoip")] geoipdb: Option<&maxminddb::Reader<Vec<u8>>>) -> Self {
        let mut endpoints = Self::do_load_from_env();

        #[cfg(feature = "geoip")]
        if let Some(geoipdb) = geoipdb {
            sort_by_location(&mut endpoints, geoipdb, self_server_ip());
        }

        Self {
            _loaded_map: endpoints
                .iter()
                .map(|v| (v.key.clone(), v.clone()))
                .collect(),
            _loaded: endpoints,
        }
    }

    pub fn get_all(&self) -> &[Endpoint] {
        self._loaded.as_slice()
    }

    fn do_load_from_env() -> Vec<Endpoint> {
        enum InsertPos {
            FirstDisplayName,
            SecondUrl,
            ThirdLocation,
        }
        struct EndpointsLoadMapValue(Option<String>, Option<String>, Option<Location>, usize);
        type EndpointsLoadMap = HashMap<String, EndpointsLoadMapValue>;

        // Todo: pretty messy this was kinda an afterthought.
        #[allow(unused)]
        fn parse_loc(input: String) -> Location {
            #[cfg(feature = "geoip")]
            {
                let parts: Vec<&str> = input.split(' ').collect();
                if parts.len() != 2 {
                    panic!(
                        "failed to parse location string: {}. needs to have two parts.",
                        input
                    );
                }
                Location::new(
                    parts[0]
                        .parse::<f64>()
                        .expect("expected to parse location string. invalid latitude."),
                    parts[1]
                        .parse::<f64>()
                        .expect("expected to parse location string. invalid longitude."),
                )
            }
        }

        #[allow(clippy::unit_arg)]
        fn insert_into(
            map: &mut EndpointsLoadMap,
            pos: InsertPos,
            key: String,
            value: String,
            order: usize,
        ) {
            match map.entry(key) {
                Entry::Occupied(mut oe) => match pos {
                    InsertPos::FirstDisplayName => oe.get_mut().0 = Some(value),
                    InsertPos::SecondUrl => oe.get_mut().1 = Some(value),
                    InsertPos::ThirdLocation => oe.get_mut().2 = Some(parse_loc(value)),
                },
                Entry::Vacant(ve) => {
                    ve.insert(match pos {
                        InsertPos::FirstDisplayName => {
                            EndpointsLoadMapValue(Some(value), None, None, order)
                        }
                        InsertPos::SecondUrl => {
                            EndpointsLoadMapValue(None, Some(value), None, order)
                        }
                        InsertPos::ThirdLocation => {
                            EndpointsLoadMapValue(None, None, Some(parse_loc(value)), order)
                        }
                    });
                }
            }
        }

        let mut endpoints: EndpointsLoadMap = HashMap::with_capacity(10);

        for (key, value) in env::vars() {
            if let Some(captures) = ENDPOINTS_NAME_PATTERN.captures(&key) {
                insert_into(
                    &mut endpoints,
                    InsertPos::FirstDisplayName,
                    captures[2].to_string(),
                    value,
                    captures[1].parse().unwrap(),
                );
            } else if let Some(captures) = ENDPOINTS_URL_PATTERN.captures(&key) {
                insert_into(
                    &mut endpoints,
                    InsertPos::SecondUrl,
                    captures[2].to_string(),
                    value,
                    captures[1].parse().unwrap(),
                );
            } else if let Some(captures) = ENDPOINTS_LOC_PATTERN.captures(&key) {
                insert_into(
                    &mut endpoints,
                    InsertPos::ThirdLocation,
                    captures[2].to_string(),
                    value,
                    captures[1].parse().unwrap(),
                );
            }
        }

        let mut final_endpoints = Vec::with_capacity(endpoints.len());
        let endpoints_iter = endpoints
            .into_iter()
            .sorted_by_key(|(_, EndpointsLoadMapValue(_, _, _, o))| *o);
        for (key, EndpointsLoadMapValue(name, url, loc, _)) in endpoints_iter {
            if let (Some(display_name), Some(url), Some(location)) = (name, url, loc) {
                final_endpoints.push(Endpoint {
                    key,
                    display_name,
                    url,
                    location,
                })
            }
        }
        final_endpoints
    }
}

struct HomeUrl {}

impl SimpleConfig for HomeUrl {
    const VAR_NAME: &'static str = "DEPBOX_HOME_URL";
}

struct SelfName {}

impl SimpleConfig for SelfName {
    const VAR_NAME: &'static str = "DEPBOX_SELF_NAME";
}

impl SelfName {
    pub fn default_value() -> String {
        "Deposit Box".to_string()
    }
}

struct Theme {}

impl SimpleConfig for Theme {
    const VAR_NAME: &'static str = "DEPBOX_THEME";
}

struct ReleaseInfoEnable {}

impl SimpleConfigBool for ReleaseInfoEnable {
    const VAR_NAME: &'static str = "DEPBOX_RELEASE_INFO_ENABLE";
}

struct ReleaseInfoDomain {}

impl SimpleConfig for ReleaseInfoDomain {
    const VAR_NAME: &'static str = "DEPBOX_RELEASE_INFO_DOMAIN";
}

struct BannerEnable {}

impl SimpleConfigBool for BannerEnable {
    const VAR_NAME: &'static str = "DEPBOX_BANNER_ENABLE";
}

struct MaxmindDbPath {}

impl SimpleConfig for MaxmindDbPath {
    const VAR_NAME: &'static str = "DEPBOX_MAXMINDDB_PATH";
}

struct GithubToken {}

impl SimpleConfig for GithubToken {
    const VAR_NAME: &'static str = "DEPBOX_GITHUB_TOKEN";
}

struct ProductsYmlPath {}

impl SimpleConfig for ProductsYmlPath {
    const VAR_NAME: &'static str = "DEPBOX_OVERWRITE_PRODUCTS_YML_PATH";
}
