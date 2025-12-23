#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Serde JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),

    #[error("UTF-8 decode error: {0}")]
    Utf8DecodeError(#[from] std::string::FromUtf8Error),
}

pub type Result<T> = std::result::Result<T, Error>;
