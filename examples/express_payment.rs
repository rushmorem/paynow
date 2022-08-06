use paynow::payment::express::Method;
use paynow::payment::Submit;
use paynow::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::error::Error;
use url::Url;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
struct Config {
    id: u64,
    key: Uuid,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = envy::prefixed("PAYNOW_INTEGRATION_").from_env()?;
    let client = Client::new(config.id, config.key);
    let method = Method::ecocash("0771111111");
    let reference = "c1dfbc5b-9e5b-40bf-846e-22006078a436";
    let amount = Decimal::new(30_000_00, 2);
    let result_url = Url::parse("https://example.net")?;
    let auth_email = "billing@webenchanter.com";
    let trace_id = Uuid::new_v4().simple().to_string();
    let payment = client.express_payment(
        method,
        reference,
        amount,
        &result_url,
        auth_email,
        &trace_id,
    );
    dbg!(payment.submit(&client).await?);
    Ok(())
}
