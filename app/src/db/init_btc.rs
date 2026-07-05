use crate::db::init::run_sql;
use clickhouse::Client;

pub async fn init_btc_db(client: &Client) -> anyhow::Result<()> {
    let sql = include_str!("../../sql/init_database_btc.sql");
    run_sql(client, sql).await
}
