use std::fmt;

#[derive(Debug, Clone)]
pub enum RelationshipType {
    NativeTransfer,
    Trc20Transfer,
    Swap,
    Bridge,
    ExchangeDeposit,
    ExchangeWithdraw,
    LiquidityAdd,
    LiquidityRemove,
    Mint,
    Burn,
    InternalTransfer,
}

impl fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let relationship_type = match self {
            Self::NativeTransfer => "native_transfer",

            Self::Trc20Transfer => "trc20_transfer",

            Self::Swap => "swap",

            Self::Bridge => "bridge",

            Self::ExchangeDeposit => "exchange_deposit",

            Self::ExchangeWithdraw => "exchange_withdraw",

            Self::LiquidityAdd => "liquidity_add",

            Self::LiquidityRemove => "liquidity_remove",

            Self::Mint => "mint",

            Self::Burn => "burn",

            Self::InternalTransfer => "internal_transfer",
        };

        f.write_str(relationship_type)
    }
}
