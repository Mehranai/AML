use crate::db::init::run_sql;
use anyhow::Context;
use clickhouse::Client;

const ETH_DB: &str = "eth_db";

pub async fn init_eth_db(
    client: &Client,
    allow_destructive_schema_cleanup: bool,
) -> anyhow::Result<()> {
    let address_relationships_existed = table_exists(client, "address_relationships").await?;
    let wallet_asset_balance_deltas_existed =
        table_exists(client, "wallet_asset_balance_deltas").await?;

    let sql = include_str!("../../sql/init_database_eth.sql");
    run_sql(client, sql).await?;

    if allow_destructive_schema_cleanup {
        eprintln!("[ETH SCHEMA] Destructive cleanup is enabled by configuration");
        drop_obsolete_eth_schema(client).await?;
    } else {
        warn_destructive_cleanup_disabled(client).await?;
    }

    if !address_relationships_existed || table_is_empty(client, "address_relationships").await? {
        backfill_address_relationships(client).await?;
    }

    if !wallet_asset_balance_deltas_existed
        || table_is_empty(client, "wallet_asset_balance_deltas").await?
    {
        backfill_wallet_asset_balance_deltas(client).await?;
    }

    Ok(())
}

async fn drop_obsolete_eth_schema(client: &Client) -> anyhow::Result<()> {
    for object in obsolete_eth_objects() {
        let stmt = format!("DROP TABLE IF EXISTS {}.{}", ETH_DB, object);

        eprintln!(
            "[ETH SCHEMA] Dropping obsolete ClickHouse object {}.{}",
            ETH_DB, object
        );

        client
            .query(&stmt)
            .execute()
            .await
            .with_context(|| format!("failed to drop obsolete ETH object {}", object))?;
    }

    Ok(())
}

async fn warn_destructive_cleanup_disabled(client: &Client) -> anyhow::Result<()> {
    let mut obsolete_count = 0usize;

    for object in obsolete_eth_objects() {
        if table_exists(client, object).await? {
            obsolete_count += 1;
        }
    }

    if obsolete_count > 0 {
        eprintln!(
            "[ETH SCHEMA] Destructive cleanup is disabled; {obsolete_count} obsolete objects remain. Set ETH_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP=true to drop them explicitly."
        );
    }

    Ok(())
}

async fn table_exists(client: &Client, table: &str) -> anyhow::Result<bool> {
    let count = client
        .query(
            r#"
            SELECT count()
            FROM system.tables
            WHERE database = ?
              AND name = ?
            "#,
        )
        .bind(ETH_DB)
        .bind(table)
        .fetch_one::<u64>()
        .await
        .with_context(|| format!("failed to inspect ClickHouse object {}", table))?;

    Ok(count > 0)
}

async fn table_is_empty(client: &Client, table: &str) -> anyhow::Result<bool> {
    let stmt = format!("SELECT count() FROM {}.{}", ETH_DB, table);

    let count = client
        .query(&stmt)
        .fetch_one::<u64>()
        .await
        .with_context(|| format!("failed to count ClickHouse table {}", table))?;

    Ok(count == 0)
}

async fn backfill_wallet_asset_balance_deltas(client: &Client) -> anyhow::Result<()> {
    let statements = [
        r#"
        INSERT INTO eth_db.wallet_asset_balance_deltas
        (
            tx_hash,
            block_number,
            timestamp,
            address,
            asset_type,
            asset_id,
            delta_raw,
            direction
        )
        SELECT
            hash AS tx_hash,
            block_number,
            0 AS timestamp,
            from_addr AS address,
            'native' AS asset_type,
            'ETH' AS asset_id,
            -toInt256(value) AS delta_raw,
            -1 AS direction
        FROM eth_db.transactions
        WHERE from_addr != ''
          AND lower(from_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(value) > 0
        "#,
        r#"
        INSERT INTO eth_db.wallet_asset_balance_deltas
        (
            tx_hash,
            block_number,
            timestamp,
            address,
            asset_type,
            asset_id,
            delta_raw,
            direction
        )
        SELECT
            hash AS tx_hash,
            block_number,
            0 AS timestamp,
            to_addr AS address,
            'native' AS asset_type,
            'ETH' AS asset_id,
            toInt256(value) AS delta_raw,
            1 AS direction
        FROM eth_db.transactions
        WHERE to_addr != ''
          AND lower(to_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(value) > 0
        "#,
        r#"
        INSERT INTO eth_db.wallet_asset_balance_deltas
        (
            tx_hash,
            block_number,
            timestamp,
            address,
            asset_type,
            asset_id,
            delta_raw,
            direction
        )
        SELECT
            tx_hash,
            block_number,
            0 AS timestamp,
            from_addr AS address,
            'erc20' AS asset_type,
            token_address AS asset_id,
            -toInt256(amount) AS delta_raw,
            -1 AS direction
        FROM eth_db.token_transfers
        WHERE from_addr != ''
          AND lower(from_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(amount) > 0
        "#,
        r#"
        INSERT INTO eth_db.wallet_asset_balance_deltas
        (
            tx_hash,
            block_number,
            timestamp,
            address,
            asset_type,
            asset_id,
            delta_raw,
            direction
        )
        SELECT
            tx_hash,
            block_number,
            0 AS timestamp,
            to_addr AS address,
            'erc20' AS asset_type,
            token_address AS asset_id,
            toInt256(amount) AS delta_raw,
            1 AS direction
        FROM eth_db.token_transfers
        WHERE to_addr != ''
          AND lower(to_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(amount) > 0
        "#,
    ];

    eprintln!("[ETH SCHEMA] Backfilling wallet_asset_balance_deltas from existing transfers");

    for statement in statements {
        client
            .query(statement)
            .execute()
            .await
            .context("failed to backfill ETH wallet_asset_balance_deltas")?;
    }

    Ok(())
}

async fn backfill_address_relationships(client: &Client) -> anyhow::Result<()> {
    let statements = [
        r#"
        INSERT INTO eth_db.address_relationships
        (
            relationship_id,
            from_address,
            to_address,
            token_address,
            tx_hash,
            block_number,
            timestamp,
            amount,
            transfer_type,
            protocol,
            event_type,
            risk_score,
            hop_count
        )
        SELECT
            concat(
                hash,
                ':native:0:',
                lower(from_addr),
                ':',
                lower(to_addr),
                ':ETH'
            ) AS relationship_id,
            lower(from_addr) AS from_address,
            lower(to_addr) AS to_address,
            'ETH' AS token_address,
            hash AS tx_hash,
            block_number,
            0 AS timestamp,
            value AS amount,
            'native_transfer' AS transfer_type,
            'ethereum' AS protocol,
            'transfer' AS event_type,
            0 AS risk_score,
            1 AS hop_count
        FROM eth_db.transactions
        WHERE from_addr != ''
          AND to_addr != ''
          AND lower(from_addr) != '0x0000000000000000000000000000000000000000'
          AND lower(to_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(value) > 0
        "#,
        r#"
        INSERT INTO eth_db.address_relationships
        (
            relationship_id,
            from_address,
            to_address,
            token_address,
            tx_hash,
            block_number,
            timestamp,
            amount,
            transfer_type,
            protocol,
            event_type,
            risk_score,
            hop_count
        )
        SELECT
            concat(
                tx_hash,
                ':erc20:',
                toString(log_index),
                ':',
                lower(from_addr),
                ':',
                lower(to_addr),
                ':',
                lower(token_address)
            ) AS relationship_id,
            lower(from_addr) AS from_address,
            lower(to_addr) AS to_address,
            lower(token_address) AS token_address,
            tx_hash,
            block_number,
            0 AS timestamp,
            amount,
            'erc20_transfer' AS transfer_type,
            'erc20' AS protocol,
            'transfer' AS event_type,
            0 AS risk_score,
            1 AS hop_count
        FROM eth_db.token_transfers
        WHERE from_addr != ''
          AND to_addr != ''
          AND lower(from_addr) != '0x0000000000000000000000000000000000000000'
          AND lower(to_addr) != '0x0000000000000000000000000000000000000000'
          AND toInt256(amount) > 0
        "#,
    ];

    eprintln!("[ETH SCHEMA] Backfilling address_relationships from existing transfers");

    for statement in statements {
        client
            .query(statement)
            .execute()
            .await
            .context("failed to backfill ETH address_relationships")?;
    }

    Ok(())
}

fn obsolete_eth_objects() -> &'static [&'static str] {
    &[
        "mv_token_balance",
        "mv_token_delta_from",
        "mv_token_delta_to",
        "address_token_balance",
        "address_token_delta",
        "address_tags",
    ]
}
