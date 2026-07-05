use super::client::Neo4jClient;
use neo4rs::query;

#[allow(clippy::too_many_arguments)]
pub async fn merge_transfer_edge(
    neo4j: &Neo4jClient,
    edge_id: &str,
    from: &str,
    to: &str,
    tx_hash: &str,
    token: &str,
    amount: &str,
    block_number: u64,
    timestamp: u64,
    risk_score: u8,
    transfer_type: &str,
    operation_type: &str,
    relationship_type: &str,
    protocol: &str,
    exchange_flow_type: Option<&str>,
    exchange_name: Option<&str>,
    exchange_confidence: Option<f32>,
) -> anyhow::Result<()> {
    let relationship_type = safe_relationship_type(relationship_type);

    let delete_previous_edge = query(
        "
        MATCH (a:Wallet { address: $from })-[old { id: $edge_id }]->(b:Wallet { address: $to })
        DELETE old
        ",
    )
    .param("from", from)
    .param("to", to)
    .param("edge_id", edge_id);

    neo4j
        .graph
        .run(delete_previous_edge)
        .await
        .map_err(|err| anyhow::anyhow!("failed to remove stale Neo4j flow edge: {:?}", err))?;

    let q = query(&format!(
        "
        MERGE (a:Wallet {{ address: $from }})
        SET a:TronAddress
        MERGE (b:Wallet {{ address: $to }})
        SET b:TronAddress
        MERGE (a)-[t:{relationship_type} {{ id: $edge_id }}]->(b)
        SET t.tx_hash = $tx_hash,
            t.token = $token,
            t.amount = $amount,
            t.block_number = $block_number,
            t.timestamp = $timestamp,
            t.risk_score = $risk_score,
            t.transfer_type = $transfer_type,
            t.operation_type = $operation_type,
            t.exchange_flow_type = $exchange_flow_type,
            t.exchange_name = $exchange_name,
            t.exchange_confidence_bps = $exchange_confidence_bps,
            t.protocol = $protocol,
            t.chain = 'tron'
        ",
    ))
    .param("from", from)
    .param("to", to)
    .param("edge_id", edge_id)
    .param("tx_hash", tx_hash)
    .param("token", token)
    .param("amount", amount)
    .param("block_number", block_number as i64)
    .param("timestamp", timestamp as i64)
    .param("risk_score", risk_score as i64)
    .param("transfer_type", transfer_type)
    .param("operation_type", operation_type)
    .param("exchange_flow_type", exchange_flow_type.unwrap_or(""))
    .param("exchange_name", exchange_name.unwrap_or(""))
    .param(
        "exchange_confidence_bps",
        (exchange_confidence.unwrap_or(0.0) * 10_000.0) as i64,
    )
    .param("protocol", protocol);

    neo4j
        .graph
        .run(q)
        .await
        .map_err(|err| anyhow::anyhow!("{:?}", err))?;

    Ok(())
}

fn safe_relationship_type(relationship_type: &str) -> &'static str {
    match relationship_type {
        "SWAP" => "SWAP",
        "BRIDGE" => "BRIDGE",
        "EXCHANGE_DEPOSIT" => "EXCHANGE_DEPOSIT",
        "EXCHANGE_WITHDRAWAL" => "EXCHANGE_WITHDRAWAL",
        "EXCHANGE_SWEEP" => "EXCHANGE_SWEEP",
        "EXCHANGE_TRANSFER" => "EXCHANGE_TRANSFER",
        "INTERNAL_TRANSFER" => "INTERNAL_TRANSFER",
        "LIQUIDITY_ADD" => "LIQUIDITY_ADD",
        "LIQUIDITY_REMOVE" => "LIQUIDITY_REMOVE",
        "MINT" => "MINT",
        "BURN" => "BURN",
        "NATIVE_TRANSFER" => "NATIVE_TRANSFER",
        "TRC20_TRANSFER" => "TRC20_TRANSFER",
        _ => "MONEY_FLOW",
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn merge_exchange_interaction(
    neo4j: &Neo4jClient,
    wallet_address: &str,
    exchange_name: &str,
    exchange_address: &str,
    exchange_role: &str,
    direction: &str,
    tx_hash: &str,
    token: &str,
    amount: &str,
    block_number: u64,
    confidence: f32,
) -> anyhow::Result<()> {
    let interaction_id = format!(
        "{}:{}:{}:{}:{}",
        wallet_address, exchange_name, exchange_address, tx_hash, direction
    );

    let q = query(
        "
        MERGE (w:Wallet { address: $wallet_address })
        SET w:TronAddress

        MERGE (exchange_wallet:Wallet { address: $exchange_address })
        SET exchange_wallet:TronAddress,
            exchange_wallet.node_type = 'exchange_wallet',
            exchange_wallet.exchange_name = $exchange_name,
            exchange_wallet.exchange_role = $exchange_role,
            exchange_wallet.exchange_confidence_bps = $confidence_bps

        MERGE (e:Exchange { name: $exchange_name })
        SET e.entity_type = 'exchange',
            e.chain = 'tron'

        MERGE (exchange_wallet)-[belongs:BELONGS_TO]->(e)
        SET belongs.role = $exchange_role,
            belongs.confidence_bps = $confidence_bps,
            belongs.chain = 'tron'

        MERGE (w)-[i:INTERACTED_WITH { id: $interaction_id }]->(e)
        SET i.direction = $direction,
            i.exchange_address = $exchange_address,
            i.exchange_role = $exchange_role,
            i.tx_hash = $tx_hash,
            i.token = $token,
            i.amount = $amount,
            i.block_number = $block_number,
            i.confidence_bps = $confidence_bps,
            i.chain = 'tron'
        ",
    )
    .param("wallet_address", wallet_address)
    .param("exchange_name", exchange_name)
    .param("exchange_address", exchange_address)
    .param("exchange_role", exchange_role)
    .param("direction", direction)
    .param("tx_hash", tx_hash)
    .param("token", token)
    .param("amount", amount)
    .param("block_number", block_number as i64)
    .param("confidence_bps", (confidence * 10_000.0) as i64)
    .param("interaction_id", interaction_id);

    neo4j
        .graph
        .run(q)
        .await
        .map_err(|err| anyhow::anyhow!("failed to merge Neo4j exchange interaction: {:?}", err))?;

    Ok(())
}
