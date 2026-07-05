use crate::db::init::run_sql;
use anyhow::Context;
use clickhouse::Client;

pub async fn init_eth_db(client: &Client) -> anyhow::Result<()> {
    let sql = include_str!("../../sql/init_database_eth.sql");
    run_sql(client, sql).await?;
    drop_obsolete_eth_schema(client).await
}

async fn drop_obsolete_eth_schema(client: &Client) -> anyhow::Result<()> {
    let objects = [
        "mv_token_balance",
        "mv_token_delta_from",
        "mv_token_delta_to",
        "address_token_balance",
        "address_token_delta",
        "address_tags",
    ];

    for object in objects {
        let stmt = format!("DROP TABLE IF EXISTS eth_db.{}", object);

        eprintln!(
            "[ETH SCHEMA] Dropping obsolete ClickHouse object eth_db.{}",
            object
        );

        client
            .query(&stmt)
            .execute()
            .await
            .with_context(|| format!("failed to drop obsolete ETH object {}", object))?;
    }

    Ok(())
}
