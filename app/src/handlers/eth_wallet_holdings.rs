use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;

use crate::config::AppConfig;
use crate::handlers::eth_common::{EthApiError, clickhouse_client, normalize_wallet_address};
use crate::services::ethereum::wallet_holdings::{
    EthereumWalletHoldings, build_ethereum_wallet_holdings,
};

#[derive(Debug, Deserialize)]
pub struct EthereumWalletHoldingsQuery {
    pub limit: Option<u64>,
}

pub async fn eth_wallet_holdings(
    Path(address): Path<String>,
    Query(params): Query<EthereumWalletHoldingsQuery>,
) -> Result<Json<EthereumWalletHoldings>, EthApiError> {
    let config = AppConfig::from_env();
    let address = normalize_wallet_address(&address)?;
    let clickhouse = clickhouse_client(&config);

    let holdings = build_ethereum_wallet_holdings(clickhouse, &address, params.limit)
        .await
        .map_err(EthApiError::internal)?;

    Ok(Json(holdings))
}
