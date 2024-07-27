pub mod artifacttype;
pub mod config;
#[cfg(feature = "geoip")]
pub mod geoip;
#[cfg(feature = "github")]
mod github;
mod markdown;
pub mod nightly;
mod pre_release;
pub mod release_map;
pub mod routes;
pub mod storage;
pub mod templates;
#[cfg(feature = "amazon_translate")]
mod translate;
