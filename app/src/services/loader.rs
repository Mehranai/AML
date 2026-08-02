use anyhow::{Result, anyhow};
use clickhouse::Client;
use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::helper::tron::TronClient;

use crate::services::tron::batcher::relationships::RelationshipBatcher;
use crate::services::tron::batcher::semantic_events::SemanticEventBatcher;
use crate::services::tron::batcher::token_metadata_discoveries::TokenMetadataDiscoveryBatcher;
use crate::services::tron::batcher::transactions::TransactionBatcher;

// concurency
use crate::config::AppConfig;
use crate::services::tron::batcher::exchange_flows::ExchangeFlowBatcher;
use crate::services::tron::batcher::transaction_features::TransactionFeatureBatcher;
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
    pub relationship_batcher: Arc<RelationshipBatcher>,
    pub semantic_event_batcher: Arc<SemanticEventBatcher>,
    pub token_metadata_discovery_batcher: Arc<TokenMetadataDiscoveryBatcher>,
    // batcher
    pub config: Arc<AppConfig>,
    pub transaction_feature_batcher: Arc<TransactionFeatureBatcher>,
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

        let max_batch_rows = config.tron_ingestion_batch_max_rows.clamp(100, 100_000);
        let flush_interval =
            Duration::from_secs(config.tron_ingestion_flush_interval_seconds.clamp(5, 3_600));

        let transaction_batcher =
            TransactionBatcher::create(clickhouse.clone(), max_batch_rows, flush_interval);

        let relationship_batcher =
            RelationshipBatcher::create(clickhouse.clone(), max_batch_rows, flush_interval);

        let semantic_event_batcher =
            SemanticEventBatcher::create(clickhouse.clone(), max_batch_rows, flush_interval);

        let token_metadata_discovery_batcher = TokenMetadataDiscoveryBatcher::create(
            clickhouse.clone(),
            max_batch_rows,
            flush_interval,
        );

        let transaction_feature_batcher =
            TransactionFeatureBatcher::create(clickhouse.clone(), max_batch_rows, flush_interval);

        let exchange_flow_batcher =
            ExchangeFlowBatcher::create(clickhouse.clone(), max_batch_rows, flush_interval);

        Ok(Self {
            clickhouse,
            tron_client,
            rpc_limiter,
            transaction_batcher,
            relationship_batcher,
            semantic_event_batcher,
            token_metadata_discovery_batcher,
            config: Arc::new(config.clone()),
            transaction_feature_batcher,
            exchange_flow_batcher,
        })
    }

    pub async fn flush_batches(&self) -> Result<()> {
        self.transaction_batcher.flush_all().await?;
        self.relationship_batcher.flush_all().await?;
        self.semantic_event_batcher.flush_all().await?;
        self.token_metadata_discovery_batcher.flush_all().await?;
        self.transaction_feature_batcher.flush_all().await?;
        self.exchange_flow_batcher.flush_all().await?;

        Ok(())
    }
}
