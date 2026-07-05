use std::sync::Arc;

use anyhow::Context;
use clickhouse::Client;
use serde::{Deserialize, Serialize};

use crate::services::tron::risk_math::clamp01;

#[derive(Debug, Clone, Serialize)]
pub struct WalletExposureSummary {
    pub address: String,
    pub score: f32,
    pub source_count: u32,
    pub path_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_hop_distance: Option<u8>,
    pub max_path_score: f32,
    pub top_sources: Vec<WalletExposureSource>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletExposureSource {
    pub source_address: String,
    pub entity_name: Option<String>,
    pub entity_type: Option<String>,
    pub source_risk_level: u8,
    pub seed_source: Option<String>,
    pub hop_distance: u8,
    pub exposure_score: f32,
    pub effective_score: f32,
    pub path_count: u64,
    pub last_tx_hash: String,
    pub last_seen_block: u64,
    pub exposure_type: String,
}

#[derive(Debug, Deserialize, clickhouse::Row)]
struct WalletExposureQueryRow {
    source_address: String,
    entity_name: String,
    entity_type: String,
    source_risk_level: u8,
    seed_source: String,
    hop_distance: u8,
    exposure_score: f64,
    path_count: u64,
    last_tx_hash: String,
    last_seen_block: u64,
    exposure_type: String,
}

impl WalletExposureSummary {
    fn empty(address: &str) -> Self {
        Self {
            address: address.to_string(),
            score: 0.0,
            source_count: 0,
            path_count: 0,
            min_hop_distance: None,
            max_path_score: 0.0,
            top_sources: Vec::new(),
            evidence: vec!["no_propagated_exposure_paths".to_string()],
        }
    }

    fn from_rows(address: &str, rows: Vec<WalletExposureQueryRow>) -> Self {
        if rows.is_empty() {
            return Self::empty(address);
        }

        let mut top_sources = rows
            .into_iter()
            .map(|row| {
                let exposure_score = clamp01(row.exposure_score as f32);
                let source_risk = normalize_seed_risk_level(row.source_risk_level);
                let effective_score = clamp01(exposure_score * source_risk);

                WalletExposureSource {
                    source_address: row.source_address,
                    entity_name: non_empty(row.entity_name),
                    entity_type: non_empty(row.entity_type),
                    source_risk_level: row.source_risk_level,
                    seed_source: non_empty(row.seed_source),
                    hop_distance: row.hop_distance,
                    exposure_score,
                    effective_score,
                    path_count: row.path_count,
                    last_tx_hash: row.last_tx_hash,
                    last_seen_block: row.last_seen_block,
                    exposure_type: row.exposure_type,
                }
            })
            .collect::<Vec<_>>();

        top_sources.sort_by(|left, right| {
            right
                .effective_score
                .partial_cmp(&left.effective_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.hop_distance.cmp(&right.hop_distance))
        });

        let source_count = top_sources.len() as u32;
        let path_count = top_sources.iter().map(|source| source.path_count).sum();
        let min_hop_distance = top_sources.iter().map(|source| source.hop_distance).min();
        let max_path_score = top_sources
            .iter()
            .map(|source| source.exposure_score)
            .fold(0.0_f32, f32::max);
        let max_effective_score = top_sources
            .iter()
            .map(|source| source.effective_score)
            .fold(0.0_f32, f32::max);

        let source_bonus = ((source_count as f32) / 5.0).min(1.0) * 0.08;
        let path_bonus = ((path_count as f32).log10().max(0.0) / 3.0).min(1.0) * 0.07;
        let hop_bonus = match min_hop_distance {
            Some(1) => 0.10,
            Some(2) => 0.06,
            Some(3) => 0.03,
            _ => 0.0,
        };
        let score = clamp01(max_effective_score * 0.75 + source_bonus + path_bonus + hop_bonus);

        let mut evidence = vec![
            format!("propagated_exposure_sources={source_count}"),
            format!("propagated_exposure_paths={path_count}"),
            format!("propagated_exposure_max_path_score={max_path_score:.2}"),
            format!("propagated_exposure_score={score:.2}"),
        ];

        if let Some(hop) = min_hop_distance {
            evidence.push(format!("propagated_exposure_min_hop={hop}"));
        }

        for source in top_sources.iter().take(3) {
            evidence.push(format!(
                "exposure_source:{} risk_level={} hop={} effective={:.2}",
                source.source_address,
                source.source_risk_level,
                source.hop_distance,
                source.effective_score
            ));
        }

        Self {
            address: address.to_string(),
            score,
            source_count,
            path_count,
            min_hop_distance,
            max_path_score,
            top_sources,
            evidence,
        }
    }
}

pub async fn load_wallet_exposure_summary(
    clickhouse: Arc<Client>,
    address: &str,
    limit: Option<u64>,
) -> anyhow::Result<WalletExposureSummary> {
    let limit = limit.unwrap_or(25).clamp(1, 100);

    let rows = clickhouse
        .query(
            r#"
            SELECT
                ae.source_address AS source_address,
                anyLast(seed.entity_name) AS entity_name,
                anyLast(seed.entity_type) AS entity_type,
                max(seed.risk_level) AS source_risk_level,
                anyLast(seed.source) AS seed_source,
                min(ae.hop_distance) AS hop_distance,
                max(ae.exposure_score) AS exposure_score,
                sum(ae.path_count) AS path_count,
                argMax(ae.last_tx_hash, ae.last_seen_block) AS last_tx_hash,
                max(ae.last_seen_block) AS last_seen_block,
                anyLast(ae.exposure_type) AS exposure_type
            FROM address_exposure AS ae
            LEFT JOIN
            (
                SELECT
                    address,
                    argMax(entity_name, created_at) AS entity_name,
                    argMax(entity_type, created_at) AS entity_type,
                    max(risk_level) AS risk_level,
                    argMax(source, created_at) AS source
                FROM exposure_seeds
                GROUP BY address
            ) AS seed
                ON seed.address = ae.source_address
            WHERE ae.exposed_address = ?
            GROUP BY ae.source_address
            ORDER BY exposure_score DESC, hop_distance ASC
            LIMIT ?
            "#,
        )
        .bind(address)
        .bind(limit)
        .fetch_all::<WalletExposureQueryRow>()
        .await
        .context("failed to load TRON propagated wallet exposure")?;

    Ok(WalletExposureSummary::from_rows(address, rows))
}

fn normalize_seed_risk_level(risk_level: u8) -> f32 {
    let normalized = match risk_level {
        0 => 0.0,
        1..=4 => f32::from(risk_level) / 4.0,
        5..=10 => f32::from(risk_level) / 10.0,
        _ => f32::from(risk_level) / 100.0,
    };

    clamp01(normalized)
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
