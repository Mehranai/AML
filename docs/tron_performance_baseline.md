# TRON Historical Ingestion Performance Baseline

Measured on 2026-08-01 against the local ClickHouse 23.8 and Neo4j Docker
services. The blockchain source was the configured remote TronGrid API; a local
TRON full/solidity node is not present in this workspace yet.

## Historical Sample

Command:

```powershell
cargo run --bin tron_benchmark_ingestion -- 2036 2040
```

Results:

- Source: `remote_api`
- Blocks completed: 5
- Transactions: 43
- Ingestion time: 68,994 ms
- Throughput: 0.0725 blocks/s and 0.6232 transactions/s
- Core physical row increase: 92
- Compressed-byte increase: 10,281 bytes
- Active-part increase: 3
- Open RPC behavior: repeated TronGrid HTTP 429 suspension windows

This measurement is dominated by remote API throttling and is not a valid
estimate of local-node throughput. It does confirm that bounded historical
ingestion, block completion, metric collection, and benchmark persistence work
end to end.

## Investigation Latency

Profile: depth 2, edge limit 200, 90-day fingerprint window, AI disabled.
Wallet: `TSnjgPDQfuxx72iaPy82v3T8HrsN4GVJzW` (171 directly stored edges).

- Original N+1 graph and metadata reads: 7,091 ms
- Batched graph frontiers: 2,673 ms
- Batched frontiers and node metadata: 276 ms
- Total improvement: 96.1%

The graph edge limit is now global. Graph expansion performs one relationship
query per breadth-first frontier, and exchange/entity metadata is loaded in two
batched queries instead of two serial queries per node.

## Storage Decisions

- Canonical blockchain evidence has no TTL because AML investigations require
  historical continuity.
- Benchmark rows have a 365-day TTL and contain one compact row per run.
- Background batch flushing changed from 1 second to a configurable 120-second
  default; successful blocks still flush immediately at their boundary.
- Batch memory is bounded by a configurable 10,000-row default.
- Monthly evidence partitions and the existing relationship sort key were kept;
  this dataset did not justify a table rewrite.
- Added address bloom indexes only for holdings deltas, semantic subjects, and
  exchange-flow endpoints.
- Removed duplicate relationship address indexes and the unused `amount_usd`
  column.

## Required Local-Node Follow-Up

Repeat the benchmark over low-, medium-, and high-activity ranges after a local
TRON full/solidity node is deployed. Use at least 10,000 blocks per activity
class and record p50/p95 throughput, receipt latency, ClickHouse merge pressure,
parts per partition, and unified investigation p50/p95. Only then should RPC
concurrency, codecs, or partition widths be changed again.
