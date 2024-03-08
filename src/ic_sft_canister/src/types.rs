use std::{
    collections::{BTreeMap, BTreeSet},
    convert::From,
    string::ToString,
};

use candid::{CandidType, Nat, Principal};
use icrc_ledger_types::{
    icrc::generic_metadata_value::MetadataValue,
    icrc1::account::{Account, Subaccount},
};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use crate::{nat_to_u64, store::Settings, ANONYMOUS, SECOND};

pub type Metadata = BTreeMap<String, MetadataValue>;

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
    pub max_approvals_per_token_or_collection: Option<u64>,
    pub max_revoke_approvals: Option<u64>,
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
    pub max_approvals_per_token_or_collection: Option<u64>,
    pub max_revoke_approvals: Option<u64>,
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
    pub metadata: BTreeMap<String, MetadataValue>,
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
    pub metadata: Option<BTreeMap<String, MetadataValue>>,
    pub supply_cap: Option<u32>,
    pub author: Option<Principal>,
}

#[derive(CandidType, Serialize)]
pub struct Standard {
    pub name: String,
    pub url: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TransferArg {
    pub from_subaccount: Option<Subaccount>,
    pub to: Account,
    pub token_id: Nat,
    pub memo: Option<ByteBuf>,
    pub created_at_time: Option<u64>,
}

impl TransferArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), TransferError> {
        if self.from_subaccount.is_some() {
            return Err(TransferError::GenericError {
                error_code: Nat::from(0u64),
                message: "Subaccount is not supported".to_string(),
            });
        }

        if self.to.owner == ANONYMOUS || &self.to.owner == caller || self.to.subaccount.is_some() {
            return Err(TransferError::InvalidRecipient);
        }

        if let Some(ref memo) = self.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(TransferError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "Memo size is too large".to_string(),
                });
            }
        }
        if let Some(created_at_time) = self.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(TransferError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - settings.tx_window * SECOND {
                return Err(TransferError::TooOld);
            }
        }
        Ok(())
    }
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub enum TransferError {
    NonExistingTokenId,
    InvalidRecipient,
    Unauthorized,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    Duplicate { duplicate_of: Nat },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

pub type TransferResult = Result<Nat, TransferError>;

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

#[derive(CandidType, Deserialize)]
pub struct ApprovalArgs {
    pub from_subaccount: Option<Subaccount>,
    pub spender: Account,
    pub token_ids: Option<Vec<u128>>,
    pub expires_at: Option<u64>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>, // as nanoseconds since the UNIX epoch in the UTC timezone
}

#[derive(CandidType, Serialize, Clone)]
pub struct Transaction {
    pub ts: Nat,
    pub op: String, // "7mint" | "7burn" | "7xfer"
    pub tid: Nat,
    pub from: Option<Account>,
    pub to: Option<Account>,
    pub meta: Option<MetadataValue>,
    pub memo: Option<ByteBuf>,
}
