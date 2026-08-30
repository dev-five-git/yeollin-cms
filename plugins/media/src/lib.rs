//! Runtime-backed image library for Yeollin CMS.

pub mod models;
pub mod routes;

use serde::{de, Deserialize, Deserializer, Serialize};
use vespera::Schema;

pub(crate) const PLUGIN_NAME: &str = "media";
pub(crate) const HARD_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
const REQUEST_BODY_BYTES: usize = HARD_UPLOAD_BYTES + 64 * 1024;
const DEFAULT_UPLOAD_MEGABYTES: u32 = 5;
const MAX_UPLOAD_MEGABYTES: u32 = 10;

/// Administrator-configured limit below the plugin's 10 MiB hard ceiling.
#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MediaSettings {
    pub max_upload_megabytes: u32,
}

impl Default for MediaSettings {
    fn default() -> Self {
        Self {
            max_upload_megabytes: DEFAULT_UPLOAD_MEGABYTES,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMediaSettings {
    max_upload_megabytes: u32,
}

impl<'de> Deserialize<'de> for MediaSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawMediaSettings::deserialize(deserializer)?;
        if !(1..=MAX_UPLOAD_MEGABYTES).contains(&raw.max_upload_megabytes) {
            return Err(de::Error::custom(format!(
                "maxUploadMegabytes must be between 1 and {MAX_UPLOAD_MEGABYTES}"
            )));
        }
        Ok(Self {
            max_upload_megabytes: raw.max_upload_megabytes,
        })
    }
}

yeollin_plugin::yeollin_plugin! {
    name: "media",
    author: "DevFive",
    description: "Runtime-backed image uploads with stable content references",
    settings: MediaSettings,
    public_api_routes: ["/file"],
    runtime_storage: true,
    request_body_limit: REQUEST_BODY_BYTES,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_setting_is_bounded_by_the_hard_limit() {
        for invalid in [0, MAX_UPLOAD_MEGABYTES + 1] {
            let value = serde_json::json!({ "maxUploadMegabytes": invalid });
            assert!(serde_json::from_value::<MediaSettings>(value).is_err());
        }
        let maximum: MediaSettings = serde_json::from_value(
            serde_json::json!({ "maxUploadMegabytes": MAX_UPLOAD_MEGABYTES }),
        )
        .unwrap();
        assert_eq!(maximum.max_upload_megabytes, MAX_UPLOAD_MEGABYTES);
    }

    #[test]
    fn metadata_declares_the_exact_public_file_route() {
        let metadata = metadata();
        assert_eq!(metadata.public_api_routes, ["/api/media/file"]);
        assert!(metadata.requires_runtime_storage);
        assert_eq!(metadata.request_body_limit, Some(REQUEST_BODY_BYTES));
    }
}
