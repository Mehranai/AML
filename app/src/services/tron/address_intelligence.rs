use std::collections::{HashMap, HashSet};

use crate::services::tron::aml::types::SimpleTransfer;

#[derive(Debug, Clone)]
pub struct AddressProfile {
    pub address: String,
    pub total_in_tx: u64,
    pub total_out_tx: u64,
    pub unique_senders: u64,
    pub unique_receivers: u64,
    pub total_volume_in: u128,
    pub total_volume_out: u128,
    pub interacted_tokens: HashSet<String>,
    pub probable_exchange: bool,
    pub probable_deposit_wallet: bool,
    pub probable_sweeper: bool,
    pub risk_score: f32,
}

pub fn build_address_profiles(transfers: &[SimpleTransfer]) -> HashMap<String, AddressProfile> {
    let mut profiles = HashMap::<String, AddressProfile>::new();

    for t in transfers {
        {
            let profile = profiles
                .entry(t.from.clone())
                .or_insert_with(|| AddressProfile {
                    address: t.from.clone(),
                    total_in_tx: 0,
                    total_out_tx: 0,
                    unique_senders: 0,
                    unique_receivers: 0,
                    total_volume_in: 0,
                    total_volume_out: 0,
                    interacted_tokens: HashSet::new(),
                    probable_exchange: false,
                    probable_deposit_wallet: false,
                    probable_sweeper: false,
                    risk_score: 0.0,
                });

            profile.total_out_tx += 1;
            profile.total_volume_out += t.amount;

            profile.interacted_tokens.insert(t.token.clone());
        }

        {
            let profile = profiles
                .entry(t.to.clone())
                .or_insert_with(|| AddressProfile {
                    address: t.to.clone(),
                    total_in_tx: 0,
                    total_out_tx: 0,
                    unique_senders: 0,
                    unique_receivers: 0,
                    total_volume_in: 0,
                    total_volume_out: 0,
                    interacted_tokens: HashSet::new(),
                    probable_exchange: false,
                    probable_deposit_wallet: false,
                    probable_sweeper: false,
                    risk_score: 0.0,
                });

            profile.total_in_tx += 1;
            profile.total_volume_in += t.amount;

            profile.interacted_tokens.insert(t.token.clone());
        }
    }

    //
    // relationship analysis
    //

    let mut senders = HashMap::<String, HashSet<String>>::new();

    let mut receivers = HashMap::<String, HashSet<String>>::new();

    for t in transfers {
        senders
            .entry(t.to.clone())
            .or_default()
            .insert(t.from.clone());

        receivers
            .entry(t.from.clone())
            .or_default()
            .insert(t.to.clone());
    }

    for (address, profile) in profiles.iter_mut() {
        if let Some(s) = senders.get(address) {
            profile.unique_senders = s.len() as u64;
        }

        if let Some(r) = receivers.get(address) {
            profile.unique_receivers = r.len() as u64;
        }

        //
        // exchange heuristics
        //

        if profile.unique_senders >= 100 && profile.total_in_tx >= 500 {
            profile.probable_sweeper = true;
            profile.risk_score += 0.3;
        }

        if profile.unique_receivers >= 100 && profile.total_out_tx >= 500 {
            profile.probable_exchange = true;
            profile.risk_score += 0.5;
        }

        if profile.total_in_tx >= 10 && profile.total_out_tx <= 3 {
            profile.probable_deposit_wallet = true;
            profile.risk_score += 0.2;
        }

        if profile.risk_score > 1.0 {
            profile.risk_score = 1.0;
        }
    }

    profiles
}
