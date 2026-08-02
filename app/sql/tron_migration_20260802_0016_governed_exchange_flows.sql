-- Legacy topology heuristics are candidate evidence, not authoritative entity
-- labels. Keep their history but remove them from active graph attribution.

INSERT INTO tron_db.exchange_addresses
(
    address,
    entity_id,
    exchange_name,
    address_role,
    confidence,
    detection_source,
    first_seen_block,
    last_seen_block,
    is_active,
    created_at
)
SELECT
    address,
    entity_id,
    exchange_name,
    address_role,
    confidence,
    detection_source,
    first_seen_block,
    last_seen_block,
    toUInt8(0),
    now()
FROM tron_db.exchange_addresses FINAL
WHERE is_active = 1
  AND detection_source IN (
      'many_deposit_wallets_to_one_sweeper',
      'one_wallet_to_many_withdrawals'
  );

CREATE OR REPLACE VIEW tron_db.exchange_flows_canonical AS
WITH current_flows AS
(
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
    GROUP BY flow_id
)
SELECT *
FROM current_flows
WHERE from_address IN
(
    SELECT address
    FROM tron_db.exchange_addresses FINAL
    WHERE is_active = 1
)
OR to_address IN
(
    SELECT address
    FROM tron_db.exchange_addresses FINAL
    WHERE is_active = 1
);

-- Rebuild operation labels so stale heuristic exchange classifications fall
-- back to their verifiable transfer primitive. Governed exchange operations
-- are attached at graph read time from exchange_flows_canonical.
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
        ifNull(feature.transaction_type, '') NOT IN ('', 'unknown', 'exchange_flow'),
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
