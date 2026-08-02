use std::collections::HashSet;
use std::sync::Arc;

use clickhouse::Client;
use serde::Deserialize;

use crate::models::tron::exchange::ExchangeAddressRow;
use crate::services::tron::aml::types::SimpleTransfer;

use super::types::ExchangeAttribution;

#[derive(Debug, Deserialize, clickhouse::Row)]
struct StoredExchangeAttributionRow {
    entity_id: String,
    exchange_name: String,
    address_role: String,
    confidence: f32,
    detection_source: String,
    first_seen_block: u64,
    last_seen_block: u64,
}

#[derive(Debug, Clone)]
pub struct ExchangeDetection {
    pub address: ExchangeAddressRow,
}

pub async fn load_exchange_attribution(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<ExchangeAttribution>> {
    let row = load_exchange_row(clickhouse, address).await?;

    Ok(row.map(|row| ExchangeAttribution {
        exchange_name: row.exchange_name,
        role: row.address_role,
        confidence: row.confidence,
        detection_source: row.detection_source,
        cluster_id: Some(row.entity_id),
    }))
}

pub async fn detect_exchange_attributions(
    clickhouse: Arc<Client>,
    _block_number: u64,
    transfers: &[SimpleTransfer],
) -> anyhow::Result<Vec<ExchangeDetection>> {
    let candidates = transfers
        .iter()
        .flat_map(|transfer| [&transfer.from, &transfer.to])
        .filter(|address| !address.is_empty())
        .cloned()
        .collect::<HashSet<_>>();
    let mut detections = Vec::new();

    for address in candidates {
        if let Some(row) = load_exchange_row(&clickhouse, &address).await? {
            detections.push(ExchangeDetection {
                address: ExchangeAddressRow {
                    address,
                    entity_id: row.entity_id,
                    exchange_name: row.exchange_name,
                    address_role: row.address_role,
                    confidence: row.confidence,
                    detection_source: row.detection_source,
                    first_seen_block: row.first_seen_block,
                    last_seen_block: row.last_seen_block,
                    is_active: 1,
                },
            });
        }
    }

    Ok(detections)
}

async fn load_exchange_row(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<StoredExchangeAttributionRow>> {
    clickhouse
        .query(
            r#"
            SELECT
                entity_id,
                exchange_name,
                address_role,
                confidence,
                detection_source,
                first_seen_block,
                last_seen_block
            FROM exchange_addresses FINAL
            WHERE address = ? AND is_active = 1
            LIMIT 1
            "#,
        )
        .bind(address)
        .fetch_optional::<StoredExchangeAttributionRow>()
        .await
        .map_err(Into::into)
}
