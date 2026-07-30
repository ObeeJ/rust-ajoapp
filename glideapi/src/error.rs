use thiserror::Error;
use crate::response::Response;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias — users write Result<T>, not Result<T, Error>
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn into_response(self) -> Response {
        let (status, body) = match self {
            Error::NotFound(m) => (404u16, m),
            Error::BadRequest(m) => (400u16, m),
            Error::Internal(m) => (500u16, m),
        };
        Response { status, body, headers: vec![] }
    }
}
