use std::{env, str::FromStr};

#[derive(Debug, Clone)]
pub enum AppMode {
    Eth,
    Btc,
    Bsc,
    Tron,
}

impl AppMode {
    fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "eth" | "ethereum" => Some(Self::Eth),
            "btc" | "bitcoin" => Some(Self::Btc),
            "bsc" => Some(Self::Bsc),
            "tron" | "trx" => Some(Self::Tron),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SyncMode {
    Backfill,
    Live,
    Auto,
}

impl SyncMode {
    fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "backfill" => Some(Self::Backfill),
            "live" => Some(Self::Live),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub mode: AppMode,
    pub sync_mode: SyncMode,

    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_pass: String,

    pub clickhouse_db_eth: String,
    pub clickhouse_db_btc: String,
    pub clickhouse_db_bsc: String,
    pub clickhouse_db_tron: String,

    pub eth_rpc_url: Option<String>,
    pub bsc_rpc_url: Option<String>,
    pub btc_api_url: Option<String>,
    pub tron_rpc_url: Option<String>,
    pub tron_api_key: Option<String>,

    pub btc_start_block: u64,
    pub eth_start_block: u64,
    pub bsc_start_block: u64,
    pub tron_start_block: u64,

    pub total_btc_txs: u64,
    pub total_eth_txs: u64,
    pub total_bsc_txs: u64,
    pub total_tron_txs: u64,

    pub rpc_timeout_seconds: u64,
    pub rpc_max_concurrency: usize,
    pub tx_worker_concurrency: usize,

    pub neo4j_uri: String,
    pub neo4j_username: String,
    pub neo4j_password: String,

    pub tron_allow_destructive_schema_cleanup: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let mode = env_optional("APP_MODE")
            .and_then(|value| AppMode::from_env_value(&value))
            .unwrap_or(AppMode::Tron);
        let sync_mode = env_optional("SYNC_MODE")
            .and_then(|value| SyncMode::from_env_value(&value))
            .unwrap_or(SyncMode::Auto);

        Self {
            mode,
            sync_mode,
            clickhouse_url: env_string("CLICKHOUSE_URL", "http://localhost:8123"),
            clickhouse_user: env_string("CLICKHOUSE_USER", "admin"),
            clickhouse_pass: env_string_any(
                &["CLICKHOUSE_PASSWORD", "CLICKHOUSE_PASS"],
                "mehran.admin",
            ),

            clickhouse_db_eth: env_string("CLICKHOUSE_DB_ETH", "eth_db"),
            clickhouse_db_btc: env_string("CLICKHOUSE_DB_BTC", "btc_db"),
            clickhouse_db_bsc: env_string("CLICKHOUSE_DB_BSC", "bsc_db"),
            clickhouse_db_tron: env_string("CLICKHOUSE_DB_TRON", "tron_db"),

            eth_rpc_url: env_optional_any(&["ETH_RPC_URL", "ETH_RPC_HTTP"]),
            bsc_rpc_url: env_optional_any(&["BSC_RPC_URL", "BSC_RPC_HTTP"]),
            btc_api_url: env_optional("BTC_API_URL")
                .or_else(|| Some("https://blockstream.info/api".to_string())),
            tron_rpc_url: env_optional_any(&["TRON_RPC_URL", "TRON_RPC_HTTP"])
                .or_else(|| Some("https://api.trongrid.io".to_string())),
            tron_api_key: env_optional_any(&["TRON_API_KEY", "TRONGRID_API_KEY"]),

            btc_start_block: env_parse("BTC_START_BLOCK", 831_000),
            eth_start_block: env_parse("ETH_START_BLOCK", 90_000),
            bsc_start_block: env_parse("BSC_START_BLOCK", 15_000_000),
            tron_start_block: env_parse("TRON_START_BLOCK", 0),

            total_btc_txs: env_parse("TOTAL_BTC_TXS", 500),
            total_eth_txs: env_parse("TOTAL_ETH_TXS", 500),
            total_bsc_txs: env_parse("TOTAL_BSC_TXS", 500),
            total_tron_txs: env_parse("TOTAL_TRON_TXS", 200),

            rpc_timeout_seconds: env_parse("RPC_TIMEOUT_SECONDS", 120),
            rpc_max_concurrency: env_parse("RPC_MAX_CONCURRENCY", 2),
            tx_worker_concurrency: env_parse("TX_WORKER_CONCURRENCY", 2),

            neo4j_uri: env_string("NEO4J_URI", "localhost:7687"),
            neo4j_username: env_string("NEO4J_USERNAME", "neo4j"),
            neo4j_password: env_string("NEO4J_PASSWORD", ""),

            tron_allow_destructive_schema_cleanup: env_bool(
                "TRON_ALLOW_DESTRUCTIVE_SCHEMA_CLEANUP",
                false,
            ),
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env_optional(key).unwrap_or_else(|| default.to_string())
}

fn env_string_any(keys: &[&str], default: &str) -> String {
    env_optional_any(keys).unwrap_or_else(|| default.to_string())
}

fn env_optional_any(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| env_optional(key))
}

fn env_optional(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_parse<T>(key: &str, default: T) -> T
where
    T: FromStr + Copy,
{
    env_optional(key)
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    env_optional(key)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "y" | "on"
            )
        })
        .unwrap_or(default)
}
