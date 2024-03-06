use candid::{Nat, Principal};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::types::{ChallengeArg, CreateTokenArg, UpdateCollectionArg, UpdateTokenArg, SECOND};
use crate::{is_authenticated, is_controller, SftId};
use crate::{sha3_256, store};

/// Set the minters.
#[ic_cdk::update(guard = "is_controller")]
pub fn admin_set_minters(args: BTreeSet<Principal>) -> Result<(), String> {
    let now = ic_cdk::api::time() / SECOND;
    store::collection::with_mut(|r| {
        r.updated_at = now;
        r.minters = args;
    });
    Ok(())
}

/// Set the managers.
#[ic_cdk::update(guard = "is_controller")]
pub fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    let now = ic_cdk::api::time() / SECOND;
    store::collection::with_mut(|r| {
        r.updated_at = now;
        r.managers = args;
    });
    Ok(())
}

/// Update the collection.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_update_collection(args: UpdateCollectionArg) -> Result<(), String> {
    let caller = ic_cdk::caller();

    store::collection::with(|c| {
        if !c.managers.contains(&caller) {
            ic_cdk::trap("Caller is not a manager");
        }

        if args.supply_cap.is_some() && c.supply_cap.is_some() {
            ic_cdk::trap("Supply cap can not be changed");
        }
    });

    let now = ic_cdk::api::time() / SECOND;
    store::collection::with_mut(|r| {
        r.updated_at = now;

        if let Some(name) = args.name {
            r.name = name;
        }
        if let Some(description) = args.description {
            r.description = Some(description);
        }
        if let Some(logo) = args.logo {
            r.logo = Some(logo);
        }
        if let Some(assets_origin) = args.assets_origin {
            r.assets_origin = Some(assets_origin);
        }
        if let Some(supply_cap) = args.supply_cap {
            r.supply_cap = Some(supply_cap);
        }

        if let Some(max_query_batch_size) = args.max_query_batch_size {
            r.settings.max_query_batch_size = max_query_batch_size;
        }
        if let Some(max_update_batch_size) = args.max_update_batch_size {
            r.settings.max_update_batch_size = max_update_batch_size;
        }
        if let Some(default_take_value) = args.default_take_value {
            r.settings.default_take_value = default_take_value;
        }
        if let Some(max_take_value) = args.max_take_value {
            r.settings.max_take_value = max_take_value;
        }
        if let Some(max_memo_size) = args.max_memo_size {
            r.settings.max_memo_size = max_memo_size;
        }
        if let Some(atomic_batch_transfers) = args.atomic_batch_transfers {
            r.settings.atomic_batch_transfers = atomic_batch_transfers;
        }
        if let Some(tx_window) = args.tx_window {
            r.settings.tx_window = tx_window;
        }
        if let Some(permitted_drift) = args.permitted_drift {
            r.settings.permitted_drift = permitted_drift;
        }
    });

    Ok(())
}

/// Create a challenge for sft_create_token_by_challenge API.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_challenge(args: ChallengeArg) -> Result<ByteBuf, String> {
    let caller = ic_cdk::caller();

    store::collection::with(|c| {
        if !c.managers.contains(&caller) {
            ic_cdk::trap("Caller is not a manager");
        }
    });
    let mut args = args;
    args.ts = ic_cdk::api::time() / SECOND;
    store::signing::with_secret(|secret| args.sign_to_bytes(secret))
}

/// Create a token.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_create_token(args: CreateTokenArg) -> Result<Nat, String> {
    let caller = ic_cdk::caller();

    store::collection::with(|c| {
        if !c.managers.contains(&caller) {
            ic_cdk::trap("Caller is not a manager");
        }

        if let Some(supply_cap) = c.supply_cap {
            if c.total_supply >= supply_cap {
                ic_cdk::trap("Supply cap reached");
            }
        }
    });

    let now = ic_cdk::api::time() / SECOND;
    let hash = sha3_256(&args.asset_content);
    create_token(args, hash, now)
}

#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_create_token_by_challenge(args: CreateTokenArg) -> Result<Nat, String> {
    let caller = ic_cdk::caller();
    if caller != args.author {
        ic_cdk::trap("Caller is not the author");
    }

    let challenge_data = args
        .challenge
        .as_ref()
        .unwrap_or_else(|| ic_cdk::trap("challenge is required"));

    store::collection::with(|c| {
        if let Some(supply_cap) = c.supply_cap {
            if c.total_supply >= supply_cap {
                ic_cdk::trap("Supply cap reached");
            }
        }
    });

    let now = ic_cdk::api::time() / SECOND;
    let expire_at = now - 60 * 10;
    let hash = sha3_256(&args.asset_content);
    store::signing::with_secret(|secret| {
        ChallengeArg {
            author: caller,
            asset_hash: hash,
            ts: 0,
        }
        .verify_from_bytes(secret, challenge_data, expire_at)
    })?;

    create_token(args, hash, now)
}

/// Update a token before minted.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_update_token(args: UpdateTokenArg) -> Result<(), String> {
    let caller = ic_cdk::caller();

    let id = SftId::from(&args.id);
    let mut token = store::tokens::with(|r| r.get(id.token_index() as u64)).unwrap_or_else(|| {
        ic_cdk::trap("Token not found");
    });

    store::collection::with(|c| {
        if !c.managers.contains(&caller) && token.author != caller {
            ic_cdk::trap("Caller is not a manager or author");
        }
    });

    if token.total_supply > 0 {
        ic_cdk::trap("Token has been minted, can not be updated");
    }

    if let Some(supply_cap) = args.supply_cap {
        if let Some(old_supply_cap) = token.supply_cap {
            if supply_cap >= old_supply_cap {
                ic_cdk::trap("Supply cap can not be increased");
            }
        }
    }

    let now = ic_cdk::api::time() / SECOND;
    token.updated_at = now;

    if let Some(name) = args.name {
        token.name = name;
    }
    if let Some(description) = args.description {
        token.description = Some(description);
    }
    if let Some(asset_name) = args.asset_name {
        token.asset_name = asset_name;
    }
    if let Some(asset_content_type) = args.asset_content_type {
        token.asset_content_type = asset_content_type;
    }

    if let Some(asset_content) = args.asset_content {
        let hash = sha3_256(&asset_content);
        store::assets::with_mut(|r| {
            r.remove(&token.asset_hash);
            r.insert(hash, asset_content.to_vec());
        });
        token.asset_hash = hash;
    }

    if let Some(metadata) = args.metadata {
        token.metadata = metadata;
    }

    if let Some(supply_cap) = args.supply_cap {
        token.supply_cap = Some(supply_cap);
    }

    if let Some(author) = args.author {
        token.author = author;
    }

    store::tokens::with_mut(|r| r.set(id.token_index() as u64, &token));

    Ok(())
}

fn create_token(args: CreateTokenArg, hash: [u8; 32], now_sec: u64) -> Result<Nat, String> {
    store::assets::with_mut(|r| {
        if r.contains_key(&hash) {
            return Err("Asset already exists".to_string());
        }

        r.insert(hash, args.asset_content.to_vec());
        Ok::<(), String>(())
    })?;

    let id = store::tokens::with_mut(|r| {
        let id = r.len() as u32 + 1;
        let token = store::Token {
            id,
            name: args.name,
            description: args.description,
            asset_name: args.asset_name,
            asset_content_type: args.asset_content_type,
            asset_hash: hash,
            metadata: args.metadata,
            supply_cap: args.supply_cap,
            author: args.author,
            total_supply: 0,
            created_at: now_sec,
            updated_at: now_sec,
        };
        match r.push(&token) {
            Err(err) => Err(format!("Failed to create token: {}", err)),
            Ok(_) => Ok(Nat::from(id)),
        }
    })?;

    store::collection::with_mut(|r| {
        r.total_supply += 1;
        r.updated_at = now_sec;
    });

    Ok(id)
}
