CREATE DATABASE IF NOT EXISTS eth_db;

---------------------------------------------------------
-- WALLET INFO
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.wallet_info (
    address String,
    balance String,
    nonce UInt64,
    wallet_type String,
    person_id String,
    inserted_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY address;

---------------------------------------------------------
-- TRANSACTIONS
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.transactions (
    hash String,
    block_number UInt64,
    from_addr String,
    to_addr String,
    value String,
    sensivity UInt8,
    inserted_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (block_number, hash);

---------------------------------------------------------
-- OWNER INFO
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.owner_info (
    address String,
    person_name String,
    person_id String,
    personal_id UInt16,
    inserted_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY address;

---------------------------------------------------------
-- TOKEN TRANSFERS
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.token_transfers (
    tx_hash String,
    block_number UInt64,
    log_index UInt32,
    token_address String,
    from_addr String,
    to_addr String,
    amount String,
    inserted_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (tx_hash, log_index);

---------------------------------------------------------
-- TOKEN METADATA
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.token_metadata (
    token_address String,
    name String,
    symbol String,
    decimals UInt8,
    total_supply String,
    is_verified UInt8,
    created_at DateTime DEFAULT now(),
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
ORDER BY token_address;

---------------------------------------------------------
-- ADDRESS RELATIONSHIPS
-- Canonical wallet graph edges for native ETH and ERC20 transfers.
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.address_relationships (
    relationship_id String,
    from_address String,
    to_address String,
    token_address String,
    tx_hash String,
    block_number UInt64,
    timestamp UInt64 DEFAULT 0,
    amount String,
    transfer_type String,
    protocol String,
    event_type String DEFAULT '',
    risk_score UInt8 DEFAULT 0,
    hop_count UInt16 DEFAULT 1,
    inserted_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY relationship_id;

---------------------------------------------------------
-- WALLET ASSET BALANCE DELTAS
-- Reconstructable ETH/ERC20 holdings model.
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.wallet_asset_balance_deltas (
    tx_hash String,
    block_number UInt64,
    timestamp UInt64 DEFAULT 0,
    address String,
    asset_type String,
    asset_id String,
    delta_raw Int256,
    direction Int8,
    inserted_at DateTime DEFAULT now()
) ENGINE = MergeTree
ORDER BY (address, asset_type, asset_id, block_number, tx_hash);

CREATE MATERIALIZED VIEW IF NOT EXISTS eth_db.mv_wallet_asset_delta_eth_from
TO eth_db.wallet_asset_balance_deltas
AS
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
  AND toInt256(value) > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS eth_db.mv_wallet_asset_delta_eth_to
TO eth_db.wallet_asset_balance_deltas
AS
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
  AND toInt256(value) > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS eth_db.mv_wallet_asset_delta_token_from
TO eth_db.wallet_asset_balance_deltas
AS
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
  AND toInt256(amount) > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS eth_db.mv_wallet_asset_delta_token_to
TO eth_db.wallet_asset_balance_deltas
AS
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
  AND toInt256(amount) > 0;

CREATE VIEW IF NOT EXISTS eth_db.wallet_asset_balances AS
SELECT
    balances.address,
    balances.asset_type,
    balances.asset_id,
    if(
        balances.asset_type = 'native',
        'ETH',
        ifNull(nullIf(metadata.symbol, ''), balances.asset_id)
    ) AS asset_symbol,
    if(
        balances.asset_type = 'native',
        'Ethereum',
        ifNull(nullIf(metadata.name, ''), balances.asset_id)
    ) AS asset_name,
    if(
        balances.asset_type = 'native',
        toUInt8(18),
        ifNull(metadata.decimals, toUInt8(0))
    ) AS decimals,
    balances.balance_raw,
    toFloat64(balances.balance_raw) / pow(
        10,
        if(
            balances.asset_type = 'native',
            toUInt8(18),
            ifNull(metadata.decimals, toUInt8(0))
        )
    ) AS balance_decimal,
    balances.last_seen_block
FROM
(
    SELECT
        address,
        asset_type,
        asset_id,
        sum(delta_raw) AS balance_raw,
        max(block_number) AS last_seen_block
    FROM eth_db.wallet_asset_balance_deltas
    GROUP BY
        address,
        asset_type,
        asset_id
    HAVING balance_raw > 0
) AS balances
LEFT JOIN eth_db.token_metadata AS metadata
    ON balances.asset_type = 'erc20'
   AND balances.asset_id = metadata.token_address;

ALTER TABLE eth_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_relationship_from from_address TYPE bloom_filter GRANULARITY 4;

ALTER TABLE eth_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_relationship_to to_address TYPE bloom_filter GRANULARITY 4;

ALTER TABLE eth_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_relationship_tx tx_hash TYPE bloom_filter GRANULARITY 4;

ALTER TABLE eth_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_relationship_transfer_type transfer_type TYPE set(100) GRANULARITY 4;

---------------------------------------------------------
-- SYNC STATE
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.sync_state (
    chain String,
    last_synced_block UInt64,
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
ORDER BY chain;
