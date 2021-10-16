use serde::Deserialize;
use std::{fs, path::PathBuf};
use once_cell::sync::Lazy;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub log_view_font: String,
    pub minimap_font: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            log_view_font: "13px Monospace".to_string(),
            minimap_font: "1px Monospace".to_string()
        }
    }
}


pub static SETTINGS: Lazy<Settings> = Lazy::new(|| {
    let settings_path = PathBuf::from("config").join("settings.json");
    let settings = if settings_path.exists() {
        fs::read_to_string(settings_path).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    };
    settings
});