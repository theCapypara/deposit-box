use crate::r#impl::config::SimpleConfig;
use async_compat::Compat;
use aws_sdk_translate::primitives::Blob;
use aws_sdk_translate::types::{Document, Formality, TranslationSettings};
use cached::proc_macro::cached;
use cached::SizedCache;
use indexmap::IndexMap;
use log::{debug, warn};
use rocket::futures::executor;
use std::borrow::Cow;
use std::string::FromUtf8Error;
use thiserror::Error;

static SUPPORTED_LANGS: &[&str] = &[
    "af", "sq", "am", "ar", "hy", "az", "bn", "bs", "bg", "ca", "zh", "zh-TW", "hr", "cs", "da",
    "fa-AF", "nl", "en", "et", "fa", "tl", "fi", "fr", "fr-CA", "ka", "de", "el", "gu", "ht", "ha",
    "he", "hi", "hu", "is", "id", "ga", "it", "ja", "kn", "kk", "ko", "lv", "lt", "mk", "ms", "ml",
    "mt", "mr", "mn", "no", "ps", "pl", "pt", "pt-PT", "pa", "ro", "ru", "sr", "si", "sk", "sl",
    "so", "es", "es-MX", "sw", "sv", "ta", "te", "th", "tr", "uk", "ur", "uz", "vi", "cy",
];

#[allow(clippy::too_many_arguments)]
pub(crate) async fn translate_artifact_release<'a>(
    lang: &str,
    client: &aws_sdk_translate::Client,
    description: &mut Option<Cow<'a, str>>,
    extra_description: &mut IndexMap<Cow<'a, str>, Cow<'a, str>>,
    translate_note_text_en: &mut Option<Cow<'a, str>>,
    translate_note_text: &mut Option<Cow<'a, str>>,
) -> Result<(), TranslateError> {
    let lang_lower = lang.to_lowercase();
    if &lang_lower == "en" || lang_lower.starts_with("en-") {
        return Ok(());
    }
    static NOTE_TEXT: &str = "The description has been automatically translated by machine translation. Click here to view in English: ";
    *translate_note_text = Some(translate_str(lang, NOTE_TEXT, client).await?.into());
    *translate_note_text_en = Some(NOTE_TEXT.into());
    if let Some(description) = description.as_mut() {
        *description = Cow::Owned(translate_html(lang, &description, client).await?);
    }
    let mut new_extra_description = IndexMap::with_capacity(extra_description.capacity());
    for (k, v) in extra_description.into_iter() {
        new_extra_description.insert(
            translate_str(lang, k, client).await?.into(),
            translate_html(lang, v, client).await?.into(),
        );
    }
    *extra_description = new_extra_description;

    Ok(())
}

pub async fn translate_str(
    lang: &str,
    input: impl AsRef<str>,
    client: &aws_sdk_translate::Client,
) -> Result<String, TranslateError> {
    _do_translate(lang, input.as_ref(), false, client).await
}

pub async fn translate_html(
    lang: &str,
    input: impl AsRef<str>,
    client: &aws_sdk_translate::Client,
) -> Result<String, TranslateError> {
    _do_translate(lang, input.as_ref(), true, client).await
}

#[cached(
    type = "SizedCache<String, String>",
    create = "{ SizedCache::with_size(5000) }",
    convert = r#"{ format!("{}::{}::{}", lang, is_html, input) }"#,
    sync_writes = true,
    result = true
)]
async fn _do_translate(
    lang: &str,
    input: &str,
    is_html: bool,
    client: &aws_sdk_translate::Client,
) -> Result<String, TranslateError> {
    if lang.len() < 2
        || (!SUPPORTED_LANGS.contains(&lang) && !SUPPORTED_LANGS.contains(&&lang[..2]))
    {
        return Err(TranslateError::UnsupportedLang);
    }
    let content_type = if is_html { "text/html" } else { "text/plain" };
    let doc = Compat::new(
        client
            .translate_document()
            .set_source_language_code(Some("en".into()))
            .set_target_language_code(Some(lang.into()))
            .set_document(Some(
                Document::builder()
                    .set_content(Some(Blob::new(input.as_bytes())))
                    .set_content_type(Some(content_type.into()))
                    .build(),
            ))
            .set_settings(Some(
                TranslationSettings::builder()
                    .set_formality(Some(Formality::Informal))
                    .build(),
            ))
            .send(),
    )
    .await
    .map_err(|e| TranslateError::AwsError(e.to_string()))?;
    let doc_doc = doc
        .translated_document()
        .ok_or(TranslateError::NoDocument)?;
    let doc_blob = doc_doc.content.as_ref().ok_or(TranslateError::NoDocument)?;
    let doc_string: String = String::from_utf8(doc_blob.as_ref().to_vec())?;
    Ok(doc_string)
}

pub(crate) struct TranslateConfig {
    pub client: aws_sdk_translate::Client,
}

impl TranslateConfig {
    pub fn get() -> Option<Self> {
        let key_id = TranslateAwsKeyId::get_checked();
        let key = TranslateAwsSecretAccessKey::get_checked();
        match (key_id, key) {
            (Ok(key_id), Ok(key)) => {
                let config = executor::block_on(Compat::new(
                    aws_config::ConfigLoader::default()
                        .credentials_provider(aws_sdk_translate::config::Credentials::new(
                            key_id,
                            key,
                            None,
                            None,
                            "deposit_box_env_provider",
                        ))
                        .load(),
                ));
                let client = aws_sdk_translate::Client::new(&config);
                debug!("Loaded AWS Translate SDK client.");
                Some(TranslateConfig { client })
            }
            _ => {
                warn!("Translate: Either the key or secret key were invalid. Translation service not available.");
                None
            }
        }
    }
}

struct TranslateAwsKeyId {}

impl SimpleConfig for TranslateAwsKeyId {
    const VAR_NAME: &'static str = "DEPBOX_TRANSLATE_AWS_ACCESS_KEY_ID";
}

struct TranslateAwsSecretAccessKey {}

impl SimpleConfig for TranslateAwsSecretAccessKey {
    const VAR_NAME: &'static str = "DEPBOX_TRANSLATE_AWS_SECRET_ACCESS_KEY";
}

#[derive(Debug, Error)]
pub enum TranslateError {
    #[error("AWS error: {0}")]
    AwsError(String),
    #[error("No document returned.")]
    NoDocument,
    #[error("Invalid UTF-8: {0}")]
    Utf8Error(#[from] FromUtf8Error),
    #[error("Language not supported.")]
    UnsupportedLang,
}
