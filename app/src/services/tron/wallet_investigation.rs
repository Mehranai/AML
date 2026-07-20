use std::sync::Arc;

use clickhouse::Client;
use serde::Serialize;

use crate::services::tron::{
    neo4j::{client::Neo4jClient, flow_graph::build_wallet_flow_graph, types::WalletFlowGraph},
    wallet_ai_risk::{
        WalletAiRiskAssessment, build_and_persist_wallet_ai_risk, build_disabled_wallet_ai_risk,
    },
    wallet_exposure::load_wallet_exposure_summary,
    wallet_fingerprint::{WalletFingerprint, build_wallet_fingerprint},
    wallet_holdings::{WalletHoldings, build_wallet_holdings},
};

#[derive(Debug, Clone)]
pub struct WalletInvestigationOptions {
    pub graph_depth: u8,
    pub graph_edge_limit: u64,
    pub window_days: Option<u16>,
    pub top_counterparties: Option<usize>,
    pub max_events: Option<u64>,
    pub holdings_limit: Option<u64>,
    pub ai_risk_enabled: bool,
}

impl WalletInvestigationOptions {
    pub fn new(
        graph_depth: Option<u8>,
        graph_edge_limit: Option<u64>,
        window_days: Option<u16>,
        top_counterparties: Option<usize>,
        max_events: Option<u64>,
        holdings_limit: Option<u64>,
        ai_risk_enabled: bool,
    ) -> Self {
        Self {
            graph_depth: graph_depth.unwrap_or(3).clamp(1, 6),
            graph_edge_limit: graph_edge_limit.unwrap_or(500).clamp(1, 2_000),
            window_days,
            top_counterparties,
            max_events,
            holdings_limit,
            ai_risk_enabled,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WalletInvestigation {
    pub address: String,
    pub graph: WalletFlowGraph,
    pub holdings: WalletHoldings,
    pub fingerprint: WalletFingerprint,
    pub ai_risk: WalletAiRiskAssessment,
    pub data_quality: InvestigationDataQuality,
}

#[derive(Debug, Serialize)]
pub struct InvestigationDataQuality {
    pub graph_depth: u8,
    pub graph_edge_limit: u64,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub holdings_asset_count: u64,
    pub holdings_metadata_gap_count: usize,
    pub fingerprint_event_limit: u64,
    pub fingerprint_is_truncated: bool,
    pub observed_transfers: u64,
    pub warnings: Vec<String>,
}

pub async fn build_wallet_investigation(
    clickhouse: Arc<Client>,
    neo4j: &Neo4jClient,
    address: &str,
    options: WalletInvestigationOptions,
) -> anyhow::Result<WalletInvestigation> {
    let graph = build_wallet_flow_graph(
        clickhouse.clone(),
        neo4j,
        address,
        options.graph_depth,
        options.graph_edge_limit,
    )
    .await?;

    let holdings =
        build_wallet_holdings(clickhouse.clone(), address, options.holdings_limit).await?;

    let fingerprint = build_wallet_fingerprint(
        clickhouse.clone(),
        address,
        options.window_days,
        options.top_counterparties,
        options.max_events,
    )
    .await?;

    let exposure = load_wallet_exposure_summary(clickhouse.clone(), address, Some(25)).await?;
    let ai_risk = if options.ai_risk_enabled {
        build_and_persist_wallet_ai_risk(clickhouse, &fingerprint, exposure).await?
    } else {
        build_disabled_wallet_ai_risk(&fingerprint, exposure)
    };
    let data_quality = build_data_quality(
        &graph,
        &holdings,
        &fingerprint,
        options.graph_depth,
        options.graph_edge_limit,
    );

    Ok(WalletInvestigation {
        address: address.to_string(),
        graph,
        holdings,
        fingerprint,
        ai_risk,
        data_quality,
    })
}

fn build_data_quality(
    graph: &WalletFlowGraph,
    holdings: &WalletHoldings,
    fingerprint: &WalletFingerprint,
    graph_depth: u8,
    graph_edge_limit: u64,
) -> InvestigationDataQuality {
    let mut warnings = Vec::new();

    if fingerprint.is_truncated {
        warnings.push("fingerprint_event_sample_truncated".to_string());
    }

    if fingerprint.flows.total_transfers == 0 {
        warnings.push("no_observed_flow_history".to_string());
    } else if fingerprint.flows.total_transfers < 3 {
        warnings.push("low_observed_flow_volume".to_string());
    }

    if graph.edges.len() as u64 >= graph_edge_limit {
        warnings.push("graph_edge_limit_reached".to_string());
    }

    if holdings.total_asset_count == 0 {
        warnings.push("no_indexed_wallet_holdings".to_string());
    }

    if holdings.metadata_gap_count > 0 {
        warnings.push("token_metadata_gaps_in_holdings".to_string());
    }

    if graph_depth >= 6 {
        warnings.push("graph_depth_capped".to_string());
    }

    InvestigationDataQuality {
        graph_depth,
        graph_edge_limit,
        graph_nodes: graph.nodes.len(),
        graph_edges: graph.edges.len(),
        holdings_asset_count: holdings.total_asset_count,
        holdings_metadata_gap_count: holdings.metadata_gap_count,
        fingerprint_event_limit: fingerprint.sampled_event_limit,
        fingerprint_is_truncated: fingerprint.is_truncated,
        observed_transfers: fingerprint.flows.total_transfers,
        warnings,
    }
}
