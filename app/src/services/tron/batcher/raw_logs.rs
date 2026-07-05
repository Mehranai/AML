use crate::models::tron::modules::TronRawLogRow;

use super::generic::GenericBatcher;

pub type RawLogBatcher = GenericBatcher<TronRawLogRow>;
