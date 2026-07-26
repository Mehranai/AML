# Solana Node

This directory contains a Docker-based Agave RPC node setup for local development
and integration work.

## Important Production Reality

Agave's own validator documentation says Docker is not recommended or generally
supported for live clusters such as `mainnet-beta`. Docker is useful here as a
repeatable local package, but a serious high-speed Solana mainnet archival RPC
node should run on a tuned Ubuntu host with direct NVMe disks.

A local Solana node also does not reconstruct all historical Solana transactions
from genesis just because it is started today. It can retain the ledger it
downloads from this point forward, and it can serve transaction history for
locally available ledger data. Full historical AML coverage still requires a
historical backfill source plus our own ClickHouse indexer.

## Default Behavior

The compose file runs a non-voting mainnet RPC node:

- RPC HTTP: `http://127.0.0.1:8899`
- RPC WebSocket: `ws://127.0.0.1:8900`
- Cluster: `mainnet-beta`
- `--no-voting` enabled
- `--full-rpc-api` enabled
- `--enable-rpc-transaction-history` enabled
- `--enable-cpi-and-log-storage` enabled
- `--limit-ledger-size` disabled

Disabling `--limit-ledger-size` avoids intentional ledger pruning, but disk usage
will grow continuously.

## Run

From this directory:

```powershell
docker compose --profile mainnet up -d --build solana-mainnet-rpc
```

Follow logs:

```powershell
docker logs -f solana-mainnet-rpc
```

Check RPC health:

```powershell
Invoke-RestMethod `
  -Method Post `
  -Uri http://127.0.0.1:8899 `
  -ContentType "application/json" `
  -Body '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

## Hardware Guidance

For a real RPC node, plan around:

- 16 cores / 32 threads or more
- 512 GB RAM if using all account indexes
- Separate high-TBW NVMe disks for ledger and accounts
- 1 Gbps symmetric network minimum, 10 Gbps preferred
- Linux host tuning for file descriptors, memory locking, UDP buffers, and
  `vm.max_map_count`

Running this on normal Docker Desktop storage is only suitable for tests.

## Next AML Step

The node is only the data source. To make Solana useful for the AML platform, the
next implementation should add:

- Solana ClickHouse schema
- Solana slot/transaction checkpointing
- SOL transfer parser
- SPL Token and Token-2022 transfer parser
- inner-instruction parser
- canonical `address_relationships`
- wallet holdings aggregation
- optional Geyser or Yellowstone stream ingestion
