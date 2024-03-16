use candid::{Nat, Principal};
use icrc_ledger_types::icrc::generic_metadata_value::MetadataValue;

use crate::types::{
    ApproveCollectionArg, ApproveCollectionError, ApproveCollectionResult, ApproveTokenArg,
    ApproveTokenError, ApproveTokenResult, MintArg, MintError, MintResult,
    RevokeCollectionApprovalArg, RevokeCollectionApprovalError, RevokeCollectionApprovalResult,
    RevokeTokenApprovalArg, RevokeTokenApprovalError, RevokeTokenApprovalResult, SftId,
    TransferArg, TransferError, TransferFromArg, TransferFromError, TransferFromResult,
    TransferResult,
};
use crate::utils::{sha3_256, to_cbor_bytes};
use crate::{is_authenticated, store, SECOND};

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
                        Some(caller),
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
                            let tx_log = store::Transaction::approve(
                                now / SECOND,
                                id.to_u64(),
                                caller,
                                arg.approval_info.spender.owner,
                                arg.approval_info.expires_at,
                                arg.approval_info.memo.to_owned(),
                            );

                            match store::transactions::append(&tx_log) {
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

                let tx_log = store::Transaction::approve_collection(
                    now / SECOND,
                    caller,
                    arg.approval_info.spender.owner,
                    arg.approval_info.expires_at,
                    arg.approval_info.memo.to_owned(),
                );

                match store::transactions::append(&tx_log) {
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
                            let tx_log = store::Transaction::revoke(
                                now / SECOND,
                                id.to_u64(),
                                caller,
                                spender,
                                arg.memo.to_owned(),
                            );

                            match store::transactions::append(&tx_log) {
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
                let tx_log = store::Transaction::revoke_collection(
                    now / SECOND,
                    caller,
                    spenders[i],
                    args[idx].memo.to_owned(),
                );

                match store::transactions::append(&tx_log) {
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
                            let tx_log = store::Transaction::transfer_from(
                                now / SECOND,
                                id.to_u64(),
                                arg.from.owner,
                                arg.to.owner,
                                caller,
                                arg.memo.clone(),
                            );

                            match store::transactions::append(&tx_log) {
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
