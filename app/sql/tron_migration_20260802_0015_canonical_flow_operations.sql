-- Enrich the canonical relationship read model with transaction-level
-- semantics without duplicating them in the high-volume relationship table.

DROP VIEW IF EXISTS tron_db.address_relationships_canonical;

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
    multiIf(
        ifNull(feature.is_swap, toUInt8(0)) > 0, 'swap',
        ifNull(feature.is_bridge, toUInt8(0)) > 0, 'bridge',
        ifNull(feature.is_liquidity_add, toUInt8(0)) > 0, 'liquidity_add',
        ifNull(feature.is_liquidity_remove, toUInt8(0)) > 0, 'liquidity_remove',
        ifNull(feature.is_mint, toUInt8(0)) > 0, 'mint',
        ifNull(feature.is_burn, toUInt8(0)) > 0, 'burn',
        ifNull(feature.transaction_type, '') NOT IN ('', 'unknown'),
            feature.transaction_type,
        transfer.transfer_type
    ) AS operation_type,
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
        argMax(transaction_type, inserted_at) AS transaction_type,
        argMax(protocol, inserted_at) AS protocol,
        argMax(is_swap, inserted_at) AS is_swap,
        argMax(is_bridge, inserted_at) AS is_bridge,
        argMax(is_mint, inserted_at) AS is_mint,
        argMax(is_burn, inserted_at) AS is_burn,
        argMax(is_liquidity_add, inserted_at) AS is_liquidity_add,
        argMax(is_liquidity_remove, inserted_at) AS is_liquidity_remove
    FROM tron_db.transaction_features
    GROUP BY tx_hash
) AS feature ON feature.tx_hash = transfer.tx_hash;
