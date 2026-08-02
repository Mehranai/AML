-- `address_relationships` is the canonical, compact value-transfer fact.
-- Transaction headers retain execution/fee data; semantic AML events remain
-- separate. Raw receipt logs and a second TRC20 transfer copy are no longer
-- written by ingestion.

ALTER TABLE tron_db.address_relationships
    ADD INDEX IF NOT EXISTS idx_from_address from_address TYPE bloom_filter(0.001) GRANULARITY 4;

-- Earlier ingestion excluded mint/burn movements from address_relationships.
-- Preserve those holdings facts before token_transfers becomes read-only legacy.
INSERT INTO tron_db.address_relationships
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
    event_type,
    protocol,
    hop_count,
    inserted_at
)
SELECT
    concat(tx_hash, ':legacy-log:', toString(log_index)) AS relationship_id,
    from_address,
    to_address,
    token_address,
    tx_hash,
    block_number,
    timestamp,
    amount,
    'trc20_transfer' AS transfer_type,
    'transfer' AS event_type,
    '' AS protocol,
    toUInt16(1) AS hop_count,
    now() AS inserted_at
FROM tron_db.token_transfers_canonical
WHERE amount > 0
  AND (
      from_address = 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
      OR to_address = 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  );

DROP VIEW IF EXISTS tron_db.address_relationships_canonical;

CREATE VIEW tron_db.address_relationships_canonical AS
SELECT *
FROM tron_db.address_relationships
WHERE event_type = 'transfer'
  AND amount > 0
  AND from_address != ''
  AND to_address != ''
  AND from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
  AND to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
ORDER BY inserted_at DESC
LIMIT 1 BY relationship_id;

DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_trx_from_v2;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_trx_to_v2;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_token_from_v2;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_token_to_v2;

CREATE TABLE IF NOT EXISTS tron_db.wallet_asset_balance_deltas_v3
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

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_transfer_from_v3
TO tron_db.wallet_asset_balance_deltas_v3
AS
SELECT
    concat(relationship_id, ':from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    multiIf(
        token_address = 'TRX', 'native',
        startsWith(token_address, 'TRC10:'), 'trc10',
        'trc20'
    ) AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    toInt8(-1) AS direction,
    now64(3) AS inserted_at
FROM tron_db.address_relationships
WHERE event_type = 'transfer'
  AND amount > 0
  AND from_address != ''
  AND from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';

CREATE MATERIALIZED VIEW IF NOT EXISTS tron_db.mv_wallet_asset_delta_transfer_to_v3
TO tron_db.wallet_asset_balance_deltas_v3
AS
SELECT
    concat(relationship_id, ':to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    multiIf(
        token_address = 'TRX', 'native',
        startsWith(token_address, 'TRC10:'), 'trc10',
        'trc20'
    ) AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    toInt8(1) AS direction,
    now64(3) AS inserted_at
FROM tron_db.address_relationships
WHERE event_type = 'transfer'
  AND amount > 0
  AND to_address != ''
  AND to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';

INSERT INTO tron_db.wallet_asset_balance_deltas_v3
SELECT
    concat(relationship_id, ':from') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    from_address AS address,
    multiIf(
        token_address = 'TRX', 'native',
        startsWith(token_address, 'TRC10:'), 'trc10',
        'trc20'
    ) AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    toInt8(-1) AS direction,
    now64(3) AS inserted_at
FROM
(
    SELECT *
    FROM tron_db.address_relationships
    WHERE event_type = 'transfer'
      AND amount > 0
    ORDER BY inserted_at DESC
    LIMIT 1 BY relationship_id
)
WHERE from_address != ''
  AND from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';

INSERT INTO tron_db.wallet_asset_balance_deltas_v3
SELECT
    concat(relationship_id, ':to') AS delta_id,
    tx_hash,
    block_number,
    timestamp,
    to_address AS address,
    multiIf(
        token_address = 'TRX', 'native',
        startsWith(token_address, 'TRC10:'), 'trc10',
        'trc20'
    ) AS asset_type,
    token_address AS asset_id,
    amount AS amount_raw,
    toInt8(1) AS direction,
    now64(3) AS inserted_at
FROM
(
    SELECT *
    FROM tron_db.address_relationships
    WHERE event_type = 'transfer'
      AND amount > 0
    ORDER BY inserted_at DESC
    LIMIT 1 BY relationship_id
)
WHERE to_address != ''
  AND to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';

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
    multiIf(
        balances.asset_type = 'native', 'TRX',
        balances.asset_type = 'trc10', balances.asset_id,
        latest_metadata.symbol = '', balances.asset_id,
        latest_metadata.symbol
    ) AS asset_symbol,
    multiIf(
        balances.asset_type = 'native', 'TRON',
        balances.asset_type = 'trc10', '',
        latest_metadata.name
    ) AS asset_name,
    multiIf(
        balances.asset_type = 'native', toUInt8(6),
        balances.asset_type = 'trc10', toUInt8(0),
        latest_metadata.decimals
    ) AS decimals,
    balances.balance_raw,
    balances.balance_incomplete,
    if(
        balances.asset_type = 'trc10',
        toFloat64(balances.balance_raw),
        toFloat64(balances.balance_raw)
            / pow(
                10,
                if(
                    balances.asset_type = 'native',
                    toUInt8(6),
                    latest_metadata.decimals
                )
            )
    ) AS balance_decimal
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
    FROM tron_db.wallet_asset_balance_deltas_v3 FINAL
    GROUP BY address, asset_type, asset_id
    HAVING balance_raw > 0 OR balance_incomplete = 1
) AS balances
LEFT JOIN latest_metadata
    ON balances.asset_type = 'trc20'
   AND balances.asset_id = latest_metadata.token_address;

-- v2 is derived data and is fully replaced by the relationship-backed v3 ledger.
DROP TABLE IF EXISTS tron_db.wallet_asset_balance_deltas_v2;

-- These compatibility views refer to legacy facts that ingestion no longer writes.
DROP VIEW IF EXISTS tron_db.raw_logs_canonical;
DROP VIEW IF EXISTS tron_db.token_transfers_canonical;
