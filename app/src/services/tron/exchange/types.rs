use std::fmt;

#[derive(Debug, Clone)]
pub enum ExchangeWalletRole {
    Hot,
    Deposit,
    Sweep,
    Treasury,
    Withdraw,
    Internal,
}

impl fmt::Display for ExchangeWalletRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let role = match self {
            Self::Hot => "HOT",

            Self::Deposit => "DEPOSIT",

            Self::Sweep => "SWEEP",

            Self::Treasury => "TREASURY",

            Self::Withdraw => "WITHDRAW",

            Self::Internal => "INTERNAL",
        };

        f.write_str(role)
    }
}

#[derive(Debug, Clone)]
pub struct ExchangeAttribution {
    pub exchange_name: String,
    pub role: String,
    pub confidence: f32,
    pub detection_source: String,
    pub cluster_id: Option<String>,
}

pub fn exchange_entity_id(exchange_name: &str) -> String {
    let mut id = String::from("exchange:");

    for ch in exchange_name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            id.push('_');
        }
    }

    id.trim_end_matches('_').to_string()
}

pub fn unattributed_exchange_name(anchor_address: &str) -> String {
    format!(
        "Unattributed Exchange {}",
        address_fingerprint(anchor_address)
    )
}

pub fn unattributed_exchange_entity_id(anchor_address: &str) -> String {
    format!(
        "exchange:unattributed:{}",
        address_fingerprint(anchor_address).to_ascii_lowercase()
    )
}

fn address_fingerprint(address: &str) -> String {
    let alnum = address
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();

    if alnum.len() <= 8 {
        return alnum;
    }

    format!("{}{}", &alnum[..4], &alnum[alnum.len() - 4..])
}
