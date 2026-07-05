use super::generic::GenericBatcher;
use crate::models::tron::modules::TransactionRiskRow;

pub type TransactionRiskBatcher = GenericBatcher<TransactionRiskRow>;
