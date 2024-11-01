#![allow(unused_imports)] // may want to optimize this later. caused by features.
#[macro_use]
extern crate rocket;

use rocket::fs::FileServer;
use rocket::{catchers, routes, Build, Rocket};

#[cfg(feature = "flatpak")]
use r#impl::artifacttype::r#impl::flatpak::{
    get_flatpakref, get_flatpakref_beta, get_flatpakref_custom,
};
use r#impl::routes::*;

use crate::r#impl::config::Config;

mod r#impl;

#[launch]
pub fn rocket() -> Rocket<Build> {
    pretty_env_logger::init_timed();

    if let Ok(config) = Config::load(Default::default()) {
        rocket::build()
            .mount("/", {
                #[cfg(feature = "flatpak")]
                {
                    routes![
                        get_root,
                        get_product,
                        get_release_en,
                        get_release,
                        get_nightly_artifact,
                        get_banner,
                        get_banner_png,
                        favicon,
                        get_flatpakref,
                        get_flatpakref_beta,
                        get_flatpakref_custom
                    ]
                }
                #[cfg(not(feature = "flatpak"))]
                {
                    routes![
                        get_root,
                        get_product,
                        get_release_en,
                        get_release,
                        get_nightly_artifact,
                        get_banner,
                        get_banner_png,
                        favicon,
                    ]
                }
            })
            .mount("/static", FileServer::from("view/static"))
            .register(
                "/",
                catchers![not_found, internal_server_error, other_error],
            )
            .manage(config)
    } else {
        panic!("Could not load configuration.")
    }
}
