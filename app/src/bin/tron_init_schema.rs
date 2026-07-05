use anyhow::Result;
use arz_axum_for_services::{config::AppConfig, db::init_tron::init_tron_db};
use clickhouse::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env();
    let admin_client = Client::default()
        .with_url(&config.clickhouse_url)
        .with_user(&config.clickhouse_user)
        .with_password(&config.clickhouse_pass);

    init_tron_db(&admin_client, config.tron_allow_destructive_schema_cleanup).await?;

    println!("TRON schema migrations completed.");

    Ok(())
}
