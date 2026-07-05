use crate::models::tron::exchange::ExchangeFlowRow;

use super::generic::GenericBatcher;

pub type ExchangeFlowBatcher = GenericBatcher<ExchangeFlowRow>;
