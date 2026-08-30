//! Reference compile-time typed content collections.

use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::ContentFields;

/// Fields specific to the reference pages collection.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct PageFields {
    pub excerpt: String,
    pub body: String,
    pub hero_image: Option<String>,
}

impl ContentFields for PageFields {
    fn validate(&self) -> Result<(), String> {
        if self.excerpt.chars().count() > 240 {
            return Err("excerpt must be at most 240 characters".to_string());
        }
        if self.body.trim().is_empty() {
            return Err("body must not be empty".to_string());
        }
        if let Some(reference) = self.hero_image.as_deref() {
            let id = reference
                .strip_prefix("media:")
                .ok_or_else(|| "heroImage must be a media reference".to_string())?;
            if id.len() != 32
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err("heroImage must be a canonical media reference".to_string());
            }
        }
        Ok(())
    }
}

yeollin_plugin::yeollin_content_collection! {
    module: pages,
    name: "pages",
    label: "Pages",
    fields: crate::PageFields,
    order: 30,
}

yeollin_plugin::yeollin_plugin! {
    name: "content",
    author: "DevFive",
    description: "Compile-time typed content collections",
    frontend: false,
    collections: [pages::registration()],
}
