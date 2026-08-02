CREATE DATABASE IF NOT EXISTS tron_db;

-- =========================================================
-- TRANSACTIONS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.transactions
(
    tx_hash String,

    block_number UInt64,

    timestamp UInt64,

    from_address String,
    to_address String,

    contract_address String DEFAULT '',
    contract_type String,

    amount UInt256,

    fee UInt256 DEFAULT 0,
    energy_fee UInt256 DEFAULT 0,
    net_fee UInt256 DEFAULT 0,

    energy_usage UInt64 DEFAULT 0,
    energy_usage_total UInt64 DEFAULT 0,

    net_usage UInt64 DEFAULT 0,

    status UInt8 DEFAULT 1,

    memo String DEFAULT '',

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY tx_hash
    SETTINGS index_granularity = 8192;

-- =========================================================
-- RAW LOGS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.raw_logs
(
    tx_hash String,
    block_number UInt64,

    log_index UInt32,

    contract_address String,

    topics Array(String),

    data String,

    removed UInt8,

    timestamp UInt64,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 tx_hash,
                 log_index
             );

-- =========================================================
-- TOKEN METADATA
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.token_metadata
(
    token_address String,

    name String,
    symbol String,

    decimals UInt8,

    total_supply String,

    is_verified UInt8,

    created_at DateTime DEFAULT now(),

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY token_address;

-- =========================================================
-- TOKEN TRANSFERS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.token_transfers
(
    tx_hash String,

    block_number UInt64,

    timestamp UInt64,

    log_index UInt32,

    token_address String,

    from_address String,
    to_address String,

    amount UInt256,

    is_mint UInt8 DEFAULT 0,
    is_burn UInt8 DEFAULT 0,

    event_signature String,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 tx_hash,
                 log_index
             );

-- =========================================================
-- ADDRESS RELATIONSHIPS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.address_relationships
(
    relationship_id String,

    from_address String,

    to_address String,

    token_address String,

    tx_hash String,

    block_number UInt64,

    timestamp UInt64,

    amount UInt256,

    transfer_type String,

    protocol String,

    event_type String DEFAULT '',

    hop_count UInt16 DEFAULT 0,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY relationship_id;

-- =========================================================
-- ADDRESS ENTITY
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.address_entity
(
    address String,

    entity_id String,

    entity_name String,

    entity_type String,

    confidence Float32,

    source String,

    is_active UInt8 DEFAULT 1,

    created_at DateTime64(3) DEFAULT now64(3)
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY address;

-- =========================================================
-- EXCHANGE ADDRESSES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exchange_addresses
(
    address String,

    entity_id String,

    exchange_name String,

    address_role String,

    confidence Float32,

    detection_source String,

    first_seen_block UInt64,
    last_seen_block UInt64,

    is_active UInt8 DEFAULT 1,

    created_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY address;

-- =========================================================
-- EXCHANGE FLOWS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exchange_flows
(
    tx_hash String,

    block_number UInt64,

    from_address String,

    to_address String,

    exchange_name String,

    flow_type String,

    token_address String,

    amount UInt256,

    confidence Float32,

    created_at DateTime DEFAULT now()
    )
    ENGINE = MergeTree()
    ORDER BY (
                 block_number,
                 tx_hash
             );

-- =========================================================
-- CONTRACT METADATA
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.contract_metadata
(
    contract_address String,

    protocol_name String DEFAULT '',

    contract_type String,

    creator_address String,

    created_block UInt64,

    created_at DateTime DEFAULT now(),

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY contract_address;

-- =========================================================
-- TRANSACTION FEATURES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.transaction_features
(
    tx_hash String,

    block_number UInt64,

    timestamp UInt64,

    transaction_type String DEFAULT 'unknown',

    transaction_subtype String DEFAULT '',

    classification_confidence Float32 DEFAULT 0,

    classification_source String DEFAULT '',

    protocol String DEFAULT '',

    method_id String DEFAULT '',

    is_swap UInt8,

    is_bridge UInt8,

    is_mint UInt8 DEFAULT 0,

    is_burn UInt8 DEFAULT 0,

    is_liquidity_add UInt8 DEFAULT 0,

    is_liquidity_remove UInt8 DEFAULT 0,

    is_contract_call UInt8,

    unique_tokens UInt16,

    participants UInt16,

    hop_count UInt16 DEFAULT 0,

    fan_in UInt16 DEFAULT 0,

    fan_out UInt16 DEFAULT 0,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 block_number,
                 tx_hash
             );

-- =========================================================
-- EXPOSURE SEEDS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exposure_seeds
(
    address String,

    entity_name String,

    entity_type String,

    risk_level UInt8,

    source String,

    created_at DateTime64(3) DEFAULT now64(3)
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY address;

-- =========================================================
-- ADDRESS EXPOSURE
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.address_exposure
(
    source_address String,
    exposed_address String,
    hop_distance UInt8,
    exposure_score Float64,
    path_count UInt32,
    last_tx_hash String,
    last_seen_block UInt64,
    exposure_type String,
    direction String,
    best_path_amount_share Float64 DEFAULT 0,
    best_path_time_weight Float64 DEFAULT 0,
    service_mediated UInt8 DEFAULT 0,
    propagation_run_id String DEFAULT '',
    computed_at_unix_ms UInt64 DEFAULT 0,

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY (
                 source_address,
                 exposed_address
             );

CREATE TABLE IF NOT EXISTS tron_db.exposure_runs
(
    source_address String,
    propagation_run_id String,
    status String,
    max_hops UInt8,
    row_count UInt64,
    completed_at_unix_ms UInt64,
    updated_at DateTime64(3) DEFAULT now64(3)
)
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY source_address;

-- =========================================================
-- SYNC STATE
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.sync_state
(
    chain String,

    last_synced_block UInt64,

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY chain;

-- =========================================================
-- PERFORMANCE INDEXES
-- =========================================================

ALTER TABLE tron_db.transaction_features
    ADD INDEX IF NOT EXISTS idx_swap (is_swap)
    TYPE minmax
    GRANULARITY 4;

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS transaction_type String DEFAULT 'unknown';

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS transaction_subtype String DEFAULT '';

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS classification_confidence Float32 DEFAULT 0;

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS classification_source String DEFAULT '';

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS protocol String DEFAULT '';

ALTER TABLE tron_db.transaction_features
    ADD COLUMN IF NOT EXISTS method_id String DEFAULT '';

ALTER TABLE tron_db.transaction_features
    ADD INDEX IF NOT EXISTS idx_transaction_type (transaction_type)
    TYPE set(100)
    GRANULARITY 4;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_transfer_type (transfer_type)
    TYPE set(100)
    GRANULARITY 4;

ALTER TABLE tron_db.address_entity
    ADD INDEX IF NOT EXISTS idx_entity_type (entity_type)
    TYPE set(100)
    GRANULARITY 4;

-- added More
ALTER TABLE tron_db.transactions
    DROP COLUMN IF EXISTS raw_data;

ALTER TABLE tron_db.transactions
    ADD INDEX IF NOT EXISTS idx_from_address from_address TYPE bloom_filter GRANULARITY 4;

ALTER TABLE tron_db.transactions
    ADD INDEX IF NOT EXISTS idx_to_address to_address TYPE bloom_filter GRANULARITY 4;

ALTER TABLE tron_db.token_transfers
    ADD INDEX IF NOT EXISTS idx_token token_address TYPE bloom_filter GRANULARITY 4;

-- added More
