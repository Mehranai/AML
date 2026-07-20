use std::sync::Arc;

use anyhow::Result;
use clickhouse::Client;
use nanoid::nanoid;

use crate::models::address_relationship::AddressRelationshipRow;
use crate::models::owner::OwnerRow;
use crate::models::token_metadata::TokenMetadataRow;
use crate::models::token_transfer::TokenTransferRow;
use crate::models::transaction::TransactionRow;
use crate::models::wallet::WalletRow;

// --------------------------------------------------
// TRANSACTION
// --------------------------------------------------

pub async fn save_tx(
    clickhouse: Arc<Client>,
    hash: String,
    block_number: u64,
    from: String,
    to: String,
    value: String,
    sensivity: u8,
) -> Result<()> {
    let tx_row = TransactionRow {
        hash,
        block_number,
        from_addr: from,
        to_addr: to,
        value,
        sensivity,
    };

    let mut insert = clickhouse.insert::<TransactionRow>("transactions").await?;

    insert.write(&tx_row).await?;
    insert.end().await?;

    Ok(())
}

//
// --------------------------------------------------
// WALLET + OWNER
// --------------------------------------------------
//

pub async fn save_wallet(
    clickhouse: Arc<Client>,
    addr: &str,
    balance: String,
    nonce: u64,
    detected_wallet_type: String,
) -> Result<()> {
    if addr.is_empty() {
        return Ok(());
    }
    let existing: u64 = clickhouse
        .query(
            "
        SELECT count()
        FROM wallet_info
        WHERE address = ?
        ",
        )
        .bind(addr)
        .fetch_one::<u64>()
        .await?;

    if existing > 0 {
        return Ok(());
    }
    let wallet_type = detect_wallet_type(&clickhouse, addr, nonce, detected_wallet_type).await?;

    let person_id = match wallet_type.as_str() {
        "exchange" => format!("EXCHANGE_{}", addr),
        _ => get_or_create_person_id(&clickhouse, addr).await?,
    };

    let wallet = WalletRow {
        address: addr.to_string(),
        balance,
        nonce,
        wallet_type: wallet_type.clone(),
        person_id: person_id.clone(),
    };

    let owner = OwnerRow {
        address: addr.to_string(),
        person_name: "".into(),
        person_id,
        personal_id: 0,
    };

    //
    // wallet_info
    //
    let mut insert_wallet = clickhouse.insert::<WalletRow>("wallet_info").await?;

    insert_wallet.write(&wallet).await?;
    insert_wallet.end().await?;

    //
    // owner_info
    //
    let mut insert_owner = clickhouse.insert::<OwnerRow>("owner_info").await?;

    insert_owner.write(&owner).await?;
    insert_owner.end().await?;

    Ok(())
}

//
// --------------------------------------------------
// AUTO TAGGING LOGIC
// --------------------------------------------------
//

async fn detect_wallet_type(
    clickhouse: &Client,
    address: &str,
    nonce: u64,
    detected_wallet_type: String,
) -> Result<String> {
    //
    // high activity heuristic
    //
    if nonce > 10_000 {
        return Ok("exchange".into());
    }

    //
    // fan-in heuristic
    //
    let fan_in: u64 = clickhouse
        .query(
            "
            SELECT countDistinct(from_addr)
            FROM transactions
            WHERE to_addr = ?
            ",
        )
        .bind(address)
        .fetch_one::<u64>()
        .await?;

    if fan_in > 500 {
        return Ok("exchange".into());
    }

    Ok(detected_wallet_type)
}

//
// --------------------------------------------------
// PERSON ID MANAGEMENT
// --------------------------------------------------
//

async fn get_or_create_person_id(clickhouse: &Client, address: &str) -> Result<String> {
    let existing = clickhouse
        .query(
            "
            SELECT person_id
            FROM wallet_info
            WHERE address = ?
            LIMIT 1
            ",
        )
        .bind(address)
        .fetch_optional::<String>()
        .await?;

    if let Some(person_id) = existing {
        return Ok(person_id);
    }

    Ok(generate_person_id())
}

pub fn generate_person_id() -> String {
    nanoid!(10)
}

//
// --------------------------------------------------
// TOKEN TRANSFERS
// --------------------------------------------------
//

pub async fn save_token_transfer(clickhouse: Arc<Client>, row: TokenTransferRow) -> Result<()> {
    let mut insert = clickhouse
        .insert::<TokenTransferRow>("token_transfers")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

//
// --------------------------------------------------
// ADDRESS RELATIONSHIPS
// --------------------------------------------------
//

pub async fn save_address_relationship(
    clickhouse: Arc<Client>,
    row: AddressRelationshipRow,
) -> Result<()> {
    let mut insert = clickhouse
        .insert::<AddressRelationshipRow>("address_relationships")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

//
// --------------------------------------------------
// TOKEN METADATA
// --------------------------------------------------
//

pub async fn save_token_metadata(clickhouse: Arc<Client>, row: TokenMetadataRow) -> Result<()> {
    let existing: u64 = clickhouse
        .query(
            "
        SELECT count()
        FROM token_metadata
        WHERE token_address = ?
        ",
        )
        .bind(&row.token_address)
        .fetch_one::<u64>()
        .await?;

    if existing > 0 {
        return Ok(());
    }

    let mut insert = clickhouse
        .insert::<TokenMetadataRow>("token_metadata")
        .await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}

//
// --------------------------------------------------
// SYNC STATE
// --------------------------------------------------
//

#[derive(Debug, clickhouse::Row, serde::Serialize)]
pub struct SyncStateRow {
    pub chain: String,
    pub last_synced_block: u64,
}

pub async fn save_sync_state(
    clickhouse: Arc<Client>,
    chain: &str,
    last_synced_block: u64,
) -> Result<()> {
    let row = SyncStateRow {
        chain: chain.to_string(),
        last_synced_block,
    };

    let mut insert = clickhouse.insert::<SyncStateRow>("sync_state").await?;

    insert.write(&row).await?;
    insert.end().await?;

    Ok(())
}
