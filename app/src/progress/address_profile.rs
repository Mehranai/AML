use crate::models::tron::address_profile::AddressProfileRow;
use clickhouse::Client;
use std::sync::Arc;

pub async fn save_address_profiles(
    clickhouse: Arc<Client>,
    rows: Vec<AddressProfileRow>,
) -> anyhow::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut insert = clickhouse
        .insert::<AddressProfileRow>("address_profiles")
        .await?;

    for row in rows {
        insert.write(&row).await?;
    }

    insert.end().await?;

    Ok(())
}
