use crate::models::tron::exposure::AddressExposureRow;
use crate::services::tron::exposure::scorer::decay_score;
use anyhow::Result;
use clickhouse::Client;
use std::collections::{HashSet, VecDeque};

pub async fn propagate_exposure(
    clickhouse: &Client,
    seed_address: &str,
    max_hops: u8,
) -> Result<Vec<AddressExposureRow>> {
    let mut visited = HashSet::<String>::new();

    let mut queue = VecDeque::<(String, f64, u8)>::new();

    let mut results = Vec::<AddressExposureRow>::new();

    queue.push_back((seed_address.to_string(), 1.0, 0));

    while let Some((current, score, hops)) = queue.pop_front() {
        if hops > max_hops {
            continue;
        }

        if visited.contains(&current) {
            continue;
        }

        visited.insert(current.clone());

        let query = r#"
        SELECT
            to_address,
            tx_hash,
            block_number
        FROM address_relationships
        WHERE from_address = ?
        LIMIT 1000
        "#;

        let rows = clickhouse
            .query(query)
            .bind(&current)
            .fetch_all::<(String, String, u64)>()
            .await?;

        for (next_addr, tx_hash, block_number) in rows {
            let next_score = decay_score(score, hops + 1);

            results.push(AddressExposureRow {
                source_address: seed_address.to_string(),

                exposed_address: next_addr.clone(),

                hop_distance: hops + 1,

                exposure_score: next_score,

                path_count: 1,

                last_tx_hash: tx_hash,

                last_seen_block: block_number,

                exposure_type: "FLOW".to_string(),

                direction: "outgoing".to_string(),
            });

            queue.push_back((next_addr, next_score, hops + 1));
        }
    }

    Ok(results)
}
