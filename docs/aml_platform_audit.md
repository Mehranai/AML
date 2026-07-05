# AML Platform Audit

Date: 2026-07-01

This audit treats the project as a TRON-first AML investigation platform that
should grow toward Chainalysis-style capabilities: normalized ingestion,
entity attribution, typology detection, exposure tracing, explainable wallet
risk, graph investigation, analyst workflow, and model governance.

Chainalysis internals are proprietary. The target here is not to clone private
implementation details, but to build the same class of defensible AML platform:
evidence-first, explainable, reproducible, and migration-governed.

## Current Strengths

- TRON ingestion writes core facts into ClickHouse: transactions, raw logs,
  token transfers, address relationships, transaction features, transaction
  risk, exchange attribution, address profiles, counterparties, contract
  metadata, and exchange flows.
- Neo4j import can build a local wallet neighborhood graph from
  `address_relationships`.
- Wallet fingerprinting can summarize identity, direct counterparties, flow
  shape, transaction risk, timing, swap/bridge behavior, and exchange
  interaction.
- Wallet AML risk v1 now layers deterministic typology/risk scoring on top of
  the fingerprint, returning risk percentage, confidence, components,
  typologies, risk factors, protective factors, and evidence.

## P0 Deficiencies

### Schema Lifecycle Was Ambiguous

The TRON schema contained production tables, roadmap tables, and old tables
without a clear lifecycle. That makes it unclear whether a default-zero column
is valid data, missing enrichment, or obsolete design.

Resolution:

- Tables without an active writer/reader are no longer created by the TRON
  schema.
- Previously-created placeholder tables are listed as obsolete in
  `app/src/db/init_tron.rs` so local/prototype databases can be cleaned during
  init.
- `address_relationships.event_type` and `address_relationships.hop_count`
  are now active and populated by relationship creation.
- `contract_metadata.protocol_name` is now active and populated from the
  classifier.
- `transaction_features.fan_in` and `transaction_features.fan_out` are now
  computed from unique transfer senders and receivers instead of being aliases
  for participant count.

Removed from active schema until real implementations exist:

- `blocks`, `internal_transfers`, `entity_relationships`, `flow_segments`,
  `flow_edges_hourly`, `contract_interactions`, `address_behavior`,
  `wallet_fingerprints`, `wallet_counterparty_fingerprints`, `sweep_edges`,
  `aml_events`, `wallet_risk`, `exposure_paths`, `investigation_cache`,
  `method_signatures`, `address_clusters`, `graph_edges`, `cluster_edges`,
  and `schema_lifecycle`.

### Schema Migration Safety

`app/src/db/init_tron.rs` records TRON schema application in
`tron_db.schema_migrations`. Normal startup applies idempotent migrations but
does not drop incompatible, obsolete, legacy, or rebuildable objects.

Destructive cleanup is now operator-controlled through
`TRON_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP=true`. This remains appropriate for
local/prototype cleanup, not for production evidence storage without backups and
an external migration process.

### Secrets Are Hardcoded

`app/src/config.rs` now reads runtime values from environment variables and no
longer embeds ClickHouse passwords, Neo4j passwords, TRON API keys, or paid RPC
URLs.

Next step: add deployment-specific secret injection and validate required
variables at process startup for each selected mode.

## P1 Deficiencies

### Risk Engine Is Not Evidence-Complete

`transaction_risk` currently stores several columns as placeholders:
sanctioned exposure, mixer exposure, and exposure depth are not populated by
seed ingestion or propagation. Wallet AML risk v1 is explainable and persisted
to `wallet_risk_assessments`, but it still has no full model governance
registry beyond model/version fields in the stored rows.

Next step: connect address exposure to transaction and wallet scoring, then add
formal model governance/evaluation metadata.

### Semantic AML Events Are Not Persisted

The fetcher builds swaps, bridges, mints, burns, and liquidity events in memory,
but there is no active semantic-event persistence table. That avoids database
bloat for now, but loses an important evidence layer for analyst review and
model training.

Next step: add an AML event batcher after event amounts and event IDs are
normalized.

### Entity Resolution Is Thin

Exchange attribution exists, but broader entity resolution is not implemented:
sanctioned entities, mixers, darknet services, scam labels, bridges, DEXes,
OTC desks, gambling services, and manually curated intelligence are missing.
Address clustering tables were removed until a real clustering pipeline exists.

Next step: add a seed ingestion interface and a versioned entity-attribution
model before expanding clustering.

### Graph API Mutates On Read

The wallet graph handler builds the graph and imports into Neo4j from a GET
request. For investigation products, read endpoints should be separated from
mutation/import jobs so analysts can trust when evidence was written.

Next step: split graph build/import from graph read/search endpoints.

### Ingestion Is Too Coupled

`process_tx` handles extraction, metadata, classification, risk, relationships,
profiles, exchange attribution, and persistence in one function. This makes
testing, retries, and backfills harder.

Next step: split into extraction, semantic classification, enrichment, scoring,
and persistence stages with typed intermediate records.

### Multi-Hop Exposure Is Incomplete

`address_exposure` has propagation code, but transaction and wallet risk do not
yet consume it. A dedicated exposure-path table should only be introduced when
there is a real explainable path store.

Next step: compute direct and indirect exposure as a first-class risk component
and store path evidence separately from aggregate scores.

## P2 Deficiencies

- No case management, analyst notes, alert queue, or review status.
- No model evaluation dataset, false-positive review loop, or score calibration
  report.
- No asset pricing service, so USD-normalized flow thresholds cannot be
  trusted.
- Cross-chain abstractions are not mature. TRON is the only chain with the AML
  pipeline shape needed for professional investigation.
- No authorization/audit trail for analyst actions.

## Iterative Refactor Plan

1. Schema cleanup and active column handling.
   Status: completed for the first pass. Unused placeholder tables are removed
   from creation and listed as obsolete; active columns are populated.

2. Persist wallet intelligence.
   Status: wallet AML risk assessments are persisted with model version,
   components, typologies, factors, evidence, and timestamps.

3. Separate graph mutation from graph reads.
   Make import an explicit POST/job and make GET read already-imported or
   ClickHouse-only graph data.

4. Add risk seed ingestion.
   Version sanctioned, mixer, scam, exchange, bridge, DEX, and service labels.
   Make every attribution explain its source and confidence.

5. Connect exposure propagation to risk.
   Add exposure fields only together with scoring code that consumes them, then
   connect wallet risk exposure components to path evidence.

6. Persist semantic AML events.
   Add the semantic event table and writer together; do not create the table as
   a placeholder.

7. Introduce AI inference safely.
   Keep deterministic rules as the evidence layer. Add AI as a calibrated
   inference/explanation layer with model versioning, confidence, and review
   feedback.

8. Productize investigation workflow.
   Add alerts, cases, analyst review status, notes, evidence export, and audit
   logs.

## Verification Gates

Every refactor slice should pass:

- `cargo fmt`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

Database-impacting changes should also include:

- active writer/reader evidence for every new table,
- migration or obsolete-table cleanup path,
- backfill plan when historical data is affected,
- explicit rejection of placeholder schema.
