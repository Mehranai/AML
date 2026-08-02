use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow};
use clickhouse::types::UInt256;
use futures::stream::{self, StreamExt};
use serde_json::Value;

use crate::models::tron::modules::TransactionRow;

use crate::progress::core::save_sync_state;

use crate::models::tron::modules::TransactionFeatureRow;

use crate::services::loader::LoaderTron;
use crate::services::tron::ingestion_state::{
    FinalizedHashConflict, record_failed_block, record_ingested_block, record_ingestion_failure,
    record_processing_block, resolve_ingestion_failures, should_ingest_block,
};

// aml section
use crate::services::tron::aml::bridge_detector::detect_bridges;
use crate::services::tron::aml::liquidity_detector::detect_liquidity_events;
use crate::services::tron::aml::swap_detector::detect_swaps;
use crate::services::tron::aml::types::SimpleTransfer;

use crate::services::tron::tron_classifier::classifier::classify;
use crate::services::tron::tron_classifier::types::{ClassificationInput, ContractCategory};

use crate::services::tron::semantic_event_builder::build_semantic_event_rows;
use crate::services::tron::transaction_type::{
    TransactionSemanticsInput, classify_transaction_semantics,
};
use crate::services::tron::transfer_extractor::{
    TransferKind, extract_contract_transfers, extract_internal_transfers, extract_trc20_transfers,
    has_contract_call, primary_contract_summary, primary_method_data,
};
use chrono::Utc;

use crate::services::tron::aml::mint_burn_detector::detect_mints_and_burns;
use crate::services::tron::relationship_builder::build_relationships;

use crate::services::tron::exchange::detector::detect_exchange_attributions;
use crate::services::tron::exchange::flow_builder::build_exchange_flows;

const ZERO_ADDRESS: &str = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb";
const MAX_REPLAY_BLOCKS: u64 = 10_000;

async fn process_tx(loader: Arc<LoaderTron>, tx: Value, block_number: u64) -> Result<()> {
    let txid = tx["txID"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing txID"))?
        .to_string();

    let (contract_type, from, to, contract_address, value) = primary_contract_summary(&tx);
    let mut canonical_transfers = extract_contract_transfers(&tx, &txid);

    let receipt = {
        let _permit = loader.rpc_limiter.acquire().await?;

        loader.tron_client.get_tx_receipt(&txid).await?
    };

    if receipt["id"].as_str().is_none() {
        return Err(anyhow!("transaction receipt is not available for {txid}"));
    }

    let timestamp = tx["raw_data"]["timestamp"]
        .as_u64()
        .filter(|timestamp| *timestamp > 0)
        .ok_or_else(|| anyhow!("transaction {txid} has no valid timestamp"))?;

    let receipt_result = receipt["receipt"]["result"].as_str().unwrap_or("");

    let status = if receipt_result == "SUCCESS" { 1 } else { 0 };

    let fee = UInt256::from(receipt["fee"].as_u64().unwrap_or(0));

    let energy_fee = UInt256::from(receipt["energy_fee"].as_u64().unwrap_or(0));

    let net_fee = UInt256::from(receipt["net_fee"].as_u64().unwrap_or(0));

    let energy_usage = receipt["receipt"]["energy_usage"].as_u64().unwrap_or(0);

    let energy_usage_total = receipt["receipt"]["energy_usage_total"]
        .as_u64()
        .unwrap_or(0);

    let net_usage = receipt["receipt"]["net_usage"].as_u64().unwrap_or(0);

    if status == 1 {
        canonical_transfers.extend(extract_trc20_transfers(&receipt, &txid)?);
        canonical_transfers.extend(extract_internal_transfers(&receipt, &txid));
    } else {
        canonical_transfers.clear();
    }

    let semantic_transfers = canonical_transfers
        .iter()
        .map(|transfer| transfer.as_simple_transfer())
        .collect::<Vec<SimpleTransfer>>();
    let simple_transfers = semantic_transfers
        .iter()
        .filter(|transfer| transfer.from != ZERO_ADDRESS && transfer.to != ZERO_ADDRESS)
        .cloned()
        .collect::<Vec<_>>();

    loader
        .transaction_batcher
        .push(TransactionRow {
            tx_hash: txid.clone(),
            block_number,
            timestamp,

            from_address: from.clone(),
            to_address: to.clone(),

            contract_address: contract_address.clone(),

            contract_type: contract_type.clone(),

            amount: value,
            fee,
            energy_fee,
            net_fee,
            energy_usage,
            energy_usage_total,
            net_usage,
            status,
            memo: String::new(),
        })
        .await?;

    let mut discovered_tokens = HashSet::<String>::new();

    for transfer in &canonical_transfers {
        if transfer.kind == TransferKind::Trc20 {
            discovered_tokens.insert(transfer.asset_id.clone());
        }
    }

    // token metadata worker
    if !discovered_tokens.is_empty() {
        let updated_at_unix_ms = Utc::now().timestamp_millis().max(0) as u64;
        for token_address in discovered_tokens {
            loader
                .token_metadata_discovery_batcher
                .push(crate::models::tron::modules::TokenMetadataDiscoveryRow {
                    token_address,
                    discovered_block: block_number,
                    discovered_at_unix_ms: updated_at_unix_ms,
                })
                .await?;
        }
    }

    let classification = classify(
        &ClassificationInput {
            contract_address: contract_address.clone(),
            method_data: primary_method_data(&tx),
        },
        &semantic_transfers,
    );

    let is_contract_call = match classification.category {
        ContractCategory::Dex | ContractCategory::Bridge | ContractCategory::Lending => 1,

        _ => {
            if has_contract_call(&tx) {
                1
            } else {
                0
            }
        }
    };

    // AML features
    if !semantic_transfers.is_empty() {
        let semantic_actor = (!from.is_empty()).then_some(from.as_str());
        let liquidity_events = detect_liquidity_events(&semantic_transfers, semantic_actor);
        let raw_swaps = detect_swaps(&semantic_transfers, semantic_actor);
        let swaps = if liquidity_events.is_empty() {
            raw_swaps
        } else {
            Vec::new()
        };
        let mint_burns = detect_mints_and_burns(&semantic_transfers);
        let bridge_protocol_hint = classification.category == ContractCategory::Bridge;
        let bridges = detect_bridges(&semantic_transfers, bridge_protocol_hint);

        let unique_tokens = semantic_transfers
            .iter()
            .map(|t| t.token.clone())
            .collect::<HashSet<_>>()
            .len() as u16;
        let fan_in = semantic_transfers
            .iter()
            .map(|t| t.from.clone())
            .filter(|address| address != ZERO_ADDRESS)
            .collect::<HashSet<_>>()
            .len() as u16;
        let fan_out = semantic_transfers
            .iter()
            .map(|t| t.to.clone())
            .filter(|address| address != ZERO_ADDRESS)
            .collect::<HashSet<_>>()
            .len() as u16;

        let participants = semantic_transfers
            .iter()
            .flat_map(|t| vec![t.from.clone(), t.to.clone()])
            .filter(|address| address != ZERO_ADDRESS)
            .collect::<HashSet<_>>()
            .len() as u16;

        let mut aml_events = Vec::new();
        aml_events.extend(swaps.clone());
        aml_events.extend(bridges.clone());
        aml_events.extend(mint_burns.clone());
        aml_events.extend(liquidity_events.clone());

        for event in build_semantic_event_rows(
            &txid,
            block_number,
            timestamp,
            &aml_events,
            &classification.protocol,
            &classification.detection_source,
            classification.confidence,
        ) {
            loader.semantic_event_batcher.push(event).await?;
        }

        let relationships =
            build_relationships(&txid, block_number, timestamp, &canonical_transfers);

        for row in relationships {
            loader.relationship_batcher.push(row).await?;
        }

        let exchange_detections = detect_exchange_attributions(
            loader.clickhouse.clone(),
            block_number,
            &simple_transfers,
        )
        .await?;

        let exchange_flows = build_exchange_flows(
            &loader.clickhouse,
            &txid,
            block_number,
            &simple_transfers,
            &exchange_detections,
        )
        .await?;

        let semantics = classify_transaction_semantics(TransactionSemanticsInput {
            classification: &classification,
            contract_type: &contract_type,
            is_contract_call: is_contract_call == 1,
            transfers: &semantic_transfers,
            swaps: &swaps,
            bridges: &bridges,
            mint_burns: &mint_burns,
            liquidity_events: &liquidity_events,
            exchange_flows: &exchange_flows,
        });

        let feature = TransactionFeatureRow {
            tx_hash: txid.clone(),
            block_number,
            timestamp,
            transaction_type: semantics.transaction_type.clone(),
            transaction_subtype: semantics.transaction_subtype.clone(),
            classification_confidence: semantics.confidence,
            classification_source: semantics.source.clone(),
            protocol: semantics.protocol.clone(),
            method_id: semantics.method_id.clone(),
            is_swap: semantics.is_swap,
            is_bridge: semantics.is_bridge,
            is_mint: semantics.is_mint,
            is_burn: semantics.is_burn,
            is_liquidity_add: semantics.is_liquidity_add,
            is_liquidity_remove: semantics.is_liquidity_remove,
            is_contract_call,
            unique_tokens,
            participants,
            hop_count: semantic_transfers.len() as u16,
            fan_in,
            fan_out,
        };

        loader.transaction_feature_batcher.push(feature).await?;

        for flow in exchange_flows {
            loader.exchange_flow_batcher.push(flow).await?;
        }
    } else {
        let participants = [from.as_str(), to.as_str()]
            .into_iter()
            .filter(|address| !address.is_empty())
            .collect::<HashSet<_>>()
            .len() as u16;
        let semantics = classify_transaction_semantics(TransactionSemanticsInput {
            classification: &classification,
            contract_type: &contract_type,
            is_contract_call: is_contract_call == 1,
            transfers: &simple_transfers,
            swaps: &[],
            bridges: &[],
            mint_burns: &[],
            liquidity_events: &[],
            exchange_flows: &[],
        });
        loader
            .transaction_feature_batcher
            .push(TransactionFeatureRow {
                tx_hash: txid.clone(),
                block_number,
                timestamp,
                transaction_type: semantics.transaction_type.clone(),
                transaction_subtype: semantics.transaction_subtype.clone(),
                classification_confidence: semantics.confidence,
                classification_source: semantics.source.clone(),
                protocol: semantics.protocol.clone(),
                method_id: semantics.method_id.clone(),
                is_swap: semantics.is_swap,
                is_bridge: semantics.is_bridge,
                is_mint: semantics.is_mint,
                is_burn: semantics.is_burn,
                is_liquidity_add: semantics.is_liquidity_add,
                is_liquidity_remove: semantics.is_liquidity_remove,
                is_contract_call,
                unique_tokens: 0,
                participants,
                hop_count: 0,
                fan_in: (!from.is_empty()) as u16,
                fan_out: (!to.is_empty()) as u16,
            })
            .await?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct FetchOptions {
    end_block: Option<u64>,
    force_replay: bool,
    advance_checkpoint: bool,
}

pub async fn fetch_tron(loader: Arc<LoaderTron>, start_block: u64, total_txs: u64) -> Result<()> {
    fetch_tron_with_options(
        loader,
        start_block,
        total_txs,
        FetchOptions {
            end_block: None,
            force_replay: false,
            advance_checkpoint: true,
        },
    )
    .await
}

pub async fn replay_tron_range(
    loader: Arc<LoaderTron>,
    start_block: u64,
    end_block: u64,
) -> Result<()> {
    validate_replay_range(start_block, end_block)?;

    fetch_tron_with_options(
        loader,
        start_block,
        0,
        FetchOptions {
            end_block: Some(end_block),
            force_replay: true,
            advance_checkpoint: false,
        },
    )
    .await
}

pub async fn ingest_tron_range(
    loader: Arc<LoaderTron>,
    start_block: u64,
    end_block: u64,
) -> Result<()> {
    validate_replay_range(start_block, end_block)?;

    fetch_tron_with_options(
        loader,
        start_block,
        0,
        FetchOptions {
            end_block: Some(end_block),
            force_replay: false,
            advance_checkpoint: false,
        },
    )
    .await
}

async fn fetch_tron_with_options(
    loader: Arc<LoaderTron>,
    start_block: u64,
    total_txs: u64,
    options: FetchOptions,
) -> Result<()> {
    let latest_block = loader.tron_client.get_solid_block_number().await?;
    let end_block = options.end_block.unwrap_or(latest_block);

    if end_block > latest_block {
        return Err(anyhow!(
            "requested TRON end block {end_block} is above latest solid block {latest_block}"
        ));
    }

    println!(
        "TRON Latest Solid Block: {} | processing range {}..={}",
        latest_block, start_block, end_block
    );

    let mut tx_count = 0u64;

    let mut current_block = start_block;

    while current_block <= end_block {
        if total_txs > 0 && tx_count >= total_txs {
            break;
        }

        let block_result = {
            let _permit = loader.rpc_limiter.acquire().await?;

            loader.tron_client.get_block(current_block).await
        };
        let block = match block_result {
            Ok(block) => block,
            Err(error) => {
                record_error(
                    &loader.clickhouse,
                    current_block,
                    "",
                    "",
                    "FETCH_BLOCK",
                    &error,
                )
                .await?;
                return Err(error)
                    .with_context(|| format!("failed to fetch TRON block {current_block}"));
            }
        };

        let empty_txs = Vec::new();

        let txs = block["transactions"].as_array().unwrap_or(&empty_txs);
        let block_hash_result = block["blockID"]
            .as_str()
            .filter(|hash| !hash.is_empty())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("TRON block {current_block} has no blockID"));
        let block_hash = match block_hash_result {
            Ok(block_hash) => block_hash,
            Err(error) => {
                record_error(
                    &loader.clickhouse,
                    current_block,
                    "",
                    "",
                    "VALIDATE_BLOCK",
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
        let parent_hash = block["block_header"]["raw_data"]["parentHash"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let block_timestamp = block["block_header"]["raw_data"]["timestamp"]
            .as_u64()
            .unwrap_or_default();

        let should_ingest = match should_ingest_block(
            &loader.clickhouse,
            current_block,
            &block_hash,
            options.force_replay,
        )
        .await
        {
            Ok(should_ingest) => should_ingest,
            Err(error) => {
                let (error_class, retryable) =
                    if error.downcast_ref::<FinalizedHashConflict>().is_some() {
                        ("HASH_CONFLICT", false)
                    } else {
                        classify_ingestion_error(&error)
                    };
                record_ingestion_failure(
                    &loader.clickhouse,
                    current_block,
                    &block_hash,
                    "",
                    "CANONICALITY",
                    error_class,
                    &format!("{error:#}"),
                    retryable,
                )
                .await?;
                return Err(error);
            }
        };

        if !should_ingest {
            resolve_ingestion_failures(&loader.clickhouse, current_block).await?;

            if options.advance_checkpoint {
                save_sync_state(loader.clickhouse.clone(), "tron", current_block).await?;
            }
            current_block += 1;
            continue;
        }

        record_processing_block(
            &loader.clickhouse,
            current_block,
            block_hash.clone(),
            parent_hash.clone(),
            block_timestamp,
            txs.len() as u32,
        )
        .await?;

        if txs.is_empty() {
            println!("[TRON] block {} has 0 transaction(s)", current_block);

            record_ingested_block(
                &loader.clickhouse,
                current_block,
                block_hash,
                parent_hash,
                block_timestamp,
                0,
            )
            .await?;

            if options.advance_checkpoint {
                save_sync_state(loader.clickhouse.clone(), "tron", current_block).await?;
            }

            current_block += 1;

            continue;
        }

        if total_txs > 0 && tx_count > 0 && tx_count.saturating_add(txs.len() as u64) > total_txs {
            println!(
                "[TRON] stopping before block {} to preserve block-level checkpoint integrity",
                current_block
            );
            break;
        }

        let tx_vec = txs.to_vec();
        let block_tx_total = tx_vec.len() as u64;

        println!(
            "[TRON] block {} fetched {} transaction(s); processing {} transaction(s)",
            current_block,
            txs.len(),
            block_tx_total
        );

        tx_count += block_tx_total;

        let processed_in_block = Arc::new(AtomicU64::new(0));

        let tx_errors = stream::iter(tx_vec)
            .map(|tx| {
                let loader_clone = loader.clone();
                let tx_hash = tx["txID"].as_str().unwrap_or_default().to_string();

                async move { (tx_hash, process_tx(loader_clone, tx, current_block).await) }
            })
            .buffer_unordered(loader.config.tx_worker_concurrency)
            .filter_map(|(tx_hash, res)| {
                let processed_in_block = processed_in_block.clone();

                async move {
                    let processed = processed_in_block.fetch_add(1, Ordering::Relaxed) + 1;

                    match res {
                        Ok(()) => {
                            if processed == 1
                                || processed.is_multiple_of(10)
                                || processed == block_tx_total
                            {
                                println!(
                                    "[TRON] block {} processed {}/{} transaction(s)",
                                    current_block, processed, block_tx_total
                                );
                            }

                            None
                        }
                        Err(err) => {
                            eprintln!(
                                "[TRON TX ERROR] block {} processed {}/{} transaction(s): {:?}",
                                current_block, processed, block_tx_total, err
                            );

                            Some((tx_hash, err))
                        }
                    }
                }
            })
            .collect::<Vec<_>>()
            .await;

        if !tx_errors.is_empty() {
            let mut first_error = None;

            for (tx_hash, error) in tx_errors {
                record_error(
                    &loader.clickhouse,
                    current_block,
                    &block_hash,
                    &tx_hash,
                    "PROCESS_TX",
                    &error,
                )
                .await?;

                if first_error.is_none() {
                    first_error = Some(error);
                }
            }

            if let Err(flush_error) = loader.flush_batches().await {
                record_error(
                    &loader.clickhouse,
                    current_block,
                    &block_hash,
                    "",
                    "FLUSH_AFTER_TX_FAILURE",
                    &flush_error,
                )
                .await?;
            }

            let first_error = first_error.expect("transaction errors are not empty");
            record_failed_block(
                &loader.clickhouse,
                current_block,
                block_hash,
                parent_hash,
                block_timestamp,
                block_tx_total as u32,
                format!("{first_error:#}"),
            )
            .await?;

            return Err(first_error)
                .with_context(|| format!("failed to process TRON block {current_block}"));
        }

        if let Err(error) = loader.flush_batches().await {
            record_error(
                &loader.clickhouse,
                current_block,
                &block_hash,
                "",
                "FLUSH_BLOCK",
                &error,
            )
            .await?;
            record_failed_block(
                &loader.clickhouse,
                current_block,
                block_hash,
                parent_hash,
                block_timestamp,
                block_tx_total as u32,
                format!("{error:#}"),
            )
            .await?;
            return Err(error)
                .with_context(|| format!("failed to flush TRON block {current_block}"));
        }

        record_ingested_block(
            &loader.clickhouse,
            current_block,
            block_hash,
            parent_hash,
            block_timestamp,
            block_tx_total as u32,
        )
        .await?;

        if options.advance_checkpoint {
            save_sync_state(loader.clickhouse.clone(), "tron", current_block).await?;
        }

        println!(
            "TRON synced block {} | total tx {}",
            current_block, tx_count
        );

        current_block += 1;
    }

    loader.flush_batches().await?;

    Ok(())
}

fn validate_replay_range(start_block: u64, end_block: u64) -> Result<()> {
    if end_block < start_block {
        return Err(anyhow!(
            "TRON replay end block {end_block} is below start block {start_block}"
        ));
    }

    let block_count = end_block
        .checked_sub(start_block)
        .and_then(|difference| difference.checked_add(1))
        .ok_or_else(|| anyhow!("TRON replay range overflowed"))?;

    if block_count > MAX_REPLAY_BLOCKS {
        return Err(anyhow!(
            "TRON replay range contains {block_count} blocks; maximum is {MAX_REPLAY_BLOCKS}"
        ));
    }

    Ok(())
}

async fn record_error(
    clickhouse: &clickhouse::Client,
    block_number: u64,
    block_hash: &str,
    tx_hash: &str,
    stage: &str,
    error: &anyhow::Error,
) -> Result<()> {
    let (error_class, retryable) = classify_ingestion_error(error);

    record_ingestion_failure(
        clickhouse,
        block_number,
        block_hash,
        tx_hash,
        stage,
        error_class,
        &format!("{error:#}"),
        retryable,
    )
    .await
}

fn classify_ingestion_error(error: &anyhow::Error) -> (&'static str, bool) {
    if error.downcast_ref::<FinalizedHashConflict>().is_some() {
        return ("HASH_CONFLICT", false);
    }

    let message = format!("{error:#}").to_ascii_lowercase();

    if [
        "timeout",
        "timed out",
        "http request",
        "connection",
        "429",
        "frequency",
        "receipt is not available",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        return ("RPC_TRANSIENT", true);
    }

    if [
        "missing",
        "no valid",
        "invalid",
        "decode",
        "deserialize",
        "malformed",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        return ("DATA_VALIDATION", false);
    }

    ("PROCESSING", true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_range_is_inclusive_and_bounded() {
        assert!(validate_replay_range(1, 10_000).is_ok());
        assert!(validate_replay_range(1, 10_001).is_err());
        assert!(validate_replay_range(2, 1).is_err());
    }

    #[test]
    fn classifies_missing_receipt_as_retryable_rpc_failure() {
        let error = anyhow!("transaction receipt is not available");

        assert_eq!(classify_ingestion_error(&error), ("RPC_TRANSIENT", true));
    }

    #[test]
    fn classifies_invalid_block_data_as_non_retryable() {
        let error = anyhow!("TRON block 10 has no valid timestamp");

        assert_eq!(classify_ingestion_error(&error), ("DATA_VALIDATION", false));
    }
}
