use candid::Nat;
use icrc_ledger_types::icrc::generic_metadata_value::MetadataValue;

use crate::types::{
    MintArg, MintError, MintResult, SftId, TransferArg, TransferError, TransferResult,
};
use crate::utils::{sha3_256, to_cbor_bytes};
use crate::{is_authenticated, store, SECOND};

/// Performs a batch of token transfers.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc7_transfer(args: Vec<TransferArg>) -> Vec<Option<TransferResult>> {
    if args.is_empty() {
        ic_cdk::trap("No transfer args provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());

    if args.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("Exceeds max update batch size");
    }

    let caller = ic_cdk::caller();
    let now = ic_cdk::api::time();
    if settings.atomic_batch_transfers {
        if let Some(err) = args
            .iter()
            .find_map(|arg| arg.validate(now, &caller, &settings).err())
        {
            ic_cdk::trap(format!("Invalid transfer args: {:?}", err).as_str())
        }

        if let Err(err) = store::holders::with(|r| {
            for arg in &args {
                let id = SftId::from(&arg.token_id);
                match r.get(&id.0) {
                    None => return Err(TransferError::NonExistingTokenId),
                    Some(ref holders) => {
                        if !holders.is_holder(id.1, &caller) {
                            return Err(TransferError::Unauthorized);
                        }
                    }
                }
            }
            Ok(())
        }) {
            ic_cdk::trap(format!("Invalid transfer args: {:?}", err).as_str())
        }
    }

    store::holders::with_mut(|r| {
        let mut res: Vec<Option<TransferResult>> = vec![None; args.len()];
        for (index, arg) in args.iter().enumerate() {
            if let Err(err) = arg.validate(now, &caller, &settings) {
                res[index] = Some(Err(err));
                continue;
            }

            let id = SftId::from(&arg.token_id);
            match r.get(&id.0) {
                None => {
                    res[index] = Some(Err(TransferError::NonExistingTokenId));
                }
                Some(mut holders) => match holders.transfer_at(id.1, &caller, &arg.to.owner) {
                    Ok(_) => {
                        let tx_log = store::Transaction::transfer(
                            now / SECOND,
                            id.to_u64(),
                            caller,
                            arg.to.owner,
                            arg.memo.clone(),
                        );

                        match store::transactions::append(&tx_log) {
                            Ok(idx) => {
                                res[index] = Some(Ok(Nat::from(idx)));
                                r.insert(id.0, holders);
                            }
                            Err(err) => {
                                res[index] = Some(Err(TransferError::GenericBatchError {
                                    error_code: Nat::from(0u64),
                                    message: err,
                                }));
                                // break up when append log failed.
                                return res;
                            }
                        }
                    }
                    Err(err) => {
                        res[index] = Some(Err(err));
                    }
                },
            }
        }

        res
    })
}

/// Mint a token.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_mint(args: MintArg) -> MintResult {
    let caller = ic_cdk::caller();
    if !store::collection::with(|c| c.minters.contains(&caller)) {
        ic_cdk::trap("Caller is not a minter");
    }

    if args.holders.is_empty() {
        ic_cdk::trap("No mint holders provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.holders.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("Exceeds max update batch size");
    }

    let id = SftId::from(&args.token_id);
    let metadata_hash = store::tokens::with(|r| {
        if let Some(token) = r.get(id.token_index() as u64) {
            if let Some(supply_cap) = token.supply_cap {
                if token.total_supply.saturating_add(args.holders.len() as u32) >= supply_cap {
                    return Err(MintError::SupplyCapReached);
                }
            }

            let data = token.metadata();
            let data = to_cbor_bytes(&data);
            let data = sha3_256(&data);
            Ok(MetadataValue::from(&data[..]))
        } else {
            Err(MintError::NonExistingTokenId)
        }
    })?;

    let now_sec = ic_cdk::api::time() / SECOND;
    store::holders::with_mut(|r| {
        match r.get(&id.0) {
            None => Err(MintError::NonExistingTokenId),
            Some(mut holders) => {
                let mut block_idx = 0u64;
                let added_holders = args.holders.len() as u32;
                for holder in args.holders {
                    holders.append(holder);

                    let tx_log = store::Transaction::mint(
                        now_sec,
                        id.to_u64(),
                        holder,
                        metadata_hash.clone(),
                        None,
                    );

                    match store::transactions::append(&tx_log) {
                        Ok(idx) => block_idx = idx,
                        Err(err) => {
                            // break up when append log failed.
                            return Err(MintError::GenericBatchError {
                                error_code: Nat::from(0u64),
                                message: err,
                            });
                        }
                    }
                }

                r.insert(id.0, holders);
                store::tokens::with_mut(|r| {
                    let idx = id.token_index() as u64;
                    if let Some(mut token) = r.get(idx) {
                        token.total_supply += added_holders;
                        token.updated_at = now_sec;
                        r.set(idx, &token);
                    }
                });

                Ok(Nat::from(block_idx))
            }
        }
    })
}
