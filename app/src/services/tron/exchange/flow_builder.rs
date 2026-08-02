use std::collections::HashMap;

use clickhouse::Client;
use sha2::{Digest, Sha256};

use crate::models::tron::exchange::ExchangeFlowRow;
use crate::services::tron::aml::types::SimpleTransfer;
use crate::services::tron::exchange::detector::{ExchangeDetection, load_exchange_attribution};
use crate::services::tron::exchange::types::ExchangeAttribution;

pub async fn build_exchange_flows(
    clickhouse: &Client,
    tx_hash: &str,
    block_number: u64,
    transfers: &[SimpleTransfer],
    current_detections: &[ExchangeDetection],
) -> anyhow::Result<Vec<ExchangeFlowRow>> {
    let mut flows = Vec::new();
    let mut attribution_cache = current_detections
        .iter()
        .map(|detection| {
            (
                detection.address.address.clone(),
                ExchangeAttribution {
                    exchange_name: detection.address.exchange_name.clone(),
                    role: detection.address.address_role.clone(),
                    confidence: detection.address.confidence,
                    detection_source: detection.address.detection_source.clone(),
                    cluster_id: Some(detection.address.entity_id.clone()),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    for transfer in transfers {
        let from_exchange =
            exchange_for_address(clickhouse, &mut attribution_cache, &transfer.from).await?;

        let to_exchange =
            exchange_for_address(clickhouse, &mut attribution_cache, &transfer.to).await?;

        let Some((exchange, flow_type, confidence)) =
            classify_exchange_flow(from_exchange.as_ref(), to_exchange.as_ref())
        else {
            continue;
        };

        let flow_type = flow_type.to_string();
        let flow_id = exchange_flow_id(
            tx_hash,
            &transfer.from,
            &transfer.to,
            &transfer.token,
            &transfer.raw_amount.to_string(),
            &flow_type,
            &exchange.exchange_name,
        );

        flows.push(ExchangeFlowRow {
            flow_id,
            tx_hash: tx_hash.to_string(),
            block_number,
            from_address: transfer.from.clone(),
            to_address: transfer.to.clone(),
            exchange_name: exchange.exchange_name.clone(),
            flow_type,
            token_address: transfer.token.clone(),
            amount: transfer.raw_amount,
            confidence,
        });
    }

    Ok(flows)
}

fn exchange_flow_id(
    tx_hash: &str,
    from_address: &str,
    to_address: &str,
    token_address: &str,
    amount: &str,
    flow_type: &str,
    exchange_name: &str,
) -> String {
    let identity = format!(
        "{tx_hash}|{from_address}|{to_address}|{token_address}|{amount}|{flow_type}|{exchange_name}"
    );

    format!("{:x}", Sha256::digest(identity.as_bytes()))
}

async fn exchange_for_address(
    clickhouse: &Client,
    attribution_cache: &mut HashMap<String, ExchangeAttribution>,
    address: &str,
) -> anyhow::Result<Option<ExchangeAttribution>> {
    if let Some(exchange) = attribution_cache.get(address) {
        return Ok(Some(exchange.clone()));
    }

    let exchange = load_exchange_attribution(clickhouse, address).await?;

    if let Some(exchange) = &exchange {
        attribution_cache.insert(address.to_string(), exchange.clone());
    }

    Ok(exchange)
}

fn classify_exchange_flow<'a>(
    from_exchange: Option<&'a ExchangeAttribution>,
    to_exchange: Option<&'a ExchangeAttribution>,
) -> Option<(&'a ExchangeAttribution, String, f32)> {
    match (from_exchange, to_exchange) {
        (None, Some(to)) => Some((to, "deposit".to_string(), to.confidence)),
        (Some(from), None) => Some((from, "withdrawal".to_string(), from.confidence)),
        (Some(from), Some(to)) if from.exchange_name == to.exchange_name => {
            let flow_type = if from.role == "DEPOSIT"
                && matches!(to.role.as_str(), "HOT" | "SWEEP" | "TREASURY" | "INTERNAL")
            {
                "sweep"
            } else {
                "internal_transfer"
            };

            Some((
                from,
                flow_type.to_string(),
                from.confidence.min(to.confidence),
            ))
        }
        (Some(from), Some(to)) => Some((
            from,
            format!("exchange_to_exchange:{}", to.exchange_name),
            from.confidence.min(to.confidence),
        )),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(exchange_name: &str, role: &str, confidence: f32) -> ExchangeAttribution {
        ExchangeAttribution {
            exchange_name: exchange_name.to_string(),
            role: role.to_string(),
            confidence,
            detection_source: "test".to_string(),
            cluster_id: None,
        }
    }

    #[test]
    fn classifies_deposit_with_exchange_on_to_side() {
        let binance = attr("Binance", "HOT", 1.0);
        let (_, flow_type, confidence) =
            classify_exchange_flow(None, Some(&binance)).expect("exchange flow");

        assert_eq!(flow_type, "deposit");
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn classifies_deposit_wallet_to_hot_wallet_as_sweep() {
        let deposit = attr("Binance", "DEPOSIT", 0.9);
        let hot = attr("Binance", "HOT", 1.0);
        let (_, flow_type, confidence) =
            classify_exchange_flow(Some(&deposit), Some(&hot)).expect("exchange flow");

        assert_eq!(flow_type, "sweep");
        assert_eq!(confidence, 0.9);
    }

    #[test]
    fn exchange_flow_identity_is_stable_and_content_addressed() {
        let first = exchange_flow_id("tx", "from", "to", "token", "42", "deposit", "Binance");
        let replay = exchange_flow_id("tx", "from", "to", "token", "42", "deposit", "Binance");
        let changed = exchange_flow_id("tx", "from", "to", "token", "43", "deposit", "Binance");

        assert_eq!(first, replay);
        assert_ne!(first, changed);
    }
}
