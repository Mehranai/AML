use crate::db::init::run_sql;
use anyhow::Context;
use clickhouse::Client;

pub async fn init_bsc_db(client: &Client) -> anyhow::Result<()> {
    let sql = include_str!("../../sql/init_database_bsc.sql");
    run_sql(client, sql).await?;
    drop_obsolete_bsc_schema(client).await
}

async fn drop_obsolete_bsc_schema(client: &Client) -> anyhow::Result<()> {
    for object in ["address_tags"] {
        let stmt = format!("DROP TABLE IF EXISTS bsc_db.{}", object);

        eprintln!(
            "[BSC SCHEMA] Dropping obsolete ClickHouse object bsc_db.{}",
            object
        );

        client
            .query(&stmt)
            .execute()
            .await
            .with_context(|| format!("failed to drop obsolete BSC object {}", object))?;
    }

    Ok(())
}
