use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow};
use futures::stream::{self, StreamExt};
use serde_json::Value;

use crate::models::tron::modules::TransactionRiskRow;
use crate::models::tron::modules::TransactionRow;
use crate::models::tron::modules::TronRawLogRow;
use crate::models::tron::modules::TronTokenTransferRow;

use crate::progress::core::save_sync_state;

use crate::progress::progress_tron::{
    ContractMetadataRow, save_address_entity, save_exchange_address, save_exchange_cluster,
    save_exchange_deposit_address, save_exchange_entity,
};

use crate::models::tron::modules::TransactionFeatureRow;

use crate::services::loader::LoaderTron;
use crate::utils::tron_address::normalize_tron_address;

// aml section
use crate::services::tron::aml::bridge_detector::detect_bridges;
use crate::services::tron::aml::liquidity_detector::detect_liquidity_events;
use crate::services::tron::aml::swap_detector::detect_swaps;
use crate::services::tron::aml::types::SimpleTransfer;

use crate::services::tron::tron_classifier::classifier::classify;
use crate::services::tron::tron_classifier::types::{ClassificationInput, ContractCategory};

use crate::services::tron::risk_engine::compute_risk_score;
use crate::services::tron::transaction_type::{
    TransactionSemanticsInput, classify_transaction_semantics,
};
use crate::services::tron::tron_metadata_worker;

use crate::services::tron::aml::mint_burn_detector::detect_mints_and_burns;
use crate::services::tron::relationship_builder::build_relationships;

// flow detection
use crate::models::tron::exchange::{AddressEntityRow, ExchangeEntityRow};
use crate::services::tron::exchange::detector::detect_exchange_attributions;
use crate::services::tron::exchange::flow_builder::build_exchange_flows;

// intelligence system
use crate::models::tron::address_profile::AddressProfileRow;
use crate::models::tron::counterparty::CounterpartyRow;
use crate::progress::address_profile::save_address_profiles;
use crate::progress::counterparty::save_counterparties;
use crate::services::tron::address_intelligence::build_address_profiles;
use crate::services::tron::counterparty::build_counterparty_relations;

const ZERO_ADDRESS: &str = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb";

const ERC20_TRANSFER_TOPIC: &str =
    "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

fn extract_contract_type(tx: &Value) -> String {
    tx["raw_data"]["contract"][0]["type"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string()
}

fn extract_owner_address(tx: &Value) -> Option<String> {
    tx["raw_data"]["contract"][0]["parameter"]["value"]["owner_address"]
        .as_str()
        .and_then(normalize_tron_address)
}

fn extract_transfer_contract(tx: &Value) -> Option<(String, String, u64)> {
    let contract = &tx["raw_data"]["contract"][0];

    if contract["type"] != "TransferContract" {
        return None;
    }

    let value = contract["parameter"]["value"]["amount"].as_u64()?;

    let owner = contract["parameter"]["value"]["owner_address"].as_str()?;

    let to = contract["parameter"]["value"]["to_address"].as_str()?;

    let from = normalize_tron_address(owner)?;
    let to = normalize_tron_address(to)?;

    Some((from, to, value))
}

fn extract_trc20_transfers(receipt: &Value) -> Vec<(u32, String, String, String, u128)> {
    let mut transfers = Vec::new();

    let empty_logs = Vec::new();

    let logs = receipt["log"].as_array().unwrap_or(&empty_logs);

    for (i, log) in logs.iter().enumerate() {
        let empty_topics = Vec::new();

        let topics = log["topics"].as_array().unwrap_or(&empty_topics);

        if topics.len() < 3 {
            continue;
        }

        let topic0 = topics[0]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("0x")
            .trim_start_matches("0X")
            .to_lowercase();

        if topic0 != ERC20_TRANSFER_TOPIC {
            continue;
        }

        let Some(token) = log["address"].as_str().and_then(normalize_tron_address) else {
            continue;
        };

        let from = normalize_tron_address(topics[1].as_str().unwrap_or(""));

        let to = normalize_tron_address(topics[2].as_str().unwrap_or(""));

        let amount_hex = log["data"].as_str().unwrap_or("0x0");

        let amount = u128::from_str_radix(
            amount_hex.trim_start_matches("0x").trim_start_matches("0X"),
            16,
        )
        .unwrap_or(0);

        if let (Some(from), Some(to)) = (from, to) {
            transfers.push((i as u32, token, from, to, amount));
        }
    }

    transfers
}

fn extract_raw_logs(
    receipt: &Value,
    tx_hash: &str,
    block_number: u64,
    timestamp: u64,
) -> Vec<TronRawLogRow> {
    let empty_logs = Vec::new();
    let logs = receipt["log"].as_array().unwrap_or(&empty_logs);

    logs.iter()
        .enumerate()
        .map(|(i, log)| {
            let contract_address = log["address"]
                .as_str()
                .and_then(normalize_tron_address)
                .or_else(|| log["address"].as_str().map(ToString::to_string))
                .unwrap_or_default();

            let topics = log["topics"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|topic| topic.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            TronRawLogRow {
                tx_hash: tx_hash.to_string(),
                block_number,
                log_index: i as u32,
                contract_address,
                topics,
                data: log["data"].as_str().unwrap_or_default().to_string(),
                removed: 0,
                timestamp,
            }
        })
        .collect()
}

async fn process_tx(loader: Arc<LoaderTron>, tx: Value, block_number: u64) -> Result<()> {
    let txid = tx["txID"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing txID"))?
        .to_string();

    let contract_type = extract_contract_type(&tx);

    let mut from = String::new();
    let mut to = String::new();
    let mut value = 0u64;

    if let Some((f, t, v)) = extract_transfer_contract(&tx) {
        from = f;
        to = t;
        value = v;
    }

    let mut simple_transfers = Vec::<SimpleTransfer>::new();
    let mut semantic_transfers = Vec::<SimpleTransfer>::new();

    if !from.is_empty() && !to.is_empty() && value > 0 {
        let transfer = SimpleTransfer {
            token: "TRX".to_string(),

            from: from.clone(),
            to: to.clone(),

            amount: value as u128,
        };

        simple_transfers.push(transfer.clone());
        semantic_transfers.push(transfer);
    }

    let receipt = {
        let _permit = loader.rpc_limiter.acquire().await?;

        loader.tron_client.get_tx_receipt(&txid).await?
    };

    let timestamp = tx["raw_data"]["timestamp"].as_u64().unwrap_or(0);

    let receipt_result = receipt["receipt"]["result"].as_str().unwrap_or("");

    let status = if receipt_result == "SUCCESS" { 1 } else { 0 };

    let fee = receipt["fee"].as_u64().unwrap_or(0) as u128;

    let energy_fee = receipt["energy_fee"].as_u64().unwrap_or(0) as u128;

    let net_fee = receipt["net_fee"].as_u64().unwrap_or(0) as u128;

    let energy_usage = receipt["receipt"]["energy_usage"].as_u64().unwrap_or(0);

    let energy_usage_total = receipt["receipt"]["energy_usage_total"]
        .as_u64()
        .unwrap_or(0);

    let net_usage = receipt["receipt"]["net_usage"].as_u64().unwrap_or(0);

    // contract classifier
    let contract_address = tx["raw_data"]["contract"][0]["parameter"]["value"]["contract_address"]
        .as_str()
        .and_then(normalize_tron_address)
        .unwrap_or_default();
    let owner_address = extract_owner_address(&tx).unwrap_or_default();

    if from.is_empty() && !owner_address.is_empty() {
        from = owner_address.clone();
    }

    if to.is_empty() && !contract_address.is_empty() {
        to = contract_address.clone();
    }

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

            amount: value as u128,
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

    for raw_log in extract_raw_logs(&receipt, &txid, block_number, timestamp) {
        loader.raw_log_batcher.push(raw_log).await?;
    }

    let transfers = extract_trc20_transfers(&receipt);

    let mut discovered_tokens = HashSet::<String>::new();

    for (log_index, token, from_addr, to_addr, amount) in transfers {
        loader
            .token_transfer_batcher
            .push(TronTokenTransferRow {
                tx_hash: txid.clone(),
                block_number,
                timestamp,

                log_index,

                token_address: token.clone(),

                from_address: from_addr.clone(),

                to_address: to_addr.clone(),

                amount,

                is_mint: (from_addr == ZERO_ADDRESS) as u8,

                is_burn: (to_addr == ZERO_ADDRESS) as u8,

                event_signature: ERC20_TRANSFER_TOPIC.to_string(),
            })
            .await?;

        discovered_tokens.insert(token.clone());

        let semantic_transfer = SimpleTransfer {
            token,
            from: from_addr,
            to: to_addr,
            amount,
        };

        semantic_transfers.push(semantic_transfer.clone());

        if semantic_transfer.from != ZERO_ADDRESS && semantic_transfer.to != ZERO_ADDRESS {
            simple_transfers.push(semantic_transfer);
        }
    }

    // token metadata worker
    if !discovered_tokens.is_empty() {
        let tokens: Vec<String> = discovered_tokens.into_iter().collect();

        tron_metadata_worker::process_new_tokens(loader.clone(), tokens).await?;
    }

    let method_data = tx["raw_data"]["contract"][0]["parameter"]["value"]["data"]
        .as_str()
        .map(|s| s.to_string());

    let classification = classify(
        &ClassificationInput {
            contract_address: contract_address.clone(),
            method_data,
        },
        &semantic_transfers,
    );

    let is_contract_call = match classification.category {
        ContractCategory::Dex | ContractCategory::Bridge | ContractCategory::Lending => 1,

        _ => {
            if contract_type == "TriggerSmartContract" {
                1
            } else {
                0
            }
        }
    };

    // save contract metadata
    if !contract_address.is_empty() {
        let row = ContractMetadataRow {
            contract_address: contract_address.clone(),

            protocol_name: classification.protocol.clone(),

            contract_type: classification.category.to_string(),

            creator_address: from.clone(),

            created_at_block: block_number,
        };

        loader.contract_metadata_batcher.push(row).await?;
    }

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

        // risk engine
        let (risk_score, risk_level) = compute_risk_score(
            &classification,
            !swaps.is_empty(),
            !bridges.is_empty(),
            unique_tokens,
            participants,
        );

        let relationships = build_relationships(
            &txid,
            block_number,
            tx["raw_data"]["timestamp"].as_u64().unwrap_or(0),
            &simple_transfers,
            &aml_events,
            &classification.protocol,
            risk_score,
        );

        for row in relationships {
            loader.relationship_batcher.push(row).await?;
        }

        // historical address intelligence
        let profiles = build_address_profiles(&simple_transfers);
        let mut profile_rows = Vec::<AddressProfileRow>::new();

        for (_, profile) in profiles {
            profile_rows.push(AddressProfileRow {
                address: profile.address,
                total_in_tx: profile.total_in_tx,
                total_out_tx: profile.total_out_tx,
                unique_senders: profile.unique_senders,
                unique_receivers: profile.unique_receivers,
                total_volume_in: profile.total_volume_in,
                total_volume_out: profile.total_volume_out,
                interacted_tokens: profile.interacted_tokens.len() as u32,
                probable_exchange: profile.probable_exchange as u8,
                probable_deposit_wallet: profile.probable_deposit_wallet as u8,
                probable_sweeper: profile.probable_sweeper as u8,
                risk_score: profile.risk_score,
            });
        }
        save_address_profiles(loader.clickhouse.clone(), profile_rows).await?;

        let counterparty_rows = build_counterparty_relations(&simple_transfers, block_number)
            .into_iter()
            .map(|relation| CounterpartyRow {
                address: relation.address,
                counterparty: relation.counterparty,
                direction: relation.direction,
                token_address: relation.token_address,
                total_txs: relation.total_txs,
                total_volume: relation.total_volume,
                first_seen: relation.first_seen,
                last_seen: relation.last_seen,
            })
            .collect::<Vec<_>>();

        save_counterparties(loader.clickhouse.clone(), counterparty_rows).await?;

        let exchange_detections = detect_exchange_attributions(
            loader.clickhouse.clone(),
            block_number,
            &simple_transfers,
        )
        .await?;

        for detection in &exchange_detections {
            let address = detection.address.clone();

            save_exchange_entity(
                loader.clickhouse.clone(),
                ExchangeEntityRow {
                    entity_id: address.entity_id.clone(),
                    exchange_name: address.exchange_name.clone(),
                    exchange_type: "centralized_exchange".to_string(),
                    confidence: address.confidence,
                },
            )
            .await?;

            save_address_entity(
                loader.clickhouse.clone(),
                AddressEntityRow {
                    address: address.address.clone(),
                    entity_id: address.entity_id.clone(),
                    entity_name: address.exchange_name.clone(),
                    entity_type: format!("exchange_{}", address.address_role.to_lowercase()),
                    confidence: address.confidence,
                    source: address.detection_source.clone(),
                },
            )
            .await?;

            save_exchange_address(loader.clickhouse.clone(), address).await?;

            if let Some(deposit) = detection.deposit.clone() {
                save_exchange_deposit_address(loader.clickhouse.clone(), deposit).await?;
            }

            save_exchange_cluster(loader.clickhouse.clone(), detection.cluster.clone()).await?;
        }

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

        let risk_row = TransactionRiskRow {
            tx_hash: txid.clone(),
            block_number,
            timestamp,
            risk_score,
            risk_level,
            transaction_type: semantics.transaction_type.clone(),
            transaction_subtype: semantics.transaction_subtype.clone(),
            is_swap: semantics.is_swap,
            is_bridge: semantics.is_bridge,
            is_contract_call,
            unique_tokens,
            participants,
            risk_reasons: vec![format!(
                "transaction_type:{}:{}",
                semantics.transaction_type, semantics.transaction_subtype
            )],
            touches_exchange: (!exchange_flows.is_empty()) as u8,
        };

        loader.transaction_risk_batcher.push(risk_row).await?;

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
        let (risk_score, risk_level) =
            compute_risk_score(&classification, false, false, 0, participants);

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

        loader
            .transaction_risk_batcher
            .push(TransactionRiskRow {
                tx_hash: txid.clone(),
                block_number,
                timestamp,
                risk_score,
                risk_level,
                transaction_type: semantics.transaction_type.clone(),
                transaction_subtype: semantics.transaction_subtype.clone(),
                is_swap: semantics.is_swap,
                is_bridge: semantics.is_bridge,
                is_contract_call,
                unique_tokens: 0,
                participants,
                risk_reasons: vec![format!(
                    "transaction_type:{}:{}",
                    semantics.transaction_type, semantics.transaction_subtype
                )],
                touches_exchange: 0,
            })
            .await?;
    }

    Ok(())
}

pub async fn fetch_tron(loader: Arc<LoaderTron>, start_block: u64, total_txs: u64) -> Result<()> {
    let latest_block = loader.tron_client.get_block_number().await?;

    println!("TRON Latest Block: {}", latest_block);

    let mut tx_count = 0u64;

    let mut current_block = start_block;

    let mut last_synced_block = None::<u64>;

    while current_block <= latest_block {
        if tx_count >= total_txs {
            break;
        }

        let block = {
            let _permit = loader.rpc_limiter.acquire().await?;

            loader.tron_client.get_block(current_block).await?
        };

        let empty_txs = Vec::new();

        let txs = block["transactions"].as_array().unwrap_or(&empty_txs);

        if txs.is_empty() {
            println!("[TRON] block {} has 0 transaction(s)", current_block);

            last_synced_block = Some(current_block);

            save_sync_state(loader.clickhouse.clone(), "tron", current_block).await?;

            current_block += 1;

            continue;
        }

        let mut fully_processed = true;

        let tx_vec = txs.to_vec();

        let remaining = total_txs.saturating_sub(tx_count);

        let tx_vec = tx_vec
            .into_iter()
            .take(remaining as usize)
            .collect::<Vec<_>>();

        if tx_vec.len() < txs.len() {
            fully_processed = false;
        }

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

                async move { process_tx(loader_clone, tx, current_block).await }
            })
            .buffer_unordered(loader.config.tx_worker_concurrency)
            .filter_map(|res| {
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

                            Some(err)
                        }
                    }
                }
            })
            .collect::<Vec<_>>()
            .await;

        if let Some(err) = tx_errors.into_iter().next() {
            return Err(err)
                .with_context(|| format!("failed to process TRON block {}", current_block));
        }

        if fully_processed {
            last_synced_block = Some(current_block);

            loader.flush_batches().await?;

            save_sync_state(loader.clickhouse.clone(), "tron", current_block).await?;

            println!(
                "TRON synced block {} | total tx {}",
                current_block, tx_count
            );
        } else {
            println!(
                "TRON stopped mid-block {} | total tx {}",
                current_block, tx_count
            );

            break;
        }

        current_block += 1;
    }

    loader.flush_batches().await?;

    if let Some(last_synced_block) = last_synced_block {
        save_sync_state(loader.clickhouse.clone(), "tron", last_synced_block).await?;
    }

    Ok(())
}
