## Now BTC is availabe (Global API)

### How to run docker to store BTC data on Clickhouse DB

1. First start You docker on Windows
2. CD to project directory ( cd ./Dockerized_Services )

3. Run codes

```bash
docker compose build --no-cache
```

```bash
APP_MODE=btc docker compose up -d
```

4. See Log is Running Now ...

```bash
docker compose logs -f
```

### Intract with Clickhouse Database

```bash
docker exec -it clickhouse clickhouse-client
```

And then intract with database:
```sql
show databases;
use btc_db;
show tables;
select * from wallet_info;
```

### Docker Neo4j section

    docker network create blockchain-net
    docker compose up -d clickhouse neo4j
    cargo run --bin arz_axum_for_services
    cargo run --bin tron_export_wallet_graph -- TEPSrSYPDSQ7yXpMFPq91Fb1QEWpMkRGfn 5 500

And then we are going to see this
http://localhost:7474/browser/
with
neo4j/password

### use this to visualize 
    
    cargo run --bin tron_graph_api
    curl -X POST "http://localhost:3000/tron/wallet/<TRON_WALLET_ADDRESS>/neo4j/import?depth=3&limit=500"

## New way of Web section

    cargo run --bin tron_graph_api
    http://127.0.0.1:3000/

## Runtime configuration

The app reads runtime configuration from environment variables. Secrets such as
ClickHouse passwords, Neo4j passwords, and TRON API keys are not embedded in
source code. Use `.env.example` as the safe template.

For the local Docker Compose services in this repository, set the matching
credentials before running the API or ingestion loop:

```powershell
$env:CLICKHOUSE_URL="http://localhost:8123"
$env:CLICKHOUSE_USER="admin"
$env:CLICKHOUSE_PASSWORD="<clickhouse-password>"
$env:NEO4J_URI="localhost:7687"
$env:NEO4J_USERNAME="neo4j"
$env:NEO4J_PASSWORD="<neo4j-password>"
$env:TRON_RPC_URL="https://api.trongrid.io"
```

TRON schema changes are recorded in `tron_db.schema_migrations`. Normal startup
applies idempotent migrations but does not drop obsolete tables. For an
init-only migration runs, use:

```powershell
cargo run --bin tron_init_schema
```

For an intentional local cleanup run only, set:

```powershell
$env:TRON_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP="true"
```

## TRON wallet fingerprint API

Wallet fingerprinting identifies the target wallet, its direct sender wallets, and
its direct receiver wallets from historical `address_relationships` flow data. The
API combines exchange attribution, entity labels, contract metadata, transaction
risk, DeFi behavior, bridge behavior, activity timing, token diversity, and
counterparty concentration.

Run the graph API:

```bash
cargo run --bin tron_graph_api
```

Query a wallet fingerprint:

```bash
curl "http://127.0.0.1:3000/api/tron/wallet/<TRON_WALLET_ADDRESS>/fingerprint?window_days=90&top_counterparties=25&max_events=20000"
```

The response includes:

- `identity`: best current label for the requested wallet, using exchange/entity/contract/profile data.
- `fingerprint_label` and `wallet_type`: behavior class such as exchange deposit funnel, collector, distributor, DeFi swapper, bridge user, service hub, or retail wallet.
- `flows`: inbound/outbound transfer counts, unique sender and receiver counts, raw volume totals, and observed transaction risk.
- `behavior`: active hours, active days, burst score, average transaction interval, token diversity, contract/swap/bridge/exchange ratios, and counterparty concentration.
- `senders`: direct wallets that funded the target wallet, each with identity, relationship label, tokens, volume, first/last seen, risk, and share of wallet activity.
- `receivers`: direct wallets that received funds from the target wallet, with the same fingerprint details.
- `risk_flags`: compact AML flags for high risk transactions, exchange-heavy flow, burst activity, concentration, fan-in/fan-out patterns, swap-heavy activity, and bridge-heavy activity.

One Example to see output of fingerprint:
```bash
curl "http://127.0.0.1:4000/api/tron/wallet/THMMcdQ2badbBzmnzYGYCaFq9qpyiCh1rn/fingerprint?window_days=90&top_counterparties=25&max_events=20000" | ConvertFrom-Json | ConvertTo-Json -Depth 10 
```

## TRON wallet AML risk API

The AML risk endpoint builds on the wallet fingerprint and returns a first-pass
rules-based assessment. It scores transaction risk, behavioral patterns,
typology matches, direct counterparty exposure, and identity context, then
returns a `risk_percent`, `risk_level`, confidence, risk factors, protective
factors, and the evidence used by the assessment.

Query a wallet AML risk assessment:

```bash
curl "http://127.0.0.1:3000/api/tron/wallet/<TRON_WALLET_ADDRESS>/aml-risk?window_days=90&top_counterparties=25&max_events=20000"
```

The current model version is `wallet_aml_risk_v1_rules`. It is intentionally
deterministic so analysts can inspect the evidence before we add AI inference on
top of it. Each computed assessment is persisted to
`tron_db.wallet_risk_assessments`.

Read persisted wallet risk history:

```bash
curl "http://127.0.0.1:3000/api/tron/wallet/<TRON_WALLET_ADDRESS>/risk-assessments?limit=25"
```

## TRON wallet holdings API

Wallet holdings are read from the unified `wallet_asset_balances` view, which
combines native TRX and TRC20 balances derived from indexed transfers.

```bash
curl "http://127.0.0.1:3000/api/tron/wallet/<TRON_WALLET_ADDRESS>/holdings?limit=50"
```

The response includes the native TRX balance, top indexed assets, raw balances,
decimal balances, token metadata, and metadata-gap counts.

## TRON wallet investigation API

The unified investigation endpoint returns the graph, behavioral fingerprint,
wallet holdings, AML risk assessment, and data-quality warnings in one response.

```bash
curl "http://127.0.0.1:3000/api/tron/wallet/<TRON_WALLET_ADDRESS>/investigation?depth=3&limit=500&window_days=90&top_counterparties=25&max_events=20000&holdings_limit=25"
```
