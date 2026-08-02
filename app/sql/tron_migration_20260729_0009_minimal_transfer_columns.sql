-- Keep only movement-specific evidence in the high-volume transfer fact.
-- Event type and hop count were constants; protocol belongs to the
-- transaction-level semantic feature rather than every transfer leg.

DROP VIEW IF EXISTS tron_db.address_relationships_canonical;

DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_transfer_from_v3;
DROP TABLE IF EXISTS tron_db.mv_wallet_asset_delta_transfer_to_v3;

ALTER TABLE tron_db.address_relationships DROP COLUMN IF EXISTS event_type;
ALTER TABLE tron_db.address_relationships DROP COLUMN IF EXISTS hop_count;
ALTER TABLE tron_db.address_relationships DROP COLUMN IF EXISTS protocol;

CREATE VIEW tron_db.address_relationships_canonical AS
SELECT
    transfer.relationship_id,
    transfer.from_address,
    transfer.to_address,
    transfer.token_address,
    transfer.tx_hash,
    transfer.block_number,
    transfer.timestamp,
    transfer.amount,
    transfer.transfer_type,
    ifNull(feature.protocol, '') AS protocol,
    transfer.inserted_at
FROM
(
    SELECT *
    FROM tron_db.address_relationships
    WHERE amount > 0
      AND from_address != ''
      AND to_address != ''
      AND from_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
      AND to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb'
    ORDER BY inserted_at DESC
    LIMIT 1 BY relationship_id
) AS transfer
LEFT JOIN
(
    SELECT
        tx_hash,
        argMax(protocol, inserted_at) AS protocol
    FROM tron_db.transaction_features
    GROUP BY tx_hash
) AS feature ON feature.tx_hash = transfer.tx_hash;

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
WHERE amount > 0
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
WHERE amount > 0
  AND to_address != ''
  AND to_address != 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';
