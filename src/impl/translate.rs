use crate::r#impl::config::SimpleConfig;
use log::warn;

pub(crate) struct TranslateConfig {}

impl TranslateConfig {
    pub fn get() -> Option<Self> {
        let key_id = TranslateAwsKeyId::get_checked();
        let key = TranslateAwsSecretAccessKey::get_checked();
        match (key_id, key) {
            (Ok(key_id), Ok(key)) => {
                todo!()
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
