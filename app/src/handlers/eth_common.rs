use std::{str::FromStr, sync::Arc};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use clickhouse::Client;
use ethers::types::Address;

use crate::config::AppConfig;

#[derive(Debug)]
pub struct EthApiError {
    status: StatusCode,
    message: String,
}

impl EthApiError {
    pub fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    pub fn internal(err: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{err:#}"),
        }
    }
}

impl IntoResponse for EthApiError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

pub fn normalize_wallet_address(address: &str) -> Result<String, EthApiError> {
    let parsed = Address::from_str(address).map_err(|_| {
        EthApiError::bad_request(format!("invalid Ethereum wallet address: {address}"))
    })?;

    Ok(format!("{parsed:?}").to_ascii_lowercase())
}

pub fn clickhouse_client(config: &AppConfig) -> Arc<Client> {
    Arc::new(
        Client::default()
            .with_url(&config.clickhouse_url)
            .with_user(&config.clickhouse_user)
            .with_password(&config.clickhouse_pass)
            .with_database(&config.clickhouse_db_eth),
    )
}
