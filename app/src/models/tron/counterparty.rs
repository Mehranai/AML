use clickhouse::Row;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct CounterpartyRow {
    pub address: String,
    pub counterparty: String,
    pub direction: String,
    pub token_address: String,
    pub total_txs: u64,
    pub total_volume: u128,
    pub first_seen: u64,
    pub last_seen: u64,
}
