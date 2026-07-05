use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use crate::models::token_metadata::TokenMetadataRow;
use crate::progress::core::save_token_metadata;
use crate::services::loader::LoaderTron;

pub async fn fetch_token_metadata(
    loader: Arc<LoaderTron>,
    token_address: &str,
) -> Result<Option<TokenMetadataRow>> {
    let resp = {
        let _permit = loader.rpc_limiter.acquire().await?;
        loader
            .tron_client
            .post(
                "wallet/getcontract",
                serde_json::json!({
                    "value": token_address,
                    "visible": true
                }),
            )
            .await?
    };

    let name = resp["name"].as_str().unwrap_or("").to_string();

    if name.is_empty() {
        return Ok(None);
    }

    // Tron is messy → best-effort parsing
    let symbol = resp["symbol"].as_str().unwrap_or("UNKNOWN").to_string();
    let decimals = resp["decimals"].as_u64().unwrap_or(6) as u8;

    Ok(Some(TokenMetadataRow {
        token_address: token_address.to_string(),
        name,
        symbol,
        decimals,
        total_supply: "0".to_string(),
        is_verified: 0,
    }))
}

pub async fn process_new_tokens(loader: Arc<LoaderTron>, tokens: Vec<String>) -> Result<()> {
    let unique: HashSet<_> = tokens.into_iter().collect();

    for token in unique {
        if let Some(meta) = fetch_token_metadata(loader.clone(), &token).await? {
            save_token_metadata(loader.clickhouse.clone(), meta).await?;
        }
    }

    Ok(())
}
