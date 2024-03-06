use candid::{CandidType, Principal};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, StableLog, StableVec, Storable,
};
use icrc_ledger_types::icrc::generic_metadata_value::MetadataValue;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use crate::types::{Icrc7TokenMetadata, TransferError};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
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

    pub minters: BTreeSet<Principal>,
    pub managers: BTreeSet<Principal>,
    pub settings: Settings,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub max_query_batch_size: u16,
    pub max_update_batch_size: u16,
    pub default_take_value: u16,
    pub max_take_value: u16,
    pub max_memo_size: u16,
    pub atomic_batch_transfers: bool,
    pub tx_window: u64,       // in seconds
    pub permitted_drift: u64, // in seconds
}

impl Storable for Collection {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(self, &mut buf).expect("Failed to encode Collection data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        ciborium::de::from_reader(&bytes[..]).expect("Failed to decode Collection data")
    }
}

impl Collection {
    pub fn metadata(&self) -> Icrc7TokenMetadata {
        let mut res = Icrc7TokenMetadata::new();
        res.insert("icrc7:symbol".to_string(), self.symbol.as_str().into());
        res.insert("icrc7:name".to_string(), self.name.as_str().into());
        if let Some(ref description) = self.description {
            res.insert("icrc7:description".to_string(), description.as_str().into());
        }
        if let Some(ref logo) = self.logo {
            res.insert("icrc7:logo".to_string(), logo.as_str().into());
        }
        res.insert("icrc7:total_supply".to_string(), self.total_supply.into());
        if let Some(supply_cap) = self.supply_cap {
            res.insert("icrc7:supply_cap".to_string(), supply_cap.into());
        }
        res
    }
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct Token {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub asset_name: String,
    pub asset_content_type: String,
    pub asset_hash: [u8; 32],
    pub metadata: BTreeMap<String, MetadataValue>,
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
        ciborium::ser::into_writer(self, &mut buf).expect("Failed to encode Token data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        ciborium::de::from_reader(&bytes[..]).expect("Failed to decode Token data")
    }
}

impl Token {
    pub fn metadata(&self) -> Icrc7TokenMetadata {
        let mut res = self.metadata.clone();
        res.insert("icrc7:name".to_string(), self.name.as_str().into());
        if let Some(ref description) = self.description {
            res.insert("icrc7:description".to_string(), description.as_str().into());
        }
        res.insert("asset_name".to_string(), self.asset_name.as_str().into());
        res.insert(
            "asset_content_type".to_string(),
            self.asset_content_type.as_str().into(),
        );
        res.insert("asset_hash".to_string(), self.asset_hash.as_slice().into());
        res
    }
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Approval {
    pub account: Principal,
    pub expires_at: Option<u64>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct Holder {
    pub account: Principal,
    pub approvals: BTreeSet<Approval>,
}

pub struct Holders(Vec<Holder>);

impl Storable for Holders {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(&self.0, &mut buf).expect("Failed to encode Holders data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let holders: Vec<Holder> =
            ciborium::de::from_reader(&bytes[..]).expect("Failed to decode Holders data");
        Holders(holders)
    }
}

impl Holders {
    pub fn total(&self) -> u32 {
        self.0.len() as u32
    }

    pub fn get(&self, sid: u32) -> Option<&Holder> {
        self.0.get(sid as usize)
    }

    pub fn is_holder(&self, sid: u32, account: &Principal) -> bool {
        self.0
            .get(sid as usize)
            .map_or(false, |holder| &holder.account == account)
    }

    pub fn append(&mut self, account: Principal) {
        self.0.push(Holder {
            account,
            approvals: BTreeSet::new(),
        });
    }

    pub fn transfer_at(
        &mut self,
        sid: u32,
        from: &Principal,
        to: &Principal,
    ) -> Result<(), TransferError> {
        let holder = self
            .0
            .get_mut(sid as usize)
            .ok_or(TransferError::NonExistingTokenId)?;
        if &holder.account != from {
            return Err(TransferError::Unauthorized);
        }
        holder.account = *to;
        holder.approvals.clear();
        Ok(())
    }
}

pub struct HolderTokens(BTreeMap<u32, BTreeSet<u32>>);

impl Storable for HolderTokens {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(&self.0, &mut buf).expect("Failed to encode HolderTokens data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let tokens: BTreeMap<u32, BTreeSet<u32>> =
            ciborium::de::from_reader(&bytes[..]).expect("Failed to decode HolderTokens data");
        HolderTokens(tokens)
    }
}

impl HolderTokens {
    pub fn balance_of(&self) -> u64 {
        self.0.values().map(|tokens| tokens.len() as u64).sum()
    }

    pub fn token_ids(&self) -> Vec<u32> {
        self.0.keys().cloned().collect()
    }

    pub fn get_sids(&self, tid: u32) -> Option<&BTreeSet<u32>> {
        self.0.get(&tid)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub ts: u64,    // in seconds since the epoch (1970-01-01)
    pub op: String, // "7mint" | "7burn" | "7xfer"
    pub tid: u64,
    pub from: Option<Principal>,
    pub to: Option<Principal>,
    pub meta: Option<MetadataValue>,
    pub memo: Option<ByteBuf>,
}

impl Transaction {
    pub fn mint(
        now_sec: u64,
        tid: u64,
        to: Principal,
        meta: MetadataValue,
        memo: Option<ByteBuf>,
    ) -> Self {
        Transaction {
            ts: now_sec,
            op: "7mint".to_string(),
            tid,
            from: None,
            to: Some(to),
            meta: Some(meta),
            memo,
        }
    }

    pub fn transfer(
        now_sec: u64,
        tid: u64,
        from: Principal,
        to: Principal,
        memo: Option<ByteBuf>,
    ) -> Self {
        Transaction {
            ts: now_sec,
            op: "7xfer".to_string(),
            tid,
            from: Some(from),
            to: Some(to),
            meta: None,
            memo,
        }
    }
}

impl Storable for Transaction {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(self, &mut buf).expect("Failed to encode Transaction data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        ciborium::de::from_reader(&bytes[..]).expect("Failed to decode Transaction data")
    }
}

const COLLECTION_MEMORY_ID: MemoryId = MemoryId::new(0);
const TOKENS_MEMORY_ID: MemoryId = MemoryId::new(1);
const HOLDERS_MEMORY_ID: MemoryId = MemoryId::new(2);
const HOLDER_TOKENS_MEMORY_ID: MemoryId = MemoryId::new(3);
const ASSETS_MEMORY_ID: MemoryId = MemoryId::new(4);
const TRANSACTIONS_INDEX_MEMORY_ID: MemoryId = MemoryId::new(5);
const TRANSACTIONS_DATA_MEMORY_ID: MemoryId = MemoryId::new(6);

thread_local! {
    static SIGNING_SECRET: RefCell<[u8; 32]> = RefCell::new([0; 32]);

    static COLLECTION_HEAP: RefCell<Collection> = RefCell::new(Collection::default());

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static COLLECTION: RefCell<StableCell<Collection, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(COLLECTION_MEMORY_ID)),
            Collection::default()
        ).expect("Failed to init COLLECTION store")
    );

    static TOKENS: RefCell<StableVec<Token, Memory>> = RefCell::new(
        StableVec::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(TOKENS_MEMORY_ID))
        ).expect("Failed to init TOKENS store")
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

    static ASSETS: RefCell<StableBTreeMap<[u8; 32], Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(ASSETS_MEMORY_ID)),
        )
    );

    static TRANSACTIONS: RefCell<StableLog<Transaction, Memory, Memory>> = RefCell::new(
        StableLog::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(TRANSACTIONS_INDEX_MEMORY_ID)),
            MEMORY_MANAGER.with_borrow(|m| m.get(TRANSACTIONS_DATA_MEMORY_ID)),
        ).expect("Failed to init TRANSACTIONS store")
    );
}

pub mod signing {
    use super::*;

    pub fn with_secret<R>(f: impl FnOnce(&[u8]) -> R) -> R {
        SIGNING_SECRET.with(|r| f(r.borrow().as_slice()))
    }

    pub fn set_secret(secret: [u8; 32]) {
        SIGNING_SECRET.with(|r| *r.borrow_mut() = secret);
    }
}

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

    pub fn with<R>(f: impl FnOnce(&StableBTreeMap<Principal, HolderTokens, Memory>) -> R) -> R {
        HOLDER_TOKENS.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(
        f: impl FnOnce(&mut StableBTreeMap<Principal, HolderTokens, Memory>) -> R,
    ) -> R {
        HOLDER_TOKENS.with(|r| f(&mut r.borrow_mut()))
    }
}

pub mod transactions {
    use super::*;

    pub fn total() -> u64 {
        TRANSACTIONS.with(|r| r.borrow().len())
    }

    pub fn append(tx: &Transaction) -> Result<u64, String> {
        TRANSACTIONS
            .with(|r| r.borrow_mut().append(tx))
            .map_err(|err| format!("Failed to append transaction log, error {:?}", err))
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
