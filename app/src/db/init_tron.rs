use crate::db::init::run_sql;
use anyhow::{Context, anyhow};
use clickhouse::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const TRON_DB: &str = "tron_db";

pub async fn init_tron_db(
    client: &Client,
    allow_destructive_schema_cleanup: bool,
) -> anyhow::Result<()> {
    client
        .query("CREATE DATABASE IF NOT EXISTS tron_db")
        .execute()
        .await
        .context("failed to create tron_db")?;

    ensure_schema_migrations_table(client).await?;

    apply_tron_schema_migrations(client).await?;

    if allow_destructive_schema_cleanup {
        eprintln!("[TRON SCHEMA] Destructive cleanup is enabled by configuration");
        drop_legacy_tables(client).await?;
        drop_legacy_materialized_views(client).await?;
        drop_obsolete_tron_tables(client).await?;
    } else {
        warn_destructive_cleanup_disabled(client).await?;
    }

    validate_required_schemas(client).await?;

    Ok(())
}

struct SchemaMigration {
    migration_id: &'static str,
    description: &'static str,
    sql: &'static str,
    allow_checksum_drift: bool,
}

#[derive(Debug, Deserialize, clickhouse::Row)]
struct AppliedMigration {
    migration_id: String,
    checksum: String,
}

async fn ensure_schema_migrations_table(client: &Client) -> anyhow::Result<()> {
    client
        .query(
            r#"
            CREATE TABLE IF NOT EXISTS tron_db.schema_migrations
            (
                migration_id String,
                description String,
                checksum String,
                applied_at DateTime DEFAULT now()
            )
            ENGINE = ReplacingMergeTree(applied_at)
            ORDER BY migration_id
            "#,
        )
        .execute()
        .await
        .context("failed to create tron_db.schema_migrations")?;

    Ok(())
}

async fn apply_tron_schema_migrations(client: &Client) -> anyhow::Result<()> {
    let applied = client
        .query(
            r#"
            SELECT
                migration_id,
                argMax(checksum, applied_at) AS checksum
            FROM tron_db.schema_migrations
            GROUP BY migration_id
            "#,
        )
        .fetch_all::<AppliedMigration>()
        .await
        .context("failed to load TRON schema migration ledger")?;

    for migration in tron_schema_migrations() {
        let checksum = migration_checksum(migration.sql);

        if let Some(existing) = applied
            .iter()
            .find(|item| item.migration_id == migration.migration_id)
        {
            if existing.checksum != checksum {
                if migration.allow_checksum_drift {
                    eprintln!(
                        "[TRON SCHEMA] Bootstrap migration {} checksum changed; re-running idempotent schema bootstrap",
                        migration.migration_id
                    );
                    run_sql(client, migration.sql).await.with_context(|| {
                        format!(
                            "failed to refresh bootstrap migration {}",
                            migration.migration_id
                        )
                    })?;
                    record_migration(client, migration, checksum).await?;
                } else {
                    return Err(anyhow!(
                        "TRON schema migration {} checksum changed; create a new migration instead",
                        migration.migration_id
                    ));
                }
            }

            continue;
        }

        eprintln!(
            "[TRON SCHEMA] Applying migration {}: {}",
            migration.migration_id, migration.description
        );

        run_sql(client, migration.sql)
            .await
            .with_context(|| format!("failed to apply migration {}", migration.migration_id))?;
        record_migration(client, migration, checksum).await?;
    }

    Ok(())
}

async fn record_migration(
    client: &Client,
    migration: &SchemaMigration,
    checksum: String,
) -> anyhow::Result<()> {
    client
        .query(
            r#"
            INSERT INTO tron_db.schema_migrations
                (migration_id, description, checksum)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(migration.migration_id)
        .bind(migration.description)
        .bind(checksum)
        .execute()
        .await
        .context("failed to record schema migration")?;

    Ok(())
}

fn tron_schema_migrations() -> &'static [SchemaMigration] {
    &[
        SchemaMigration {
            migration_id: "20260701_0001_tron_active_schema",
            description: "Create active TRON AML schema and idempotent indexes",
            sql: include_str!("../../sql/init_database_tron.sql"),
            allow_checksum_drift: true,
        },
        SchemaMigration {
            migration_id: "20260705_0005_wallet_ml_native",
            description: "Replace formula-based wallet AI risk storage with ML-native labels, models, features, and predictions",
            sql: include_str!("../../sql/tron_migration_20260705_0005_wallet_ml_native.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260726_0006_analytical_node",
            description: "Create Analytical Node tables for wallet analysis snapshots, evidence, subjects, and jobs",
            sql: include_str!("../../sql/tron_migration_20260726_0006_analytical_node.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260729_0007_evidence_integrity",
            description: "Add canonical replay-safe evidence views, block journal, semantic events, and event-keyed holdings",
            sql: include_str!("../../sql/tron_migration_20260729_0007_evidence_integrity.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260729_0008_compact_transfer_facts",
            description: "Use compact canonical transfer facts for full TRON value coverage and holdings",
            sql: include_str!("../../sql/tron_migration_20260729_0008_compact_transfer_facts.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260729_0009_minimal_transfer_columns",
            description: "Remove constant and transaction-level fields from canonical transfer storage",
            sql: include_str!(
                "../../sql/tron_migration_20260729_0009_minimal_transfer_columns.sql"
            ),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260729_0010_ingestion_recovery",
            description: "Add durable ingestion failures and replay-safe exchange-flow evidence",
            sql: include_str!("../../sql/tron_migration_20260729_0010_ingestion_recovery.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260801_0011_historical_performance",
            description: "Add bounded benchmark history and targeted TRON query indexes",
            sql: include_str!("../../sql/tron_migration_20260801_0011_historical_performance.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0012_entity_intelligence",
            description: "Add governed entity sources, immutable reviews, structural cluster claims, and versioned memberships",
            sql: include_str!("../../sql/tron_migration_20260802_0012_entity_intelligence.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0013_entity_intelligence_cleanup",
            description: "Prevent the schema bootstrap from recreating superseded transfer indexes",
            sql: include_str!(
                "../../sql/tron_migration_20260802_0013_entity_intelligence_cleanup.sql"
            ),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0014_bootstrap_cluster_anchors",
            description: "Create governed version-one cluster memberships for verified exchange service anchors",
            sql: include_str!(
                "../../sql/tron_migration_20260802_0014_bootstrap_cluster_anchors.sql"
            ),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0015_canonical_flow_operations",
            description: "Expose transaction semantics through the canonical TRON flow read model",
            sql: include_str!(
                "../../sql/tron_migration_20260802_0015_canonical_flow_operations.sql"
            ),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0016_governed_exchange_flows",
            description: "Exclude unreviewed exchange guesses from canonical AML graph evidence",
            sql: include_str!("../../sql/tron_migration_20260802_0016_governed_exchange_flows.sql"),
            allow_checksum_drift: false,
        },
        SchemaMigration {
            migration_id: "20260802_0017_deactivate_heuristic_entities",
            description: "Deactivate legacy topology guesses in the entity projection",
            sql: include_str!(
                "../../sql/tron_migration_20260802_0017_deactivate_heuristic_entities.sql"
            ),
            allow_checksum_drift: false,
        },
    ]
}

fn migration_checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}

async fn warn_destructive_cleanup_disabled(client: &Client) -> anyhow::Result<()> {
    let mut obsolete_count = 0usize;

    for table in obsolete_tron_tables() {
        if table_exists(client, table).await? {
            obsolete_count += 1;
        }
    }

    if obsolete_count > 0 {
        eprintln!(
            "[TRON SCHEMA] Destructive cleanup is disabled; {obsolete_count} obsolete objects remain. Set TRON_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP=true to drop them explicitly."
        );
    }

    Ok(())
}

async fn drop_legacy_tables(client: &Client) -> anyhow::Result<()> {
    let tables = client
        .query(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = ?
              AND position(name, '_legacy_') > 0
            "#,
        )
        .bind(TRON_DB)
        .fetch_all::<TableInfo>()
        .await
        .context("failed to inspect legacy Tron tables")?;

    for table in tables {
        let stmt = format!("DROP TABLE IF EXISTS {}.{}", TRON_DB, table.name);

        eprintln!(
            "[TRON SCHEMA] Dropping legacy ClickHouse table {}.{}",
            TRON_DB, table.name
        );

        client
            .query(&stmt)
            .execute()
            .await
            .with_context(|| format!("failed to drop legacy table {}", table.name))?;
    }

    Ok(())
}

async fn drop_obsolete_tron_tables(client: &Client) -> anyhow::Result<()> {
    for table in obsolete_tron_tables() {
        if table_exists(client, table).await? {
            let stmt = format!("DROP TABLE IF EXISTS {}.{}", TRON_DB, table);

            eprintln!(
                "[TRON SCHEMA] Dropping obsolete ClickHouse table {}.{}",
                TRON_DB, table
            );

            client
                .query(&stmt)
                .execute()
                .await
                .with_context(|| format!("failed to drop obsolete table {}", table))?;
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize, clickhouse::Row)]
struct ColumnInfo {
    name: String,
    #[serde(rename = "type")]
    data_type: String,
}

#[derive(Debug)]
struct TableSchema {
    table: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

async fn validate_required_schemas(client: &Client) -> anyhow::Result<()> {
    let mut incompatible_tables = Vec::new();

    for schema in required_tron_schemas() {
        let columns = load_columns(client, schema.table).await?;

        if columns.is_empty() {
            continue;
        }

        if !schema_matches(&columns, schema.columns) {
            incompatible_tables.push(schema.table);
        }
    }

    if !incompatible_tables.is_empty() {
        return Err(anyhow!(
            "TRON schema validation failed for active objects: {}",
            incompatible_tables.join(", ")
        ));
    }

    Ok(())
}

async fn load_columns(client: &Client, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    client
        .query(
            r#"
            SELECT
                name,
                type
            FROM system.columns
            WHERE database = ?
              AND table = ?
            "#,
        )
        .bind(TRON_DB)
        .bind(table)
        .fetch_all::<ColumnInfo>()
        .await
        .with_context(|| format!("failed to inspect ClickHouse schema for {}", table))
}

fn schema_matches(actual: &[ColumnInfo], required: &[(&str, &str)]) -> bool {
    required.iter().all(|(required_name, required_type)| {
        actual.iter().any(|column| {
            column.name == *required_name && schema_type_matches(&column.data_type, required_type)
        })
    })
}

fn schema_type_matches(actual: &str, required: &str) -> bool {
    actual == required
        || (required == "String" && actual == "LowCardinality(String)")
        || (actual == "String" && required == "LowCardinality(String)")
        || (required.starts_with("DateTime") && actual.starts_with("DateTime"))
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
        .bind(TRON_DB)
        .bind(table)
        .fetch_one::<u64>()
        .await
        .with_context(|| format!("failed to inspect ClickHouse table {}", table))?;

    Ok(count > 0)
}

#[derive(Debug, Deserialize, clickhouse::Row)]
struct TableInfo {
    name: String,
}

async fn drop_legacy_materialized_views(client: &Client) -> anyhow::Result<()> {
    let views = client
        .query(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = ?
              AND engine = 'MaterializedView'
              AND (
                    name IN ('mv_token_delta_from', 'mv_token_delta_to')
                    OR startsWith(name, 'mv_token_delta_from_legacy_')
                    OR startsWith(name, 'mv_token_delta_to_legacy_')
                    OR startsWith(name, 'mv_token_balance_legacy_')
                  )
            "#,
        )
        .bind(TRON_DB)
        .fetch_all::<TableInfo>()
        .await
        .context("failed to inspect legacy Tron materialized views")?;

    for view in views {
        let stmt = format!("DROP TABLE IF EXISTS {}.{}", TRON_DB, view.name);

        eprintln!(
            "[TRON SCHEMA] Dropping legacy ClickHouse materialized view {}.{}",
            TRON_DB, view.name
        );

        client
            .query(&stmt)
            .execute()
            .await
            .with_context(|| format!("failed to drop legacy materialized view {}", view.name))?;
    }

    Ok(())
}

fn required_tron_schemas() -> &'static [TableSchema] {
    &[
        TableSchema {
            table: "transactions",
            columns: &[
                ("tx_hash", "String"),
                ("block_number", "UInt64"),
                ("timestamp", "UInt64"),
                ("from_address", "String"),
                ("to_address", "String"),
                ("contract_address", "String"),
                ("contract_type", "String"),
                ("amount", "UInt256"),
                ("status", "UInt8"),
            ],
        },
        TableSchema {
            table: "address_relationships",
            columns: &[
                ("relationship_id", "String"),
                ("from_address", "String"),
                ("to_address", "String"),
                ("token_address", "String"),
                ("tx_hash", "String"),
                ("amount", "UInt256"),
                ("transfer_type", "String"),
            ],
        },
        TableSchema {
            table: "transaction_features",
            columns: &[
                ("tx_hash", "String"),
                ("timestamp", "UInt64"),
                ("is_swap", "UInt8"),
                ("is_contract_call", "UInt8"),
                ("fan_in", "UInt16"),
                ("fan_out", "UInt16"),
            ],
        },
        TableSchema {
            table: "contract_metadata",
            columns: &[
                ("contract_address", "String"),
                ("protocol_name", "String"),
                ("contract_type", "String"),
                ("creator_address", "String"),
                ("created_block", "UInt64"),
            ],
        },
        TableSchema {
            table: "address_entity",
            columns: &[
                ("address", "String"),
                ("entity_id", "String"),
                ("entity_name", "String"),
                ("entity_type", "String"),
                ("confidence", "Float32"),
                ("source", "String"),
                ("is_active", "UInt8"),
                ("created_at", "DateTime64(3)"),
            ],
        },
        TableSchema {
            table: "exchange_addresses",
            columns: &[
                ("address", "String"),
                ("entity_id", "String"),
                ("exchange_name", "String"),
                ("address_role", "String"),
                ("confidence", "Float32"),
                ("detection_source", "String"),
                ("first_seen_block", "UInt64"),
                ("last_seen_block", "UInt64"),
                ("is_active", "UInt8"),
            ],
        },
        TableSchema {
            table: "exchange_flows_v2",
            columns: &[
                ("flow_id", "String"),
                ("tx_hash", "String"),
                ("block_number", "UInt64"),
                ("from_address", "String"),
                ("to_address", "String"),
                ("exchange_name", "String"),
                ("amount", "UInt256"),
            ],
        },
        TableSchema {
            table: "wallet_asset_balance_deltas_v3",
            columns: &[
                ("delta_id", "String"),
                ("tx_hash", "String"),
                ("block_number", "UInt64"),
                ("timestamp", "UInt64"),
                ("address", "String"),
                ("asset_type", "String"),
                ("asset_id", "String"),
                ("amount_raw", "UInt256"),
                ("direction", "Int8"),
            ],
        },
        TableSchema {
            table: "ingested_blocks",
            columns: &[
                ("chain", "LowCardinality(String)"),
                ("block_number", "UInt64"),
                ("block_hash", "String"),
                ("parent_hash", "String"),
                ("block_timestamp", "UInt64"),
                ("transaction_count", "UInt32"),
                ("finality_status", "LowCardinality(String)"),
                ("ingestion_status", "LowCardinality(String)"),
            ],
        },
        TableSchema {
            table: "ingestion_failures",
            columns: &[
                ("failure_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("block_number", "UInt64"),
                ("block_hash", "String"),
                ("tx_hash", "String"),
                ("stage", "LowCardinality(String)"),
                ("error_class", "LowCardinality(String)"),
                ("error_message", "String"),
                ("retryable", "UInt8"),
                ("attempt_count", "UInt32"),
                ("status", "LowCardinality(String)"),
                ("first_failed_at_unix_ms", "UInt64"),
                ("last_failed_at_unix_ms", "UInt64"),
                ("resolved_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "ingestion_benchmarks",
            columns: &[
                ("run_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("source_kind", "LowCardinality(String)"),
                ("start_block", "UInt64"),
                ("end_block", "UInt64"),
                ("requested_blocks", "UInt32"),
                ("completed_blocks", "UInt32"),
                ("transaction_count", "UInt64"),
                ("elapsed_ms", "UInt64"),
                ("blocks_per_second", "Float64"),
                ("transactions_per_second", "Float64"),
                ("rows_before", "UInt64"),
                ("rows_after", "UInt64"),
                ("compressed_bytes_before", "UInt64"),
                ("compressed_bytes_after", "UInt64"),
                ("investigation_address", "String"),
                ("investigation_latency_ms", "UInt64"),
                ("status", "LowCardinality(String)"),
                ("error_message", "String"),
                ("metrics_json", "String"),
                ("started_at_unix_ms", "UInt64"),
                ("completed_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "semantic_aml_events",
            columns: &[
                ("event_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("tx_hash", "String"),
                ("block_number", "UInt64"),
                ("timestamp", "UInt64"),
                ("event_type", "LowCardinality(String)"),
                ("subject_address", "String"),
                ("protocol", "String"),
                ("evidence_json", "String"),
            ],
        },
        TableSchema {
            table: "entity_labels",
            columns: &[
                ("label_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("address", "String"),
                ("entity_id", "String"),
                ("entity_name", "String"),
                ("entity_type", "LowCardinality(String)"),
                ("address_role", "LowCardinality(String)"),
                ("confidence", "Float32"),
                ("risk_percent", "UInt8"),
                ("source", "String"),
                ("source_record_id", "String"),
                ("supersedes_label_id", "String"),
                ("submitted_by", "String"),
                ("evidence_refs", "Array(String)"),
                ("review_status", "LowCardinality(String)"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "intelligence_sources",
            columns: &[
                ("chain", "LowCardinality(String)"),
                ("source_id", "String"),
                ("source_name", "String"),
                ("source_type", "LowCardinality(String)"),
                ("trust_tier", "LowCardinality(String)"),
                ("is_active", "UInt8"),
                ("created_by", "String"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "intelligence_reviews",
            columns: &[
                ("review_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("subject_type", "LowCardinality(String)"),
                ("subject_id", "String"),
                ("decision", "LowCardinality(String)"),
                ("reviewer", "String"),
                ("reason", "String"),
                ("evidence_refs", "Array(String)"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "address_cluster_claims",
            columns: &[
                ("claim_id", "String"),
                ("chain", "LowCardinality(String)"),
                ("address", "String"),
                ("cluster_id", "String"),
                ("cluster_type", "LowCardinality(String)"),
                ("address_role", "LowCardinality(String)"),
                ("claim_method", "LowCardinality(String)"),
                ("confidence", "Float32"),
                ("source", "String"),
                ("evidence_tx_hashes", "Array(String)"),
                ("evidence_addresses", "Array(String)"),
                ("evidence_json", "String"),
                ("review_status", "LowCardinality(String)"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "address_cluster_memberships",
            columns: &[
                ("chain", "LowCardinality(String)"),
                ("address", "String"),
                ("cluster_id", "String"),
                ("cluster_type", "LowCardinality(String)"),
                ("address_role", "LowCardinality(String)"),
                ("confidence", "Float32"),
                ("source_claim_id", "String"),
                ("review_id", "String"),
                ("cluster_version", "UInt32"),
                ("is_active", "UInt8"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "cluster_versions",
            columns: &[
                ("chain", "LowCardinality(String)"),
                ("cluster_id", "String"),
                ("version", "UInt32"),
                ("cluster_type", "LowCardinality(String)"),
                ("display_name", "String"),
                ("change_type", "LowCardinality(String)"),
                ("source_claim_ids", "Array(String)"),
                ("active_member_count", "UInt64"),
                ("created_by", "String"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "token_metadata_discoveries",
            columns: &[
                ("token_address", "String"),
                ("discovered_block", "UInt64"),
                ("discovered_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "token_metadata_jobs",
            columns: &[
                ("token_address", "String"),
                ("discovered_block", "UInt64"),
                ("status", "LowCardinality(String)"),
                ("attempt_count", "UInt8"),
                ("last_error", "String"),
                ("updated_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_asset_balances",
            columns: &[
                ("address", "String"),
                ("asset_type", "String"),
                ("asset_id", "String"),
                ("asset_symbol", "String"),
                ("asset_name", "String"),
                ("decimals", "UInt8"),
                ("balance_raw", "UInt256"),
                ("balance_incomplete", "UInt8"),
                ("balance_decimal", "Float64"),
            ],
        },
        TableSchema {
            table: "exposure_seeds",
            columns: &[
                ("address", "String"),
                ("entity_name", "String"),
                ("entity_type", "String"),
                ("risk_level", "UInt8"),
                ("source", "String"),
                ("source_label_id", "String"),
                ("is_active", "UInt8"),
                ("created_at", "DateTime64(3)"),
            ],
        },
        TableSchema {
            table: "address_exposure",
            columns: &[
                ("source_address", "String"),
                ("exposed_address", "String"),
                ("hop_distance", "UInt8"),
                ("exposure_score", "Float64"),
                ("path_count", "UInt32"),
                ("last_tx_hash", "String"),
                ("last_seen_block", "UInt64"),
                ("exposure_type", "String"),
                ("direction", "String"),
                ("best_path_amount_share", "Float64"),
                ("best_path_time_weight", "Float64"),
                ("service_mediated", "UInt8"),
                ("propagation_run_id", "String"),
                ("computed_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "exposure_runs",
            columns: &[
                ("source_address", "String"),
                ("propagation_run_id", "String"),
                ("status", "String"),
                ("max_hops", "UInt8"),
                ("row_count", "UInt64"),
                ("completed_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_labels",
            columns: &[
                ("address", "String"),
                ("label", "UInt8"),
                ("label_name", "String"),
                ("typologies", "Array(String)"),
                ("confidence", "Float32"),
                ("source", "String"),
                ("case_id", "String"),
                ("evidence_refs", "Array(String)"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_feature_snapshots",
            columns: &[
                ("snapshot_id", "String"),
                ("address", "String"),
                ("window_days", "UInt16"),
                ("feature_schema_version", "String"),
                ("feature_names", "Array(String)"),
                ("features_json", "String"),
                ("evidence_refs", "Array(String)"),
                ("generated_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_training_runs",
            columns: &[
                ("training_run_id", "String"),
                ("model_id", "String"),
                ("model_version", "String"),
                ("feature_schema_version", "String"),
                ("training_dataset_id", "String"),
                ("dataset_sha256", "String"),
                ("label_policy", "String"),
                ("train_sample_count", "UInt64"),
                ("validation_sample_count", "UInt64"),
                ("test_sample_count", "UInt64"),
                ("positive_label_count", "UInt64"),
                ("negative_label_count", "UInt64"),
                ("metrics_json", "String"),
                ("parameters_json", "String"),
                ("artifact_uri", "String"),
                ("artifact_json", "String"),
                ("status", "String"),
                ("started_at_unix_ms", "UInt64"),
                ("completed_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_model_registry",
            columns: &[
                ("model_id", "String"),
                ("model_version", "String"),
                ("model_family", "String"),
                ("feature_schema_version", "String"),
                ("calibration_version", "String"),
                ("artifact_json", "String"),
                ("artifact_sha256", "String"),
                ("metrics_json", "String"),
                ("training_run_id", "String"),
                ("training_dataset_id", "String"),
                ("label_policy", "String"),
                ("model_quality_score", "Float32"),
                ("status", "String"),
                ("trained_at_unix_ms", "UInt64"),
                ("activated_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_predictions",
            columns: &[
                ("prediction_id", "String"),
                ("snapshot_id", "String"),
                ("model_id", "String"),
                ("model_version", "String"),
                ("model_family", "String"),
                ("calibration_version", "String"),
                ("address", "String"),
                ("window_days", "UInt16"),
                ("risk_probability", "Float32"),
                ("risk_percent", "UInt8"),
                ("risk_level", "String"),
                ("confidence", "Float32"),
                ("feature_importance_json", "String"),
                ("model_patterns_json", "String"),
                ("evidence_refs", "Array(String)"),
                ("predicted_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_ml_model_deployments",
            columns: &[
                ("environment", "LowCardinality(String)"),
                ("feature_schema_version", "String"),
                ("deployment_id", "String"),
                ("model_id", "String"),
                ("model_version", "String"),
                ("status", "LowCardinality(String)"),
                ("deployed_by", "String"),
                ("deployed_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "analysis_subjects",
            columns: &[
                ("chain", "String"),
                ("subject_type", "String"),
                ("subject_id", "String"),
                ("address", "String"),
                ("entity_id", "String"),
                ("latest_snapshot_id", "String"),
                ("latest_status", "String"),
                ("latest_risk_available", "UInt8"),
                ("latest_risk_level", "String"),
                ("latest_risk_probability", "Float32"),
                ("latest_confidence", "Float32"),
                ("latest_data_cutoff_block", "UInt64"),
                ("latest_data_cutoff_unix_ms", "UInt64"),
                ("latest_input_version", "String"),
                ("created_at_unix_ms", "UInt64"),
                ("updated_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_analysis_snapshots",
            columns: &[
                ("snapshot_id", "String"),
                ("chain", "String"),
                ("address", "String"),
                ("entity_id", "String"),
                ("analysis_version", "String"),
                ("analysis_status", "String"),
                ("risk_available", "UInt8"),
                ("risk_level", "String"),
                ("risk_probability", "Float32"),
                ("risk_percent", "UInt8"),
                ("confidence", "Float32"),
                ("wallet_type", "String"),
                ("fingerprint_label", "String"),
                ("graph_depth", "UInt8"),
                ("graph_node_count", "UInt32"),
                ("graph_edge_count", "UInt32"),
                ("exchange_interaction_count", "UInt32"),
                ("holdings_asset_count", "UInt64"),
                ("holdings_metadata_gap_count", "UInt32"),
                ("observed_transfers", "UInt64"),
                ("incoming_transfers", "UInt64"),
                ("outgoing_transfers", "UInt64"),
                ("exposure_score", "Float32"),
                ("exposure_source_count", "UInt32"),
                ("exposure_path_count", "UInt64"),
                ("exposure_min_hop_distance", "UInt8"),
                ("data_cutoff_block", "UInt64"),
                ("data_cutoff_unix_ms", "UInt64"),
                ("analysis_input_version", "String"),
                ("source_tables", "Array(String)"),
                ("model_id", "String"),
                ("model_version", "String"),
                ("feature_schema_version", "String"),
                ("snapshot_json", "String"),
                ("warnings", "Array(String)"),
                ("evidence_refs", "Array(String)"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "wallet_analysis_evidence",
            columns: &[
                ("evidence_id", "String"),
                ("snapshot_id", "String"),
                ("chain", "String"),
                ("address", "String"),
                ("evidence_type", "String"),
                ("evidence_key", "String"),
                ("evidence_value", "String"),
                ("severity", "String"),
                ("related_tx_hash", "String"),
                ("related_address", "String"),
                ("created_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "analysis_jobs",
            columns: &[
                ("job_id", "String"),
                ("chain", "String"),
                ("subject_type", "String"),
                ("subject_id", "String"),
                ("requested_by", "String"),
                ("status", "String"),
                ("parameters_json", "String"),
                ("snapshot_id", "String"),
                ("error_message", "String"),
                ("requested_at_unix_ms", "UInt64"),
                ("started_at_unix_ms", "UInt64"),
                ("completed_at_unix_ms", "UInt64"),
                ("updated_at_unix_ms", "UInt64"),
            ],
        },
        TableSchema {
            table: "sync_state",
            columns: &[("chain", "String"), ("last_synced_block", "UInt64")],
        },
    ]
}

fn obsolete_tron_tables() -> &'static [&'static str] {
    &[
        "address_behavior",
        "address_clusters",
        "address_tags",
        "address_profiles",
        "address_counterparties",
        "address_token_balance",
        "address_token_delta",
        "aml_events",
        "blocks",
        "cluster_edges",
        "contract_interactions",
        "exchange_clusters",
        "exchange_deposit_addresses",
        "exchange_entities",
        "entity_relationships",
        "exposure_paths",
        "flow_edges_hourly",
        "flow_segments",
        "graph_edges",
        "internal_transfers",
        "investigation_cache",
        "method_signatures",
        "mv_token_balance",
        "transaction_risk",
        "wallet_asset_balance_deltas",
        "wallet_asset_balance_deltas_v2",
        "schema_lifecycle",
        "sweep_edges",
        "wallet_counterparty_fingerprints",
        "wallet_fingerprints",
        "wallet_info",
        "wallet_risk",
        "wallet_risk_assessments",
        "wallet_feature_snapshots",
        "wallet_ai_risk_assessments",
        "owner_info",
        "contract_calls",
        "address_energy_usage",
        "wallet_state",
        "raw_logs",
        "token_transfers",
        "exchange_flows",
    ]
}
