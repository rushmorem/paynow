macro_rules! concat_express_payment {
    ($name:expr, $payment:expr, $method_args:expr) => {
        format_args!(
            "{name}{payment}{method}",
            name = $name,
            payment = concat_payment!($payment.payment),
            method = format_args!(
                "{phone}{number}{name}{cvv}{expiry}{line1}{line2}{city}{province}{country}{token}",
                phone = $method_args.phone,
                number = $method_args.number,
                name = $method_args.name,
                cvv = $method_args.cvv,
                expiry = $method_args.expiry,
                line1 = $method_args.line1,
                line2 = $method_args.line2,
                city = $method_args.city,
                province = $method_args.province,
                country = $method_args.country,
                token = $method_args.token,
                ),
        )
    }
}

use super::error::Error as PaymentError;
use super::Submit;
use crate::{status, Client, Error, Payload};
use async_trait::async_trait;
use celes::Country;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Method<'a> {
    Ecocash {
        phone: &'a str,
    },
    OneMoney {
        phone: &'a str,
    },
    VisaOrMastercard {
        #[serde(flatten)]
        card: Card<'a>,
        #[serde(flatten)]
        address: Address<'a>,
        token: &'a str,
    },
}

impl<'a> Method<'a> {
    fn name(&self) -> &'static str {
        match self {
            Method::Ecocash { .. } => "ecocash",
            Method::OneMoney { .. } => "onemoney",
            Method::VisaOrMastercard { .. } => "vmc",
        }
    }

    pub fn ecocash(phone: &'a str) -> Self {
        Method::Ecocash { phone }
    }

    pub fn onemoney(phone: &'a str) -> Self {
        Method::OneMoney { phone }
    }

    pub fn vmc(card: Card<'a>, address: Address<'a>, token: &'a str) -> Self {
        Method::VisaOrMastercard {
            card,
            address,
            token,
        }
    }
}

#[derive(Default)]
struct MethodArgs<'a> {
    phone: &'a str,
    number: &'a str,
    name: &'a str,
    cvv: &'a str,
    expiry: &'a str,
    line1: &'a str,
    line2: &'a str,
    city: &'a str,
    province: &'a str,
    country: &'a str,
    token: &'a str,
}

impl<'a> From<&'a Method<'a>> for MethodArgs<'a> {
    fn from(method: &'a Method<'a>) -> Self {
        match method {
            Method::Ecocash { phone } => Self {
                phone,
                ..Default::default()
            },
            Method::OneMoney { phone } => Self {
                phone,
                ..Default::default()
            },
            Method::VisaOrMastercard {
                card,
                address,
                token,
            } => Self {
                token,
                number: card.number,
                name: card.name,
                cvv: card.cvv,
                expiry: card.expiry,
                line1: address.line1,
                line2: address.line2.unwrap_or(""),
                city: address.city,
                province: address.province.unwrap_or(""),
                country: address.country.long_name,
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Card<'a> {
    #[serde(rename = "cardnumber")]
    pub number: &'a str,
    #[serde(rename = "cardname")]
    pub name: &'a str,
    #[serde(rename = "cardcvv")]
    pub cvv: &'a str,
    #[serde(rename = "cardexpiry")]
    pub expiry: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Address<'a> {
    #[serde(rename = "billingline1")]
    pub line1: &'a str,
    #[serde(rename = "billingline2")]
    pub line2: Option<&'a str>,
    #[serde(rename = "billingcity")]
    pub city: &'a str,
    #[serde(rename = "billingprovince")]
    pub province: Option<&'a str>,
    #[serde(rename = "billingcountry")]
    pub country: &'a Country,
}

#[derive(Debug, Clone, Serialize)]
pub struct Payment<'a> {
    #[serde(flatten)]
    pub(crate) payment: super::Payment<'a>,
    #[serde(flatten)]
    pub(crate) method: Method<'a>,
}

impl<'a> Payment<'a> {
    pub fn additional_info(&mut self, info: &'a str) -> &mut Self {
        self.payment.additional_info = Some(info);
        self
    }

    pub fn tokenize(&mut self, tokenize: bool) -> &mut Self {
        self.payment.tokenize = Some(tokenize);
        self
    }
}

#[async_trait]
impl Submit for &'_ Payment<'_> {
    type Response = Response;

    async fn submit(self, client: &Client) -> Result<Self::Response, Error> {
        #[derive(Debug, Clone, Serialize)]
        struct Msg<'a> {
            method: &'static str,
            #[serde(flatten)]
            payment: &'a Payment<'a>,
            hash: String,
        }
        let endpoint = client
            .base
            .join("remotetransaction")
            .map_err(Error::InvalidExpressPaymentUrl)?;
        let payload = Msg {
            method: self.method.name(),
            hash: client.hash(concat_express_payment!(
                self.method.name(),
                self,
                MethodArgs::from(&self.method)
            )),
            payment: self,
        };
        let res: Response = client
            .submit(endpoint, Payload::Form(&payload))
            .await
            .map_err(|err| match err {
                Error::UnexpectedResponse(error, msg) => {
                    match serde_urlencoded::from_str::<'_, super::error::Response>(&msg) {
                        Ok(res) => match PaymentError::from(res) {
                            PaymentError::InvalidId => Error::InvalidId(client.id),
                            PaymentError::AmountOverflow => {
                                Error::AmountOverflow(self.payment.amount)
                            }
                            PaymentError::InvalidAmount => {
                                Error::InvalidAmount(self.payment.amount)
                            }
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
                "{status}{instructions}{paynow_reference}{poll_url}",
                status = res.status,
                instructions = res.instructions,
                paynow_reference = res.paynow_reference,
                poll_url = res.poll_url
            ),
        )?;
        Ok(res)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    status: status::Ok,
    instructions: String,
    #[serde(rename = "paynowreference")]
    paynow_reference: u64,
    #[serde(rename = "pollurl")]
    poll_url: Url,
    hash: String,
}

impl Response {
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn take_instructions(self) -> String {
        self.instructions
    }

    pub fn paynow_reference(self) -> u64 {
        self.paynow_reference
    }

    pub fn poll_url(&self) -> &Url {
        &self.poll_url
    }

    pub fn take_poll_url(self) -> Url {
        self.poll_url
    }
}
