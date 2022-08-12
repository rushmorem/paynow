use paynow::payment::Submit;
use paynow::{ApiKey, Client};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::error::Error;
use url::Url;

#[derive(Deserialize, Debug)]
struct Config {
    id: u64,
    key: ApiKey,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = envy::prefixed("PAYNOW_INTEGRATION_").from_env()?;
    let client = Client::new(config.id, config.key);
    let reference = "c1dfbc5b-9e5b-40bf-846e-22006078a436";
    let amount = Decimal::new(3141874, 2);
    let return_url = Url::parse("https://example.net")?;
    let result_url = Url::parse("https://example.net")?;
    let payment = client.payment(reference, amount, &return_url, &result_url);
    dbg!(payment.submit(&client).await?);
    Ok(())
}
