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

    amount UInt128,

    fee UInt128 DEFAULT 0,
    energy_fee UInt128 DEFAULT 0,
    net_fee UInt128 DEFAULT 0,

    energy_usage UInt64 DEFAULT 0,
    energy_usage_total UInt64 DEFAULT 0,

    net_usage UInt64 DEFAULT 0,

    status UInt8 DEFAULT 1,

    memo String DEFAULT '',

    inserted_at DateTime DEFAULT now()
)
    ENGINE = MergeTree()
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (block_number, tx_hash)
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
    ENGINE = MergeTree()
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 block_number,
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

    amount UInt128,

    is_mint UInt8 DEFAULT 0,
    is_burn UInt8 DEFAULT 0,

    event_signature String,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = MergeTree()
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 token_address,
                 from_address,
                 to_address,
                 block_number,
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

    amount UInt128,

    transfer_type String,

    protocol String,

    event_type String DEFAULT '',

    risk_score UInt8 DEFAULT 0,

    hop_count UInt16 DEFAULT 0,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = MergeTree()
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 from_address,
                 timestamp,
                 tx_hash
             );

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

    created_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY address;

-- =========================================================
-- EXCHANGE ENTITIES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exchange_entities
(
    entity_id String,

    exchange_name String,

    exchange_type String,

    confidence Float32,

    created_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY entity_id;

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

    created_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY address;

-- =========================================================
-- EXCHANGE DEPOSIT ADDRESSES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exchange_deposit_addresses
(
    address String,

    exchange_name String,

    hot_wallet String,

    confidence Float32,

    detection_method String,

    first_seen_block UInt64,
    last_seen_block UInt64,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    ORDER BY address;

-- =========================================================
-- EXCHANGE CLUSTERS
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.exchange_clusters
(
    cluster_id String,

    exchange_name String,

    address String,

    role String,

    confidence Float32,

    discovered_from String,

    created_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(created_at)
    ORDER BY (
                 cluster_id,
                 address
             );

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

    amount UInt128,

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
-- TRANSACTION RISK
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.transaction_risk
(
    tx_hash String,
    block_number UInt64,

    timestamp UInt64,

    risk_score UInt8,
    risk_level String,

    transaction_type String DEFAULT 'unknown',

    transaction_subtype String DEFAULT '',

    is_swap UInt8,
    is_bridge UInt8,
    is_contract_call UInt8,

    unique_tokens UInt16,

    participants UInt16,
    risk_reasons Array(String) DEFAULT [],

    touches_exchange UInt8 DEFAULT 0,

    inserted_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 risk_score,
                 block_number
             );

-- =========================================================
-- WALLET ASSET BALANCES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.wallet_asset_balance_deltas
(
    tx_hash String,

    block_number UInt64,

    timestamp UInt64,

    address String,

    asset_type String,

    asset_id String,

    delta_raw Int256,

    direction Int8,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = MergeTree()
    PARTITION BY toYYYYMM(toDateTime(intDiv(timestamp, 1000)))
    ORDER BY (
                 address,
                 asset_type,
                 asset_id,
                 block_number,
                 tx_hash
             );

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_trx_from
TO tron_db.wallet_asset_balance_deltas
AS
SELECT
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    'native' AS asset_type,
    'TRX' AS asset_id,
    -toInt256(amount) AS delta_raw,
    -1 AS direction,
    now() AS inserted_at
FROM tron_db.transactions
WHERE from_address != ''
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_trx_to
TO tron_db.wallet_asset_balance_deltas
AS
SELECT
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    'native' AS asset_type,
    'TRX' AS asset_id,
    toInt256(amount) AS delta_raw,
    1 AS direction,
    now() AS inserted_at
FROM tron_db.transactions
WHERE to_address != ''
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_token_from
TO tron_db.wallet_asset_balance_deltas
AS
SELECT
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    'trc20' AS asset_type,
    token_address AS asset_id,
    -toInt256(amount) AS delta_raw,
    -1 AS direction,
    now() AS inserted_at
FROM tron_db.token_transfers
WHERE from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_token_to
TO tron_db.wallet_asset_balance_deltas
AS
SELECT
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    'trc20' AS asset_type,
    token_address AS asset_id,
    toInt256(amount) AS delta_raw,
    1 AS direction,
    now() AS inserted_at
FROM tron_db.token_transfers
WHERE to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND amount > 0;

CREATE VIEW IF NOT EXISTS tron_db.wallet_asset_balances AS
SELECT
    balances.address,
    balances.asset_type,
    balances.asset_id,
    if(
        balances.asset_type = 'native',
        'TRX',
        if(token_metadata.symbol = '', balances.asset_id, token_metadata.symbol)
    ) AS asset_symbol,
    if(
        balances.asset_type = 'native',
        'TRON',
        token_metadata.name
    ) AS asset_name,
    if(
        balances.asset_type = 'native',
        toUInt8(6),
        token_metadata.decimals
    ) AS decimals,
    balances.balance_raw,
    if(
        if(balances.asset_type = 'native', toUInt8(6), token_metadata.decimals) = 0,
        0,
        toFloat64(balances.balance_raw) / pow(10, if(balances.asset_type = 'native', toUInt8(6), token_metadata.decimals))
    ) AS balance_decimal
FROM
(
    SELECT
        address,
        asset_type,
        asset_id,
        if(sum(delta_raw) < 0, toInt256(0), sum(delta_raw)) AS balance_raw
    FROM tron_db.wallet_asset_balance_deltas
    GROUP BY
        address,
        asset_type,
        asset_id
    HAVING balance_raw > 0
) AS balances
LEFT JOIN tron_db.token_metadata AS token_metadata
    ON balances.asset_type = 'trc20'
   AND balances.asset_id = token_metadata.token_address;

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

    created_at DateTime DEFAULT now()
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

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY (
                 source_address,
                 exposed_address
             );

-- =========================================================
-- ADDRESS PROFILES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.address_profiles
(
    address String,

    total_in_tx UInt64,
    total_out_tx UInt64,

    unique_senders UInt64,
    unique_receivers UInt64,

    total_volume_in UInt128,
    total_volume_out UInt128,

    interacted_tokens UInt32,

    probable_exchange UInt8,
    probable_deposit_wallet UInt8,
    probable_sweeper UInt8,

    risk_score Float32,

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY address;

-- =========================================================
-- ADDRESS COUNTERPARTIES
-- =========================================================

CREATE TABLE IF NOT EXISTS tron_db.address_counterparties
(
    address String,

    counterparty String,

    direction String,

    token_address String,

    total_txs UInt64,

    total_volume UInt128,

    first_seen UInt64,

    last_seen UInt64,

    updated_at DateTime DEFAULT now()
    )
    ENGINE = ReplacingMergeTree(updated_at)
    ORDER BY (
                 address,
                 counterparty,
                 direction,
                 token_address
             );

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

ALTER TABLE tron_db.transaction_risk
    ADD INDEX IF NOT EXISTS idx_risk (risk_score)
    TYPE minmax
    GRANULARITY 4;

ALTER TABLE tron_db.transaction_risk
    ADD COLUMN IF NOT EXISTS transaction_type String DEFAULT 'unknown';

ALTER TABLE tron_db.transaction_risk
    ADD COLUMN IF NOT EXISTS transaction_subtype String DEFAULT '';

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

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_from from_address TYPE bloom_filter GRANULARITY 4;

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_to to_address TYPE bloom_filter GRANULARITY 4;
-- added More
