use std::path::PathBuf;
use directories::ProjectDirs;

pub fn root_dir() -> Option<ProjectDirs> {
    directories::ProjectDirs::from("de", "", "KTail")
}

pub fn log_dir() -> PathBuf {
    root_dir().as_ref().map(|r| r.data_dir().join("logs")).unwrap_or(PathBuf::from("."))
}

pub fn config_dir() -> Option<PathBuf> {
    root_dir().as_ref().map(|pd| pd.config_dir().to_path_buf())
}