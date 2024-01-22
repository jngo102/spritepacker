use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Settings {
    #[serde(rename = "Language")]
    pub language: String,
    #[serde(rename = "Sprites Path")]
    pub sprites_path: String,
    #[serde(rename = "Dark")]
    pub dark: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en-US".to_string(),
            sprites_path: "".to_string(),
            dark: true,
        }
    }
}
