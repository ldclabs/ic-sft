use crate::{is_authenticated, schema::Validate, store};
use candid::Nat;
use ic_sft_types::{
    nat_to_u64, Metadata, SftId, Transaction, TransferArg, TransferError, TransferResult,
};
use icrc_ledger_types::icrc1::account::Account;

// Returns all the collection-level metadata of the NFT collection in a single query.
#[ic_cdk::query]
pub fn icrc7_collection_metadata() -> Metadata {
    store::collection::with(|c| c.metadata())
}

// Returns the token symbol of the NFT collection (e.g., `MS`).
#[ic_cdk::query]
pub fn icrc7_symbol() -> String {
    store::collection::with(|c| c.symbol.clone())
}

// Returns the name of the NFT collection (e.g., `My Super NFT`).
#[ic_cdk::query]
pub fn icrc7_name() -> String {
    store::collection::with(|c| c.name.clone())
}

// Returns the text description of the collection.
#[ic_cdk::query]
pub fn icrc7_description() -> Option<String> {
    store::collection::with(|c| c.description.clone())
}

// Returns a link to the logo of the collection. It may be a [DataURL](https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URLs) that contains the logo image itself.
#[ic_cdk::query]
pub fn icrc7_logo() -> Option<String> {
    store::collection::with(|c| c.logo.clone())
}

// Returns the total number of NFTs on all accounts.
#[ic_cdk::query]
pub fn icrc7_total_supply() -> Nat {
    store::collection::with(|c| c.total_supply.into())
}

// Returns the maximum number of NFTs possible for this collection. Any attempt to mint more NFTs
// than this supply cap shall be rejected.
#[ic_cdk::query]
pub fn icrc7_supply_cap() -> Option<Nat> {
    store::collection::with(|c| c.supply_cap.map(Nat::from))
}

// Returns the maximum batch size for batch query calls this ledger implementation supports.
#[ic_cdk::query]
pub fn icrc7_max_query_batch_size() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.max_query_batch_size.into()))
}

// Returns the maximum number of token ids allowed for being used as input in a batch update method.
#[ic_cdk::query]
pub fn icrc7_max_update_batch_size() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.max_update_batch_size.into()))
}

// Returns the default parameter the ledger uses for `take` in case the parameter is `null` in paginated queries.
#[ic_cdk::query]
pub fn icrc7_default_take_value() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.default_take_value.into()))
}

// Returns the maximum `take` value for paginated query calls this ledger implementation supports. The value applies to all paginated calls the ledger exposes.
#[ic_cdk::query]
pub fn icrc7_max_take_value() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.max_take_value.into()))
}

// Returns the maximum size of `memo`s as supported by an implementation.
#[ic_cdk::query]
pub fn icrc7_max_memo_size() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.max_memo_size.into()))
}

// Returns `true` if and only if batch transfers of the ledger are executed atomically, i.e., either all transfers execute or none, `false` otherwise.
#[ic_cdk::query]
pub fn icrc7_atomic_batch_transfers() -> Option<bool> {
    store::collection::with(|c| Some(c.settings.atomic_batch_transfers))
}

// Returns the time window in seconds during which transactions can be deduplicated.
#[ic_cdk::query]
pub fn icrc7_tx_window() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.tx_window.into()))
}

// Returns the time duration in seconds by which the transaction deduplication window can be extended.
#[ic_cdk::query]
pub fn icrc7_permitted_drift() -> Option<Nat> {
    store::collection::with(|c| Some(c.settings.permitted_drift.into()))
}

// Returns the token metadata for `token_ids`, a list of token ids.
#[ic_cdk::query]
pub fn icrc7_token_metadata(token_ids: Vec<Nat>) -> Vec<Option<Metadata>> {
    if token_ids.is_empty() {
        return vec![];
    }

    let max_query_batch_size = store::collection::with(|c| c.settings.max_query_batch_size);
    if token_ids.len() > max_query_batch_size as usize {
        ic_cdk::trap("exceeds max query batch size");
    }

    store::tokens::with(|r| {
        token_ids
            .iter()
            .map(|id| {
                let id = SftId::from(id);
                r.get(id.token_index() as u64).map(|t| t.metadata())
            })
            .collect()
    })
}

// Returns the owner `Account` of each token in a list `token_ids` of token ids.
#[ic_cdk::query]
pub fn icrc7_owner_of(token_ids: Vec<Nat>) -> Vec<Option<Account>> {
    if token_ids.is_empty() {
        return vec![];
    }

    let max_query_batch_size = store::collection::with(|c| c.settings.max_query_batch_size);
    if token_ids.len() > max_query_batch_size as usize {
        ic_cdk::trap("exceeds max query batch size");
    }

    store::holders::with(|r| {
        token_ids
            .iter()
            .map(|id| {
                let id = SftId::from(id);
                r.get(&id.0).and_then(|hs| {
                    hs.get(id.1).map(|h| Account {
                        owner: *h,
                        subaccount: None,
                    })
                })
            })
            .collect()
    })
}

// Returns the balance of the `account` provided as an argument, i.e., the number of tokens held by the account. For a non-existing account, the value `0` is returned.
#[ic_cdk::query]
pub fn icrc7_balance_of(accounts: Vec<Account>) -> Vec<Nat> {
    if accounts.is_empty() {
        return vec![];
    }

    let max_query_batch_size = store::collection::with(|c| c.settings.max_query_batch_size);
    if accounts.len() > max_query_batch_size as usize {
        ic_cdk::trap("exceeds max query batch size");
    }

    store::holder_tokens::with(|r| {
        let res: Vec<Nat> = accounts
            .into_iter()
            .map(|acc| {
                r.get(&acc.owner)
                    .map(|tokens| tokens.balance_of())
                    .unwrap_or(0u64)
            })
            .map(Nat::from)
            .collect();
        res
    })
}

// Returns the list of tokens in this ledger, sorted by their token id.
#[ic_cdk::query]
pub fn icrc7_tokens(prev: Option<Nat>, take: Option<Nat>) -> Vec<Nat> {
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));

    store::tokens::with(|r| {
        let max_tid = r.len() as u32;
        let start_tid = if let Some(ref prev) = prev {
            SftId::from(prev).0
        } else {
            1u32
        };
        let mut res: Vec<Nat> = Vec::with_capacity(take as usize);
        for tid in start_tid..=max_tid {
            res.push(Nat::from(SftId(tid, 0).to_u64()));
            if res.len() as u16 >= take {
                return res;
            }
        }
        res
    })
}

// Returns a vector of `token_id`s of all tokens held by `account`, sorted by `token_id`.
#[ic_cdk::query]
pub fn icrc7_tokens_of(account: Account, prev: Option<Nat>, take: Option<Nat>) -> Vec<Nat> {
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));

    store::holder_tokens::with(|r| {
        r.get(&account.owner)
            .map(|tokens| {
                let SftId(start_tid, mut start_sid) = if let Some(ref prev) = prev {
                    SftId::from(prev).next()
                } else {
                    SftId::MIN
                };

                let tids = tokens.token_ids();
                let mut res: Vec<Nat> = Vec::with_capacity(take as usize);
                for tid in tids {
                    if tid < start_tid {
                        continue;
                    }

                    if let Some(sids) = tokens.get_sids(tid) {
                        for sid in sids {
                            if sid < start_sid {
                                continue;
                            }
                            res.push(Nat::from(SftId(tid, sid).to_u64()));
                            if res.len() as u16 >= take {
                                return res;
                            }
                        }
                    }
                    start_sid = 1;
                }
                res
            })
            .unwrap_or_default()
    })
}

// Performs a batch of token transfers.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc7_transfer(args: Vec<TransferArg>) -> Vec<Option<TransferResult>> {
    if args.is_empty() {
        ic_cdk::trap("no transfer args provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());

    if args.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("exceeds max update batch size");
    }

    let caller = ic_cdk::caller();
    let now = ic_cdk::api::time();
    if settings.atomic_batch_transfers && args.len() > 1 {
        if let Some(err) = args
            .iter()
            .find_map(|arg| arg.validate(now, &caller, &settings).err())
        {
            ic_cdk::trap(format!("invalid transfer args: {:?}", err).as_str())
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
            ic_cdk::trap(format!("invalid transfer args: {:?}", err).as_str())
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
                Some(mut holders) => match holders.transfer_to(&caller, &arg.to.owner, id.1) {
                    Ok(_) => {
                        let tx_log = Transaction::transfer(
                            now,
                            id.to_u64(),
                            caller,
                            arg.to.owner,
                            arg.memo.clone(),
                        );

                        match store::blocks::append(tx_log) {
                            Ok(idx) => {
                                res[index] = Some(Ok(Nat::from(idx)));
                                r.insert(id.0, holders);
                                store::holder_tokens::update_for_transfer(
                                    caller,
                                    arg.to.owner,
                                    id.0,
                                    id.1,
                                );
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
