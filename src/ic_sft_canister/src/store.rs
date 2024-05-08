use candid::{Nat, Principal};
use ciborium::{from_reader, into_writer};
use ic_sft_types::{
    ApprovalInfo, ApproveTokenError, Metadata, RevokeCollectionApprovalError,
    RevokeCollectionApprovalResult, RevokeTokenApprovalError, SftId, TransferError,
    TransferFromError, Value,
};
use ic_sft_types::{Block, Transaction};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, StableLog, StableVec, Storable,
};
use icrc_ledger_types::{icrc::generic_value::Hash, icrc1::account::Account};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use crate::utils::mac_256;

type Memory = VirtualMemory<DefaultMemoryImpl>;

const COLLECTION_MEMORY_ID: MemoryId = MemoryId::new(0);
const KEYS_MEMORY_ID: MemoryId = MemoryId::new(1);
const TOKENS_MEMORY_ID: MemoryId = MemoryId::new(2);
const HOLDERS_MEMORY_ID: MemoryId = MemoryId::new(3);
const HOLDER_TOKENS_MEMORY_ID: MemoryId = MemoryId::new(4);
const HOLDER_APPROVALS_MEMORY_ID: MemoryId = MemoryId::new(5);
const ASSETS_MEMORY_ID: MemoryId = MemoryId::new(6);
const BLOCKS_INDEX_MEMORY_ID: MemoryId = MemoryId::new(7);
const BLOCKS_DATA_MEMORY_ID: MemoryId = MemoryId::new(8);

thread_local! {
    static CHALLENGE_SECRET: RefCell<[u8; 32]> = const { RefCell::new([0; 32]) };

    static COLLECTION_HEAP: RefCell<Collection> = RefCell::new(Collection::default());

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static COLLECTION: RefCell<StableCell<Collection, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(COLLECTION_MEMORY_ID)),
            Collection::default()
        ).expect("failed to init COLLECTION store")
    );

    static KEYS: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(KEYS_MEMORY_ID)),
        )
    );

    static TOKENS: RefCell<StableVec<Token, Memory>> = RefCell::new(
        StableVec::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(TOKENS_MEMORY_ID))
        ).expect("failed to init TOKENS store")
    );

    static HOLDERS: RefCell<StableBTreeMap<u32, Holders, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HOLDERS_MEMORY_ID)),
        )
    );

    static HOLDER_TOKENS: RefCell<StableBTreeMap<Principal, HolderTokens, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HOLDER_TOKENS_MEMORY_ID)),
        )
    );

    static HOLDER_APPROVALS: RefCell<StableBTreeMap<Principal, Approvals, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HOLDER_APPROVALS_MEMORY_ID)),
        )
    );

    static ASSETS: RefCell<StableBTreeMap<[u8; 32], Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(ASSETS_MEMORY_ID)),
        )
    );

    static BLOCKS: RefCell<StableLog<Block, Memory, Memory>> = RefCell::new(
        StableLog::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(BLOCKS_INDEX_MEMORY_ID)),
            MEMORY_MANAGER.with_borrow(|m| m.get(BLOCKS_DATA_MEMORY_ID)),
        ).expect("failed to init BLOCKS store")
    );
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Collection {
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub assets_origin: Option<String>, // for example, "https://assets.panda.fans"
    pub total_supply: u64,
    pub supply_cap: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_block_index: Option<u64>,
    pub last_block_hash: Option<Hash>,

    pub minters: BTreeSet<Principal>,
    pub managers: BTreeSet<Principal>,
    pub settings: Settings,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    pub max_query_batch_size: u16,
    pub max_update_batch_size: u16,
    pub default_take_value: u16,
    pub max_take_value: u16,
    pub max_memo_size: u16,
    pub atomic_batch_transfers: bool,
    pub tx_window: u64,                             // in seconds
    pub permitted_drift: u64,                       // in seconds
    pub max_approvals_per_token_or_collection: u16, // in seconds
    pub max_revoke_approvals: u16,                  // in seconds
}

impl Storable for Collection {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Collection data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Collection data")
    }
}

impl Collection {
    pub fn metadata(&self) -> Metadata {
        let mut res = Metadata::new();
        res.insert("icrc7:symbol".to_string(), Value::Text(self.symbol.clone()));
        res.insert("icrc7:name".to_string(), Value::Text(self.name.clone()));
        if let Some(ref description) = self.description {
            res.insert(
                "icrc7:description".to_string(),
                Value::Text(description.clone()),
            );
        }
        if let Some(ref logo) = self.logo {
            res.insert("icrc7:logo".to_string(), Value::Text(logo.clone()));
        }
        res.insert(
            "icrc7:total_supply".to_string(),
            Value::Nat(self.total_supply.into()),
        );
        if let Some(supply_cap) = self.supply_cap {
            res.insert(
                "icrc7:supply_cap".to_string(),
                Value::Nat(supply_cap.into()),
            );
        }
        res
    }

    pub fn icrc37_metadata(&self) -> Metadata {
        let mut res = Metadata::new();
        if self.settings.max_approvals_per_token_or_collection > 0 {
            res.insert(
                "icrc37:max_approvals_per_token_or_collection".to_string(),
                Value::Nat((self.settings.max_approvals_per_token_or_collection as u64).into()),
            );
        }
        if self.settings.max_revoke_approvals > 0 {
            res.insert(
                "icrc37:max_revoke_approvals".to_string(),
                Value::Nat((self.settings.max_revoke_approvals as u64).into()),
            );
        }
        res
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Token {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub asset_name: String,
    pub asset_content_type: String,
    pub asset_hash: [u8; 32],
    pub metadata: Metadata,
    pub author: Principal,
    pub supply_cap: Option<u32>,
    pub total_supply: u32,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Storable for Token {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Token data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Token data")
    }
}

impl Token {
    pub fn metadata(&self) -> Metadata {
        let mut res = self.metadata.clone();
        res.insert("icrc7:name".to_string(), Value::Text(self.name.clone()));
        if let Some(ref description) = self.description {
            res.insert(
                "icrc7:description".to_string(),
                Value::Text(description.clone()),
            );
        }
        res.insert(
            "asset_name".to_string(),
            Value::Text(self.asset_name.clone()),
        );
        res.insert(
            "asset_content_type".to_string(),
            Value::Text(self.asset_content_type.clone()),
        );
        res.insert(
            "asset_hash".to_string(),
            Value::Blob(ByteBuf::from(self.asset_hash.as_slice())),
        );
        res
    }
}

// spender -> (created_at, expires_at)
// in seconds since the epoch (1970-01-01), 0 means None
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Approvals(BTreeMap<Principal, (u64, u64)>);
pub type ApprovalItem<'a> = (&'a Principal, &'a (u64, u64));

impl Storable for Approvals {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(&self.0, &mut buf).expect("failed to encode Approvals data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Approvals data")
    }
}

impl Approvals {
    pub fn to_info(item: ApprovalItem) -> ApprovalInfo {
        ApprovalInfo {
            spender: Account {
                owner: *item.0,
                subaccount: None,
            },
            from_subaccount: None,
            created_at_time: if item.1 .0 > 0 { Some(item.1 .0) } else { None },
            expires_at: if item.1 .1 > 0 { Some(item.1 .1) } else { None },
            memo: None,
        }
    }

    pub fn total(&self) -> u32 {
        self.0.len() as u32
    }

    pub fn iter(&self) -> impl Iterator<Item = ApprovalItem> {
        self.0.iter()
    }

    pub fn get(&self, spender: &Principal) -> Option<(u64, u64)> {
        self.0.get(spender).cloned()
    }

    pub fn insert(&mut self, spender: Principal, create_at_sec: u64, exp_sec: u64) {
        self.0.insert(spender, (create_at_sec, exp_sec));
    }

    pub fn revoke(&mut self, spender: &Principal) -> Option<(u64, u64)> {
        self.0.remove(spender)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Holders(Vec<Principal>);

impl Storable for Holders {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(&self.0, &mut buf).expect("failed to encode Holders data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Holders data")
    }
}

impl Holders {
    pub fn total(&self) -> u32 {
        self.0.len() as u32
    }

    pub fn get(&self, sid: u32) -> Option<&Principal> {
        self.0.get(sid as usize)
    }

    pub fn is_holder(&self, sid: u32, account: &Principal) -> bool {
        self.0
            .get(sid as usize)
            .map_or(false, |holder| holder == account)
    }

    pub fn append(&mut self, account: Principal) {
        self.0.push(account);
    }

    pub fn transfer_to(
        &mut self,
        from: &Principal,
        to: &Principal,
        sid: u32,
    ) -> Result<(), TransferError> {
        let holder = self
            .0
            .get_mut(sid as usize)
            .ok_or(TransferError::NonExistingTokenId)?;
        if holder != from {
            return Err(TransferError::Unauthorized);
        }
        *holder = *to;
        Ok(())
    }

    pub fn transfer_from(
        &mut self,
        from: &Principal,
        to: &Principal,
        sid: u32,
    ) -> Result<(), TransferFromError> {
        let holder = self
            .0
            .get_mut(sid as usize)
            .ok_or(TransferFromError::NonExistingTokenId)?;
        if holder != from {
            return Err(TransferFromError::Unauthorized);
        }
        *holder = *to;
        Ok(())
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct HolderTokens(BTreeMap<u32, BTreeMap<u32, Option<Approvals>>>);

impl Storable for HolderTokens {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(&self.0, &mut buf).expect("failed to encode HolderTokens data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode HolderTokens data")
    }
}

impl HolderTokens {
    pub fn balance_of(&self) -> u64 {
        self.0.values().map(|records| records.len() as u64).sum()
    }

    pub fn token_ids(&self) -> Vec<u32> {
        self.0.keys().cloned().collect()
    }

    pub fn get_sids(&self, tid: u32) -> Option<Vec<u32>> {
        self.0
            .get(&tid)
            .map(|records| records.keys().cloned().collect())
    }

    pub fn clear_for_transfer(&mut self, tid: u32, sid: u32) -> usize {
        if let Some(records) = self.0.get_mut(&tid) {
            records.remove(&sid);
            if records.is_empty() {
                self.0.remove(&tid);
            }
        }
        self.0.len()
    }

    pub fn get_approvals(&self, tid: u32, sid: u32) -> Option<&Approvals> {
        if let Some(records) = self.0.get(&tid) {
            records.get(&sid).and_then(|approvals| approvals.as_ref())
        } else {
            None
        }
    }

    pub fn insert_approvals(
        &mut self,
        max_approvals: u16,
        tid: u32,
        sid: u32,
        spender: Principal,
        create_at_sec: u64,
        exp_sec: u64,
    ) -> Result<(), ApproveTokenError> {
        match self.0.get_mut(&tid) {
            None => Err(ApproveTokenError::NonExistingTokenId),
            Some(records) => match records.get_mut(&sid) {
                None => Err(ApproveTokenError::NonExistingTokenId),
                Some(None) => {
                    let mut approvals = Approvals::default();
                    approvals.insert(spender, create_at_sec, exp_sec);
                    records.insert(sid, Some(approvals));
                    Ok(())
                }
                Some(Some(approvals)) => {
                    if approvals.total() >= max_approvals as u32 {
                        Err(ApproveTokenError::GenericBatchError {
                            error_code: Nat::from(0u64),
                            message: "exceeds the maximum number of approvals".to_string(),
                        })
                    } else {
                        approvals.insert(spender, create_at_sec, exp_sec);
                        Ok(())
                    }
                }
            },
        }
    }

    pub fn revoke(
        &mut self,
        tid: u32,
        sid: u32,
        spender: Option<Principal>,
    ) -> Result<(), RevokeTokenApprovalError> {
        if let Some(records) = self.0.get_mut(&tid) {
            if let Some(approvals) = records.get_mut(&sid) {
                match spender {
                    Some(spender) => match approvals {
                        Some(approvals) => {
                            if approvals.0.remove(&spender).is_none() {
                                return Err(RevokeTokenApprovalError::ApprovalDoesNotExist);
                            }
                            return Ok(());
                        }
                        None => {
                            return Err(RevokeTokenApprovalError::ApprovalDoesNotExist);
                        }
                    },
                    None => {
                        *approvals = None;
                    }
                }
            }
        }

        Err(RevokeTokenApprovalError::NonExistingTokenId)
    }
}

pub mod keys {
    use super::*;

    pub async fn load() {
        let keys = KEYS.with(|r| r.borrow().iter().collect::<BTreeMap<String, Vec<u8>>>());
        {
            let mut secret: Vec<u8> = match keys.get("CHALLENGE_SECRET") {
                Some(secret) => secret.clone(),
                None => vec![],
            };
            if secret.len() != 32 {
                let rr = ic_cdk::api::management_canister::main::raw_rand()
                    .await
                    .expect("failed to get random bytes");
                secret = mac_256(&rr.0, b"CHALLENGE_SECRET").to_vec();
            }
            CHALLENGE_SECRET.with(|r| r.borrow_mut().copy_from_slice(&secret));
        }
    }

    pub fn save() {
        KEYS.with(|r| {
            r.borrow_mut().insert(
                "CHALLENGE_SECRET".to_string(),
                CHALLENGE_SECRET.with(|r| r.borrow().to_vec()),
            );
        });
    }

    pub fn with_challenge_secret<R>(f: impl FnOnce(&[u8; 32]) -> R) -> R {
        CHALLENGE_SECRET.with(|r| f(&r.borrow()))
    }
}

// pub mod challenge {
//     use super::*;

//     pub fn with_secret<R>(f: impl FnOnce(&[u8]) -> R) -> R {
//         CHALLENGE_SECRET.with(|r| f(r.borrow().as_slice()))
//     }

//     pub fn set_secret(secret: [u8; 32]) {
//         CHALLENGE_SECRET.with(|r| *r.borrow_mut() = secret);
//     }
// }

pub mod collection {
    use super::*;

    pub fn take_value(take: Option<u64>) -> u16 {
        with(|c| {
            take.map_or(c.settings.default_take_value, |t| {
                t.min(c.settings.max_take_value as u64) as u16
            })
        })
    }

    pub fn with<R>(f: impl FnOnce(&Collection) -> R) -> R {
        COLLECTION_HEAP.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Collection) -> R) -> R {
        COLLECTION_HEAP.with(|r| f(&mut r.borrow_mut()))
    }

    pub fn load() {
        COLLECTION.with(|r| {
            COLLECTION_HEAP.with(|h| {
                *h.borrow_mut() = r.borrow().get().clone();
            });
        });
    }

    pub fn save() {
        COLLECTION_HEAP.with(|h| {
            COLLECTION.with(|r| {
                r.borrow_mut()
                    .set(h.borrow().clone())
                    .expect("failed to set COLLECTION data");
            });
        });
    }
}

pub mod tokens {
    use super::*;

    pub fn with<R>(f: impl FnOnce(&StableVec<Token, Memory>) -> R) -> R {
        TOKENS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut StableVec<Token, Memory>) -> R) -> R {
        TOKENS.with(|r| f(&mut r.borrow_mut()))
    }
}

pub mod holders {
    use super::*;

    pub fn with<R>(f: impl FnOnce(&StableBTreeMap<u32, Holders, Memory>) -> R) -> R {
        HOLDERS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut StableBTreeMap<u32, Holders, Memory>) -> R) -> R {
        HOLDERS.with(|r| f(&mut r.borrow_mut()))
    }
}

pub mod holder_tokens {
    use super::*;

    pub fn is_approved(
        from: &Principal,
        spender: &Principal,
        tid: u32,
        sid: u32,
        now_sec: u64,
    ) -> bool {
        with(|r| {
            if let Some(tokens) = r.get(from) {
                if let Some(records) = tokens.0.get(&tid) {
                    if let Some(Some(approvals)) = records.get(&sid) {
                        return approvals
                            .0
                            .get(spender)
                            .map_or(false, |(_, expire_at)| expire_at > &now_sec);
                    }
                }
            }
            false
        })
    }

    pub fn spenders_is_approved(
        from: &Principal,
        args: &[(SftId, &Principal)],
        now_sec: u64,
    ) -> Vec<bool> {
        with(|r| {
            let mut res = vec![false; args.len()];
            if let Some(tokens) = r.get(from) {
                for (i, (id, spender)) in args.iter().enumerate() {
                    if let Some(records) = tokens.0.get(&id.0) {
                        if let Some(Some(approvals)) = records.get(&id.1) {
                            res[i] = approvals
                                .0
                                .get(spender)
                                .map_or(false, |(_, expire_at)| expire_at > &now_sec);
                        }
                    }
                }
            }
            res
        })
    }

    // used by atomic_batch_transfers checking
    pub fn all_is_approved<'a>(
        spender: &Principal,
        args: &'a [&(SftId, &Principal)],
        now_sec: u64,
    ) -> Result<(), &'a Principal> {
        with(|r| {
            for arg in args.iter() {
                match r.get(arg.1) {
                    None => return Err(arg.1),
                    Some(tokens) => match tokens.0.get(&arg.0 .0) {
                        None => return Err(arg.1),
                        Some(records) => match records.get(&arg.0 .1) {
                            None => return Err(arg.1),
                            Some(None) => return Err(arg.1),
                            Some(Some(approvals)) => match approvals.get(spender) {
                                None => return Err(arg.1),
                                Some((_, expire_at)) => {
                                    if expire_at <= now_sec {
                                        return Err(arg.1);
                                    }
                                }
                            },
                        },
                    },
                }
            }

            Ok(())
        })
    }

    pub fn update_for_transfer(from: Principal, to: Principal, tid: u32, sid: u32) {
        with_mut(|r| {
            if let Some(mut tokens) = r.get(&from) {
                if tokens.clear_for_transfer(tid, sid) == 0 {
                    r.remove(&from);
                } else {
                    r.insert(from, tokens);
                }
            }

            let mut tokens = r.get(&to).unwrap_or_default();
            tokens.0.entry(tid).or_default().insert(sid, None);
            r.insert(to, tokens);
        });
    }

    pub fn with<R>(f: impl FnOnce(&StableBTreeMap<Principal, HolderTokens, Memory>) -> R) -> R {
        HOLDER_TOKENS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(
        f: impl FnOnce(&mut StableBTreeMap<Principal, HolderTokens, Memory>) -> R,
    ) -> R {
        HOLDER_TOKENS.with(|r| f(&mut r.borrow_mut()))
    }
}

pub mod approvals {
    use super::*;

    pub fn is_approved(from: &Principal, spender: &Principal, now_sec: u64) -> bool {
        with(|r| {
            if let Some(approvals) = r.get(from) {
                if let Some((_, expire_at)) = approvals.0.get(spender) {
                    return expire_at > &now_sec;
                }
            }
            false
        })
    }

    // used by atomic_batch_transfers checking
    pub fn find_unapproved<'a>(
        spender: &Principal,
        args: &'a [(SftId, &Principal)],
        now_sec: u64,
    ) -> Vec<&'a (SftId, &'a Principal)> {
        with(|r| {
            args.iter()
                .filter(|(_, from)| match r.get(from) {
                    None => true,
                    Some(approvals) => match approvals.0.get(spender) {
                        None => true,
                        Some((_, expire_at)) => expire_at <= &now_sec,
                    },
                })
                .collect()
        })
    }

    pub fn spenders_is_approved(
        from: &Principal,
        spenders: &[&Principal],
        now_sec: u64,
    ) -> Vec<bool> {
        with(|r| {
            let mut res = vec![false; spenders.len()];
            if let Some(approvals) = r.get(from) {
                for (i, spender) in spenders.iter().enumerate() {
                    if let Some((_, expire_at)) = approvals.0.get(spender) {
                        res[i] = expire_at > &now_sec;
                    }
                }
            }
            res
        })
    }

    pub fn revoke(
        from: &Principal,
        spenders: &[Option<Principal>],
    ) -> Vec<Option<RevokeCollectionApprovalResult>> {
        with_mut(|r| {
            let mut res: Vec<Option<RevokeCollectionApprovalResult>> = vec![None; spenders.len()];
            if let Some(mut approvals) = r.get(from) {
                for (i, spender) in spenders.iter().enumerate() {
                    match spender {
                        Some(spender) => {
                            if approvals.0.remove(spender).is_none() {
                                res[i] =
                                    Some(Err(RevokeCollectionApprovalError::ApprovalDoesNotExist));
                            }
                        }
                        None => {
                            r.remove(from);
                            return res; // no need to continue
                        }
                    }
                }
            } else {
                res.fill(Some(Err(
                    RevokeCollectionApprovalError::ApprovalDoesNotExist,
                )));
            };

            res
        })
    }

    pub fn with<R>(f: impl FnOnce(&StableBTreeMap<Principal, Approvals, Memory>) -> R) -> R {
        HOLDER_APPROVALS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(
        f: impl FnOnce(&mut StableBTreeMap<Principal, Approvals, Memory>) -> R,
    ) -> R {
        HOLDER_APPROVALS.with(|r| f(&mut r.borrow_mut()))
    }
}

pub mod blocks {
    use super::*;

    pub fn total() -> u64 {
        BLOCKS.with(|r| r.borrow().len())
    }

    pub fn append(tx: Transaction) -> Result<u64, String> {
        collection::with_mut(|c| {
            let blk = Block::new(c.last_block_hash, tx);
            let i = BLOCKS
                .with(|r| r.borrow_mut().append(&blk))
                .map_err(|err| format!("failed to append transaction log, error {:?}", err))?;
            c.last_block_index = Some(i);
            c.last_block_hash = Some(blk.hash());
            Ok(i)
        })
    }
}

pub mod assets {
    use super::*;

    pub fn total() -> u64 {
        ASSETS.with(|r| r.borrow().len())
    }

    pub fn with<R>(f: impl FnOnce(&StableBTreeMap<[u8; 32], Vec<u8>, Memory>) -> R) -> R {
        ASSETS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut StableBTreeMap<[u8; 32], Vec<u8>, Memory>) -> R) -> R {
        ASSETS.with(|r| f(&mut r.borrow_mut()))
    }
}
