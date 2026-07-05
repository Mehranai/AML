use anyhow::{Result, anyhow};
use clickhouse::Client;
use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::helper::tron::TronClient;

//Batcher
use crate::services::tron::batcher::raw_logs::RawLogBatcher;
use crate::services::tron::batcher::relationships::RelationshipBatcher;
use crate::services::tron::batcher::token_transfers::TokenTransferBatcher;
use crate::services::tron::batcher::transactions::TransactionBatcher;

// concurency
use crate::config::AppConfig;
use crate::services::tron::batcher::contract_metadata::ContractMetadataBatcher;
use crate::services::tron::batcher::exchange_flows::ExchangeFlowBatcher;
use crate::services::tron::batcher::transaction_features::TransactionFeatureBatcher;
use crate::services::tron::batcher::transaction_risk::TransactionRiskBatcher;
use std::time::Duration;

pub struct LoaderEth {
    pub clickhouse: Arc<Client>,
    pub eth_provider: Arc<Provider<Http>>,
    pub rpc_limiter: Arc<Semaphore>,
}

impl LoaderEth {
    pub async fn new(config: &crate::config::AppConfig) -> anyhow::Result<Self> {
        let clickhouse = Arc::new(
            Client::default()
                //.with_url("tcp://clickhouse:9000")
                .with_url(&config.clickhouse_url)
                .with_user(&config.clickhouse_user)
                .with_password(&config.clickhouse_pass)
                .with_database(&config.clickhouse_db_eth),
        );

        let eth_rpc_url = config
            .eth_rpc_url
            .as_ref()
            .ok_or_else(|| anyhow!("ETH_RPC_URL or ETH_RPC_HTTP must be set for eth mode"))?;

        let rpc_limiter = Arc::new(Semaphore::new(config.rpc_max_concurrency));

        let eth_provider = Arc::new(Provider::<Http>::try_from(eth_rpc_url.as_str())?);

        Ok(Self {
            clickhouse,
            eth_provider,
            rpc_limiter,
        })
    }
}

pub struct LoaderBtc {
    pub clickhouse: Arc<Client>,
}

impl LoaderBtc {
    pub async fn new(config: &crate::config::AppConfig) -> anyhow::Result<Self> {
        let clickhouse = Arc::new(
            Client::default()
                .with_url(&config.clickhouse_url)
                .with_user(&config.clickhouse_user)
                .with_password(&config.clickhouse_pass)
                .with_database(&config.clickhouse_db_btc),
        );

        Ok(Self { clickhouse })
    }
}

pub struct LoaderBsc {
    pub clickhouse: Arc<Client>,
    pub bsc_provider: Arc<Provider<Http>>,
    pub rpc_limiter: Arc<Semaphore>,
}

impl LoaderBsc {
    pub async fn new(config: &crate::config::AppConfig) -> anyhow::Result<Self> {
        let clickhouse = Arc::new(
            Client::default()
                .with_url(&config.clickhouse_url)
                .with_user(&config.clickhouse_user)
                .with_password(&config.clickhouse_pass)
                .with_database(&config.clickhouse_db_bsc),
        );

        let bsc_rpc_url = config
            .bsc_rpc_url
            .as_ref()
            .ok_or_else(|| anyhow!("BSC_RPC_URL or BSC_RPC_HTTP must be set for bsc mode"))?;

        let bsc_provider = Arc::new(Provider::<Http>::try_from(bsc_rpc_url.as_str())?);

        let rpc_limiter = Arc::new(Semaphore::new(config.rpc_max_concurrency));

        Ok(Self {
            clickhouse,
            bsc_provider,
            rpc_limiter,
        })
    }
}

pub struct LoaderTron {
    pub clickhouse: Arc<Client>,
    pub tron_client: Arc<TronClient>,
    pub rpc_limiter: Arc<Semaphore>,
    pub transaction_batcher: Arc<TransactionBatcher>,
    pub token_transfer_batcher: Arc<TokenTransferBatcher>,
    pub raw_log_batcher: Arc<RawLogBatcher>,
    pub relationship_batcher: Arc<RelationshipBatcher>,
    // batcher
    pub config: Arc<AppConfig>,
    pub transaction_feature_batcher: Arc<TransactionFeatureBatcher>,
    pub transaction_risk_batcher: Arc<TransactionRiskBatcher>,
    pub contract_metadata_batcher: Arc<ContractMetadataBatcher>,
    pub exchange_flow_batcher: Arc<ExchangeFlowBatcher>,
}

impl LoaderTron {
    pub async fn new(config: &crate::config::AppConfig) -> Result<Self> {
        // ClickHouse (tron_db)
        let clickhouse = Arc::new(
            Client::default()
                .with_url(&config.clickhouse_url)
                .with_user(&config.clickhouse_user)
                .with_password(&config.clickhouse_pass)
                .with_database(&config.clickhouse_db_tron),
        );

        // Tron RPC
        let tron_rpc_url = config
            .tron_rpc_url
            .as_ref()
            .ok_or_else(|| anyhow!("TRON_RPC_URL or TRON_RPC_HTTP must be set for tron mode"))?;

        let tron_client = Arc::new(TronClient::new(
            tron_rpc_url,
            config.tron_api_key.clone(),
            config.rpc_timeout_seconds,
        )?);

        // Rate limiter
        let rpc_limiter = Arc::new(Semaphore::new(config.rpc_max_concurrency));

        // batcher
        let transaction_batcher =
            TransactionBatcher::create(clickhouse.clone(), 50_000, Duration::from_secs(1));

        let token_transfer_batcher =
            TokenTransferBatcher::create(clickhouse.clone(), 50_000, Duration::from_secs(1));

        let raw_log_batcher =
            RawLogBatcher::create(clickhouse.clone(), 50_000, Duration::from_secs(1));

        let relationship_batcher =
            RelationshipBatcher::create(clickhouse.clone(), 50_000, Duration::from_secs(1));

        let transaction_feature_batcher =
            TransactionFeatureBatcher::create(clickhouse.clone(), 10_000, Duration::from_secs(1));

        let transaction_risk_batcher =
            TransactionRiskBatcher::create(clickhouse.clone(), 10_000, Duration::from_secs(1));

        let contract_metadata_batcher =
            ContractMetadataBatcher::create(clickhouse.clone(), 10_000, Duration::from_secs(1));

        let exchange_flow_batcher =
            ExchangeFlowBatcher::create(clickhouse.clone(), 10_000, Duration::from_secs(1));

        Ok(Self {
            clickhouse,
            tron_client,
            rpc_limiter,
            transaction_batcher,
            token_transfer_batcher,
            raw_log_batcher,
            relationship_batcher,
            config: Arc::new(config.clone()),
            transaction_feature_batcher,
            transaction_risk_batcher,
            contract_metadata_batcher,
            exchange_flow_batcher,
        })
    }

    pub async fn flush_batches(&self) -> Result<()> {
        self.transaction_batcher.flush_all().await?;
        self.token_transfer_batcher.flush_all().await?;
        self.raw_log_batcher.flush_all().await?;
        self.relationship_batcher.flush_all().await?;
        self.transaction_feature_batcher.flush_all().await?;
        self.transaction_risk_batcher.flush_all().await?;
        self.contract_metadata_batcher.flush_all().await?;
        self.exchange_flow_batcher.flush_all().await?;

        Ok(())
    }
}
