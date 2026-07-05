use clickhouse::Client;
use std::sync::Arc;

use crate::models::tron::exchange::{
    AddressEntityRow, ExchangeAddressRow, ExchangeClusterRow, ExchangeDepositAddressRow,
    ExchangeEntityRow,
};
use crate::models::tron::exposure::{AddressExposureRow, ExposureSeedRow};

// --------------------------------------------------
// CONTRACT METADATA
// --------------------------------------------------

#[derive(Debug, Clone, clickhouse::Row, serde::Serialize)]
pub struct ContractMetadataRow {
    pub contract_address: String,

    pub protocol_name: String,

    pub contract_type: String,

    pub creator_address: String,

    #[serde(rename = "created_block")]
    pub created_at_block: u64,
}

// --------------------------------------------------
// EXCHANGE ATTRIBUTION
// --------------------------------------------------

pub async fn save_exchange_entity(
    clickhouse: Arc<Client>,
    row: ExchangeEntityRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<ExchangeEntityRow>("exchange_entities")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

pub async fn save_address_entity(
    clickhouse: Arc<Client>,
    row: AddressEntityRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<AddressEntityRow>("address_entity")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

pub async fn save_exchange_address(
    clickhouse: Arc<Client>,
    row: ExchangeAddressRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<ExchangeAddressRow>("exchange_addresses")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

pub async fn save_exchange_deposit_address(
    clickhouse: Arc<Client>,
    row: ExchangeDepositAddressRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<ExchangeDepositAddressRow>("exchange_deposit_addresses")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

pub async fn save_exchange_cluster(
    clickhouse: Arc<Client>,
    row: ExchangeClusterRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<ExchangeClusterRow>("exchange_clusters")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

// --------------------------------------------------
// LOW FREQUENCY UTILITIES
// --------------------------------------------------

pub async fn save_exposure_seed(
    clickhouse: Arc<Client>,
    row: ExposureSeedRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<ExposureSeedRow>("exposure_seeds")
        .await?;

    insert.write(&row).await?;

    insert.end().await?;

    Ok(())
}

pub async fn save_address_exposure(
    clickhouse: Arc<Client>,
    row: AddressExposureRow,
) -> anyhow::Result<()> {
    let mut insert = clickhouse
        .insert::<AddressExposureRow>("address_exposure")
        .await?;

    insert.write(&row).await?;

    insert.end().await?;

    Ok(())
}
