use crate::services::tron::aml::types::SimpleTransfer;

pub fn is_peel_chain(transfers: &[SimpleTransfer]) -> bool {
    if transfers.len() < 3 {
        return false;
    }

    //
    // simplistic heuristic:
    // many sequential outputs
    //
    let mut unique_receivers = std::collections::HashSet::new();

    for t in transfers {
        unique_receivers.insert(&t.to);
    }

    unique_receivers.len() >= 3
}

pub fn is_smurfing_pattern(transfers: &[SimpleTransfer]) -> bool {
    if transfers.len() < 5 {
        return false;
    }

    //
    // many small txs
    //
    transfers.iter().all(|t| t.amount < 1_000_000u128)
}

pub fn has_high_fanout(transfers: &[SimpleTransfer]) -> bool {
    let mut receivers = std::collections::HashSet::new();

    for t in transfers {
        receivers.insert(&t.to);
    }

    receivers.len() >= 10
}
