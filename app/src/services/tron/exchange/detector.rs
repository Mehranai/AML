use std::collections::HashSet;
use std::sync::Arc;

use clickhouse::Client;
use serde::Deserialize;

use crate::models::tron::exchange::{
    ExchangeAddressRow, ExchangeClusterRow, ExchangeDepositAddressRow,
};
use crate::services::tron::aml::types::SimpleTransfer;

use super::seeds::exchange_seeds;

use super::types::{
    ExchangeAttribution, ExchangeWalletRole, exchange_entity_id, unattributed_exchange_entity_id,
    unattributed_exchange_name,
};

pub fn detect_exchange(address: &str) -> Option<ExchangeAttribution> {
    //
    // seed matching
    //

    if let Some(attr) = exchange_seeds().get(address) {
        return Some(attr.clone());
    }

    None
}

#[derive(Debug, Deserialize, clickhouse::Row)]
struct StoredExchangeAttributionRow {
    entity_id: String,
    exchange_name: String,
    role: String,
    confidence: f32,
    detection_source: String,
    #[serde(rename = "last_seen_block")]
    _last_seen_block: u64,
}

pub async fn load_exchange_attribution(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<ExchangeAttribution>> {
    if let Some(seed) = detect_exchange(address) {
        return Ok(Some(seed));
    }

    let row = clickhouse
        .query(
            r#"
            SELECT
                entity_id,
                exchange_name,
                role,
                confidence,
                detection_source,
                last_seen_block
            FROM
            (
                SELECT
                    entity_id,
                    exchange_name,
                    address_role AS role,
                    confidence,
                    detection_source,
                    last_seen_block
                FROM exchange_addresses
                WHERE address = ?
                UNION ALL
                SELECT
                    '' AS entity_id,
                    exchange_name,
                    'DEPOSIT' AS role,
                    confidence,
                    detection_method AS detection_source,
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
        .fetch_optional::<StoredExchangeAttributionRow>()
        .await?;

    Ok(row.map(|row| ExchangeAttribution {
        exchange_name: row.exchange_name.clone(),
        role: row.role,
        confidence: row.confidence,
        detection_source: row.detection_source,
        cluster_id: Some(if row.entity_id.is_empty() {
            exchange_entity_id(&row.exchange_name)
        } else {
            row.entity_id
        }),
    }))
}

#[derive(Debug, Clone)]
pub struct ExchangeDetection {
    pub address: ExchangeAddressRow,
    pub deposit: Option<ExchangeDepositAddressRow>,
    pub cluster: ExchangeClusterRow,
}

#[derive(Debug, Clone)]
struct AddressStats {
    inbound_txs: u64,
    outbound_txs: u64,
    unique_senders: u64,
    unique_receivers: u64,
    first_seen_block: u64,
    last_seen_block: u64,
}

#[derive(Debug, Clone)]
struct ExchangeCounterparty {
    address: String,
    exchange_name: String,
    role: String,
    confidence: f32,
    cluster_id: String,
    last_seen_block: u64,
    tx_count: u64,
}

const MIN_SWEEP_INBOUND_TXS: u64 = 25;
const MIN_SWEEP_UNIQUE_SENDERS: u64 = 20;
const MIN_WITHDRAW_OUTBOUND_TXS: u64 = 25;
const MIN_WITHDRAW_UNIQUE_RECEIVERS: u64 = 20;

pub async fn detect_exchange_attributions(
    clickhouse: Arc<Client>,
    block_number: u64,
    transfers: &[SimpleTransfer],
) -> anyhow::Result<Vec<ExchangeDetection>> {
    let mut candidates = HashSet::<String>::new();

    for transfer in transfers {
        candidates.insert(transfer.from.clone());
        candidates.insert(transfer.to.clone());
    }

    let mut detections = Vec::<ExchangeDetection>::new();
    let mut seen = HashSet::<String>::new();

    for address in candidates {
        if let Some(seed) = detect_exchange(&address) {
            let cluster_id = seed
                .cluster_id
                .clone()
                .unwrap_or_else(|| exchange_entity_id(&seed.exchange_name));

            push_detection(
                &mut detections,
                &mut seen,
                build_address_row(ExchangeAddressBuild {
                    address: &address,
                    entity_id: &cluster_id,
                    exchange_name: &seed.exchange_name,
                    role: &seed.role,
                    confidence: seed.confidence,
                    detection_source: &seed.detection_source,
                    first_seen_block: block_number,
                    last_seen_block: block_number,
                }),
                None,
                cluster_id,
                "seed",
            );

            continue;
        }

        let stats = load_address_stats(&clickhouse, &address).await?;
        let stats = stats_with_current_transfers(stats, &address, block_number, transfers);
        let exchange_counterparty =
            match current_exchange_counterparty(&clickhouse, &address, block_number, transfers)
                .await?
            {
                Some(counterparty) => Some(counterparty),
                None => find_exchange_counterparty(&clickhouse, &address).await?,
            };

        if let Some(counterparty) = exchange_counterparty
            && is_probable_deposit(&stats, &counterparty)
        {
            let confidence = deposit_confidence(&stats, counterparty.confidence);

            let exchange_name = counterparty.exchange_name.clone();
            let cluster_id = counterparty.cluster_id.clone();
            let row = build_address_row(ExchangeAddressBuild {
                address: &address,
                entity_id: &cluster_id,
                exchange_name: &exchange_name,
                role: &ExchangeWalletRole::Deposit.to_string(),
                confidence,
                detection_source: "deposit_sweep_to_exchange_wallet",
                first_seen_block: stats.first_seen_block,
                last_seen_block: stats.last_seen_block.max(counterparty.last_seen_block),
            });

            let deposit = ExchangeDepositAddressRow {
                address: address.clone(),
                exchange_name: exchange_name.clone(),
                hot_wallet: counterparty.address.clone(),
                confidence,
                detection_method: format!(
                    "swept_to_{}_{}",
                    counterparty.role.to_lowercase(),
                    counterparty.tx_count
                ),
                first_seen_block: stats.first_seen_block,
                last_seen_block: stats.last_seen_block.max(counterparty.last_seen_block),
            };

            push_detection(
                &mut detections,
                &mut seen,
                row,
                Some(deposit),
                cluster_id,
                &counterparty.address,
            );

            continue;
        }

        if let Some((role, confidence, detection_source, discovered_from)) =
            probable_exchange_role(&stats)
        {
            let exchange_name = unattributed_exchange_name(&address);
            let cluster_id = unattributed_exchange_entity_id(&address);
            let role = role.to_string();
            let row = build_address_row(ExchangeAddressBuild {
                address: &address,
                entity_id: &cluster_id,
                exchange_name: &exchange_name,
                role: &role,
                confidence,
                detection_source,
                first_seen_block: stats.first_seen_block,
                last_seen_block: stats.last_seen_block,
            });

            push_detection(
                &mut detections,
                &mut seen,
                row,
                None,
                cluster_id,
                discovered_from,
            );
        }
    }

    Ok(detections)
}

fn push_detection(
    detections: &mut Vec<ExchangeDetection>,
    seen: &mut HashSet<String>,
    address: ExchangeAddressRow,
    deposit: Option<ExchangeDepositAddressRow>,
    cluster_id: String,
    discovered_from: &str,
) {
    let key = format!("{}:{}", address.address, address.address_role);

    if !seen.insert(key) {
        return;
    }

    let cluster = ExchangeClusterRow {
        cluster_id,
        exchange_name: address.exchange_name.clone(),
        address: address.address.clone(),
        role: address.address_role.clone(),
        confidence: address.confidence,
        discovered_from: discovered_from.to_string(),
    };

    detections.push(ExchangeDetection {
        address,
        deposit,
        cluster,
    });
}

struct ExchangeAddressBuild<'a> {
    address: &'a str,
    entity_id: &'a str,
    exchange_name: &'a str,
    role: &'a str,
    confidence: f32,
    detection_source: &'a str,
    first_seen_block: u64,
    last_seen_block: u64,
}

fn build_address_row(input: ExchangeAddressBuild<'_>) -> ExchangeAddressRow {
    ExchangeAddressRow {
        address: input.address.to_string(),
        entity_id: input.entity_id.to_string(),
        exchange_name: input.exchange_name.to_string(),
        address_role: input.role.to_string(),
        confidence: input.confidence,
        detection_source: input.detection_source.to_string(),
        first_seen_block: input.first_seen_block,
        last_seen_block: input.last_seen_block,
    }
}

async fn load_address_stats(clickhouse: &Client, address: &str) -> anyhow::Result<AddressStats> {
    let row = clickhouse
        .query(
            r#"
            SELECT
                countIf(to_address = ?) AS inbound_txs,
                countIf(from_address = ?) AS outbound_txs,
                uniqExactIf(from_address, to_address = ?) AS unique_senders,
                uniqExactIf(to_address, from_address = ?) AS unique_receivers,
                min(block_number) AS first_seen_block,
                max(block_number) AS last_seen_block
            FROM address_relationships
            WHERE from_address = ? OR to_address = ?
            "#,
        )
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .fetch_one::<(u64, u64, u64, u64, u64, u64)>()
        .await?;

    Ok(AddressStats {
        inbound_txs: row.0,
        outbound_txs: row.1,
        unique_senders: row.2,
        unique_receivers: row.3,
        first_seen_block: row.4,
        last_seen_block: row.5,
    })
}

fn stats_with_current_transfers(
    mut stats: AddressStats,
    address: &str,
    block_number: u64,
    transfers: &[SimpleTransfer],
) -> AddressStats {
    let mut current_senders = HashSet::<String>::new();
    let mut current_receivers = HashSet::<String>::new();

    for transfer in transfers {
        if transfer.to == address {
            stats.inbound_txs += 1;
            current_senders.insert(transfer.from.clone());
        }

        if transfer.from == address {
            stats.outbound_txs += 1;
            current_receivers.insert(transfer.to.clone());
        }
    }

    if !current_senders.is_empty() || !current_receivers.is_empty() {
        stats.unique_senders += current_senders.len() as u64;
        stats.unique_receivers += current_receivers.len() as u64;

        if stats.first_seen_block == 0 {
            stats.first_seen_block = block_number;
        } else {
            stats.first_seen_block = stats.first_seen_block.min(block_number);
        }

        stats.last_seen_block = stats.last_seen_block.max(block_number);
    }

    stats
}

async fn find_exchange_counterparty(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<ExchangeCounterparty>> {
    let rows = clickhouse
        .query(
            r#"
            SELECT
                ar.to_address,
                ea.entity_id,
                ea.exchange_name,
                ea.address_role,
                ea.confidence,
                min(ar.block_number),
                max(ar.block_number),
                count()
            FROM address_relationships AS ar
            INNER JOIN exchange_addresses AS ea
                ON ar.to_address = ea.address
            WHERE ar.from_address = ?
              AND ea.address_role IN ('HOT', 'SWEEP', 'TREASURY', 'INTERNAL')
            GROUP BY
                ar.to_address,
                ea.entity_id,
                ea.exchange_name,
                ea.address_role,
                ea.confidence
            ORDER BY ea.confidence DESC, count() DESC, max(ar.block_number) DESC
            LIMIT 1
            "#,
        )
        .bind(address)
        .fetch_all::<(String, String, String, String, f32, u64, u64, u64)>()
        .await?;

    if let Some(row) = rows.into_iter().next() {
        return Ok(Some(ExchangeCounterparty {
            address: row.0,
            cluster_id: row.1,
            exchange_name: row.2,
            role: row.3,
            confidence: row.4,
            last_seen_block: row.6,
            tx_count: row.7,
        }));
    }

    seed_counterparty_for_recent_outgoing(clickhouse, address).await
}

async fn current_exchange_counterparty(
    clickhouse: &Client,
    address: &str,
    block_number: u64,
    transfers: &[SimpleTransfer],
) -> anyhow::Result<Option<ExchangeCounterparty>> {
    if let Some(counterparty) = current_seed_exchange_counterparty(address, block_number, transfers)
    {
        return Ok(Some(counterparty));
    }

    for transfer in transfers.iter().filter(|transfer| transfer.from == address) {
        if let Some(exchange) = load_exchange_attribution(clickhouse, &transfer.to).await?
            && is_exchange_counterparty_role(&exchange.role)
        {
            return Ok(Some(ExchangeCounterparty {
                address: transfer.to.clone(),
                cluster_id: exchange
                    .cluster_id
                    .clone()
                    .unwrap_or_else(|| exchange_entity_id(&exchange.exchange_name)),
                exchange_name: exchange.exchange_name,
                role: exchange.role,
                confidence: exchange.confidence,
                last_seen_block: block_number,
                tx_count: 1,
            }));
        }
    }

    current_inferred_exchange_counterparty(clickhouse, address, block_number, transfers).await
}

async fn current_inferred_exchange_counterparty(
    clickhouse: &Client,
    address: &str,
    block_number: u64,
    transfers: &[SimpleTransfer],
) -> anyhow::Result<Option<ExchangeCounterparty>> {
    for transfer in transfers.iter().filter(|transfer| transfer.from == address) {
        if transfer.to == address {
            continue;
        }

        let stats = load_address_stats(clickhouse, &transfer.to).await?;
        let stats = stats_with_current_transfers(stats, &transfer.to, block_number, transfers);

        if let Some((role, confidence, _, _)) = probable_exchange_role(&stats) {
            return Ok(Some(ExchangeCounterparty {
                address: transfer.to.clone(),
                cluster_id: unattributed_exchange_entity_id(&transfer.to),
                exchange_name: unattributed_exchange_name(&transfer.to),
                role: role.to_string(),
                confidence,
                last_seen_block: stats.last_seen_block.max(block_number),
                tx_count: 1,
            }));
        }
    }

    Ok(None)
}

fn current_seed_exchange_counterparty(
    address: &str,
    block_number: u64,
    transfers: &[SimpleTransfer],
) -> Option<ExchangeCounterparty> {
    let seed_map = exchange_seeds();

    transfers
        .iter()
        .filter(|transfer| transfer.from == address)
        .find_map(|transfer| {
            seed_map.get(&transfer.to).and_then(|seed| {
                if !is_exchange_counterparty_role(&seed.role) {
                    return None;
                }

                Some(ExchangeCounterparty {
                    address: transfer.to.clone(),
                    cluster_id: seed
                        .cluster_id
                        .clone()
                        .unwrap_or_else(|| exchange_entity_id(&seed.exchange_name)),
                    exchange_name: seed.exchange_name.clone(),
                    role: seed.role.clone(),
                    confidence: seed.confidence,
                    last_seen_block: block_number,
                    tx_count: 1,
                })
            })
        })
}

fn is_exchange_counterparty_role(role: &str) -> bool {
    matches!(role, "HOT" | "SWEEP" | "TREASURY" | "INTERNAL")
}

async fn seed_counterparty_for_recent_outgoing(
    clickhouse: &Client,
    address: &str,
) -> anyhow::Result<Option<ExchangeCounterparty>> {
    let seed_map = exchange_seeds();
    let rows = clickhouse
        .query(
            r#"
            SELECT
                to_address,
                min(block_number),
                max(block_number),
                count()
            FROM address_relationships
            WHERE from_address = ?
            GROUP BY to_address
            ORDER BY max(block_number) DESC
            LIMIT 50
            "#,
        )
        .bind(address)
        .fetch_all::<(String, u64, u64, u64)>()
        .await?;

    for row in rows {
        if let Some(seed) = seed_map.get(&row.0) {
            return Ok(Some(ExchangeCounterparty {
                address: row.0,
                cluster_id: seed
                    .cluster_id
                    .clone()
                    .unwrap_or_else(|| exchange_entity_id(&seed.exchange_name)),
                exchange_name: seed.exchange_name.clone(),
                role: seed.role.clone(),
                confidence: seed.confidence,
                last_seen_block: row.2,
                tx_count: row.3,
            }));
        }
    }

    Ok(None)
}

fn is_probable_deposit(stats: &AddressStats, counterparty: &ExchangeCounterparty) -> bool {
    let sends_to_exchange = counterparty.tx_count > 0;
    let low_outbound_diversity = stats.unique_receivers <= 2;
    let has_customer_side = stats.inbound_txs >= 2 || stats.unique_senders >= 2;
    let mostly_sweeps_to_exchange =
        stats.outbound_txs <= 10 && stats.outbound_txs <= stats.inbound_txs.saturating_add(2);

    sends_to_exchange
        && stats.outbound_txs > 0
        && low_outbound_diversity
        && has_customer_side
        && mostly_sweeps_to_exchange
}

fn is_probable_sweep_wallet(stats: &AddressStats) -> bool {
    stats.inbound_txs >= MIN_SWEEP_INBOUND_TXS
        && stats.unique_senders >= MIN_SWEEP_UNIQUE_SENDERS
        && stats.unique_senders > stats.unique_receivers.saturating_mul(3)
}

fn is_probable_hot_wallet(stats: &AddressStats) -> bool {
    stats.inbound_txs >= MIN_SWEEP_INBOUND_TXS
        && stats.outbound_txs >= MIN_WITHDRAW_OUTBOUND_TXS
        && stats.unique_senders >= MIN_SWEEP_UNIQUE_SENDERS
        && stats.unique_receivers >= MIN_WITHDRAW_UNIQUE_RECEIVERS
}

fn is_probable_withdraw_wallet(stats: &AddressStats) -> bool {
    stats.outbound_txs >= MIN_WITHDRAW_OUTBOUND_TXS
        && stats.unique_receivers >= MIN_WITHDRAW_UNIQUE_RECEIVERS
        && stats.unique_receivers > stats.unique_senders.saturating_mul(3)
}

fn probable_exchange_role(
    stats: &AddressStats,
) -> Option<(ExchangeWalletRole, f32, &'static str, &'static str)> {
    if is_probable_hot_wallet(stats) {
        return Some((
            ExchangeWalletRole::Hot,
            hot_wallet_confidence(stats),
            "high_fan_in_and_fan_out_exchange_hot_wallet",
            "fan_in_fan_out",
        ));
    }

    if is_probable_sweep_wallet(stats) {
        return Some((
            ExchangeWalletRole::Sweep,
            sweeper_confidence(stats),
            "many_deposit_wallets_to_one_sweeper",
            "inbound_fan_in",
        ));
    }

    if is_probable_withdraw_wallet(stats) {
        return Some((
            ExchangeWalletRole::Withdraw,
            withdraw_confidence(stats),
            "one_wallet_to_many_withdrawals",
            "outbound_fan_out",
        ));
    }

    None
}

fn hot_wallet_confidence(stats: &AddressStats) -> f32 {
    let mut score: f32 = 0.60;

    if stats.unique_senders >= 50 {
        score += 0.10;
    }

    if stats.unique_receivers >= 50 {
        score += 0.10;
    }

    if stats.inbound_txs >= 100 && stats.outbound_txs >= 100 {
        score += 0.10;
    }

    score.min(0.90)
}

fn deposit_confidence(stats: &AddressStats, counterparty_confidence: f32) -> f32 {
    let mut score = 0.45 + (counterparty_confidence * 0.30);

    if stats.unique_receivers <= 1 {
        score += 0.10;
    }

    if stats.inbound_txs > 0 {
        score += 0.05;
    }

    if stats.outbound_txs <= 5 {
        score += 0.05;
    }

    score.min(0.95)
}

fn sweeper_confidence(stats: &AddressStats) -> f32 {
    let mut score: f32 = 0.50;

    if stats.unique_senders >= 50 {
        score += 0.15;
    }

    if stats.inbound_txs >= 100 {
        score += 0.10;
    }

    if stats.unique_receivers <= 10 {
        score += 0.05;
    }

    score.min(0.85)
}

fn withdraw_confidence(stats: &AddressStats) -> f32 {
    let mut score: f32 = 0.45;

    if stats.unique_receivers >= 50 {
        score += 0.15;
    }

    if stats.outbound_txs >= 100 {
        score += 0.10;
    }

    if stats.unique_senders <= 10 {
        score += 0.05;
    }

    score.min(0.80)
}

pub fn build_deposit_attribution(exchange_name: &str) -> ExchangeAttribution {
    ExchangeAttribution {
        exchange_name: exchange_name.to_string(),

        role: ExchangeWalletRole::Deposit.to_string(),

        confidence: 0.65,

        detection_source: "deposit_heuristic".to_string(),

        cluster_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(
        inbound_txs: u64,
        outbound_txs: u64,
        unique_senders: u64,
        unique_receivers: u64,
    ) -> AddressStats {
        AddressStats {
            inbound_txs,
            outbound_txs,
            unique_senders,
            unique_receivers,
            first_seen_block: 1,
            last_seen_block: 2,
        }
    }

    #[test]
    fn classifies_hot_wallet_from_balanced_fan_in_and_fan_out() {
        let stats = stats(100, 100, 60, 60);

        assert!(is_probable_hot_wallet(&stats));
        assert!(!is_probable_sweep_wallet(&stats));
        assert!(!is_probable_withdraw_wallet(&stats));
    }

    #[test]
    fn classifies_sweep_wallet_from_large_inbound_fan_in() {
        let stats = stats(100, 5, 60, 3);

        assert!(is_probable_sweep_wallet(&stats));
        assert!(!is_probable_hot_wallet(&stats));
    }

    #[test]
    fn classifies_deposit_wallet_when_it_sweeps_to_exchange() {
        let stats = stats(2, 1, 2, 1);
        let counterparty = ExchangeCounterparty {
            address: "TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX".to_string(),
            exchange_name: "Binance".to_string(),
            role: ExchangeWalletRole::Hot.to_string(),
            confidence: 1.0,
            cluster_id: exchange_entity_id("Binance"),
            last_seen_block: 2,
            tx_count: 1,
        };

        assert!(is_probable_deposit(&stats, &counterparty));
        assert!(deposit_confidence(&stats, counterparty.confidence) >= 0.90);
    }

    #[test]
    fn does_not_classify_single_customer_deposit_as_exchange_deposit_wallet() {
        let stats = stats(1, 1, 1, 1);
        let counterparty = ExchangeCounterparty {
            address: "TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX".to_string(),
            exchange_name: "Binance".to_string(),
            role: ExchangeWalletRole::Hot.to_string(),
            confidence: 1.0,
            cluster_id: exchange_entity_id("Binance"),
            last_seen_block: 2,
            tx_count: 1,
        };

        assert!(!is_probable_deposit(&stats, &counterparty));
    }

    #[test]
    fn unattributed_exchange_clusters_are_address_scoped() {
        let first = unattributed_exchange_entity_id("TAAAA1111");
        let second = unattributed_exchange_entity_id("TBBBB2222");

        assert_ne!(first, second);
        assert!(first.starts_with("exchange:unattributed:"));
    }

    #[test]
    fn uses_current_transfer_to_seed_exchange_as_counterparty() {
        let transfers = vec![SimpleTransfer {
            token: "TRX".to_string(),
            from: "deposit_wallet".to_string(),
            to: "TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX".to_string(),
            amount: 100,
        }];

        let counterparty = current_seed_exchange_counterparty("deposit_wallet", 10, &transfers)
            .expect("seed exchange counterparty");

        assert_eq!(counterparty.exchange_name, "Binance");
        assert_eq!(counterparty.last_seen_block, 10);
    }

    #[test]
    fn current_transfers_are_counted_before_clickhouse_flush() {
        let base = AddressStats {
            inbound_txs: 0,
            outbound_txs: 0,
            unique_senders: 0,
            unique_receivers: 0,
            first_seen_block: 0,
            last_seen_block: 0,
        };
        let transfers = vec![
            SimpleTransfer {
                token: "TRX".to_string(),
                from: "customer".to_string(),
                to: "deposit_wallet".to_string(),
                amount: 10,
            },
            SimpleTransfer {
                token: "TRX".to_string(),
                from: "deposit_wallet".to_string(),
                to: "TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX".to_string(),
                amount: 10,
            },
        ];

        let stats = stats_with_current_transfers(base, "deposit_wallet", 42, &transfers);

        assert_eq!(stats.inbound_txs, 1);
        assert_eq!(stats.outbound_txs, 1);
        assert_eq!(stats.unique_senders, 1);
        assert_eq!(stats.unique_receivers, 1);
        assert_eq!(stats.first_seen_block, 42);
        assert_eq!(stats.last_seen_block, 42);
    }
}
