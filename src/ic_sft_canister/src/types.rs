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
use num_traits::cast::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use crate::{mac_256, store::Settings};

pub const SECOND: u64 = 1_000_000_000;
pub static ANONYMOUS: Principal = Principal::anonymous();

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
}

#[derive(CandidType, Deserialize, Serialize)]
pub struct ChallengeArg {
    pub author: Principal,
    pub asset_hash: [u8; 32],
    pub ts: u64,
}

impl ChallengeArg {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut buf: Vec<u8> = Vec::new();
        ciborium::into_writer(&self.ts, &mut buf)
            .map_err(|_err| "failed to encode time".to_string())?;
        Ok(buf)
    }

    pub fn sign_to_bytes(&self, key: &[u8]) -> Result<ByteBuf, String> {
        let mut time: Vec<u8> = Vec::new();
        ciborium::into_writer(&self.ts, &mut time)
            .map_err(|_err| "failed to encode time".to_string())?;

        let mac = &mac_256(key, self.to_bytes()?.as_slice())[0..16];
        let mut challenge: Vec<u8> = Vec::new();
        ciborium::into_writer(&vec![time.as_slice(), mac], &mut challenge)
            .map_err(|_err| "failed to encode challenge".to_string())?;

        Ok(ByteBuf::from(challenge))
    }

    pub fn verify_from_bytes(
        &mut self,
        key: &[u8],
        challenge: &[u8],
        expire_at: u64,
    ) -> Result<(), String> {
        let arr: Vec<Vec<u8>> =
            ciborium::from_reader(challenge).map_err(|_err| "failed to decode challenge")?;

        if arr.len() != 2 {
            return Err("invalid challenge".to_string());
        }
        self.ts = ciborium::from_reader(&arr[0][..]).map_err(|_err| "failed to decode time")?;
        if self.ts < expire_at {
            return Err("challenge expired".to_string());
        }

        let mac = &mac_256(key, self.to_bytes()?.as_slice())[0..16];
        if mac != &arr[1][..] {
            return Err("failed to verify challenge".to_string());
        }

        Ok(())
    }
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

pub type Icrc7TokenMetadata = BTreeMap<String, MetadataValue>;

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

// #[derive(CandidType, Deserialize)]
// pub struct TransferArgs{
//     pub spender_subaccount: Option<Subaccount>,
//     pub from: Account,
//     pub to: Account,
//     pub token_ids: Vec<u128>,
//     pub memo: Option<Vec<u8>>,
//     pub created_at_time: Option<u64>,
//     pub is_atomic: Option<bool>,
// }

#[derive(CandidType, Deserialize)]
pub struct ApprovalArgs {
    pub from_subaccount: Option<Subaccount>,
    pub spender: Account,
    pub token_ids: Option<Vec<u128>>,
    pub expires_at: Option<u64>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>, // as nanoseconds since the UNIX epoch in the UTC timezone
}

// #[derive(CandidType, Deserialize)]
// pub struct MintArgs{
//     pub id: u128,
//     pub name: String,
//     pub description: Option<String>,
//     pub image: Option<Vec<u8>>,
//     pub to: Account,
// }

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
