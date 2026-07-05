use once_cell::sync::Lazy;

use std::collections::HashMap;

use super::types::{ContractCategory, ProtocolInfo};

pub static METHOD_SIGNATURES: Lazy<HashMap<&'static str, ProtocolInfo>> = Lazy::new(|| {
    let mut map = HashMap::new();

    //
    // swapExactTokensForTokens
    //
    map.insert(
        "38ed1739",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    //
    // swapExactETHForTokens
    //
    map.insert(
        "7ff36ab5",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    map.insert(
        "18cbafe5",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    map.insert(
        "8803dbee",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    map.insert(
        "fb3bdb41",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    map.insert(
        "4a25d94a",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    map.insert(
        "5c11d795",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.85,
        },
    );

    //
    // addLiquidity
    //
    map.insert(
        "e8e33700",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    map.insert(
        "f305d719",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    map.insert(
        "baa2abde",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    map.insert(
        "02751cec",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    map.insert(
        "2195995c",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    map.insert(
        "af2979eb",
        ProtocolInfo {
            protocol: "GenericDex",
            category: ContractCategory::Dex,
            confidence: 0.80,
        },
    );

    //
    // lending borrow
    //
    map.insert(
        "c5ebeaec",
        ProtocolInfo {
            protocol: "GenericLending",
            category: ContractCategory::Lending,
            confidence: 0.90,
        },
    );

    map.insert(
        "40c10f19",
        ProtocolInfo {
            protocol: "GenericToken",
            category: ContractCategory::Token,
            confidence: 0.75,
        },
    );

    map.insert(
        "1249c58b",
        ProtocolInfo {
            protocol: "GenericToken",
            category: ContractCategory::Token,
            confidence: 0.70,
        },
    );

    map.insert(
        "42966c68",
        ProtocolInfo {
            protocol: "GenericToken",
            category: ContractCategory::Token,
            confidence: 0.75,
        },
    );

    map.insert(
        "89afcb44",
        ProtocolInfo {
            protocol: "GenericToken",
            category: ContractCategory::Token,
            confidence: 0.70,
        },
    );

    map
});

pub fn detect_method(method_data: &str) -> Option<(String, ProtocolInfo)> {
    if method_data.len() < 8 {
        return None;
    }

    let method_id = &method_data[0..8];

    METHOD_SIGNATURES
        .get(method_id)
        .map(|info| (method_id.to_string(), info.clone()))
}
