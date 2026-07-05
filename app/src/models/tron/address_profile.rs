use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct AddressProfileRow {
    pub address: String,
    pub total_in_tx: u64,
    pub total_out_tx: u64,
    pub unique_senders: u64,
    pub unique_receivers: u64,
    pub total_volume_in: u128,
    pub total_volume_out: u128,
    pub interacted_tokens: u32,
    pub probable_exchange: u8,
    pub probable_deposit_wallet: u8,
    pub probable_sweeper: u8,
    pub risk_score: f32,
}
