# Blockchain AML Services

Rust services for canonical blockchain ingestion, wallet investigation, graph
projection, exposure analysis, and PyTorch-backed AML inference.

TRON is the most complete chain implementation. Its master architecture and
remaining production gaps are documented in:

```text
../docs/explain_tron.md
```

## Local TRON Stack

The local defaults match `docker-compose.yml`.
Configuration uses real process environment variables first, then `app/.env` as
a local fallback. You do not need to set PowerShell `$env:` values for settings
already present in that file.

```powershell
cd D:\Sarbazi\dockerizd_eth_code\app
docker network inspect blockchain-net *> $null
if ($LASTEXITCODE -ne 0) { docker network create blockchain-net }
docker compose up -d clickhouse neo4j
docker compose ps
```

Apply migrations:

```powershell
cargo run --bin tron_init_schema
```

Run finalized-block ingestion:

```powershell
cargo run --bin arz_axum_for_services
```

`SYNC_MODE=live` and `SYNC_MODE=auto` poll continuously. `SYNC_MODE=backfill`
runs one pass. `TOTAL_TRON_TXS=0` removes the per-pass transaction limit while
preserving complete-block checkpoints.

TRON ingestion stores one canonical transfer fact in `address_relationships`
for native, TRC10, TRC20, and internal value movements. It does not duplicate
new transfers into `raw_logs` or `token_transfers`. TRC10 holdings remain exact
raw quantities until TRC10 metadata enrichment is added.

Run token metadata enrichment in another terminal:

```powershell
cargo run --bin tron_token_metadata_worker
```

Run the API and UI:

```powershell
cargo run --bin tron_graph_api
```

Open:

```text
http://127.0.0.1:4001/
```

Dependency readiness:

```powershell
Invoke-RestMethod http://127.0.0.1:4001/ready
```

Apply migrations and smoke-test both Docker dependencies:

```powershell
cargo test --test tron_stack_smoke -- --ignored
```

## Investigation APIs

```text
GET  /api/tron/wallet/{address}/graph
GET  /api/tron/wallet/{address}/holdings
GET  /api/tron/wallet/{address}/fingerprint
GET  /api/tron/wallet/{address}/ai-risk
GET  /api/tron/wallet/{address}/investigation
GET  /api/tron/wallet/{source}/paths/{target}
GET  /api/analysis/tron/wallet/{address}
POST /api/tron/wallet/{address}/neo4j/import
```

GET requests read ClickHouse and do not mutate Neo4j. Use the explicit POST
route to project a wallet subgraph.

Example:

```powershell
$address = "<TRON_WALLET_ADDRESS>"
Invoke-RestMethod "http://127.0.0.1:4001/api/tron/wallet/$address/investigation?depth=3&limit=500"
```

## Entity Intelligence, Clustering, and Exposure

Register each governed intelligence source before importing its labels:

```powershell
cargo run --bin tron_register_intelligence_source -- .\source.json
```

Submit replay-safe entity claims from JSONL. New claims remain pending unless
the record contains an explicit independent reviewer:

```powershell
cargo run --bin tron_ingest_entity_labels -- .\labels.jsonl
```

Discover pending exchange-deposit and service cluster claims from stored
canonical transfers, then approve or reject them explicitly:

```powershell
cargo run --bin tron_discover_address_clusters -- 0 1000000 5000
cargo run --bin tron_review_intelligence -- CLUSTER_CLAIM <claim_id> APPROVED <reviewer> "<reason>"
```

The unified investigation response and UI expose active attribution, cluster
versions, source trust, evidence, and pending reviews. Full operator formats
and governance behavior are documented in
`../docs/tron_entity_intelligence.md`.

Propagate exposure:

```powershell
cargo run --bin tron_propagate_exposure
```

## PyTorch Risk Model

Build one feature row per unique labeled wallet:

```powershell
cd D:\Sarbazi\dockerizd_eth_code
python ml\tron_wallet_risk\build_training_csv_from_api.py `
  --labels ml\tron_wallet_risk\my_labeled_wallets.csv `
  --output ml\tron_wallet_risk\training.csv
```

Train a candidate:

```powershell
python ml\tron_wallet_risk\train.py `
  --input ml\tron_wallet_risk\training.csv `
  --output-dir ml\tron_wallet_risk\artifacts\candidate_v1
```

Review the untouched test metrics. `--activate` generates a production
deployment only when the configured sample-count, AUC, and Brier gates pass:

```powershell
python ml\tron_wallet_risk\train.py `
  --input ml\tron_wallet_risk\training.csv `
  --output-dir ml\tron_wallet_risk\artifacts\model_v1 `
  --model-version v1 `
  --activate
```

The training and model-registration tools are retained, but runtime inference is
intentionally disabled while graph and evidence workflows are tested. Wallet
APIs return no laundering probability and do not substitute a formula. Re-enable
inference as a deliberate implementation phase after the evidence pipeline is
accepted; there is no runtime environment toggle in the current build.

Detailed ML instructions:

```text
../ml/tron_wallet_risk/README.md
```

## Ingestion Recovery

Every fetched solid block is journaled in `ingested_blocks` as `PROCESSING`,
`FAILED`, or `COMPLETE`. Transaction and block failures are stored in
`ingestion_failures` with a stable identity, attempt count, retryability, and
resolution status.

Replay one finalized block or an inclusive range:

```powershell
cargo run --bin tron_replay_blocks -- 84890000
cargo run --bin tron_replay_blocks -- 84890000 84890099
```

Replay is limited to 10,000 blocks per command, accepts only blocks at or below
the current solid head, and never changes `sync_state`. Existing evidence is
logically deduplicated by canonical event IDs. A finalized block hash conflict
is recorded and stops ingestion; the replay command does not rewrite competing
histories.

Inspect unresolved failures:

```sql
SELECT *
FROM tron_db.ingestion_failures FINAL
WHERE status = 'OPEN'
ORDER BY last_failed_at_unix_ms DESC;
```

The detailed TRON completion checklist is
[`docs/tron_completion_todo.md`](../docs/tron_completion_todo.md).

## Historical Benchmark and Monitoring

Run bounded historical ingestion without replaying blocks already marked
`COMPLETE`:

```powershell
cargo run --bin tron_benchmark_ingestion -- 2036 2040
cargo run --bin tron_benchmark_ingestion -- 2036 2040 <TRON_WALLET_ADDRESS>
```

The command accepts at most 10,000 blocks. It persists one compact row in
`ingestion_benchmarks`, including source kind, completed blocks, transactions,
elapsed time, throughput, rows, compressed bytes, active parts, and optional
unified-investigation latency. Benchmark history expires after 365 days;
canonical blockchain evidence does not expire.

Inspect ingestion health:

```text
GET /api/tron/ingestion/health
GET /api/tron/ingestion/health?gap_window_blocks=1000&stale_after_seconds=600&max_lag_blocks=20
```

The response reports the solid head, checkpoint, lag, stale processing blocks,
failed blocks, open failures, concrete missing journal ranges, and core
ClickHouse rows/bytes/parts.

Batch tuning variables:

```text
TRON_INGESTION_BATCH_MAX_ROWS=10000
TRON_INGESTION_FLUSH_INTERVAL_SECONDS=120
```

Blocks are still flushed immediately on successful completion. The interval is
only a background safety flush and is deliberately longer to avoid tiny parts
while remote receipts are slow. See
[`docs/tron_performance_baseline.md`](../docs/tron_performance_baseline.md) for
the measured baseline and local-node follow-up.

## Schema Safety

Schema changes are recorded in `tron_db.schema_migrations`. Applied migration
checksums are immutable. Startup migrates active objects in place and validates
their required columns.

Obsolete objects, including legacy `raw_logs`, `token_transfers`, and the v2
holdings ledger, are removed only with:

```powershell
$env:TRON_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP="true"
cargo run --bin tron_init_schema
```

This flag does not permit dropping active tables merely because they need a new
column.

## Production Configuration

Local defaults are intentionally convenient for the supplied Docker services.
Production must inject ClickHouse, Neo4j, and node credentials through a secret
manager.

Important variables:

```text
CLICKHOUSE_URL
CLICKHOUSE_USER
CLICKHOUSE_PASSWORD
TRON_RPC_URL
TRON_API_KEY
NEO4J_URI
NEO4J_USERNAME
NEO4J_PASSWORD
```

Never expose ClickHouse, Neo4j, or a node RPC endpoint publicly without
authentication, network policy, TLS, and monitoring.

`app/.env` is currently present in repository history. Before any public or
shared deployment, remove it from version control/history and rotate every
credential or API key it has contained. `.gitignore` now prevents new `.env`
files from being added accidentally.
