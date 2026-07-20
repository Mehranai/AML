use std::sync::Arc;

use anyhow::Result;
use ethers::prelude::*;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::models::address_relationship::AddressRelationshipRow;
use crate::models::token_transfer::TokenTransferRow;
use crate::models::transaction::Sensivity;
use crate::progress::core::{
    save_address_relationship, save_sync_state, save_token_transfer, save_tx, save_wallet,
};
use crate::services::loader::LoaderEth;
use crate::services::token_metadata_worker;

const ERC20_TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

fn calc_sensivity_eth(value_wei: U256) -> Sensivity {
    let eth_value = value_wei.as_u128() as f64 / 1e18;

    if eth_value > 100.0 {
        Sensivity::Red
    } else if eth_value > 10.0 {
        Sensivity::Yellow
    } else {
        Sensivity::Green
    }
}

// استخراج Transfer Logs
fn normalize_eth_address(address: Address) -> String {
    format!("{address:?}").to_ascii_lowercase()
}

fn relationship_id(
    tx_hash: &str,
    transfer_type: &str,
    index: u32,
    from: &str,
    to: &str,
    token: &str,
) -> String {
    format!("{tx_hash}:{transfer_type}:{index}:{from}:{to}:{token}")
}

fn native_transfer_relationship(
    tx_hash: &str,
    block_number: u64,
    from: Address,
    to: Address,
    value: U256,
) -> Option<AddressRelationshipRow> {
    if value.is_zero() || from == Address::zero() || to == Address::zero() {
        return None;
    }

    let from_address = normalize_eth_address(from);
    let to_address = normalize_eth_address(to);
    let token_address = "ETH".to_string();

    Some(AddressRelationshipRow {
        relationship_id: relationship_id(
            tx_hash,
            "native",
            0,
            &from_address,
            &to_address,
            &token_address,
        ),
        from_address,
        to_address,
        token_address,
        tx_hash: tx_hash.to_string(),
        block_number,
        timestamp: 0,
        amount: value.to_string(),
        transfer_type: "native_transfer".to_string(),
        protocol: "ethereum".to_string(),
        event_type: "transfer".to_string(),
        risk_score: 0,
        hop_count: 1,
    })
}

fn erc20_transfer_relationship(
    tx_hash: &str,
    block_number: u64,
    log_index: u32,
    token: Address,
    from: Address,
    to: Address,
    amount: U256,
) -> Option<AddressRelationshipRow> {
    if amount.is_zero() || from == Address::zero() || to == Address::zero() {
        return None;
    }

    let from_address = normalize_eth_address(from);
    let to_address = normalize_eth_address(to);
    let token_address = normalize_eth_address(token);

    Some(AddressRelationshipRow {
        relationship_id: relationship_id(
            tx_hash,
            "erc20",
            log_index,
            &from_address,
            &to_address,
            &token_address,
        ),
        from_address,
        to_address,
        token_address,
        tx_hash: tx_hash.to_string(),
        block_number,
        timestamp: 0,
        amount: amount.to_string(),
        transfer_type: "erc20_transfer".to_string(),
        protocol: "erc20".to_string(),
        event_type: "transfer".to_string(),
        risk_score: 0,
        hop_count: 1,
    })
}

fn extract_token_transfers(
    receipt: &TransactionReceipt,
) -> Vec<(u32, Address, Address, Address, U256)> {
    let mut transfers = Vec::new();

    let transfer_topic: H256 = ERC20_TRANSFER_TOPIC.parse().unwrap();

    for log in &receipt.logs {
        if log.topics.len() == 3 && log.topics[0] == transfer_topic {
            let token_address = log.address;

            let from = Address::from_slice(&log.topics[1].as_bytes()[12..]);
            let to = Address::from_slice(&log.topics[2].as_bytes()[12..]);

            let amount = U256::from_big_endian(&log.data.0);

            transfers.push((
                log.log_index.unwrap_or(U256::zero()).as_u32(),
                token_address,
                from,
                to,
                amount,
            ));
        }
    }

    transfers
}

async fn save_wallet_eth(
    provider: Arc<Provider<Http>>,
    clickhouse: Arc<clickhouse::Client>,
    limiter: Arc<tokio::sync::Semaphore>,
    addr: Address,
) -> Result<()> {
    if addr == Address::zero() {
        return Ok(());
    }

    let (balance, nonce, wallet_type) = {
        let _permit = limiter.acquire().await?;

        let balance = provider.get_balance(addr, None).await?;
        let nonce = provider.get_transaction_count(addr, None).await?;
        let code = provider.get_code(addr, None).await?;

        let wallet_type = if code.0.is_empty() {
            "wallet".to_string()
        } else {
            "smart_contract".to_string()
        };

        (balance, nonce, wallet_type)
    };

    save_wallet(
        clickhouse,
        &addr.to_string(),
        balance.to_string(),
        nonce.as_u64(),
        wallet_type,
    )
    .await?;

    Ok(())
}

async fn process_tx(
    provider: Arc<Provider<Http>>,
    clickhouse: Arc<clickhouse::Client>,
    limiter: Arc<tokio::sync::Semaphore>,
    tx: Transaction,
    block_number: u64,
) -> Result<Vec<Address>> {
    let hash = format!("{:#x}", tx.hash);
    let from = tx.from;
    let to = tx.to.unwrap_or_default();
    let value = tx.value;
    let from_address = normalize_eth_address(from);
    let to_address = tx.to.map(normalize_eth_address).unwrap_or_default();

    save_tx(
        clickhouse.clone(),
        hash.clone(),
        block_number,
        from_address,
        to_address,
        value.to_string(),
        calc_sensivity_eth(value) as u8,
    )
    .await?;

    if let Some(to_address) = tx.to
        && let Some(row) =
            native_transfer_relationship(&hash, block_number, from, to_address, value)
    {
        save_address_relationship(clickhouse.clone(), row).await?;
    }

    // Receipt (Rate limited)
    let receipt_opt = {
        let _permit = limiter.acquire().await?;
        provider.get_transaction_receipt(tx.hash).await?
    };

    let mut discovered_tokens: Vec<Address> = vec![];

    if let Some(receipt) = receipt_opt {
        let transfers = extract_token_transfers(&receipt);

        for (log_index, token, from_addr, to_addr, amount) in transfers {
            discovered_tokens.push(token);

            let token_address = normalize_eth_address(token);
            let from_address = normalize_eth_address(from_addr);
            let to_address = normalize_eth_address(to_addr);

            save_token_transfer(
                clickhouse.clone(),
                TokenTransferRow {
                    tx_hash: hash.clone(),
                    block_number,
                    log_index,
                    token_address,
                    from_addr: from_address,
                    to_addr: to_address,
                    amount: amount.to_string(),
                },
            )
            .await?;

            if let Some(row) = erc20_transfer_relationship(
                &hash,
                block_number,
                log_index,
                token,
                from_addr,
                to_addr,
                amount,
            ) {
                save_address_relationship(clickhouse.clone(), row).await?;
            }
        }
    }

    // Save wallet info (Rate limited)
    save_wallet_eth(provider.clone(), clickhouse.clone(), limiter.clone(), from).await?;

    if tx.to.is_some() {
        save_wallet_eth(provider, clickhouse, limiter.clone(), to).await?;
    }

    Ok(discovered_tokens)
}

pub async fn fetch_eth(loader: Arc<LoaderEth>, start_block: u64, total_txs: u64) -> Result<()> {
    let provider = loader.eth_provider.clone();
    let clickhouse = loader.clickhouse.clone();
    let limiter = loader.rpc_limiter.clone();

    let latest_block = provider.get_block_number().await?.as_u64();
    println!("ETH Latest Block: {}", latest_block);

    let mut tx_count: u64 = 0;
    let mut last_synced_block: u64 = start_block;
    let mut current_block = start_block;

    while current_block <= latest_block {
        if tx_count >= total_txs {
            break;
        }

        // فقط header بلاک رو بگیر (hash tx ها)
        let block_opt = {
            let _permit = limiter.acquire().await?;
            provider.get_block(current_block).await?
        };

        let Some(block) = block_opt else {
            current_block += 1;
            continue;
        };

        let tx_hashes = block.transactions;

        if tx_hashes.is_empty() {
            println!(
                "[ETH] Block {} has 0 txs (valid empty block)",
                current_block
            );

            // اینجا بلاک خالیه ولی sync کردنش مشکلی نداره
            last_synced_block = current_block;
            save_sync_state(clickhouse.clone(), "eth", last_synced_block).await?;

            current_block += 1;
            continue;
        }

        let mut tasks = FuturesUnordered::new();
        let mut discovered_tokens_all: Vec<Address> = vec![];

        let mut fully_processed_block = true;

        for tx_hash in tx_hashes {
            if tx_count >= total_txs {
                fully_processed_block = false;
                break;
            }

            let provider = provider.clone();
            let clickhouse = clickhouse.clone();
            let limiter = limiter.clone();
            let block_number = current_block;

            tasks.push(tokio::spawn(async move {
                // tx رو جدا بگیر
                let tx_opt = {
                    let _permit = limiter.acquire().await?;
                    provider.get_transaction(tx_hash).await?
                };

                let Some(tx) = tx_opt else {
                    return Ok::<Vec<Address>, anyhow::Error>(vec![]);
                };

                process_tx(provider, clickhouse, limiter, tx, block_number).await
            }));

            tx_count += 1;
            println!("[ETH] --> Queued tx #{}", tx_count);
        }

        while let Some(res) = tasks.next().await {
            let tokens = res??;
            discovered_tokens_all.extend(tokens);
        }

        // Token metadata worker
        if !discovered_tokens_all.is_empty() {
            token_metadata_worker::process_new_tokens(
                clickhouse.clone(),
                provider.clone(),
                limiter.clone(),
                discovered_tokens_all,
            )
            .await?;
        }

        // فقط اگر بلاک کامل پردازش شد sync_state رو آپدیت کن
        if fully_processed_block {
            last_synced_block = current_block;

            save_sync_state(clickhouse.clone(), "eth", last_synced_block).await?;

            println!(
                "ETH synced block {} | total tx processed {}",
                last_synced_block, tx_count
            );
        } else {
            println!(
                "ETH stopped mid-block {} (tx limit reached) | total tx processed {}",
                current_block, tx_count
            );
            break;
        }

        current_block += 1;
    }

    // save final sync_state
    save_sync_state(clickhouse.clone(), "eth", last_synced_block).await?;

    Ok(())
}
