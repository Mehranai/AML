// src/models/mod.rs
pub mod blockstreams;
pub mod owner;
pub mod sync_state;
pub mod token_metadata;
pub mod token_transfer;
pub mod transaction;
pub mod tron;
pub mod wallet;

// Structs for ClickHouse
pub use owner::OwnerRow;
pub use sync_state::SyncStateRow;
pub use token_metadata::TokenMetadataRow;
pub use token_transfer::TokenTransferRow;
pub use transaction::TransactionRow;
pub use wallet::WalletRow;
