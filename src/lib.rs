//! Rust support for Paynow Zimbabwe's HTTP API.
//!
//! Before you can start making requests to Paynow's API, you
//! need to get an integration ID and integration key from
//! Paynow. Details about how you can retrieve the ID and
//! key are explained in detail on [this page].
//!
//! [this page]: https://developers.paynow.co.zw/docs/integration_generation.html
//!
//! # Usage
//!
//! See [examples].
//!
//! [examples]: https://github.com/rushmorem/paynow/tree/main/examples

pub mod payment;
pub mod status;

use payment::error::{Error as PaymentError, Response};
use payment::{express, Payment};
use reqwest::header::CONTENT_LENGTH;
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use status::{MerchantTrace, Update};
use std::fmt::Arguments;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Client {
    id: u64,
    key: Uuid,
    req: reqwest::Client,
    base: Url,
}

impl Client {
    pub fn new(id: u64, key: Uuid) -> Self {
        Self {
            id,
            key,
            req: reqwest::Client::new(),
            // we know this is a valid URL so this should never panic
            base: Url::parse("https://www.paynow.co.zw/interface/").unwrap(),
        }
    }

    pub fn payment<'a>(
        &self,
        reference: &'a str,
        amount: Decimal,
        return_url: &'a Url,
        result_url: &'a Url,
    ) -> Payment<'a> {
        Payment {
            amount,
            reference,
            result_url,
            id: self.id,
            status: status::Message,
            additional_info: None,
            return_url: Some(return_url),
            auth_email: None,
            tokenize: None,
            merchant_trace: None,
        }
    }

    pub fn express_payment<'a>(
        &self,
        method: express::Method<'a>,
        reference: &'a str,
        amount: Decimal,
        result_url: &'a Url,
        auth_email: &'a str,
        merchant_trace: &'a str,
    ) -> express::Payment<'a> {
        let payment = Payment {
            amount,
            reference,
            result_url,
            id: self.id,
            return_url: None,
            tokenize: None,
            additional_info: None,
            merchant_trace: Some(merchant_trace),
            auth_email: Some(auth_email),
            status: status::Message,
        };
        express::Payment { payment, method }
    }

    pub async fn poll_status(&self, poll_url: Url) -> Result<Update, Error> {
        let status = self
            .submit::<(), Update>(poll_url, Payload::Empty)
            .await
            .map_err(|err| match err {
                Error::UnexpectedResponse(error, msg) => {
                    match serde_urlencoded::from_str::<'_, Response>(&msg) {
                        Ok(res) => match PaymentError::from(res) {
                            PaymentError::InvalidId => Error::InvalidId(self.id),
                            PaymentError::InsufficientBalance => Error::InsufficientBalance,
                            PaymentError::Response(msg) => {
                                Error::Response(reqwest::StatusCode::OK, msg)
                            }
                            _ => Error::UnexpectedResponse(error, msg),
                        },
                        Err(..) => Error::UnexpectedResponse(error, msg),
                    }
                }
                error => error,
            })?;
        status.validate(self)?;
        Ok(status)
    }

    pub async fn trace_payment(&self, merchant_trace: &str) -> Result<Update, Error> {
        #[derive(Deserialize)]
        struct NotFound {
            status: status::NotFound,
            hash: String,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Msg {
            Status(Update),
            Error(NotFound),
        }
        let id = self.id;
        let status = status::Message;
        let trace = MerchantTrace {
            id,
            status,
            merchant_trace,
            hash: self.hash(format_args!(
                "{id}{merchant_trace}{status}",
                id = id,
                merchant_trace = merchant_trace,
                status = status
            )),
        };
        let endpoint = self
            .base
            .join("trace")
            .map_err(Error::InvalidTracePaymentUrl)?;
        let res = self
            .submit::<_, Msg>(endpoint, Payload::Form(&trace))
            .await
            .map_err(|err| match err {
                Error::UnexpectedResponse(error, msg) => {
                    match serde_urlencoded::from_str::<'_, Response>(&msg) {
                        Ok(res) => match PaymentError::from(res) {
                            PaymentError::InvalidId => Error::InvalidId(self.id),
                            PaymentError::InsufficientBalance => Error::InsufficientBalance,
                            PaymentError::Response(msg) => {
                                Error::Response(reqwest::StatusCode::OK, msg)
                            }
                            _ => Error::UnexpectedResponse(error, msg),
                        },
                        Err(..) => Error::UnexpectedResponse(error, msg),
                    }
                }
                error => error,
            })?;
        match res {
            Msg::Status(status) => {
                status.validate(self)?;
                Ok(status)
            }
            Msg::Error(error) => {
                self.validate_hash(&error.hash, format_args!("{}", error.status))?;
                Err(Error::NotFound(merchant_trace.to_owned()))
            }
        }
    }

    fn hash(&self, msg: Arguments) -> String {
        let mut hasher = Sha512::new();
        hasher.update(format!("{msg}{key}", msg = msg, key = self.key));
        format!("{:X}", hasher.finalize())
    }

    fn validate_hash(&self, hash: &str, msg: Arguments) -> Result<(), Error> {
        let expected_hash = self.hash(msg);
        if hash != expected_hash {
            return Err(Error::HashMismatch(msg.to_string()));
        }
        Ok(())
    }

    async fn submit<T, O>(&self, endpoint: Url, payload: Payload<'_, T>) -> Result<O, Error>
    where
        T: Serialize,
        O: DeserializeOwned,
    {
        let mut req = self.req.post(endpoint);
        req = match payload {
            Payload::Form(payload) => req.form(payload),
            Payload::Empty => req.header(CONTENT_LENGTH, 0),
        };
        let res = req.send().await.map_err(Error::SendingRequest)?;
        let code = res.status();
        let msg = res.text().await.map_err(Error::GettingText)?;
        if !code.is_success() {
            return Err(Error::Response(code, msg));
        }
        serde_urlencoded::from_str(&msg).map_err(|e| Error::UnexpectedResponse(e, msg))
    }
}

enum Payload<'a, T: Serialize> {
    Empty,
    Form(&'a T),
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to send request to Paynow")]
    SendingRequest(#[source] reqwest::Error),
    #[error("failed to retrieve Paynow response text")]
    GettingText(#[source] reqwest::Error),
    #[error("got unexpected response from Paynow")]
    UnexpectedResponse(#[source] serde_urlencoded::de::Error, String),

    #[error("amount is larger than what Paynow can handle")]
    AmountOverflow(Decimal),
    #[error("invalid amount")]
    InvalidAmount(Decimal),
    #[error("invalid ID")]
    InvalidId(u64),
    #[error("payment URL is invalid")]
    InvalidPaymentUrl(#[source] url::ParseError),
    #[error("express payment URL is invalid")]
    InvalidExpressPaymentUrl(#[source] url::ParseError),
    #[error("merchant trace URL is invalid")]
    InvalidTracePaymentUrl(#[source] url::ParseError),
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("received invalid hash")]
    HashMismatch(String),
    #[error("merchant trace ID not found")]
    NotFound(String),
    #[error("Paynow returned an error")]
    Response(reqwest::StatusCode, String),
    #[error("time format error")]
    TimeFormat(
        #[source]
        #[from]
        time::error::Format,
    ),
}
