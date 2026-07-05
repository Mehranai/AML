use super::client::Neo4jClient;
use neo4rs::query;

#[allow(clippy::too_many_arguments)]
pub async fn upsert_wallet_with_metadata(
    neo4j: &Neo4jClient,
    address: &str,
    label: &str,
    node_type: &str,
    entity_name: Option<&str>,
    entity_type: Option<&str>,
    exchange_name: Option<&str>,
    exchange_role: Option<&str>,
    confidence: Option<f32>,
) -> anyhow::Result<()> {
    let q = query(
        "
        MERGE (w:Wallet { address: $address })
        SET w:TronAddress,
            w.label = $label,
            w.node_type = $node_type,
            w.entity_name = $entity_name,
            w.entity_type = $entity_type,
            w.exchange_name = $exchange_name,
            w.exchange_role = $exchange_role,
            w.exchange_confidence_bps = $confidence_bps
        ",
    )
    .param("address", address)
    .param("label", label)
    .param("node_type", node_type)
    .param("entity_name", entity_name.unwrap_or(""))
    .param("entity_type", entity_type.unwrap_or(""))
    .param("exchange_name", exchange_name.unwrap_or(""))
    .param("exchange_role", exchange_role.unwrap_or(""))
    .param(
        "confidence_bps",
        (confidence.unwrap_or(0.0) * 10_000.0) as i64,
    );

    neo4j
        .graph
        .run(q)
        .await
        .map_err(|err| anyhow::anyhow!("{:?}", err))?;

    apply_node_type_label(neo4j, address, node_type).await?;

    if let Some(exchange) = exchange_name
        && !exchange.is_empty()
    {
        upsert_exchange(
            neo4j,
            address,
            exchange,
            exchange_role.unwrap_or(""),
            confidence,
        )
        .await?;
    }

    Ok(())
}

async fn apply_node_type_label(
    neo4j: &Neo4jClient,
    address: &str,
    node_type: &str,
) -> anyhow::Result<()> {
    let label_query = match node_type {
        "exchange_wallet" => "MATCH (w:Wallet { address: $address }) SET w:ExchangeWallet",
        "bridge" => "MATCH (w:Wallet { address: $address }) SET w:Bridge:Protocol",
        "mint" => "MATCH (w:Wallet { address: $address }) SET w:TokenLifecycle:Mint",
        "burn" => "MATCH (w:Wallet { address: $address }) SET w:TokenLifecycle:Burn",
        "protocol" => "MATCH (w:Wallet { address: $address }) SET w:Protocol",
        entity if entity.starts_with("exchange_") => {
            "MATCH (w:Wallet { address: $address }) SET w:ExchangeWallet"
        }
        _ => "MATCH (w:Wallet { address: $address }) SET w:ExternalWallet",
    };

    neo4j
        .graph
        .run(query(label_query).param("address", address))
        .await
        .map_err(|err| anyhow::anyhow!("failed to label Neo4j node type: {:?}", err))?;

    Ok(())
}

pub async fn upsert_exchange(
    neo4j: &Neo4jClient,
    address: &str,
    exchange: &str,
    role: &str,
    confidence: Option<f32>,
) -> anyhow::Result<()> {
    let q = query(
        "
        MERGE (e:Exchange { name: $exchange })
        SET e.entity_type = 'exchange',
            e.chain = 'tron'

        MERGE (w:Wallet { address: $address })
        SET w:TronAddress:ExchangeWallet

        MERGE (w)-[r:BELONGS_TO]->(e)
        SET r.role = $role,
            r.confidence_bps = $confidence_bps,
            r.chain = 'tron'
        ",
    )
    .param("address", address)
    .param("exchange", exchange)
    .param("role", role)
    .param(
        "confidence_bps",
        (confidence.unwrap_or(0.0) * 10_000.0) as i64,
    );

    neo4j
        .graph
        .run(q)
        .await
        .map_err(|err| anyhow::anyhow!("failed to upsert Neo4j exchange attribution: {:?}", err))?;

    Ok(())
}
