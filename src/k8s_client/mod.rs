use std::error::Error;
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
mod kube_config;
mod kube_client;

pub use kube_client::*;
pub use kube_config::*;
pub use reqwest::Response;