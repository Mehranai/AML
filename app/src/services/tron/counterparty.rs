use crate::services::tron::aml::types::SimpleTransfer;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CounterpartyRelation {
    pub address: String,
    pub counterparty: String,
    pub direction: String,
    pub token_address: String,
    pub total_txs: u64,
    pub total_volume: u128,
    pub first_seen: u64,
    pub last_seen: u64,
}

pub fn build_counterparty_relations(
    transfers: &[SimpleTransfer],
    block_number: u64,
) -> Vec<CounterpartyRelation> {
    let mut map = HashMap::<(String, String, String, String), CounterpartyRelation>::new();

    for t in transfers {
        add_relation(
            &mut map,
            &t.from,
            &t.to,
            "out",
            &t.token,
            t.amount,
            block_number,
        );

        add_relation(
            &mut map,
            &t.to,
            &t.from,
            "in",
            &t.token,
            t.amount,
            block_number,
        );
    }

    map.into_values().collect()
}

fn add_relation(
    map: &mut HashMap<(String, String, String, String), CounterpartyRelation>,
    address: &str,
    counterparty: &str,
    direction: &str,
    token_address: &str,
    amount: u128,
    block_number: u64,
) {
    let key = (
        address.to_string(),
        counterparty.to_string(),
        direction.to_string(),
        token_address.to_string(),
    );

    let entry = map.entry(key).or_insert_with(|| CounterpartyRelation {
        address: address.to_string(),
        counterparty: counterparty.to_string(),
        direction: direction.to_string(),
        token_address: token_address.to_string(),
        total_txs: 0,
        total_volume: 0,
        first_seen: block_number,
        last_seen: block_number,
    });

    entry.total_txs += 1;
    entry.total_volume += amount;
    entry.first_seen = entry.first_seen.min(block_number);
    entry.last_seen = entry.last_seen.max(block_number);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_inbound_and_outbound_counterparty_rows_per_token() {
        let transfers = vec![SimpleTransfer {
            token: "TRX".to_string(),
            from: "from".to_string(),
            to: "to".to_string(),
            amount: 10,
        }];

        let rows = build_counterparty_relations(&transfers, 100);

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| {
            row.address == "from"
                && row.counterparty == "to"
                && row.direction == "out"
                && row.token_address == "TRX"
                && row.total_volume == 10
        }));
        assert!(rows.iter().any(|row| {
            row.address == "to"
                && row.counterparty == "from"
                && row.direction == "in"
                && row.token_address == "TRX"
                && row.total_volume == 10
        }));
    }
}
