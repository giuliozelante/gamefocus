use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("profile error: {0}")]
    Profile(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("process {pid}: {message}")]
    Process { pid: u32, message: String },

    #[error("boost provider '{provider}': {message}")]
    Boost { provider: String, message: String },

    #[error("{0}")]
    Other(String),
}
