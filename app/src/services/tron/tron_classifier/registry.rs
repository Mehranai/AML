use once_cell::sync::Lazy;

use std::collections::HashMap;

use super::types::{ContractCategory, ProtocolInfo};

pub static KNOWN_PROTOCOLS: Lazy<HashMap<&'static str, ProtocolInfo>> = Lazy::new(|| {
    let mut map = HashMap::new();

    //
    // SunSwap
    //
    map.insert(
        "TVGfBNTX1f7m7zYqKkvjYw6S7xKQjF9kYx",
        ProtocolInfo {
            protocol: "SunSwap",
            category: ContractCategory::Dex,
            confidence: 0.99,
        },
    );

    //
    // JustLend
    //
    map.insert(
        "TMwFHYXLJaRUPeW6421aqXL4ZEzPRFGkGT",
        ProtocolInfo {
            protocol: "JustLend",
            category: ContractCategory::Lending,
            confidence: 0.99,
        },
    );

    //
    // BitTorrent Bridge
    //
    map.insert(
        "TBdTs1DFKpXwq8rrhTCevhuswybqqgi3g9",
        ProtocolInfo {
            protocol: "BitTorrentBridge",
            category: ContractCategory::Bridge,
            confidence: 0.99,
        },
    );

    map
});
