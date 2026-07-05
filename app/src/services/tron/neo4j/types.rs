use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub entity_name: Option<String>,
    pub entity_type: Option<String>,
    pub exchange_name: Option<String>,
    pub exchange_role: Option<String>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub tx_hash: String,
    pub token_address: String,
    pub amount: String,
    pub block_number: u64,
    pub timestamp: u64,
    pub transfer_type: String,
    pub operation_type: String,
    pub relationship_type: String,
    pub protocol: String,
    pub exchange_flow_type: Option<String>,
    pub exchange_name: Option<String>,
    pub exchange_confidence: Option<f32>,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeFlowSummary {
    pub exchange_name: String,
    pub exchange_role: String,
    pub address: String,
    pub direction: String,
    pub tx_hash: String,
    pub token_address: String,
    pub amount: String,
    pub block_number: u64,
    pub operation_type: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletFlowGraph {
    pub address: String,
    pub depth: u8,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub incoming_origins: Vec<FlowNode>,
    pub exchange_interactions: Vec<ExchangeFlowSummary>,
    pub neo4j: Neo4jVisualization,
}

#[derive(Debug, Clone, Serialize)]
pub struct Neo4jVisualization {
    pub browser_url: String,
    pub cypher: String,
    pub imported_wallet_nodes: usize,
    pub imported_transfer_edges: usize,
    pub imported_exchange_interactions: usize,
}
