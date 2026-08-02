# TRON Completion To-Do

This checklist defines the remaining work for a production TRON AML v1. A task
is complete only when its code, migration, tests, and operator documentation
have all passed against the local ClickHouse and Neo4j stack.

## 1. Ingestion Recovery - Complete

- [x] Persist block and transaction ingestion failures with retry history.
- [x] Mark blocks `PROCESSING`, `FAILED`, or `COMPLETE` durably.
- [x] Add bounded, idempotent block-range replay.
- [x] Refuse conflicting finalized hashes without an explicit repair operation.
- [x] Make exchange-flow evidence replay-safe.

## 2. Historical Ingestion and Performance - Remote Baseline Complete

- [x] Run a bounded representative historical backfill through the configured
      TronGrid source.
- [ ] Repeat representative low/medium/high-activity backfills after the local
      TRON node is deployed.
- [x] Measure rows, compressed bytes, insert throughput, and investigation
      latency.
- [x] Tune indexes, batch sizes, graph queries, and benchmark retention from
      measurements; retain the existing evidence partitions.
- [x] Add ingestion lag, stale-block, failure, storage, and data-gap monitoring.

## 3. Entity Intelligence and Clustering - Implementation Complete

- [x] Define structural cluster claims separately from entity attributions.
- [x] Add governed service/address feeds with evidence and review state.
- [x] Implement TRON-specific exchange deposit and service clustering.
- [x] Add analyst confirmation, rejection, and cluster version history.

Production label coverage remains an operational data-acquisition task: import
the licensed vendor, law-enforcement, public-research, and analyst sources that
the deployment is authorized to use.

## 4. Explainable Exposure Paths

- [ ] Persist the actual path behind every propagated exposure result.
- [ ] Preserve amount, time, direction, service mediation, and seed provenance.
- [ ] Add path invalidation/recomputation when canonical evidence changes.
- [ ] Present defensible path evidence in the investigation UI.

## 5. ML Risk Model

- [ ] Generate leakage-safe feature snapshots for governed wallet labels.
- [ ] Split training, validation, and test data by entity and time.
- [ ] Train and calibrate the PyTorch model.
- [ ] Select operating thresholds using precision/recall and review capacity.
- [ ] Add drift, prevalence, and model-quality monitoring.

## 6. Production Readiness

- [ ] Add explicit hash-conflict repair after the local TRON node is the
      configured canonical authority.
- [ ] Add authoritative holdings reconciliation with the local node.
- [ ] Add TRC10 metadata enrichment.
- [ ] Add authentication, analyst cases, notes, dispositions, and audit logs.
- [ ] Add backups, restore tests, observability, and disaster recovery.
- [ ] Validate TVM execution coverage beyond receipt-reported value movements.
