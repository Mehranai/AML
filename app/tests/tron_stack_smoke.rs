use arz_axum_for_services::config::AppConfig;
use arz_axum_for_services::db::init_tron::init_tron_db;
use arz_axum_for_services::services::tron::neo4j::client::Neo4jClient;
use clickhouse::Client;
use neo4rs::query;
use serde::Deserialize;

#[derive(Debug, Deserialize, clickhouse::Row)]
struct ObjectCount {
    object_count: u64,
}

#[tokio::test]
#[ignore = "requires local ClickHouse and Neo4j services"]
async fn tron_schema_and_graph_dependencies_are_ready() -> anyhow::Result<()> {
    let config = AppConfig::from_env();
    let admin = Client::default()
        .with_url(&config.clickhouse_url)
        .with_user(&config.clickhouse_user)
        .with_password(&config.clickhouse_pass);

    init_tron_db(&admin, false).await?;

    let clickhouse = admin.with_database(&config.clickhouse_db_tron);
    let required_objects = [
        "transactions_canonical",
        "address_relationships_canonical",
        "ingested_blocks",
        "ingestion_failures",
        "ingestion_benchmarks",
        "intelligence_sources",
        "intelligence_reviews",
        "entity_labels",
        "address_cluster_claims",
        "address_cluster_memberships",
        "cluster_versions",
        "semantic_aml_events",
        "exchange_flows_v2",
        "exchange_flows_canonical",
        "wallet_asset_balance_deltas_v3",
        "wallet_asset_balances",
        "exposure_runs",
        "wallet_ml_model_deployments",
        "wallet_analysis_snapshots",
    ];
    let count = clickhouse
        .query(
            r#"
            SELECT count() AS object_count
            FROM system.tables
            WHERE database = ?
              AND name IN ?
            "#,
        )
        .bind(&config.clickhouse_db_tron)
        .bind(required_objects)
        .fetch_one::<ObjectCount>()
        .await?;
    assert_eq!(count.object_count, required_objects.len() as u64);

    let redundant_transfer_columns = clickhouse
        .query(
            r#"
            SELECT count()
            FROM system.columns
            WHERE database = ?
              AND table = 'address_relationships'
              AND name IN ('event_type', 'hop_count', 'protocol', 'amount_usd')
            "#,
        )
        .bind(&config.clickhouse_db_tron)
        .fetch_one::<u64>()
        .await?;
    assert_eq!(redundant_transfer_columns, 0);

    let redundant_transfer_indexes = clickhouse
        .query(
            r#"
            SELECT count()
            FROM system.data_skipping_indices
            WHERE database = ?
              AND table = 'address_relationships'
              AND name IN ('idx_from', 'idx_to')
            "#,
        )
        .bind(&config.clickhouse_db_tron)
        .fetch_one::<u64>()
        .await?;
    assert_eq!(redundant_transfer_indexes, 0);

    clickhouse
        .query("SELECT count() FROM address_relationships_canonical")
        .fetch_one::<u64>()
        .await?;
    clickhouse
        .query("SELECT count() FROM wallet_asset_balances")
        .fetch_one::<u64>()
        .await?;
    clickhouse
        .query("SELECT count() FROM exchange_flows_canonical")
        .fetch_one::<u64>()
        .await?;
    clickhouse
        .query("SELECT count() FROM ingestion_failures FINAL")
        .fetch_one::<u64>()
        .await?;
    clickhouse
        .query("SELECT count() FROM intelligence_sources FINAL WHERE chain = 'tron'")
        .fetch_one::<u64>()
        .await?;
    clickhouse
        .query("SELECT count() FROM address_cluster_claims FINAL WHERE chain = 'tron'")
        .fetch_one::<u64>()
        .await?;

    let neo4j = Neo4jClient::new(
        &config.neo4j_uri,
        &config.neo4j_username,
        &config.neo4j_password,
    )
    .await?;
    neo4j.graph.run(query("RETURN 1 AS ready")).await?;

    Ok(())
}
