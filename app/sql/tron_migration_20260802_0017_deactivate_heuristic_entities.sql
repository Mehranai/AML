-- Mirror the exchange-attribution cleanup in the generic entity projection.
-- Topology heuristics remain available as historical evidence but cannot be
-- rendered as reviewed entity identity.

INSERT INTO tron_db.address_entity
(
    address,
    entity_id,
    entity_name,
    entity_type,
    confidence,
    source,
    is_active,
    created_at
)
SELECT
    address,
    entity_id,
    entity_name,
    entity_type,
    confidence,
    source,
    toUInt8(0),
    now()
FROM tron_db.address_entity FINAL
WHERE is_active = 1
  AND source IN (
      'many_deposit_wallets_to_one_sweeper',
      'one_wallet_to_many_withdrawals'
  );
