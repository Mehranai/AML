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
-- SYNC STATE
---------------------------------------------------------
CREATE TABLE IF NOT EXISTS eth_db.sync_state (
    chain String,
    last_synced_block UInt64,
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
ORDER BY chain;
