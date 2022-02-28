use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use anyhow::{Result, anyhow};
use std::fs;
use serde::{Serialize, Deserialize};

const CONFIG_NAME: &'static str = "config.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct Highlighter {
   pub search: String,
   pub color: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
   pub k8s_configs: Vec<String>,
   pub highlighters: HashMap<String, Highlighter>,
   pub log_view_settings: LogViewSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogViewSettings {
   pub wrap_text: bool,
   pub show_pod_names: bool,
   pub show_container_names: bool,
   pub show_timestamps: bool,
   pub font: String,
}

impl Default for Config {
   fn default() -> Self {
      #[cfg(target_os = "linux")]
      let font = "13px Monospace";
      #[cfg(target_os = "windows")]
      let font = "15px Consolas";
      #[cfg(target_os = "macos")]
      let font = "14px Courier";

      let mut highlighters = HashMap::new();
      highlighters.insert("Warnings".to_string(), Highlighter {
         search: r".*\s((?i)warn(?-i))\s.*".to_string(),
         color: "rgb(207,111,57)".to_string(),
      });
      highlighters.insert("Errors".to_string(), Highlighter {
         search: r".*\s((?i)error|fatal|failed(?-i))\s.*".to_string(),
         color: "rgb(244,94,94)".to_string(),
      });

      Config {
         k8s_configs: vec![],
         highlighters,
         log_view_settings: LogViewSettings {
            show_pod_names: false,
            show_container_names: false,
            show_timestamps: false,
            wrap_text: false,
            font: font.to_string(),
         }
      }
   }
}


impl Config {
   pub fn save(&self) -> Result<()> {
      let json = serde_json::to_string_pretty(self)?;
      let path = crate::dirs::config_dir().ok_or(anyhow!("No config path!"))?;
      if !path.exists() {
         if let Err(e) = fs::create_dir_all(&path) {
            log::error!("Could not create config dir: {}", e);
         }
      }
      fs::write(path.join(CONFIG_NAME), &json)?;
      Ok(())
   }

   pub fn load() -> Result<Self> {
      let path = crate::dirs::config_dir().ok_or(anyhow!("No config path!"))?;
      let json = fs::read_to_string(path.join(CONFIG_NAME))?;
      Ok(serde_json::from_str(&json)?)
   }
}

pub static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| {
   let cfg = Config::load().unwrap_or(Config::default());
   Mutex::new(cfg)
});