use clickhouse::Row;
use serde::Serialize;

//
// ======================================================
// TRANSACTIONS
// ======================================================
//

#[derive(Debug, Row, Serialize, Clone)]
pub struct TransactionRow {
    pub tx_hash: String,

    pub block_number: u64,

    // Tron raw timestamp in milliseconds.
    pub timestamp: u64,

    pub from_address: String,
    pub to_address: String,

    pub contract_address: String,

    pub contract_type: String,

    pub amount: u128,

    pub fee: u128,

    pub energy_fee: u128,

    pub net_fee: u128,

    pub energy_usage: u64,

    pub energy_usage_total: u64,

    pub net_usage: u64,

    pub status: u8,

    pub memo: String,
}

//
// ======================================================
// TOKEN TRANSFERS
// ======================================================
//

#[derive(Debug, Row, Serialize, Clone)]
pub struct TronTokenTransferRow {
    pub tx_hash: String,

    pub block_number: u64,

    pub timestamp: u64,

    pub log_index: u32,

    pub token_address: String,

    pub from_address: String,

    pub to_address: String,

    pub amount: u128,

    pub is_mint: u8,

    pub is_burn: u8,

    pub event_signature: String,
}

//
// ======================================================
// RAW LOGS
// ======================================================
//

#[derive(Debug, Row, Serialize, Clone)]
pub struct TronRawLogRow {
    pub tx_hash: String,

    pub block_number: u64,

    pub log_index: u32,

    pub contract_address: String,

    pub topics: Vec<String>,

    pub data: String,

    pub removed: u8,

    pub timestamp: u64,
}

//
// ======================================================
// TRANSACTION FEATURES
// ======================================================
//

#[derive(Debug, Row, Serialize, Clone)]
pub struct TransactionFeatureRow {
    pub tx_hash: String,

    pub block_number: u64,

    pub timestamp: u64,

    pub transaction_type: String,

    pub transaction_subtype: String,

    pub classification_confidence: f32,

    pub classification_source: String,

    pub protocol: String,

    pub method_id: String,

    pub is_swap: u8,

    pub is_bridge: u8,

    pub is_mint: u8,

    pub is_burn: u8,

    pub is_liquidity_add: u8,

    pub is_liquidity_remove: u8,

    pub is_contract_call: u8,

    pub unique_tokens: u16,

    pub participants: u16,

    pub hop_count: u16,

    pub fan_in: u16,

    pub fan_out: u16,
}

//
// ======================================================
// TRANSACTION RISK
// ======================================================
//

#[derive(Debug, Row, Serialize, Clone)]
pub struct TransactionRiskRow {
    pub tx_hash: String,

    pub block_number: u64,

    pub timestamp: u64,

    pub risk_score: u8,

    pub risk_level: String,

    pub transaction_type: String,

    pub transaction_subtype: String,

    pub is_swap: u8,

    pub is_bridge: u8,

    pub is_contract_call: u8,

    pub unique_tokens: u16,

    pub participants: u16,

    pub risk_reasons: Vec<String>,

    pub touches_exchange: u8,
}
