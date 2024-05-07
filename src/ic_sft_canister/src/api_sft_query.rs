use crate::store;
use crate::{ANONYMOUS, SECOND};
use candid::{Nat, Principal};
use ic_sft_types::{
    nat_to_u64, CollectionApproval, IsApprovedArg, Metadata, SftId, SupportedStandard,
    TokenApproval,
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

// Returns a vector of `token_id`s of all semi-fungible tokens in the `token_id` Token, sorted by `token_id`.
#[ic_cdk::query]
pub fn sft_tokens_in(token_id: Nat, prev: Option<Nat>, take: Option<Nat>) -> Vec<Nat> {
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));

    store::holders::with(|r| {
        let id = SftId::from(&token_id);
        r.get(&id.0)
            .map(|hs| {
                let max_sid = hs.total();
                let start_sid = if let Some(ref prev) = prev {
                    SftId::from(prev).1
                } else {
                    1u32
                };
                let mut res: Vec<Nat> = Vec::with_capacity(take as usize);
                for sid in start_sid..=max_sid {
                    res.push(Nat::from(SftId(id.0, sid).to_u64()));
                    if res.len() as u16 >= take {
                        return res;
                    }
                }
                res
            })
            .unwrap_or_default()
    })
}

// Returns the list of standards this ledger implements.
#[ic_cdk::query]
pub fn icrc10_supported_standards() -> Vec<SupportedStandard> {
    vec![
        SupportedStandard {
            name: "ICRC-3".to_string(),
            url: "https://github.com/dfinity/ICRC-1/tree/main/standards/ICRC-3".to_string(),
        },
        SupportedStandard {
            name: "ICRC-7".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-7".into(),
        },
        SupportedStandard {
            name: "ICRC-10".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-10".into(),
        },
        SupportedStandard {
            name: "ICRC-37".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-37".into(),
        },
    ]
}

// Returns the approval-related metadata of the ledger implementation.
#[ic_cdk::query]
pub fn icrc37_metadata() -> Metadata {
    store::collection::with(|c| c.icrc37_metadata())
}

// Returns the maximum number of approvals this ledger implementation allows to be active per token or per principal for the collection.
#[ic_cdk::query]
pub fn icrc37_max_approvals_per_token_or_collection() -> Option<Nat> {
    store::collection::with(|c| Some(Nat::from(c.settings.max_approvals_per_token_or_collection)))
}

// Returns the maximum number of approvals that may be revoked in a single invocation of `icrc37_revoke_token_approvals` or `icrc37_revoke_collection_approvals`.
#[ic_cdk::query]
pub fn icrc37_max_revoke_approvals() -> Option<Nat> {
    store::collection::with(|c| Some(Nat::from(c.settings.max_revoke_approvals)))
}

// Returns `true` if an active approval, i.e., a token-level approval or collection-level approval
#[ic_cdk::query]
pub fn icrc37_is_approved(args: Vec<IsApprovedArg>) -> Vec<bool> {
    if args.is_empty() {
        return vec![];
    }

    let max_query_batch_size = store::collection::with(|c| c.settings.max_query_batch_size);
    if args.len() > max_query_batch_size as usize {
        ic_cdk::trap("exceeds max query batch size");
    }
    let caller = ic_cdk::caller();
    if caller == ANONYMOUS {
        return vec![false; args.len()];
    }

    let now_sec = ic_cdk::api::time() / SECOND;
    let spenders: Vec<&Principal> = args.iter().map(|a| &a.spender.owner).collect();
    let mut res = store::approvals::spenders_is_approved(&caller, &spenders, now_sec);
    let mut query_idx: Vec<usize> = Vec::new();
    let mut query: Vec<(SftId, &Principal)> = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if !res[i] {
            query_idx.push(i);
            query.push((SftId::from(&a.token_id), &a.spender.owner));
        }
    }
    let res2 = store::holder_tokens::spenders_is_approved(&caller, &query, now_sec);
    for (i, idx) in query_idx.into_iter().enumerate() {
        res[idx] = res2[i];
    }

    res
}

// Returns the token-level approvals that exist for the given `token_id`.
#[ic_cdk::query]
pub fn icrc37_get_token_approvals(
    token_id: Nat,
    prev: Option<TokenApproval>,
    take: Option<Nat>,
) -> Vec<TokenApproval> {
    let id = SftId::from(&token_id);
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));
    let holder = store::holders::with(|r| r.get(&id.0).and_then(|hs| hs.get(id.1).cloned()));
    let holder = match holder {
        Some(h) => h,
        None => return vec![],
    };

    store::holder_tokens::with(|r| {
        if let Some(tokens) = r.get(&holder) {
            if let Some(approvals) = tokens.get_approvals(id.0, id.1) {
                let prev = prev.map(|p| p.approval_info.spender.owner);
                let mut res: Vec<TokenApproval> = Vec::with_capacity(take as usize);
                for approval in approvals.iter() {
                    if let Some(ref prev) = prev {
                        if approval.0 <= prev {
                            continue;
                        }
                    }
                    res.push(TokenApproval {
                        token_id: token_id.clone(),
                        approval_info: store::Approvals::to_info(approval),
                    });

                    if res.len() as u16 >= take {
                        return res;
                    }
                }
                return res;
            }
        }

        vec![]
    })
}

// Returns the collection-level approvals that exist for the specified `owner`.
#[ic_cdk::query]
pub fn icrc37_get_collection_approvals(
    owner: Account,
    prev: Option<CollectionApproval>,
    take: Option<Nat>,
) -> Vec<CollectionApproval> {
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));

    store::approvals::with(|r| {
        if let Some(approvals) = r.get(&owner.owner) {
            let prev = prev.map(|p| p.spender.owner);
            let mut res: Vec<CollectionApproval> = Vec::with_capacity(take as usize);
            for approval in approvals.iter() {
                if let Some(ref prev) = prev {
                    if approval.0 <= prev {
                        continue;
                    }
                }
                res.push(store::Approvals::to_info(approval));

                if res.len() as u16 >= take {
                    return res;
                }
            }
            return res;
        }

        vec![]
    })
}
