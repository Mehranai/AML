CREATE TABLE IF NOT EXISTS tron_db.ingestion_failures
(
    failure_id String,
    chain LowCardinality(String),
    block_number UInt64,
    block_hash String,
    tx_hash String,
    stage LowCardinality(String),
    error_class LowCardinality(String),
    error_message String CODEC(ZSTD(3)),
    retryable UInt8,
    attempt_count UInt32,
    status LowCardinality(String),
    first_failed_at_unix_ms UInt64,
    last_failed_at_unix_ms UInt64,
    resolved_at_unix_ms UInt64,
    updated_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY (chain, failure_id);

ALTER TABLE tron_db.ingestion_failures
    ADD INDEX IF NOT EXISTS idx_ingestion_failure_status status TYPE set(16) GRANULARITY 4;

CREATE TABLE IF NOT EXISTS tron_db.exchange_flows_v2
(
    flow_id String,
    tx_hash String,
    block_number UInt64,
    from_address String,
    to_address String,
    exchange_name String,
    flow_type LowCardinality(String),
    token_address String,
    amount UInt256,
    confidence Float32,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
PARTITION BY intDiv(block_number, 1000000)
ORDER BY flow_id;

INSERT INTO tron_db.exchange_flows_v2
(
    flow_id,
    tx_hash,
    block_number,
    from_address,
    to_address,
    exchange_name,
    flow_type,
    token_address,
    amount,
    confidence,
    inserted_at
)
SELECT
    lower(hex(SHA256(concat(
        tx_hash,
        '|',
        from_address,
        '|',
        to_address,
        '|',
        token_address,
        '|',
        toString(amount),
        '|',
        flow_type,
        '|',
        exchange_name
    )))) AS flow_id,
    tx_hash,
    block_number,
    from_address,
    to_address,
    exchange_name,
    flow_type,
    token_address,
    amount,
    max(confidence) AS confidence,
    toDateTime64(max(created_at), 3) AS inserted_at
FROM tron_db.exchange_flows
GROUP BY
    tx_hash,
    block_number,
    from_address,
    to_address,
    exchange_name,
    flow_type,
    token_address,
    amount;

CREATE OR REPLACE VIEW tron_db.exchange_flows_canonical AS
SELECT
    flow_id,
    argMax(tx_hash, inserted_at) AS tx_hash,
    argMax(block_number, inserted_at) AS block_number,
    argMax(from_address, inserted_at) AS from_address,
    argMax(to_address, inserted_at) AS to_address,
    argMax(exchange_name, inserted_at) AS exchange_name,
    argMax(flow_type, inserted_at) AS flow_type,
    argMax(token_address, inserted_at) AS token_address,
    argMax(amount, inserted_at) AS amount,
    argMax(confidence, inserted_at) AS confidence
FROM tron_db.exchange_flows_v2
GROUP BY flow_id;
