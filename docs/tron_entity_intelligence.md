# TRON Entity Intelligence and Address Clustering

This layer keeps three different statements separate:

1. An entity label claims that an address belongs to a named organization.
2. A cluster claim says that addresses appear structurally related.
3. An active projection contains only claims approved for investigation use.

Heuristic behavior never becomes an entity attribution automatically.

## Storage

`intelligence_sources` is the governed registry for analyst, law-enforcement,
regulatory, vendor, public-research, internal, and heuristic sources. A source
has a trust tier and can be disabled without deleting its history.

`entity_labels` stores replay-safe address-to-entity claims, including the
source record id, role, confidence, case, evidence references, submitter, and
optional superseded claim. New operator submissions start as `PENDING`.

`address_cluster_claims` stores structural evidence independently from entity
ownership. Evidence includes transaction hashes, related addresses, detector
version, activity metrics, confidence, and supersession.

`intelligence_reviews` is immutable review history for both claim types. The
latest `APPROVED` or `REJECTED` decision controls the active projection.

`address_cluster_memberships` is the compact current membership projection.
`cluster_versions` preserves every approved or rejected membership change and
the resulting active member count.

`address_entity`, `exchange_addresses`, and `exposure_seeds` are compact active
projections used by investigation queries. Rejected claims retract only a
projection created from that same claim, so rejecting old evidence cannot
erase a newer independent attribution.

All claim timestamps use `created_at_unix_ms`. There are no validity-range
columns.

## Register a Source

Create a JSON file:

```json
{
  "source_id": "analyst_team_primary",
  "source_name": "Primary analyst review team",
  "source_type": "ANALYST",
  "trust_tier": "HIGH",
  "reference_url": "",
  "license": "internal",
  "is_active": true,
  "created_by": "aml_admin"
}
```

Register it:

```powershell
cargo run --bin tron_register_intelligence_source -- .\source.json
```

## Submit Entity Labels

The importer accepts one JSON object per line. `source_record_id` and at least
one evidence reference are required so replaying a feed is idempotent and every
claim is defensible.

```json
{"address":"T...","entity_id":"exchange:example","entity_name":"Example Exchange","entity_type":"centralized_exchange","address_role":"HOT","confidence":0.98,"risk_percent":0,"source":"analyst_team_primary","source_record_id":"case-421-address-1","submitted_by":"analyst@example","case_id":"CASE-421","evidence_refs":["case:CASE-421/document:7"],"review_status":"PENDING"}
```

```powershell
cargo run --bin tron_ingest_entity_labels -- .\labels.jsonl
```

An independently reviewed feed may include `review_status`, `reviewed_by`, and
`review_reason`. Otherwise use the explicit review command.

## Review Claims

```powershell
cargo run --bin tron_review_intelligence -- `
  ENTITY_LABEL <label_id> APPROVED analyst@example "Evidence confirmed"

cargo run --bin tron_review_intelligence -- `
  CLUSTER_CLAIM <claim_id> REJECTED analyst@example "Customer payment, not common control"
```

The review command is intentionally local/operator-only. An unauthenticated
HTTP route is not allowed to mutate governed intelligence.

## Discover Structural Clusters

```powershell
cargo run --bin tron_discover_address_clusters -- `
  <start_block> <end_block> <max_claims>
```

The version-one TRON worker produces only pending claims:

- `TRON_EXCHANGE_DEPOSIT_SWEEP_V1` requires multiple independent funders, a
  narrow outgoing pattern, and a sweep into an already approved exchange
  service address. A normal customer making one exchange deposit is excluded.
- `TRON_SERVICE_ACTIVITY_V1` identifies high fan-in/fan-out hot-wallet,
  consolidation, and withdrawal-service structures without assigning an
  organization name.

Claim ids include the structural evidence, so an unchanged rerun is skipped.
New evidence creates a new claim that references the prior claim through
`supersedes_claim_id`.

## Investigation Output

The unified wallet investigation response includes `intelligence` with:

- active direct entity attribution
- active exchange/service projection
- active cluster memberships and versions
- entity and structural claims
- source name and trust tier
- latest review status, reviewer, and reason
- pending-review count

Approved memberships are also projected into Neo4j as
`(:Wallet)-[:MEMBER_OF]->(:AddressCluster)` and graph nodes expose their current
cluster id and role.

The current repository no longer reads or writes the legacy
`exchange_entities`, `exchange_deposit_addresses`, or `exchange_clusters`
tables. Existing installations keep them quarantined until destructive schema
cleanup is explicitly authorized.
