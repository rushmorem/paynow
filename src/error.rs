use crate::Status;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid integration key; {0}")]
    InvalidKey(uuid::Error),
    #[error("invalid URL")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Paynow returned an error")]
    Response {
        status: Status,
        error: String,
    },
    #[error("client error")]
    Client(#[from] reqwest::Error),
}
