use crate::{is_authenticated, store, SECOND};
use candid::Nat;
use ic_sft_types::{MintArg, MintError, MintResult, SftId, Transaction};

// Mint a token.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn sft_mint(args: MintArg) -> MintResult {
    let caller = ic_cdk::caller();
    if !store::collection::with(|c| c.minters.contains(&caller)) {
        ic_cdk::trap("caller is not a minter");
    }

    if args.holders.is_empty() {
        ic_cdk::trap("no mint holders provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.holders.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("exceeds max update batch size");
    }

    let id = SftId::from(&args.token_id);
    let metadata = store::tokens::with(|r| {
        if let Some(token) = r.get(id.token_index() as u64) {
            if let Some(supply_cap) = token.supply_cap {
                if token.total_supply.saturating_add(args.holders.len() as u32) >= supply_cap {
                    return Err(MintError::SupplyCapReached);
                }
            }

            Ok(token.metadata())
        } else {
            Err(MintError::NonExistingTokenId)
        }
    })?;

    let now = ic_cdk::api::time();
    store::holders::with_mut(|r| {
        match r.get(&id.0) {
            None => Err(MintError::NonExistingTokenId),
            Some(mut holders) => {
                let mut block_idx = 0u64;
                let added_holders = args.holders.len() as u32;
                for holder in args.holders {
                    holders.append(holder);

                    let tx_log = Transaction::mint(
                        now,
                        id.to_u64(),
                        Some(caller),
                        holder,
                        metadata.clone(),
                        None,
                    );

                    match store::blocks::append(tx_log) {
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
                        token.updated_at = now / SECOND;
                        r.set(idx, &token);
                    }
                });

                Ok(Nat::from(block_idx))
            }
        }
    })
}
