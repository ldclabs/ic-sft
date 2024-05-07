use candid::{CandidType, Nat, Principal};
use icrc_ledger_types::icrc::generic_value::{ICRC3Map, ICRC3Value};
use num_traits::cast::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{collections::BTreeSet, convert::From, string::ToString};

pub mod icrc3;
pub mod icrc37;
pub mod icrc7;

pub type Metadata = ICRC3Map;
pub type Value = ICRC3Value;

pub use icrc_ledger_types::icrc1::transfer::Memo;

pub use icrc3::*;
pub use icrc37::*;
pub use icrc7::*;

pub fn nat_to_u64(nat: &Nat) -> u64 {
    nat.0.to_u64().unwrap_or(0)
}

pub struct SftId(pub u32, pub u32);

impl SftId {
    pub const MIN: SftId = SftId(1, 1);

    pub fn token_index(&self) -> u32 {
        self.0.saturating_sub(1)
    }

    pub fn to_u64(&self) -> u64 {
        (self.0 as u64) << 32 | self.1 as u64
    }

    pub fn next(&self) -> Self {
        SftId(self.0, self.1.saturating_add(1))
    }
}

impl ToString for SftId {
    fn to_string(&self) -> String {
        format!("{}-{}", self.0, self.1)
    }
}

impl From<u64> for SftId {
    fn from(id: u64) -> Self {
        Self((id >> 32) as u32, (id & u32::MAX as u64) as u32)
    }
}

impl From<&Nat> for SftId {
    fn from(id: &Nat) -> Self {
        Self::from(nat_to_u64(id))
    }
}

#[derive(CandidType, Deserialize)]
pub struct InitArg {
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub assets_origin: Option<String>,
    pub supply_cap: Option<u64>,
    pub max_query_batch_size: Option<u16>,
    pub max_update_batch_size: Option<u16>,
    pub max_take_value: Option<u16>,
    pub default_take_value: Option<u16>,
    pub max_memo_size: Option<u16>,
    pub atomic_batch_transfers: Option<bool>,
    pub tx_window: Option<u64>,
    pub permitted_drift: Option<u64>,
    pub max_approvals_per_token_or_collection: Option<u16>,
    pub max_revoke_approvals: Option<u16>,
}

#[derive(CandidType, Deserialize)]
pub struct UpdateCollectionArg {
    pub name: Option<String>,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub assets_origin: Option<String>,
    pub supply_cap: Option<u64>,
    pub max_query_batch_size: Option<u16>,
    pub max_update_batch_size: Option<u16>,
    pub max_take_value: Option<u16>,
    pub default_take_value: Option<u16>,
    pub max_memo_size: Option<u16>,
    pub atomic_batch_transfers: Option<bool>,
    pub tx_window: Option<u64>,
    pub permitted_drift: Option<u64>,
    pub max_approvals_per_token_or_collection: Option<u16>,
    pub max_revoke_approvals: Option<u16>,
}

#[derive(CandidType, Deserialize, Serialize)]
pub struct ChallengeArg {
    pub author: Principal,
    pub asset_hash: [u8; 32],
}

#[derive(CandidType, Deserialize)]
pub struct CreateTokenArg {
    pub name: String,
    pub description: Option<String>,
    pub asset_name: String,
    pub asset_content_type: String,
    pub asset_content: ByteBuf,
    pub metadata: Metadata,
    pub supply_cap: Option<u32>,
    pub author: Principal,
    pub challenge: Option<ByteBuf>,
}

#[derive(CandidType, Deserialize)]
pub struct UpdateTokenArg {
    pub id: Nat,
    pub name: Option<String>,
    pub description: Option<String>,
    pub asset_name: Option<String>,
    pub asset_content_type: Option<String>,
    pub asset_content: Option<ByteBuf>,
    pub metadata: Option<Metadata>,
    pub supply_cap: Option<u32>,
    pub author: Option<Principal>,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct MintArg {
    pub token_id: Nat,
    pub holders: BTreeSet<Principal>,
}

#[derive(CandidType, Serialize, Clone)]
pub enum MintError {
    NonExistingTokenId,
    SupplyCapReached,
    GenericBatchError { error_code: Nat, message: String },
}

pub type MintResult = Result<Nat, MintError>;

#[derive(CandidType, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SupportedStandard {
    pub name: String,
    pub url: String,
}
