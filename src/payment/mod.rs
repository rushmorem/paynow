#[macro_use]
mod macros {
    #[doc(hidden)]
    #[macro_export]
    macro_rules! concat_payment {
        ($payment:expr) => {
            format_args!("{id}{reference}{amount}{additional_info}{return_url}{result_url}{auth_email}{tokenize}{merchant_trace}{status}",
                         id=$payment.id,
                         reference=$payment.reference,
                         amount=$payment.amount,
                         additional_info=$payment.additional_info.unwrap_or(""),
                         return_url=$payment.return_url.map(|x| x.to_string()).unwrap_or(String::new()),
                         result_url=$payment.result_url,
                         auth_email=$payment.auth_email.unwrap_or(""),
                         tokenize=$payment.tokenize.map(|x| x.to_string()).unwrap_or(String::new()),
                         merchant_trace=$payment.merchant_trace.unwrap_or(""),
                         status=$payment.status,
                         )
        }
    }
}

pub mod express;

use crate::{status, Client, Error, Hash, Payload};
use async_trait::async_trait;
use error::Error as PaymentError;
use rust_decimal::Decimal;
use secrecy::Secret;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Payment<'a> {
    pub(crate) id: u64,
    pub(crate) reference: &'a str,
    pub(crate) amount: Decimal,
    #[serde(rename = "additionalinfo")]
    pub(crate) additional_info: Option<&'a str>,
    #[serde(rename = "returnurl")]
    pub(crate) return_url: Option<&'a Url>,
    #[serde(rename = "resulturl")]
    pub(crate) result_url: &'a Url,
    #[serde(rename = "authemail")]
    pub(crate) auth_email: Option<&'a str>,
    pub(crate) tokenize: Option<bool>,
    #[serde(rename = "merchanttrace")]
    pub(crate) merchant_trace: Option<&'a str>,
    pub(crate) status: status::Message,
}

impl<'a> Payment<'a> {
    pub fn additional_info(&mut self, info: &'a str) -> &mut Self {
        self.additional_info = Some(info);
        self
    }

    pub fn auth_email(&mut self, email: &'a str) -> &mut Self {
        self.auth_email = Some(email);
        self
    }

    pub fn merchant_trace(&mut self, id: &'a str) -> &mut Self {
        self.merchant_trace = Some(id);
        self
    }

    pub fn tokenize(&mut self, tokenize: bool) -> &mut Self {
        self.tokenize = Some(tokenize);
        self
    }
}

#[async_trait]
pub trait Submit {
    type Response;

    async fn submit(self, client: &Client) -> Result<Self::Response, Error>;
}

#[async_trait]
impl Submit for &'_ Payment<'_> {
    type Response = Response;

    async fn submit(self, client: &Client) -> Result<Self::Response, Error> {
        #[derive(Debug, Clone, Serialize)]
        struct Msg<'a> {
            #[serde(flatten)]
            payment: &'a Payment<'a>,
            hash: Secret<Hash>,
        }
        let endpoint = client
            .base
            .join("initiatetransaction")
            .map_err(Error::InvalidPaymentUrl)?;
        let payload = Msg {
            hash: client.hash(concat_payment!(self)),
            payment: self,
        };
        let res: Response = client
            .submit(endpoint, Payload::Form(&payload))
            .await
            .map_err(|err| match err {
                Error::UnexpectedResponse(error, msg) => {
                    match serde_urlencoded::from_str::<'_, error::Response>(&msg) {
                        Ok(res) => match PaymentError::from(res) {
                            PaymentError::InvalidId => Error::InvalidId(client.id),
                            PaymentError::AmountOverflow => Error::AmountOverflow(self.amount),
                            PaymentError::InvalidAmount => Error::InvalidAmount(self.amount),
                            PaymentError::InsufficientBalance => Error::InsufficientBalance,
                            PaymentError::Response(msg) => {
                                Error::Response(reqwest::StatusCode::OK, msg)
                            }
                        },
                        Err(..) => Error::UnexpectedResponse(error, msg),
                    }
                }
                error => error,
            })?;
        client.validate_hash(
            &res.hash,
            format_args!(
                "{status}{browser_url}{poll_url}",
                status = res.status,
                browser_url = res.browser_url,
                poll_url = res.poll_url
            ),
        )?;
        Ok(res)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    status: status::Ok,
    #[serde(rename = "browserurl")]
    browser_url: Url,
    #[serde(rename = "pollurl")]
    poll_url: Url,
    hash: Secret<Hash>,
}

impl Response {
    /// Get a reference to the browser URL
    #[must_use]
    pub fn browser_url(&self) -> &Url {
        &self.browser_url
    }

    /// Consume browser URL
    #[must_use]
    pub fn take_browser_url(self) -> Url {
        self.browser_url
    }

    /// Get reference to poll URL
    #[must_use]
    pub fn poll_url(&self) -> &Url {
        &self.poll_url
    }

    /// Consume poll URL
    #[must_use]
    pub fn take_poll_url(self) -> Url {
        self.poll_url
    }
}

pub(crate) mod error {
    use crate::status;
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub(crate) enum Error {
        InvalidId,
        InvalidAmount,
        AmountOverflow,
        InsufficientBalance,
        Response(String),
    }

    #[derive(Debug, Clone, Deserialize)]
    pub(crate) struct Response {
        #[allow(dead_code)]
        pub(crate) status: status::Error,
        pub(crate) error: String,
    }

    impl From<Response> for Error {
        fn from(res: Response) -> Self {
            match res.error.as_str() {
                "Invalid Id." => Self::InvalidId,
                "Invalid amount field." => Self::InvalidAmount,
                "Conversion overflows." => Self::AmountOverflow,
                "Insufficient balance" => Self::InsufficientBalance,
                _ => Self::Response(res.error),
            }
        }
    }
}
