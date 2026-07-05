use crate::models::tron::exchange::ExchangeFlowRow;
use crate::models::tron::modules::{
    TransactionFeatureRow, TransactionRiskRow, TransactionRow, TronRawLogRow, TronTokenTransferRow,
};
use crate::models::tron::relationship::AddressRelationshipRow;
use crate::progress::progress_tron::ContractMetadataRow;

use super::traits::BatchInsert;

impl BatchInsert for TransactionRow {
    const TABLE: &'static str = "transactions";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for TronTokenTransferRow {
    const TABLE: &'static str = "token_transfers";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for TronRawLogRow {
    const TABLE: &'static str = "raw_logs";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for AddressRelationshipRow {
    const TABLE: &'static str = "address_relationships";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for TransactionFeatureRow {
    const TABLE: &'static str = "transaction_features";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for TransactionRiskRow {
    const TABLE: &'static str = "transaction_risk";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for ContractMetadataRow {
    const TABLE: &'static str = "contract_metadata";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}

impl BatchInsert for ExchangeFlowRow {
    const TABLE: &'static str = "exchange_flows";

    fn as_value(&self) -> Self::Value<'_> {
        self.clone()
    }
}
