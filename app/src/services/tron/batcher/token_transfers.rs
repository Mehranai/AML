use crate::models::tron::modules::TronTokenTransferRow;

use super::generic::GenericBatcher;

pub type TokenTransferBatcher = GenericBatcher<TronTokenTransferRow>;
