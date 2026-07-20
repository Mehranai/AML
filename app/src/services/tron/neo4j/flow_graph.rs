use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use clickhouse::Client;
use serde::Deserialize;

use super::client::Neo4jClient;
use super::edges::{merge_exchange_interaction, merge_transfer_edge};
use super::nodes::upsert_wallet_with_metadata;
use super::types::{
    ExchangeFlowSummary, FlowEdge, FlowNode, Neo4jVisualization, WalletFlowGraph, WalletPath,
    WalletPathGraph,
};

#[derive(Debug, Clone)]
struct ExchangeMetadata {
    exchange_name: String,
    exchange_role: String,
    confidence: f32,
}

#[derive(Debug, Clone)]
struct EntityMetadata {
    entity_name: String,
    entity_type: String,
    confidence: f32,
}

#[derive(Debug, Clone, Copy)]
enum PathSearchDirection {
    Outgoing,
    Incoming,
    Any,
}

impl PathSearchDirection {
    fn from_param(value: Option<&str>) -> Self {
        match value
            .unwrap_or("outgoing")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "incoming" | "reverse" => Self::Incoming,
            "any" | "both" | "undirected" => Self::Any,
            _ => Self::Outgoing,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Outgoing => "outgoing",
            Self::Incoming => "incoming",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone)]
struct PathSearchState {
    current: String,
    node_ids: Vec<String>,
    edges: Vec<FlowEdge>,
}

#[derive(Debug, Clone)]
struct FoundPath {
    node_ids: Vec<String>,
    edges: Vec<FlowEdge>,
}

#[derive(Debug, Clone)]
struct PathSearchResult {
    paths: Vec<FoundPath>,
    searched_node_count: usize,
    truncated: bool,
}

#[derive(Debug, Clone, Deserialize, clickhouse::Row)]
struct RelationshipReadRow {
    relationship_id: String,
    from_address: String,
    to_address: String,
    token_address: String,
    tx_hash: String,
    block_number: u64,
    timestamp_unix: u64,
    amount_string: String,
    transfer_type: String,
    protocol: String,
    exchange_flow_type: String,
    exchange_name: String,
    exchange_confidence: f32,
    risk_score: u8,
}

#[derive(Debug, Clone, Deserialize, clickhouse::Row)]
struct ExchangeMetadataRow {
    exchange_name: String,
    address_role: String,
    confidence: f32,
    #[serde(rename = "last_seen_block")]
    _last_seen_block: u64,
}

#[derive(Debug, Clone, Deserialize, clickhouse::Row)]
struct EntityMetadataRow {
    entity_name: String,
    entity_type: String,
    confidence: f32,
}

pub async fn build_wallet_flow_graph(
    clickhouse: Arc<Client>,
    neo4j: &Neo4jClient,
    address: &str,
    depth: u8,
    per_address_limit: u64,
) -> anyhow::Result<WalletFlowGraph> {
    neo4j.ensure_schema().await?;

    let depth = depth.clamp(1, 6);
    let per_address_limit = per_address_limit.clamp(1, 2_000);

    let edges =
        load_relationship_neighborhood(clickhouse.clone(), address, depth, per_address_limit)
            .await?;

    let mut node_ids = HashSet::<String>::new();
    for edge in &edges {
        node_ids.insert(edge.from.clone());
        node_ids.insert(edge.to.clone());
    }
    node_ids.insert(address.to_string());

    let mut exchange_metadata = HashMap::<String, ExchangeMetadata>::new();
    let mut entity_metadata = HashMap::<String, EntityMetadata>::new();
    for node_id in &node_ids {
        if let Some(exchange) = load_exchange_metadata(&clickhouse, node_id).await? {
            exchange_metadata.insert(node_id.clone(), exchange);
        } else if let Some(entity) = load_entity_metadata(&clickhouse, node_id).await? {
            entity_metadata.insert(node_id.clone(), entity);
        }
    }

    let mut nodes = Vec::<FlowNode>::new();
    for node_id in node_ids {
        let node = build_flow_node(
            &node_id,
            exchange_metadata.get(&node_id),
            entity_metadata.get(&node_id),
            &edges,
        );

        upsert_wallet_with_metadata(
            neo4j,
            &node.id,
            &node.label,
            &node.node_type,
            node.entity_name.as_deref(),
            node.entity_type.as_deref(),
            node.exchange_name.as_deref(),
            node.exchange_role.as_deref(),
            node.confidence,
        )
        .await?;

        nodes.push(node);
    }

    for edge in &edges {
        merge_transfer_edge(
            neo4j,
            &edge.id,
            &edge.from,
            &edge.to,
            &edge.tx_hash,
            &edge.token_address,
            &edge.amount,
            edge.block_number,
            edge.timestamp,
            edge.risk_score,
            &edge.transfer_type,
            &edge.operation_type,
            &edge.relationship_type,
            &edge.protocol,
            edge.exchange_flow_type.as_deref(),
            edge.exchange_name.as_deref(),
            edge.exchange_confidence,
        )
        .await?;
    }

    let incoming_origins = incoming_origin_nodes(address, &nodes, &edges);

    let exchange_interactions = exchange_summaries(address, &edges, &exchange_metadata);

    for interaction in &exchange_interactions {
        merge_exchange_interaction(
            neo4j,
            address,
            &interaction.exchange_name,
            &interaction.address,
            &interaction.exchange_role,
            &interaction.direction,
            &interaction.tx_hash,
            &interaction.token_address,
            &interaction.amount,
            interaction.block_number,
            interaction.confidence,
        )
        .await?;
    }

    let neo4j_visualization = Neo4jVisualization {
        browser_url: "http://localhost:7474/browser/".to_string(),
        cypher: neo4j_browser_cypher(address, depth, per_address_limit),
        imported_wallet_nodes: nodes.len(),
        imported_transfer_edges: edges.len(),
        imported_exchange_interactions: exchange_interactions.len(),
    };

    Ok(WalletFlowGraph {
        address: address.to_string(),
        depth,
        nodes,
        edges,
        incoming_origins,
        exchange_interactions,
        neo4j: neo4j_visualization,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn build_wallet_path_graph(
    clickhouse: Arc<Client>,
    neo4j: &Neo4jClient,
    source_address: &str,
    target_address: &str,
    max_depth: Option<u8>,
    max_paths: Option<usize>,
    per_address_limit: Option<u64>,
    direction: Option<&str>,
) -> anyhow::Result<WalletPathGraph> {
    neo4j.ensure_schema().await?;

    let max_depth = max_depth.unwrap_or(10).clamp(1, 10);
    let max_paths = max_paths.unwrap_or(20).clamp(1, 50);
    let per_address_limit = per_address_limit.unwrap_or(500).clamp(1, 2_000);
    let direction = PathSearchDirection::from_param(direction);

    let search_result = find_wallet_paths(
        clickhouse.clone(),
        source_address,
        target_address,
        max_depth,
        max_paths,
        per_address_limit,
        direction,
    )
    .await?;

    let mut edge_by_id = HashMap::<String, FlowEdge>::new();
    let mut node_ids =
        HashSet::<String>::from([source_address.to_string(), target_address.to_string()]);
    let mut paths = Vec::<WalletPath>::new();

    for (index, path) in search_result.paths.iter().enumerate() {
        for node_id in &path.node_ids {
            node_ids.insert(node_id.clone());
        }

        for edge in &path.edges {
            node_ids.insert(edge.from.clone());
            node_ids.insert(edge.to.clone());
            edge_by_id
                .entry(edge.id.clone())
                .or_insert_with(|| edge.clone());
        }

        paths.push(WalletPath {
            path_index: index + 1,
            hop_count: path.edges.len(),
            node_ids: path.node_ids.clone(),
            edge_ids: path.edges.iter().map(|edge| edge.id.clone()).collect(),
        });
    }

    let edges = edge_by_id.into_values().collect::<Vec<_>>();
    let mut exchange_metadata = HashMap::<String, ExchangeMetadata>::new();
    let mut entity_metadata = HashMap::<String, EntityMetadata>::new();

    for node_id in &node_ids {
        if let Some(exchange) = load_exchange_metadata(&clickhouse, node_id).await? {
            exchange_metadata.insert(node_id.clone(), exchange);
        } else if let Some(entity) = load_entity_metadata(&clickhouse, node_id).await? {
            entity_metadata.insert(node_id.clone(), entity);
        }
    }

    let mut nodes = Vec::<FlowNode>::new();
    for node_id in node_ids {
        let node = build_flow_node(
            &node_id,
            exchange_metadata.get(&node_id),
            entity_metadata.get(&node_id),
            &edges,
        );

        upsert_wallet_with_metadata(
            neo4j,
            &node.id,
            &node.label,
            &node.node_type,
            node.entity_name.as_deref(),
            node.entity_type.as_deref(),
            node.exchange_name.as_deref(),
            node.exchange_role.as_deref(),
            node.confidence,
        )
        .await?;

        nodes.push(node);
    }

    for edge in &edges {
        merge_transfer_edge(
            neo4j,
            &edge.id,
            &edge.from,
            &edge.to,
            &edge.tx_hash,
            &edge.token_address,
            &edge.amount,
            edge.block_number,
            edge.timestamp,
            edge.risk_score,
            &edge.transfer_type,
            &edge.operation_type,
            &edge.relationship_type,
            &edge.protocol,
            edge.exchange_flow_type.as_deref(),
            edge.exchange_name.as_deref(),
            edge.exchange_confidence,
        )
        .await?;
    }

    let neo4j_visualization = Neo4jVisualization {
        browser_url: "http://localhost:7474/browser/".to_string(),
        cypher: neo4j_path_cypher(
            source_address,
            target_address,
            max_depth,
            max_paths,
            direction,
        ),
        imported_wallet_nodes: nodes.len(),
        imported_transfer_edges: edges.len(),
        imported_exchange_interactions: 0,
    };

    Ok(WalletPathGraph {
        address: source_address.to_string(),
        source_address: source_address.to_string(),
        target_address: target_address.to_string(),
        max_depth,
        direction: direction.as_str().to_string(),
        path_count: paths.len(),
        searched_node_count: search_result.searched_node_count,
        truncated: search_result.truncated,
        nodes,
        edges,
        paths,
        exchange_interactions: Vec::new(),
        neo4j: neo4j_visualization,
    })
}

async fn find_wallet_paths(
    clickhouse: Arc<Client>,
    source_address: &str,
    target_address: &str,
    max_depth: u8,
    max_paths: usize,
    per_address_limit: u64,
    direction: PathSearchDirection,
) -> anyhow::Result<PathSearchResult> {
    let mut queue = VecDeque::<PathSearchState>::from([PathSearchState {
        current: source_address.to_string(),
        node_ids: vec![source_address.to_string()],
        edges: Vec::new(),
    }]);
    let mut found_paths = Vec::<FoundPath>::new();
    let mut searched_node_count = 0usize;
    let mut truncated = false;
    let max_expanded_nodes = 25_000usize;

    while let Some(state) = queue.pop_front() {
        if state.edges.len() >= usize::from(max_depth) {
            continue;
        }

        if searched_node_count >= max_expanded_nodes {
            truncated = true;
            break;
        }

        searched_node_count += 1;
        let rows = load_path_relationships_for_address(
            &clickhouse,
            &state.current,
            direction,
            per_address_limit,
        )
        .await?;

        for row in rows {
            let edge = relationship_row_to_edge(row);

            let Some(next_address) = next_path_address(&edge, &state.current, direction) else {
                continue;
            };

            if state.node_ids.iter().any(|node| node == &next_address) {
                continue;
            }

            let mut next_node_ids = state.node_ids.clone();
            next_node_ids.push(next_address.clone());

            let mut next_edges = state.edges.clone();
            next_edges.push(edge);

            if next_address == target_address {
                found_paths.push(FoundPath {
                    node_ids: next_node_ids,
                    edges: next_edges,
                });

                if found_paths.len() >= max_paths {
                    truncated = true;
                    break;
                }
            } else {
                queue.push_back(PathSearchState {
                    current: next_address,
                    node_ids: next_node_ids,
                    edges: next_edges,
                });
            }
        }

        if found_paths.len() >= max_paths {
            break;
        }
    }

    Ok(PathSearchResult {
        paths: found_paths,
        searched_node_count,
        truncated,
    })
}

fn next_path_address(
    edge: &FlowEdge,
    current: &str,
    direction: PathSearchDirection,
) -> Option<String> {
    match direction {
        PathSearchDirection::Outgoing => {
            if edge.from == current {
                Some(edge.to.clone())
            } else {
                None
            }
        }
        PathSearchDirection::Incoming => {
            if edge.to == current {
                Some(edge.from.clone())
            } else {
                None
            }
        }
        PathSearchDirection::Any => {
            if edge.from == current {
                Some(edge.to.clone())
            } else if edge.to == current {
                Some(edge.from.clone())
            } else {
                None
            }
        }
    }
}

async fn load_relationship_neighborhood(
    clickhouse: Arc<Client>,
    address: &str,
    depth: u8,
    per_address_limit: u64,
) -> anyhow::Result<Vec<FlowEdge>> {
    let mut queue = VecDeque::<(String, u8)>::from([(address.to_string(), 0)]);
    let mut visited = HashSet::<String>::new();
    let mut edge_ids = HashSet::<String>::new();
    let mut edges = Vec::<FlowEdge>::new();

    while let Some((current, current_depth)) = queue.pop_front() {
        if current_depth >= depth || !visited.insert(current.clone()) {
            continue;
        }

        let rows = load_relationships_for_address(&clickhouse, &current, per_address_limit).await?;

        for row in rows {
            let edge = relationship_row_to_edge(row);

            if edge_ids.insert(edge.id.clone()) {
                if current_depth + 1 < depth {
                    if edge.from != current {
                        queue.push_back((edge.from.clone(), current_depth + 1));
                    }

                    if edge.to != current {
                        queue.push_back((edge.to.clone(), current_depth + 1));
                    }
                }

                edges.push(edge);
            }
        }
    }

    Ok(edges)
}

async fn load_relationships_for_address(
    clickhouse: &Client,
    address: &str,
    limit: u64,
) -> anyhow::Result<Vec<RelationshipReadRow>> {
    let rows = clickhouse
        .query(
            r#"
            SELECT
                ar.relationship_id,
                ar.from_address,
                ar.to_address,
                ar.token_address,
                ar.tx_hash,
                ar.block_number,
                toUInt64(ar.timestamp) AS timestamp_unix,
                toString(ar.amount) AS amount_string,
                ar.transfer_type,
                ar.protocol,
                ifNull(ef.exchange_flow_type, '') AS exchange_flow_type,
                ifNull(ef.exchange_name, '') AS exchange_name,
                ifNull(ef.exchange_confidence, toFloat32(0)) AS exchange_confidence,
                ar.risk_score
            FROM address_relationships AS ar
            LEFT JOIN
            (
                SELECT
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount,
                    any(exchange_name) AS exchange_name,
                    any(flow_type) AS exchange_flow_type,
                    max(confidence) AS exchange_confidence
                FROM exchange_flows
                GROUP BY
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount
            ) AS ef
                ON ar.tx_hash = ef.tx_hash
                AND ar.from_address = ef.from_address
                AND ar.to_address = ef.to_address
                AND ar.token_address = ef.token_address
                AND ar.amount = ef.amount
            WHERE ar.from_address = ? OR ar.to_address = ?
            ORDER BY ar.block_number DESC
            LIMIT ?
            "#,
        )
        .bind(address)
        .bind(address)
        .bind(limit)
        .fetch_all::<RelationshipReadRow>()
        .await?;

    Ok(rows)
}

async fn load_path_relationships_for_address(
    clickhouse: &Client,
    address: &str,
    direction: PathSearchDirection,
    limit: u64,
) -> anyhow::Result<Vec<RelationshipReadRow>> {
    let query = match direction {
        PathSearchDirection::Outgoing => {
            r#"
            SELECT
                ar.relationship_id,
                ar.from_address,
                ar.to_address,
                ar.token_address,
                ar.tx_hash,
                ar.block_number,
                toUInt64(ar.timestamp) AS timestamp_unix,
                toString(ar.amount) AS amount_string,
                ar.transfer_type,
                ar.protocol,
                ifNull(ef.exchange_flow_type, '') AS exchange_flow_type,
                ifNull(ef.exchange_name, '') AS exchange_name,
                ifNull(ef.exchange_confidence, toFloat32(0)) AS exchange_confidence,
                ar.risk_score
            FROM address_relationships AS ar
            LEFT JOIN
            (
                SELECT
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount,
                    any(exchange_name) AS exchange_name,
                    any(flow_type) AS exchange_flow_type,
                    max(confidence) AS exchange_confidence
                FROM exchange_flows
                GROUP BY
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount
            ) AS ef
                ON ar.tx_hash = ef.tx_hash
                AND ar.from_address = ef.from_address
                AND ar.to_address = ef.to_address
                AND ar.token_address = ef.token_address
                AND ar.amount = ef.amount
            WHERE ar.from_address = ?
            ORDER BY ar.block_number DESC
            LIMIT ?
            "#
        }
        PathSearchDirection::Incoming => {
            r#"
            SELECT
                ar.relationship_id,
                ar.from_address,
                ar.to_address,
                ar.token_address,
                ar.tx_hash,
                ar.block_number,
                toUInt64(ar.timestamp) AS timestamp_unix,
                toString(ar.amount) AS amount_string,
                ar.transfer_type,
                ar.protocol,
                ifNull(ef.exchange_flow_type, '') AS exchange_flow_type,
                ifNull(ef.exchange_name, '') AS exchange_name,
                ifNull(ef.exchange_confidence, toFloat32(0)) AS exchange_confidence,
                ar.risk_score
            FROM address_relationships AS ar
            LEFT JOIN
            (
                SELECT
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount,
                    any(exchange_name) AS exchange_name,
                    any(flow_type) AS exchange_flow_type,
                    max(confidence) AS exchange_confidence
                FROM exchange_flows
                GROUP BY
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount
            ) AS ef
                ON ar.tx_hash = ef.tx_hash
                AND ar.from_address = ef.from_address
                AND ar.to_address = ef.to_address
                AND ar.token_address = ef.token_address
                AND ar.amount = ef.amount
            WHERE ar.to_address = ?
            ORDER BY ar.block_number DESC
            LIMIT ?
            "#
        }
        PathSearchDirection::Any => {
            r#"
            SELECT
                ar.relationship_id,
                ar.from_address,
                ar.to_address,
                ar.token_address,
                ar.tx_hash,
                ar.block_number,
                toUInt64(ar.timestamp) AS timestamp_unix,
                toString(ar.amount) AS amount_string,
                ar.transfer_type,
                ar.protocol,
                ifNull(ef.exchange_flow_type, '') AS exchange_flow_type,
                ifNull(ef.exchange_name, '') AS exchange_name,
                ifNull(ef.exchange_confidence, toFloat32(0)) AS exchange_confidence,
                ar.risk_score
            FROM address_relationships AS ar
            LEFT JOIN
            (
                SELECT
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount,
                    any(exchange_name) AS exchange_name,
                    any(flow_type) AS exchange_flow_type,
                    max(confidence) AS exchange_confidence
                FROM exchange_flows
                GROUP BY
                    tx_hash,
                    from_address,
                    to_address,
                    token_address,
                    amount
            ) AS ef
                ON ar.tx_hash = ef.tx_hash
                AND ar.from_address = ef.from_address
                AND ar.to_address = ef.to_address
                AND ar.token_address = ef.token_address
                AND ar.amount = ef.amount
            WHERE ar.from_address = ? OR ar.to_address = ?
            ORDER BY ar.block_number DESC
            LIMIT ?
            "#
        }
    };

    let mut query = clickhouse.query(query).bind(address);
    if matches!(direction, PathSearchDirection::Any) {
        query = query.bind(address);
    }

    let rows = query.bind(limit).fetch_all::<RelationshipReadRow>().await?;

    Ok(rows)
}

async fn load_exchange_metadata(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<ExchangeMetadata>> {
    let row = clickhouse
        .query(
            r#"
            SELECT
                exchange_name,
                address_role,
                confidence,
                last_seen_block
            FROM
            (
                SELECT
                    exchange_name,
                    address_role,
                    confidence,
                    last_seen_block
                FROM exchange_addresses
                WHERE address = ?
                UNION ALL
                SELECT
                    exchange_name,
                    'DEPOSIT' AS address_role,
                    confidence,
                    last_seen_block
                FROM exchange_deposit_addresses
                WHERE address = ?
            )
            ORDER BY confidence DESC, last_seen_block DESC
            LIMIT 1
            "#,
        )
        .bind(address)
        .bind(address)
        .fetch_optional::<ExchangeMetadataRow>()
        .await?;

    Ok(row.map(|row| ExchangeMetadata {
        exchange_name: row.exchange_name,
        exchange_role: row.address_role,
        confidence: row.confidence,
    }))
}

async fn load_entity_metadata(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<EntityMetadata>> {
    let row = clickhouse
        .query(
            r#"
            SELECT
                entity_name,
                entity_type,
                confidence
            FROM address_entity
            WHERE address = ?
            ORDER BY confidence DESC, created_at DESC
            LIMIT 1
            "#,
        )
        .bind(address)
        .fetch_optional::<EntityMetadataRow>()
        .await?;

    Ok(row.map(|row| EntityMetadata {
        entity_name: row.entity_name,
        entity_type: row.entity_type,
        confidence: row.confidence,
    }))
}

fn relationship_row_to_edge(row: RelationshipReadRow) -> FlowEdge {
    let exchange_flow_type = non_empty_string(row.exchange_flow_type);
    let exchange_name = non_empty_string(row.exchange_name);
    let exchange_confidence = if row.exchange_confidence > 0.0 {
        Some(row.exchange_confidence)
    } else {
        None
    };
    let operation_type = exchange_flow_type
        .clone()
        .unwrap_or_else(|| row.transfer_type.clone());
    let relationship_type = neo4j_relationship_type(&operation_type, &row.transfer_type);

    FlowEdge {
        id: row.relationship_id,
        from: row.from_address,
        to: row.to_address,
        token_address: row.token_address,
        tx_hash: row.tx_hash,
        block_number: row.block_number,
        timestamp: row.timestamp_unix,
        amount: row.amount_string,
        transfer_type: row.transfer_type,
        operation_type,
        relationship_type,
        protocol: row.protocol,
        exchange_flow_type,
        exchange_name,
        exchange_confidence,
        risk_score: row.risk_score,
    }
}

fn build_flow_node(
    node_id: &str,
    exchange: Option<&ExchangeMetadata>,
    entity: Option<&EntityMetadata>,
    edges: &[FlowEdge],
) -> FlowNode {
    if let Some(exchange) = exchange {
        return FlowNode {
            id: node_id.to_string(),
            label: format!("{} ({})", exchange.exchange_name, exchange.exchange_role),
            node_type: "exchange_wallet".to_string(),
            entity_name: Some(exchange.exchange_name.clone()),
            entity_type: Some(format!(
                "exchange_{}",
                exchange.exchange_role.to_lowercase()
            )),
            exchange_name: Some(exchange.exchange_name.clone()),
            exchange_role: Some(exchange.exchange_role.clone()),
            confidence: Some(exchange.confidence),
        };
    }

    if let Some(entity) = entity {
        return FlowNode {
            id: node_id.to_string(),
            label: format!("{} ({})", entity.entity_name, entity.entity_type),
            node_type: entity.entity_type.clone(),
            entity_name: Some(entity.entity_name.clone()),
            entity_type: Some(entity.entity_type.clone()),
            exchange_name: None,
            exchange_role: None,
            confidence: Some(entity.confidence),
        };
    }

    let node_type = infer_node_type(node_id, edges);
    FlowNode {
        id: node_id.to_string(),
        label: node_label(node_id, &node_type),
        node_type,
        entity_name: None,
        entity_type: None,
        exchange_name: None,
        exchange_role: None,
        confidence: None,
    }
}

fn infer_node_type(node_id: &str, edges: &[FlowEdge]) -> String {
    if node_id.eq_ignore_ascii_case("bridge") {
        return "bridge".to_string();
    }

    if node_id.eq_ignore_ascii_case("mint") {
        return "mint".to_string();
    }

    if node_id.eq_ignore_ascii_case("burn") {
        return "burn".to_string();
    }

    if edges
        .iter()
        .any(|edge| edge.transfer_type == "bridge" && edge.protocol == node_id)
    {
        return "bridge".to_string();
    }

    if edges
        .iter()
        .any(|edge| edge.transfer_type == "swap" && edge.to == node_id)
    {
        return "protocol".to_string();
    }

    "wallet".to_string()
}

fn node_label(node_id: &str, node_type: &str) -> String {
    match node_type {
        "bridge" => "Bridge".to_string(),
        "mint" => "Mint".to_string(),
        "burn" => "Burn".to_string(),
        "protocol" => {
            if node_id.is_empty() {
                "Protocol".to_string()
            } else {
                node_id.to_string()
            }
        }
        _ => short_address(node_id),
    }
}

fn non_empty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();

    if value.is_empty() { None } else { Some(value) }
}

fn neo4j_relationship_type(operation_type: &str, transfer_type: &str) -> String {
    match operation_type {
        "swap" => "SWAP",
        "bridge" => "BRIDGE",
        "deposit" => "EXCHANGE_DEPOSIT",
        "withdrawal" => "EXCHANGE_WITHDRAWAL",
        "sweep" => "EXCHANGE_SWEEP",
        "internal_transfer" => "INTERNAL_TRANSFER",
        "liquidity_add" => "LIQUIDITY_ADD",
        "liquidity_remove" => "LIQUIDITY_REMOVE",
        "mint" => "MINT",
        "burn" => "BURN",
        operation if operation.starts_with("exchange_to_exchange") => "EXCHANGE_TRANSFER",
        _ => match transfer_type {
            "native_transfer" => "NATIVE_TRANSFER",
            "trc20_transfer" => "TRC20_TRANSFER",
            "internal_transfer" => "INTERNAL_TRANSFER",
            "mint" => "MINT",
            "burn" => "BURN",
            _ => "MONEY_FLOW",
        },
    }
    .to_string()
}

fn incoming_origin_nodes(address: &str, nodes: &[FlowNode], edges: &[FlowEdge]) -> Vec<FlowNode> {
    let direct_senders = edges
        .iter()
        .filter(|edge| edge.to == address)
        .map(|edge| edge.from.as_str())
        .collect::<HashSet<_>>();

    nodes
        .iter()
        .filter(|node| direct_senders.contains(node.id.as_str()))
        .cloned()
        .collect()
}

fn exchange_summaries(
    address: &str,
    edges: &[FlowEdge],
    metadata: &HashMap<String, ExchangeMetadata>,
) -> Vec<ExchangeFlowSummary> {
    let mut summaries = Vec::<ExchangeFlowSummary>::new();

    for edge in edges {
        if edge.from == address
            && let Some(exchange) = metadata.get(&edge.to)
        {
            summaries.push(summary_from_edge(edge, &edge.to, exchange, "outgoing"));
        }

        if edge.to == address
            && let Some(exchange) = metadata.get(&edge.from)
        {
            summaries.push(summary_from_edge(edge, &edge.from, exchange, "incoming"));
        }
    }

    summaries
}

fn summary_from_edge(
    edge: &FlowEdge,
    exchange_address: &str,
    exchange: &ExchangeMetadata,
    direction: &str,
) -> ExchangeFlowSummary {
    ExchangeFlowSummary {
        exchange_name: exchange.exchange_name.clone(),
        exchange_role: exchange.exchange_role.clone(),
        address: exchange_address.to_string(),
        direction: direction.to_string(),
        tx_hash: edge.tx_hash.clone(),
        token_address: edge.token_address.clone(),
        amount: edge.amount.clone(),
        block_number: edge.block_number,
        operation_type: edge.operation_type.clone(),
        confidence: exchange.confidence,
    }
}

fn short_address(address: &str) -> String {
    if address.len() <= 12 {
        return address.to_string();
    }

    format!("{}...{}", &address[..6], &address[address.len() - 4..])
}

pub fn neo4j_browser_cypher(address: &str, depth: u8, limit: u64) -> String {
    let safe_depth = depth.clamp(1, 6);
    let safe_limit = limit.clamp(1, 2_000);
    let escaped_address = address.replace('\\', "\\\\").replace('\'', "\\'");

    format!(
        "MATCH p = (w:Wallet {{ address: '{}' }})-[*1..{}]-(n) RETURN p LIMIT {}",
        escaped_address, safe_depth, safe_limit
    )
}

fn neo4j_path_cypher(
    source_address: &str,
    target_address: &str,
    max_depth: u8,
    limit: usize,
    direction: PathSearchDirection,
) -> String {
    let safe_depth = max_depth.clamp(1, 10);
    let safe_limit = limit.clamp(1, 50);
    let source = source_address.replace('\\', "\\\\").replace('\'', "\\'");
    let target = target_address.replace('\\', "\\\\").replace('\'', "\\'");

    match direction {
        PathSearchDirection::Incoming => format!(
            "MATCH p = (source:Wallet {{ address: '{}' }})<-[*1..{}]-(target:Wallet {{ address: '{}' }}) RETURN p LIMIT {}",
            source, safe_depth, target, safe_limit
        ),
        PathSearchDirection::Any => format!(
            "MATCH p = (source:Wallet {{ address: '{}' }})-[*1..{}]-(target:Wallet {{ address: '{}' }}) RETURN p LIMIT {}",
            source, safe_depth, target, safe_limit
        ),
        PathSearchDirection::Outgoing => format!(
            "MATCH p = (source:Wallet {{ address: '{}' }})-[*1..{}]->(target:Wallet {{ address: '{}' }}) RETURN p LIMIT {}",
            source, safe_depth, target, safe_limit
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_direct_exchange_interactions() {
        let edge = FlowEdge {
            id: "edge".to_string(),
            from: "wallet".to_string(),
            to: "exchange_wallet".to_string(),
            tx_hash: "tx".to_string(),
            token_address: "TRX".to_string(),
            amount: "100".to_string(),
            block_number: 10,
            timestamp: 1,
            transfer_type: "native_transfer".to_string(),
            operation_type: "native_transfer".to_string(),
            relationship_type: "NATIVE_TRANSFER".to_string(),
            protocol: "".to_string(),
            exchange_flow_type: None,
            exchange_name: None,
            exchange_confidence: None,
            risk_score: 0,
        };

        let metadata = HashMap::from([(
            "exchange_wallet".to_string(),
            ExchangeMetadata {
                exchange_name: "Binance".to_string(),
                exchange_role: "HOT".to_string(),
                confidence: 1.0,
            },
        )]);

        let summaries = exchange_summaries("wallet", &[edge], &metadata);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].direction, "outgoing");
        assert_eq!(summaries[0].exchange_name, "Binance");
        assert_eq!(summaries[0].operation_type, "native_transfer");
    }

    #[test]
    fn builds_browser_cypher_for_root_wallet() {
        let cypher = neo4j_browser_cypher("TAddress", 3, 500);

        assert_eq!(
            cypher,
            "MATCH p = (w:Wallet { address: 'TAddress' })-[*1..3]-(n) RETURN p LIMIT 500"
        );
    }
}
