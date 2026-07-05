use crate::models::tron::relationship::AddressRelationshipRow;

use crate::services::tron::aml::types::{AmlEvent, SimpleTransfer};

use crate::services::tron::relationship_types::RelationshipType;

pub fn build_relationships(
    tx_hash: &str,
    block_number: u64,
    timestamp: u64,
    transfers: &[SimpleTransfer],
    events: &[AmlEvent],
    protocol: &str,
    risk_score: u8,
) -> Vec<AddressRelationshipRow> {
    let mut rows = Vec::new();
    //
    // raw value-transfer edges
    //
    for (index, transfer) in transfers.iter().enumerate() {
        let transfer_type = if transfer.token == "TRX" {
            RelationshipType::NativeTransfer
        } else {
            RelationshipType::Trc20Transfer
        };

        rows.push(AddressRelationshipRow {
            relationship_id: relationship_id(
                tx_hash,
                "transfer",
                index,
                &transfer.from,
                &transfer.to,
                &transfer.token,
            ),
            from_address: transfer.from.clone(),
            to_address: transfer.to.clone(),
            token_address: transfer.token.clone(),
            tx_hash: tx_hash.to_string(),
            block_number,
            timestamp,
            amount: transfer.amount,
            transfer_type: transfer_type.to_string(),
            event_type: "transfer".to_string(),
            protocol: protocol.to_string(),
            risk_score,
            hop_count: 1,
        });
    }
    //
    // semantic AML events
    //
    for (index, event) in events.iter().enumerate() {
        match event {
            //
            // SWAPS
            //
            AmlEvent::Swap {
                user,
                token_in,
                token_out,
            } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(
                        tx_hash, "swap", index, user, protocol, token_in,
                    ),
                    from_address: user.clone(),
                    to_address: protocol.to_string(),
                    token_address: format!("{}:{}", token_in, token_out),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::Swap.to_string(),
                    event_type: "swap".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }
            //
            // BRIDGES
            //
            AmlEvent::BridgeIn { user, token } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(
                        tx_hash,
                        "bridge_in",
                        index,
                        "bridge",
                        user,
                        token,
                    ),
                    from_address: "bridge".to_string(),
                    to_address: user.clone(),
                    token_address: token.clone(),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::Bridge.to_string(),
                    event_type: "bridge_in".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }

            AmlEvent::BridgeOut { user, token } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(
                        tx_hash,
                        "bridge_out",
                        index,
                        user,
                        "bridge",
                        token,
                    ),

                    from_address: user.clone(),

                    to_address: "bridge".to_string(),

                    token_address: token.clone(),

                    tx_hash: tx_hash.to_string(),

                    block_number,

                    timestamp,

                    amount: 0,

                    transfer_type: RelationshipType::Bridge.to_string(),

                    event_type: "bridge_out".to_string(),

                    protocol: protocol.to_string(),

                    risk_score,

                    hop_count: 1,
                });
            }
            AmlEvent::LiquidityAdd {
                user,
                lp_token,
                sent_tokens,
            } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(
                        tx_hash,
                        "liquidity_add",
                        index,
                        user,
                        protocol,
                        lp_token,
                    ),
                    from_address: user.clone(),
                    to_address: protocol.to_string(),
                    token_address: format!("{}->{}", sent_tokens.join(","), lp_token),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::LiquidityAdd.to_string(),
                    event_type: "liquidity_add".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }
            AmlEvent::LiquidityRemove {
                user,
                lp_token,
                received_tokens,
            } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(
                        tx_hash,
                        "liquidity_remove",
                        index,
                        protocol,
                        user,
                        lp_token,
                    ),
                    from_address: protocol.to_string(),
                    to_address: user.clone(),
                    token_address: format!("{}->{}", lp_token, received_tokens.join(",")),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::LiquidityRemove.to_string(),
                    event_type: "liquidity_remove".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }
            AmlEvent::Mint { user, token } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(tx_hash, "mint", index, "mint", user, token),
                    from_address: "mint".to_string(),
                    to_address: user.clone(),
                    token_address: token.clone(),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::Mint.to_string(),
                    event_type: "mint".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }
            AmlEvent::Burn { user, token } => {
                rows.push(AddressRelationshipRow {
                    relationship_id: relationship_id(tx_hash, "burn", index, user, "burn", token),
                    from_address: user.clone(),
                    to_address: "burn".to_string(),
                    token_address: token.clone(),
                    tx_hash: tx_hash.to_string(),
                    block_number,
                    timestamp,
                    amount: 0,
                    transfer_type: RelationshipType::Burn.to_string(),
                    event_type: "burn".to_string(),
                    protocol: protocol.to_string(),
                    risk_score,
                    hop_count: 1,
                });
            }
        }
    }

    rows
}

fn relationship_id(
    tx_hash: &str,
    kind: &str,
    index: usize,
    from: &str,
    to: &str,
    token: &str,
) -> String {
    format!("{}:{}:{}:{}:{}:{}", tx_hash, kind, index, from, to, token)
}
