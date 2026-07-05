use std::collections::HashMap;

use once_cell::sync::Lazy;

use super::types::{
    ExchangeAttribution, ExchangeWalletRole, exchange_entity_id, unattributed_exchange_entity_id,
    unattributed_exchange_name,
};

#[derive(Debug, Clone, Copy)]
pub struct ExchangeHotWalletSeed {
    pub address: &'static str,
    pub exchange_name: Option<&'static str>,
    pub confidence: f32,
}

pub const fn known_exchange_hot_wallet(
    address: &'static str,
    exchange_name: &'static str,
) -> ExchangeHotWalletSeed {
    ExchangeHotWalletSeed {
        address,
        exchange_name: Some(exchange_name),
        confidence: 1.0,
    }
}

pub const fn unattributed_exchange_hot_wallet(address: &'static str) -> ExchangeHotWalletSeed {
    ExchangeHotWalletSeed {
        address,
        exchange_name: None,
        confidence: 0.95,
    }
}

// Add your manually verified TRON exchange hot wallets here.
// Use `unattributed_exchange_hot_wallet(address)` when you know it is an exchange
// hot wallet but do not want to name the exchange yet.
pub const MANUAL_EXCHANGE_HOT_WALLETS: &[ExchangeHotWalletSeed] = &[
    known_exchange_hot_wallet("TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX", "Binance"),
    known_exchange_hot_wallet("TU2TmqauSEiRf16CyFgzHV2BVxBejY9iyR", "OKX"),
];

pub static EXCHANGE_SEEDS: Lazy<HashMap<String, ExchangeAttribution>> = Lazy::new(|| {
    let mut map = HashMap::new();

    for seed in MANUAL_EXCHANGE_HOT_WALLETS {
        let exchange_name = seed
            .exchange_name
            .map(str::to_string)
            .unwrap_or_else(|| unattributed_exchange_name(seed.address));

        let cluster_id = seed
            .exchange_name
            .map(|_| exchange_entity_id(&exchange_name))
            .unwrap_or_else(|| unattributed_exchange_entity_id(seed.address));

        map.insert(
            seed.address.to_string(),
            ExchangeAttribution {
                exchange_name,
                role: ExchangeWalletRole::Hot.to_string(),
                confidence: seed.confidence,
                detection_source: "manual_hot_wallet_seed".to_string(),
                cluster_id: Some(cluster_id),
            },
        );
    }

    map
});

pub fn exchange_seeds() -> &'static HashMap<String, ExchangeAttribution> {
    &EXCHANGE_SEEDS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_hot_wallet_seeds_are_exchange_hot_wallets() {
        let seed = exchange_seeds()
            .get("TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX")
            .expect("manual Binance hot wallet seed");

        assert_eq!(seed.exchange_name, "Binance");
        assert_eq!(seed.role, "HOT");
        assert_eq!(seed.cluster_id.as_deref(), Some("exchange:binance"));
    }

    #[test]
    fn unattributed_manual_seed_keeps_exchange_semantics_without_name() {
        let seed = unattributed_exchange_hot_wallet("TAAAA1111");
        assert_eq!(seed.exchange_name, None);
        assert_eq!(seed.confidence, 0.95);
    }
}
