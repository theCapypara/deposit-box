use std::borrow::Cow;

use askama::Template;
use indexmap::IndexMap;

use crate::r#impl::artifacttype::RenderableArtifact;
use crate::r#impl::storage::{PreReleasePatternEntry, Product};

#[derive(Template)]
#[template(path = "p_404.html")]
pub struct Template404<'a> {
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
}

#[derive(Template)]
#[template(path = "p_500.html")]
pub struct Template500<'a> {
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
}

#[derive(Template)]
#[template(path = "p_products.html")]
pub struct TemplateProducts<'a> {
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
    pub default_endpoint_url: Cow<'a, str>,
    pub products: IndexMap<String, Product>,
}

#[derive(Template)]
#[template(path = "p_releases.html")]
pub struct TemplateReleases<'a> {
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
    pub default_endpoint_url: Cow<'a, str>,
    pub product_key: Cow<'a, str>,
    pub product: Product,
    pub pre_release_patterns: Vec<PreReleasePatternEntry>,
    pub has_nightly: bool,
}

#[derive(Template)]
#[template(path = "p_release.html")]
pub struct TemplateRelease<'a> {
    #[allow(dead_code)] // clippy or askama bug?
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
    pub default_endpoint_url: Cow<'a, str>,
    pub product_key: Cow<'a, str>,
    pub release_key: Cow<'a, str>,
    pub product_title: Cow<'a, str>,
    pub product_version: Cow<'a, str>,
    pub product_version_prev: Option<Cow<'a, str>>,
    pub product_version_next: Option<Cow<'a, str>>,
    pub release_date: Cow<'a, str>,
    pub product_icon: Option<Cow<'a, str>>,
    pub description: Option<Cow<'a, str>>,
    pub extra_description: IndexMap<Cow<'a, str>, Cow<'a, str>>,
    pub pre_release: Option<Cow<'a, str>>,
    pub downloads: DownloadGridTemplate<'a>,
    pub downloads_unsupported: Option<DownloadGridTemplate<'a>>,
    pub endpoints: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    pub auto_endpoint: Cow<'a, str>,
    pub translate_note_text_en: Option<Cow<'a, str>>,
    pub translate_note_text: Option<Cow<'a, str>>,
    pub has_nightly: bool,
}

#[derive(Template)]
#[template(path = "p_nightly.html")]
pub struct TemplateNightly<'a> {
    pub self_name: Cow<'a, str>,
    pub theme_name: Cow<'a, str>,
    pub home_url: Cow<'a, str>,
    pub product_key: Cow<'a, str>,
    pub product_title: Cow<'a, str>,
    pub product_icon: Option<Cow<'a, str>>,
    pub downloads: DownloadGridTemplate<'a>,
    pub default_endpoint_url: Cow<'a, str>,
    pub last_built_time: Option<i64>,
    pub description: Cow<'a, str>,
}

#[derive(Template)]
#[template(path = "b_download_grid.html")]
pub struct DownloadGridTemplate<'a> {
    pub theme_name: Cow<'a, str>,
    pub auto_endpoint: Cow<'a, str>,
    pub artifacts: Vec<RenderableArtifact<'a>>,
    pub show_file_size_and_date: bool,
}

mod filters {
    use std::borrow::Cow;
    use std::collections::BTreeMap;

    use chrono::{TimeZone, Utc};
    use relativetime::RelativeTime;

    pub fn endpoint_links(
        urls: &BTreeMap<Cow<str>, Cow<str>>,
        auto_endpoint: &str,
    ) -> askama::Result<String> {
        let mut out = Vec::with_capacity(urls.len() + 1);
        out.push(format!(
            "href=\"{}\"",
            urls.get(auto_endpoint).cloned().unwrap_or_else(|| urls
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| "#".into()))
        ));
        for (k, url) in urls {
            out.push(format!("data-href-{}=\"{}\"", k.to_lowercase(), url))
        }
        Ok(out.join(" "))
    }

    pub fn fulltime(timestamp: &&i64) -> askama::Result<String> {
        let date_time = Utc.timestamp_opt(**timestamp, 0).unwrap();
        Ok(date_time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
    }

    pub fn reltime(timestamp: &&i64) -> askama::Result<String> {
        let date_time = Utc.timestamp_opt(**timestamp, 0).unwrap();
        Ok(date_time.to_relative())
    }
}
