use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::Msg(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;