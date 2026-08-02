use clickhouse::Row;
use clickhouse::types::UInt256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Row, Serialize)]
pub struct AddressEntityRow {
    pub address: String,
    pub entity_id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub confidence: f32,
    pub source: String,
    pub is_active: u8,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct ExchangeAddressRow {
    pub address: String,
    pub entity_id: String,
    pub exchange_name: String,
    pub address_role: String,
    pub confidence: f32,
    pub detection_source: String,
    pub first_seen_block: u64,
    pub last_seen_block: u64,
    pub is_active: u8,
}

#[derive(Debug, Clone, Row, Serialize)]
pub struct ExchangeFlowRow {
    pub flow_id: String,
    pub tx_hash: String,
    pub block_number: u64,
    pub from_address: String,
    pub to_address: String,
    pub exchange_name: String,
    pub flow_type: String,
    pub token_address: String,
    pub amount: UInt256,
    pub confidence: f32,
}
