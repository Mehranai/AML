-- Canonical read views keep replayed inserts from appearing more than once while
-- existing installations are migrated away from append-only MergeTree tables.
-- Remove legacy balance projections before widening their source amount columns.
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_trx_from;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_trx_to;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_token_from;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_token_to;

ALTER TABLE tron_db.transactions MODIFY COLUMN amount UInt256;
ALTER TABLE tron_db.transactions MODIFY COLUMN fee UInt256;
ALTER TABLE tron_db.transactions MODIFY COLUMN energy_fee UInt256;
ALTER TABLE tron_db.transactions MODIFY COLUMN net_fee UInt256;
ALTER TABLE tron_db.token_transfers MODIFY COLUMN amount UInt256;
ALTER TABLE tron_db.address_relationships MODIFY COLUMN amount UInt256;
ALTER TABLE tron_db.address_relationships DROP COLUMN IF EXISTS risk_score;
ALTER TABLE tron_db.exchange_flows MODIFY COLUMN amount UInt256;
ALTER TABLE tron_db.address_entity
    ADD COLUMN IF NOT EXISTS is_active UInt8 DEFAULT 1 AFTER source;

CREATE VIEW IF NOT EXISTS tron_db.transactions_canonical AS
SELECT *
FROM tron_db.transactions
ORDER BY inserted_at DESC
LIMIT 1 BY tx_hash;

CREATE VIEW IF NOT EXISTS tron_db.raw_logs_canonical AS
SELECT *
FROM tron_db.raw_logs
ORDER BY inserted_at DESC
LIMIT 1 BY tx_hash, log_index;

CREATE VIEW IF NOT EXISTS tron_db.token_transfers_canonical AS
SELECT *
FROM tron_db.token_transfers
ORDER BY inserted_at DESC
LIMIT 1 BY tx_hash, log_index, token_address;

CREATE VIEW IF NOT EXISTS tron_db.address_relationships_canonical AS
SELECT *
FROM tron_db.address_relationships
WHERE event_type = 'transfer'
  AND from_address != ''
  AND to_address != ''
ORDER BY inserted_at DESC
LIMIT 1 BY relationship_id;

CREATE TABLE IF NOT EXISTS tron_db.ingested_blocks
(
    chain LowCardinality(String) DEFAULT 'tron',
    block_number UInt64,
    block_hash String,
    parent_hash String,
    block_timestamp UInt64,
    transaction_count UInt32,
    finality_status LowCardinality(String),
    ingestion_status LowCardinality(String),
    error_message String DEFAULT '',
    indexed_at_unix_ms UInt64,
    updated_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY (chain, block_number);

CREATE TABLE IF NOT EXISTS tron_db.semantic_aml_events
(
    event_id String,
    chain LowCardinality(String) DEFAULT 'tron',
    tx_hash String,
    block_number UInt64,
    timestamp UInt64,
    event_type LowCardinality(String),
    subject_address String,
    protocol String,
    asset_in String DEFAULT '',
    asset_out String DEFAULT '',
    detector String,
    detector_version String,
    confidence Float32,
    evidence_json String,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
ORDER BY event_id;

CREATE TABLE IF NOT EXISTS tron_db.entity_labels
(
    label_id String,
    chain LowCardinality(String),
    address String,
    entity_id String,
    entity_name String,
    entity_type LowCardinality(String),
    confidence Float32,
    source String,
    case_id String DEFAULT '',
    evidence_refs Array(String),
    review_status LowCardinality(String),
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, address, label_id);

CREATE TABLE IF NOT EXISTS tron_db.token_metadata_discoveries
(
    token_address String,
    discovered_block UInt64,
    discovered_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY token_address;

CREATE TABLE IF NOT EXISTS tron_db.token_metadata_jobs
(
    token_address String,
    discovered_block UInt64,
    status LowCardinality(String),
    attempt_count UInt8,
    last_error String DEFAULT '',
    updated_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY token_address;

-- The v2 balance ledger is event-keyed. Replayed source rows therefore replace
-- one another instead of multiplying a wallet balance.
CREATE TABLE IF NOT EXISTS tron_db.wallet_asset_balance_deltas_v2
(
    delta_id String,
    tx_hash String,
    block_number UInt64,
    timestamp UInt64,
    address String,
    asset_type LowCardinality(String),
    asset_id String,
    amount_raw UInt256,
    direction Int8,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
ORDER BY delta_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_trx_from_v2
TO tron_db.wallet_asset_balance_deltas_v2
AS
SELECT
    concat(tx_hash, ':native:from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    'native' AS asset_type,
    'TRX' AS asset_id,
    amount AS amount_raw,
    -1 AS direction,
    now64(3) AS inserted_at
FROM tron_db.transactions
WHERE status = 1
  AND from_address != ''
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_trx_to_v2
TO tron_db.wallet_asset_balance_deltas_v2
AS
SELECT
    concat(tx_hash, ':native:to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    'native' AS asset_type,
    'TRX' AS asset_id,
    amount AS amount_raw,
    1 AS direction,
    now64(3) AS inserted_at
FROM tron_db.transactions
WHERE status = 1
  AND to_address != ''
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_token_from_v2
TO tron_db.wallet_asset_balance_deltas_v2
AS
SELECT
    concat(tx_hash, ':', toString(log_index), ':', token_address, ':from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    'trc20' AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    -1 AS direction,
    now64(3) AS inserted_at
FROM tron_db.token_transfers
WHERE from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_token_to_v2
TO tron_db.wallet_asset_balance_deltas_v2
AS
SELECT
    concat(tx_hash, ':', toString(log_index), ':', token_address, ':to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    'trc20' AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    1 AS direction,
    now64(3) AS inserted_at
FROM tron_db.token_transfers
WHERE to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

INSERT INTO tron_db.wallet_asset_balance_deltas_v2
SELECT
    concat(tx_hash, ':native:from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address,
    'native',
    'TRX',
    amount,
    -1,
    now64(3)
FROM tron_db.transactions_canonical
WHERE status = 1
  AND from_address != ''
  AND amount > 0;

INSERT INTO tron_db.wallet_asset_balance_deltas_v2
SELECT
    concat(tx_hash, ':native:to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address,
    'native',
    'TRX',
    amount,
    1,
    now64(3)
FROM tron_db.transactions_canonical
WHERE status = 1
  AND to_address != ''
  AND amount > 0;

INSERT INTO tron_db.wallet_asset_balance_deltas_v2
SELECT
    concat(tx_hash, ':', toString(log_index), ':', token_address, ':from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address,
    'trc20',
    token_address,
    amount,
    -1,
    now64(3)
FROM tron_db.token_transfers_canonical
WHERE from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

INSERT INTO tron_db.wallet_asset_balance_deltas_v2
SELECT
    concat(tx_hash, ':', toString(log_index), ':', token_address, ':to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address,
    'trc20',
    token_address,
    amount,
    1,
    now64(3)
FROM tron_db.token_transfers_canonical
WHERE to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

DROP VIEW IF EXISTS tron_db.wallet_asset_balances;

CREATE VIEW tron_db.wallet_asset_balances AS
WITH latest_metadata AS
(
    SELECT
        token_address,
        argMax(name, updated_at) AS name,
        argMax(symbol, updated_at) AS symbol,
        argMax(decimals, updated_at) AS decimals
    FROM tron_db.token_metadata
    GROUP BY token_address
)
SELECT
    balances.address,
    balances.asset_type,
    balances.asset_id,
    if(
        balances.asset_type = 'native',
        'TRX',
        if(latest_metadata.symbol = '', balances.asset_id, latest_metadata.symbol)
    ) AS asset_symbol,
    if(
        balances.asset_type = 'native',
        'TRON',
        latest_metadata.name
    ) AS asset_name,
    if(
        balances.asset_type = 'native',
        toUInt8(6),
        latest_metadata.decimals
    ) AS decimals,
    balances.balance_raw,
    balances.balance_incomplete,
    toFloat64(balances.balance_raw)
        / pow(10, if(balances.asset_type = 'native', toUInt8(6), latest_metadata.decimals))
        AS balance_decimal
FROM
(
    SELECT
        address,
        asset_type,
        asset_id,
        toUInt256(if(
            sumIf(amount_raw, direction = 1) >= sumIf(amount_raw, direction = -1),
            sumIf(amount_raw, direction = 1) - sumIf(amount_raw, direction = -1),
            toInt256(0)
        )) AS balance_raw,
        toUInt8(sumIf(amount_raw, direction = 1) < sumIf(amount_raw, direction = -1))
            AS balance_incomplete
    FROM tron_db.wallet_asset_balance_deltas_v2 FINAL
    GROUP BY address, asset_type, asset_id
    HAVING balance_raw > 0 OR balance_incomplete = 1
) AS balances
LEFT JOIN latest_metadata
    ON balances.asset_type = 'trc20'
   AND balances.asset_id = latest_metadata.token_address;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_to_address to_address TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_tx_hash tx_hash TYPE bloom_filter(0.001) GRANULARITY 4;

ALTER TABLE tron_db.wallet_analysis_snapshots
    ADD COLUMN IF NOT EXISTS risk_available UInt8 DEFAULT 0 AFTER analysis_status;

ALTER TABLE tron_db.wallet_analysis_snapshots
    ADD COLUMN IF NOT EXISTS analysis_input_version String DEFAULT '' AFTER data_cutoff_unix_ms;

ALTER TABLE tron_db.analysis_subjects
    ADD COLUMN IF NOT EXISTS latest_risk_available UInt8 DEFAULT 0 AFTER latest_status;

ALTER TABLE tron_db.analysis_subjects
    ADD COLUMN IF NOT EXISTS latest_input_version String DEFAULT '' AFTER latest_data_cutoff_unix_ms;

ALTER TABLE tron_db.address_exposure
    ADD COLUMN IF NOT EXISTS best_path_amount_share Float64 DEFAULT 0;

ALTER TABLE tron_db.address_exposure
    ADD COLUMN IF NOT EXISTS best_path_time_weight Float64 DEFAULT 0;

ALTER TABLE tron_db.address_exposure
    ADD COLUMN IF NOT EXISTS service_mediated UInt8 DEFAULT 0;

ALTER TABLE tron_db.address_exposure
    ADD COLUMN IF NOT EXISTS propagation_run_id String DEFAULT '';

ALTER TABLE tron_db.address_exposure
    ADD COLUMN IF NOT EXISTS computed_at_unix_ms UInt64 DEFAULT 0;

CREATE TABLE IF NOT EXISTS tron_db.exposure_runs
(
    source_address String,
    propagation_run_id String,
    status LowCardinality(String),
    max_hops UInt8,
    row_count UInt64,
    completed_at_unix_ms UInt64,
    updated_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY source_address;

ALTER TABLE tron_db.wallet_ml_training_runs
    ADD COLUMN IF NOT EXISTS test_sample_count UInt64 DEFAULT 0 AFTER validation_sample_count;

ALTER TABLE tron_db.wallet_ml_training_runs
    ADD COLUMN IF NOT EXISTS dataset_sha256 String DEFAULT '' AFTER training_dataset_id;

ALTER TABLE tron_db.wallet_ml_model_registry
    ADD COLUMN IF NOT EXISTS artifact_sha256 String DEFAULT '' AFTER artifact_json;

CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_model_deployments
(
    environment LowCardinality(String),
    feature_schema_version String,
    deployment_id String,
    model_id String,
    model_version String,
    status LowCardinality(String),
    deployed_by String,
    notes String DEFAULT '',
    deployed_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (environment, feature_schema_version);
