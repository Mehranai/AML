# TRON Wallet Investigation Testing

## Runtime Mode

The current TRON API is in graph-and-evidence testing mode. AI inference is
disabled in code. No model is loaded, no laundering probability is calculated,
and no prediction is persisted from wallet investigation requests.

The disabled `ai_risk` object remains in the JSON response for analytical-node
contract compatibility. Its `status` and `risk_level` are `DISABLED`, and it has
no `risk_score`, `risk_percent`, or `prediction_id`.

## Start The Stack

From `app`:

```powershell
docker compose up -d clickhouse neo4j
cargo run --bin tron_init_schema
cargo run --bin tron_graph_api
```

Open `http://127.0.0.1:4001/`.

Readiness check:

```powershell
Invoke-RestMethod http://127.0.0.1:4001/ready
```

Both `clickhouse` and `neo4j` must report `ready`.

## Wallet Investigation

Enter a TRON address, choose graph depth, edge limit, and activity window, then
select **Trace**. The unified request is:

```text
GET /api/tron/wallet/{address}/investigation
```

The response and UI contain:

- ClickHouse-backed wallet graph nodes and transfer edges.
- Canonical operation labels from `transaction_features`, including swap,
  bridge, liquidity, mint, burn, contract call, and transfer operations when
  that evidence exists.
- Incoming and outgoing transfer trends with adaptive day, week, or 30-day
  buckets.
- Top incoming senders and outgoing receivers.
- Persisted semantic AML events with transaction hash, protocol, assets,
  detector, and confidence.
- Holdings, entity intelligence, exchange interactions, fingerprint, and data
  quality warnings.

The graph switches to a low-label dense mode for large wallets. This changes
only presentation; it does not remove nodes or edges from the response.

Operation semantics are joined through the canonical ClickHouse view. They are
not copied into every stored relationship row, avoiding duplicate storage.

## Source-To-Target Paths

Open the **Paths** panel and enter source and target addresses. Choose outgoing,
incoming, or any-direction traversal and a maximum of one to ten hops.

```text
GET /api/tron/wallet/{source}/paths/{target}
```

The API enforces a maximum depth of 10, returns at most 50 paths, and bounds
per-address expansion. The returned path subgraph is drawn on the main canvas.

The current local dataset contains this verified two-hop outgoing test path:

```text
source: TRXnA3LdY5LqFatpLPpyYFYmKyJJCB3ZzR
via:    TGPnfvkVkUdyCWmrVGnQ9jFW5Ca1S7XH6h
target: TYdo7v7P3UbPLpiRaJB8b6ZLJbF8nFPyWH
```

Use **Project visible graph to Neo4j** to persist only the currently visible
wallet or path graph. Projection endpoints are explicit POST requests:

```text
POST /api/tron/wallet/{address}/neo4j/import
POST /api/tron/wallet/{source}/paths/{target}/neo4j/import
```

Neo4j schema creation is guarded once per API process and contains only
idempotent create operations. Projection does not delete prior semantic edges,
rewrite unlabeled wallets, or mutate another network's graph.

## Current Local Data

At the time of this verification, the busiest indexed wallet had 171 transfer
edges and 170 outgoing counterparties. The local dataset had no persisted rows
in `semantic_aml_events`, so swap, bridge, liquidity, mint, and burn evidence
correctly displays zero. Those panels will populate when ingestion encounters
and classifies such transactions.

Legacy topology-only exchange guesses are inactive. Only governed, active
exchange attributions can enter the canonical exchange-flow view and graph.
