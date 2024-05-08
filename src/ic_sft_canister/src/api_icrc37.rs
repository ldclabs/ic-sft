use crate::{is_authenticated, schema::Validate, store, ANONYMOUS, SECOND};
use candid::{Nat, Principal};
use ic_sft_types::{
    nat_to_u64, ApproveCollectionArg, ApproveCollectionError, ApproveCollectionResult,
    ApproveTokenArg, ApproveTokenError, ApproveTokenResult, CollectionApproval, IsApprovedArg,
    Metadata, RevokeCollectionApprovalArg, RevokeCollectionApprovalError,
    RevokeCollectionApprovalResult, RevokeTokenApprovalArg, RevokeTokenApprovalError,
    RevokeTokenApprovalResult, SftId, TokenApproval, Transaction, TransferFromArg,
    TransferFromError, TransferFromResult,
};
use icrc_ledger_types::icrc1::account::Account;

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

// Entitles a `spender`, specified through an `Account`, to transfer NFTs on behalf of the caller.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc37_approve_tokens(args: Vec<ApproveTokenArg>) -> Vec<Option<ApproveTokenResult>> {
    let caller = ic_cdk::caller();

    if args.is_empty() {
        ic_cdk::trap("no ApproveTokenArgs provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("exceeds max update batch size");
    }

    store::holder_tokens::with_mut(|r| {
        let mut res: Vec<Option<ApproveTokenResult>> = vec![None; args.len()];
        let now = ic_cdk::api::time();
        match r.get(&caller) {
            None => {
                res.fill(Some(Err(ApproveTokenError::Unauthorized)));
            }
            Some(mut tokens) => {
                for (index, arg) in args.iter().enumerate() {
                    if let Err(err) = arg.validate(now, &caller, &settings) {
                        res[index] = Some(Err(err));
                        continue;
                    }

                    let id = SftId::from(&arg.token_id);
                    match tokens.insert_approvals(
                        settings.max_approvals_per_token_or_collection,
                        id.0,
                        id.1,
                        arg.approval_info.spender.owner,
                        arg.approval_info.created_at_time.unwrap_or_default() / SECOND,
                        arg.approval_info.expires_at.unwrap_or_default() / SECOND,
                    ) {
                        Ok(_) => {
                            let tx_log = Transaction::approve(
                                now,
                                id.to_u64(),
                                caller,
                                arg.approval_info.spender.owner,
                                arg.approval_info.expires_at,
                                arg.approval_info.memo.to_owned(),
                            );

                            match store::blocks::append(tx_log) {
                                Ok(idx) => {
                                    res[index] = Some(Ok(Nat::from(idx)));
                                }
                                Err(err) => {
                                    res[index] = Some(Err(ApproveTokenError::GenericBatchError {
                                        error_code: Nat::from(0u64),
                                        message: err,
                                    }));
                                    r.insert(caller, tokens);
                                    // break up when append log failed.
                                    return res;
                                }
                            }
                        }
                        Err(err) => {
                            res[index] = Some(Err(err));
                        }
                    }
                }

                r.insert(caller, tokens);
            }
        }
        res
    })
}

// Entitles a `spender`, specified through an `Account`, to transfer any NFT of the collection hosted on this ledger and owned by the caller at the time of transfer on behalf of the caller
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc37_approve_collection(
    args: Vec<ApproveCollectionArg>,
) -> Vec<Option<ApproveCollectionResult>> {
    let caller = ic_cdk::caller();

    if args.is_empty() {
        ic_cdk::trap("no ApproveCollectionArg provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("exceeds max update batch size");
    }

    store::approvals::with_mut(|r| {
        let mut res: Vec<Option<ApproveCollectionResult>> = vec![None; args.len()];
        let now = ic_cdk::api::time();
        let mut approvals = r.get(&caller).unwrap_or_default();
        let mut total = approvals.total();
        if total >= settings.max_approvals_per_token_or_collection as u32 {
            res.fill(Some(Err(ApproveCollectionError::GenericBatchError {
                error_code: Nat::from(0u64),
                message: "exceeds the maximum number of approvals".to_string(),
            })));
        } else {
            for (index, arg) in args.iter().enumerate() {
                if let Err(err) = arg.validate(now, &caller, &settings) {
                    res[index] = Some(Err(err));
                    continue;
                }
                if total >= settings.max_approvals_per_token_or_collection as u32 {
                    res[index] = Some(Err(ApproveCollectionError::GenericBatchError {
                        error_code: Nat::from(0u64),
                        message: "exceeds the maximum number of approvals".to_string(),
                    }));
                    continue;
                }

                approvals.insert(
                    arg.approval_info.spender.owner,
                    arg.approval_info.created_at_time.unwrap_or_default() / SECOND,
                    arg.approval_info.expires_at.unwrap_or_default() / SECOND,
                );
                total += 1;

                let tx_log = Transaction::approve_collection(
                    now,
                    caller,
                    arg.approval_info.spender.owner,
                    arg.approval_info.expires_at,
                    arg.approval_info.memo.to_owned(),
                );

                match store::blocks::append(tx_log) {
                    Ok(idx) => {
                        res[index] = Some(Ok(Nat::from(idx)));
                    }
                    Err(err) => {
                        res[index] = Some(Err(ApproveCollectionError::GenericBatchError {
                            error_code: Nat::from(0u64),
                            message: err,
                        }));
                        r.insert(caller, approvals);
                        // break up when append log failed.
                        return res;
                    }
                }
            }

            r.insert(caller, approvals);
        }

        res
    })
}

// Revokes the specified approvals for a token given by `token_id` from the set of active approvals.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc37_revoke_token_approvals(
    args: Vec<RevokeTokenApprovalArg>,
) -> Vec<Option<RevokeTokenApprovalResult>> {
    let caller = ic_cdk::caller();

    if args.is_empty() {
        ic_cdk::trap("no ApproveCollectionArg provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.len() > settings.max_revoke_approvals as usize {
        ic_cdk::trap("exceeds max revoke approvals");
    }

    store::holder_tokens::with_mut(|r| {
        let mut res: Vec<Option<RevokeTokenApprovalResult>> = vec![None; args.len()];
        let now = ic_cdk::api::time();
        match r.get(&caller) {
            None => {
                res.fill(Some(Err(RevokeTokenApprovalError::Unauthorized)));
            }
            Some(mut tokens) => {
                for (index, arg) in args.iter().enumerate() {
                    if let Err(err) = arg.validate(now, &caller, &settings) {
                        res[index] = Some(Err(err));
                        continue;
                    }

                    let id = SftId::from(&arg.token_id);
                    let spender = arg.spender.map(|s| s.owner);
                    match tokens.revoke(id.0, id.1, spender) {
                        Err(err) => {
                            res[index] = Some(Err(err));
                        }
                        Ok(_) => {
                            let tx_log = Transaction::revoke(
                                now,
                                id.to_u64(),
                                caller,
                                spender,
                                arg.memo.to_owned(),
                            );

                            match store::blocks::append(tx_log) {
                                Ok(idx) => {
                                    res[index] = Some(Ok(Nat::from(idx)));
                                }
                                Err(err) => {
                                    res[index] =
                                        Some(Err(RevokeTokenApprovalError::GenericBatchError {
                                            error_code: Nat::from(0u64),
                                            message: err,
                                        }));
                                    r.insert(caller, tokens);
                                    // break up when append log failed.
                                    return res;
                                }
                            }
                        }
                    }
                }

                r.insert(caller, tokens);
            }
        }

        res
    })
}

// Revokes collection-level approvals from the set of active approvals.
#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc37_revoke_collection_approvals(
    args: Vec<RevokeCollectionApprovalArg>,
) -> Vec<Option<RevokeCollectionApprovalResult>> {
    let caller = ic_cdk::caller();

    if args.is_empty() {
        ic_cdk::trap("no RevokeCollectionApprovalArg provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());
    if args.len() > settings.max_revoke_approvals as usize {
        ic_cdk::trap("exceeds max revoke approvals");
    }
    let now = ic_cdk::api::time();
    let mut idxs: Vec<usize> = Vec::new();
    let mut spenders: Vec<Option<Principal>> = Vec::new();
    let mut res: Vec<Option<RevokeCollectionApprovalResult>> = vec![None; spenders.len()];
    for (i, arg) in args.iter().enumerate() {
        if let Err(err) = arg.validate(now, &caller, &settings) {
            res[i] = Some(Err(err));
            continue;
        }

        idxs.push(i);
        spenders.push(arg.spender.map(|s| s.owner));
    }

    let res2 = store::approvals::revoke(&caller, &spenders);
    for (i, idx) in idxs.into_iter().enumerate() {
        match res2[i] {
            Some(ref val) => {
                // some error
                res[idx] = Some(val.to_owned());
            }
            None => {
                let tx_log = Transaction::revoke_collection(
                    now,
                    caller,
                    spenders[i],
                    args[idx].memo.to_owned(),
                );

                match store::blocks::append(tx_log) {
                    Ok(block_idx) => {
                        res[idx] = Some(Ok(Nat::from(block_idx)));
                    }
                    Err(err) => {
                        res[idx] = Some(Err(RevokeCollectionApprovalError::GenericBatchError {
                            error_code: Nat::from(0u64),
                            message: err,
                        }));
                        // break up when append log failed.
                        // return res; // TODO: uncomment this line
                    }
                }
            }
        }
    }

    res
}

#[ic_cdk::update(guard = "is_authenticated")]
pub fn icrc37_transfer_from(args: Vec<TransferFromArg>) -> Vec<Option<TransferFromResult>> {
    if args.is_empty() {
        ic_cdk::trap("no transfer args provided")
    }

    let settings = store::collection::with(|c| c.settings.clone());

    if args.len() > settings.max_update_batch_size as usize {
        ic_cdk::trap("exceeds max update batch size");
    }

    let caller = ic_cdk::caller();
    let now = ic_cdk::api::time();
    let now_sec = now / SECOND;
    if settings.atomic_batch_transfers && args.len() > 1 {
        if let Some(err) = args
            .iter()
            .find_map(|arg| arg.validate(now, &caller, &settings).err())
        {
            ic_cdk::trap(format!("invalid transfer from args: {:?}", err).as_str())
        }

        let query: Vec<(SftId, &Principal)> = args
            .iter()
            .map(|arg| (SftId::from(&arg.token_id), &arg.from.owner))
            .collect();

        let query = store::approvals::find_unapproved(&caller, &query, now_sec);

        if let Err(from) = store::holder_tokens::all_is_approved(&caller, &query, now_sec) {
            ic_cdk::trap(
                format!("(from: {}, spender: {}) are not approved", from, caller).as_str(),
            );
        }
    }

    store::holders::with_mut(|r| {
        let mut res: Vec<Option<TransferFromResult>> = vec![None; args.len()];
        for (index, arg) in args.iter().enumerate() {
            if let Err(err) = arg.validate(now, &caller, &settings) {
                res[index] = Some(Err(err));
                continue;
            }

            let id = SftId::from(&arg.token_id);
            if !store::approvals::is_approved(&arg.from.owner, &caller, now_sec)
                && !store::holder_tokens::is_approved(&arg.from.owner, &caller, id.0, id.1, now_sec)
            {
                res[index] = Some(Err(TransferFromError::Unauthorized));
                continue;
            }

            match r.get(&id.0) {
                None => {
                    res[index] = Some(Err(TransferFromError::NonExistingTokenId));
                }
                Some(mut holders) => {
                    match holders.transfer_from(&arg.from.owner, &arg.to.owner, id.1) {
                        Ok(_) => {
                            let tx_log = Transaction::transfer_from(
                                now,
                                id.to_u64(),
                                arg.from.owner,
                                arg.to.owner,
                                caller,
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
                                    res[index] = Some(Err(TransferFromError::GenericBatchError {
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
                    }
                }
            }
        }

        res
    })
}
