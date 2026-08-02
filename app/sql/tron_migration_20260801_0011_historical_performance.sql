CREATE TABLE IF NOT EXISTS tron_db.ingestion_benchmarks
(
    run_id String,
    chain LowCardinality(String),
    source_kind LowCardinality(String),
    start_block UInt64,
    end_block UInt64,
    requested_blocks UInt32,
    completed_blocks UInt32,
    transaction_count UInt64,
    elapsed_ms UInt64,
    blocks_per_second Float64,
    transactions_per_second Float64,
    rows_before UInt64,
    rows_after UInt64,
    compressed_bytes_before UInt64,
    compressed_bytes_after UInt64,
    investigation_address String,
    investigation_latency_ms UInt64,
    status LowCardinality(String),
    error_message String CODEC(ZSTD(3)),
    metrics_json String CODEC(ZSTD(3)),
    started_at_unix_ms UInt64,
    completed_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, run_id)
TTL toDateTime(inserted_at) + INTERVAL 365 DAY DELETE;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_from_address from_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_to_address to_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.address_relationships
    DROP INDEX IF EXISTS idx_from;

ALTER TABLE tron_db.address_relationships
    DROP INDEX IF EXISTS idx_to;

ALTER TABLE tron_db.address_relationships
    DROP COLUMN IF EXISTS amount_usd;

ALTER TABLE tron_db.wallet_asset_balance_deltas_v3
    ADD INDEX IF NOT EXISTS idx_balance_address address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.semantic_aml_events
    ADD INDEX IF NOT EXISTS idx_semantic_subject subject_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.exchange_flows_v2
    ADD INDEX IF NOT EXISTS idx_exchange_flow_from from_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.exchange_flows_v2
    ADD INDEX IF NOT EXISTS idx_exchange_flow_to to_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.ingested_blocks
    ADD INDEX IF NOT EXISTS idx_ingested_block_status ingestion_status TYPE set(8) GRANULARITY 4;
