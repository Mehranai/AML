use axum::{
    extract::{Path, Query},
    response::Json,
};
use serde::Deserialize;

use crate::config::AppConfig;
use crate::handlers::tron_common::{TronApiError, clickhouse_client, normalize_wallet_address};
use crate::services::tron::wallet_ai_risk::{
    WalletAiRiskAssessment, build_wallet_ai_risk_assessment,
};

#[derive(Debug, Deserialize)]
pub struct WalletAiRiskQuery {
    pub window_days: Option<u16>,
    pub top_counterparties: Option<usize>,
    pub max_events: Option<u64>,
}

pub async fn tron_wallet_ai_risk(
    Path(address): Path<String>,
    Query(params): Query<WalletAiRiskQuery>,
) -> Result<Json<WalletAiRiskAssessment>, TronApiError> {
    let config = AppConfig::from_env();
    let address = normalize_wallet_address(&address)?;
    let clickhouse = clickhouse_client(&config);

    let assessment = build_wallet_ai_risk_assessment(
        clickhouse,
        &address,
        params.window_days,
        params.top_counterparties,
        params.max_events,
    )
    .await
    .map_err(TronApiError::internal)?;

    Ok(Json(assessment))
}
