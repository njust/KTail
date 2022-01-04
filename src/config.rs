use std::sync::Mutex;
use once_cell::sync::Lazy;
use anyhow::Result;
use std::fs;
use serde::{Serialize, Deserialize};

const CONFIG_PATH: &'static str = "config.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct Highlighter {
   pub name: String,
   pub search: String,
   pub color: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
   pub k8s_configs: Vec<String>,
   pub log_view_font: String,
   pub highlighters: Vec<Highlighter>,
}

impl Default for Config {
   fn default() -> Self {
      #[cfg(target_os = "linux")]
      let font = "13px Monospace";
      #[cfg(target_os = "windows")]
      let font = "15px Consolas";

      Config {
         log_view_font: font.to_string(),
         k8s_configs: vec![],
         highlighters: vec![
            Highlighter {
               name: "Warnings".to_string(),
               search: r".*\s((?i)warn(?-i))\s.*".to_string(),
               color: "rgba(207,111,57,1)".to_string(),
            },
            Highlighter {
               name: "Errors".to_string(),
               search: r".*\s((?i)error|fatal|failed(?-i))\s.*".to_string(),
               color: "rgba(244,94,94,1)".to_string(),
            }
         ],
      }
   }
}


impl Config {
   pub fn save(&self) -> Result<()> {
      let json = serde_json::to_string_pretty(self)?;
      fs::write(CONFIG_PATH, &json)?;
      Ok(())
   }

   pub fn load() -> Result<Self> {
      let json = fs::read_to_string(CONFIG_PATH)?;
      Ok(serde_json::from_str(&json)?)
   }
}

pub static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| {
   let cfg = Config::load().unwrap_or(Config::default());
   Mutex::new(cfg)
});