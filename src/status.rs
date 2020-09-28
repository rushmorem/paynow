use crate::Client;
use rust_decimal::Decimal;
use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use time::Date;
use url::Url;

/// Whenever the status of a transaction is changed, for example payment made,
/// the Paynow server will send the following message to the merchant server.
/// The message will be sent as an HTTP POST to the resulturl specified by the
/// merchant when the transaction initiation occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    reference: String,
    #[serde(rename = "paynowreference")]
    paynow_reference: u64,
    amount: Decimal,
    status: Status,
    #[serde(rename = "pollurl")]
    poll_url: Url,
    #[serde(flatten)]
    token: Option<Token>,
    hash: String,
}

impl Update {
    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn take_reference(self) -> String {
        self.reference
    }

    pub fn paynow_reference(self) -> u64 {
        self.paynow_reference
    }

    pub fn amount(self) -> Decimal {
        self.amount
    }

    pub fn status(self) -> Status {
        self.status
    }

    pub fn poll_url(&self) -> &Url {
        &self.poll_url
    }

    pub fn take_poll_url(self) -> Url {
        self.poll_url
    }

    pub fn token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    pub fn take_token(self) -> Option<Token> {
        self.token
    }

    pub fn validate(&self, client: &Client) -> Result<(), crate::Error> {
        client.validate_hash(
            &self.hash,
            format_args!(
                "{reference}{paynow_reference}{amount}{status}{poll_url}{token}",
                reference = self.reference,
                paynow_reference = self.paynow_reference,
                amount = self.amount,
                status = self.status,
                poll_url = self.poll_url,
                token = match &self.token {
                    Some(x) => format!(
                        "{token}{expiry}",
                        token = x.token,
                        expiry = x.expiry.format("%d%b%Y")
                    ),
                    None => String::new(),
                },
            ),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MerchantTrace<'a> {
    pub(crate) id: u64,
    #[serde(rename = "merchanttrace")]
    pub(crate) merchant_trace: &'a str,
    pub(crate) status: Message,
    pub(crate) hash: String,
}

/// The following fields will be returned in the status response message only if
/// the merchant has been permitted to tokenize payment instruments on Paynow.
/// Contact support@paynow.co.zw to apply for this functionality.
///
/// The token can be used to carry out recurring payments for customers of
/// merchants who have recurring payments enabled, without exposing the
/// customer’s sensitive payment instrument information to the merchant.
///
/// If `tokenize=true` is specified by the merchant in the initiate transaction
/// message, then a token of the payment instrument will be returned in the
/// Status Update message along with its expiry date.
///
/// Tokens are valid for up to six (6) months from the date of issue, dependent
/// upon the expiry date of the tokenized payment instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    token: String,
    #[serde(rename = "tokenexpiry")]
    expiry: Date,
}

impl Token {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn take_token(self) -> String {
        self.token
    }

    pub fn expiry(self) -> Date {
        self.expiry
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Status {
    /// Transaction has been created in Paynow, but has not yet been paid by
    /// the customer.
    Created,
    /// The transaction has been cancelled in Paynow and may not be resumed and
    /// needs to be recreated.
    Cancelled,
    /// Transaction has been created in Paynow and an up stream system, the
    /// customer has been referred to that upstream system but has not yet made
    /// payment.
    Sent,
    /// Transaction paid successfully, but is sitting in suspense waiting on
    /// the merchant to confirm delivery of the goods.
    #[serde(rename = "Awaiting Delivery")]
    AwaitingDelivery,
    /// The user or merchant has acknowledged delivery of the goods but the
    /// funds are still sitting in suspense awaiting the 24 hour confirmation
    /// window to close.
    Delivered,
    /// Transaction paid successfully, the merchant will receive the funds at
    /// next settlement.
    Paid,
    /// Transaction has been disputed by the Customer and funds are being held
    /// in suspense until the dispute has been resolved.
    Disputed,
    /// Funds were refunded back to the customer.
    Refunded,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Status::Paid => "Paid",
                Status::AwaitingDelivery => "Awaiting Delivery",
                Status::Delivered => "Delivered",
                Status::Created => "Created",
                Status::Sent => "Sent",
                Status::Cancelled => "Cancelled",
                Status::Disputed => "Disputed",
                Status::Refunded => "Refunded",
            }
        )
    }
}

macro_rules! status {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub(crate) struct $name;

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(stringify!($name))
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct StatusVisitor;

                impl<'de> Visitor<'de> for StatusVisitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        write!(formatter, "the string {}", stringify!($name))
                    }

                    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        if s == stringify!($name) {
                            Result::Ok($name)
                        } else {
                            Err(de::Error::invalid_value(Unexpected::Str(s), &self))
                        }
                    }
                }

                deserializer.deserialize_str(StatusVisitor)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, stringify!($name))
            }
        }
    };
}

status!(Message);

status!(Error);

status!(Ok);

status!(NotFound);
