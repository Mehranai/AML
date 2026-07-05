use clickhouse::Row;
use serde::Serialize;

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeEntityRow {
    pub entity_id: String,
    pub exchange_name: String,
    pub exchange_type: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct AddressEntityRow {
    pub address: String,
    pub entity_id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeAddressRow {
    pub address: String,
    pub entity_id: String,
    pub exchange_name: String,
    pub address_role: String,
    pub confidence: f32,
    pub detection_source: String,
    pub first_seen_block: u64,
    pub last_seen_block: u64,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeDepositAddressRow {
    pub address: String,
    pub exchange_name: String,
    pub hot_wallet: String,
    pub confidence: f32,
    pub detection_method: String,
    pub first_seen_block: u64,
    pub last_seen_block: u64,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeClusterRow {
    pub cluster_id: String,
    pub exchange_name: String,
    pub address: String,
    pub role: String,
    pub confidence: f32,
    pub discovered_from: String,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeFlowRow {
    pub tx_hash: String,
    pub block_number: u64,
    pub from_address: String,
    pub to_address: String,
    pub exchange_name: String,
    pub flow_type: String,
    pub token_address: String,
    pub amount: u128,
    pub confidence: f32,
}
