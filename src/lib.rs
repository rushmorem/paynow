mod error;

pub use error::Error;

use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use url::Url;


#[derive(Debug, Clone)]
pub struct Client {
    id: u64,
    key: Uuid,
    req: reqwest::Client,
    base: Url,
    return_url: Url,
    result_url: Option<Url>,
}

impl Client {
    pub fn new(id: u64, key: Uuid, return_url: Url) -> Self {
        Self {
            id,
            key,
            return_url,
            req: reqwest::Client::new(),
            // we know this is a valid URL so this should never panic
            base: Url::parse("https://www.paynow.co.zw/interface/").unwrap(),
            result_url: None,
        }
    }

    async fn initiate_transaction(&self, txn: &InitialTransaction) -> Result<InitialResponse, Error> {
        let endpoint = self.base.join("initiatetransaction")?;
        let res = self.req.post(endpoint)
            .json(txn)
            .send()
            .await?
            .json()
            .await?;
        Ok(res)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitialTransaction {
    id: u64,
    reference: String,
    amount: Decimal,
    #[serde(rename = "additionalinfo")]
    additional_info: Option<String>,
    #[serde(rename = "returnurl")]
    return_url: Url,
    #[serde(rename = "resulturl")]
    result_url: Url,
    #[serde(rename = "authemail")]
    auth_email: Option<String>,
    tokenize: Option<bool>,
    #[serde(rename = "merchanttrace")]
    merchant_trace: Option<String>,
    status: Status,
    hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitialResponse {
    #[serde(rename = "browserurl")]
    browser_url: Url,
    #[serde(rename = "pollurl")]
    pollurl: Url,
    status: Status,
    hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Status {
    Message,
    Error,
    Ok,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    title: String,
    amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    description: String,
    items: Vec<Item>,
}

impl Payment {
    pub fn new() -> Self {
        todo!()
    }
}
