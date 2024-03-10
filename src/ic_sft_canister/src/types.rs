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
    pub from_subaccount: Option<Subaccount>, // should be None
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
        if self.from_subaccount.is_some() || self.to.subaccount.is_some() {
            return Err(TransferError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self.to.owner == ANONYMOUS || &self.to.owner == caller {
            return Err(TransferError::InvalidRecipient);
        }

        if let Some(ref memo) = self.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(TransferError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }
        if let Some(created_at_time) = self.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(TransferError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
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

#[derive(CandidType, Deserialize, Serialize)]
pub struct ApprovalInfo {
    pub spender: Account,
    pub from_subaccount: Option<Subaccount>, // should be None
    pub expires_at: Option<u64>,
    pub created_at_time: Option<u64>, // as nanoseconds since the UNIX epoch in the UTC timezone
    pub memo: Option<ByteBuf>,
}

#[derive(CandidType, Deserialize)]
pub struct ApproveTokenArg {
    pub token_id: Nat,
    pub approval_info: ApprovalInfo,
}

impl ApproveTokenArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), ApproveTokenError> {
        if self.approval_info.from_subaccount.is_some()
            || self.approval_info.spender.subaccount.is_some()
        {
            return Err(ApproveTokenError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self.approval_info.spender.owner == ANONYMOUS
            || &self.approval_info.spender.owner == caller
        {
            return Err(ApproveTokenError::InvalidSpender);
        }

        if let Some(created_at_time) = self.approval_info.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(ApproveTokenError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
                return Err(ApproveTokenError::TooOld);
            }
        }

        if let Some(expires_at) = self.approval_info.expires_at {
            if expires_at < now + settings.permitted_drift * SECOND {
                return Err(ApproveTokenError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "approval expiration time is too close".to_string(),
                });
            }
        }

        if let Some(ref memo) = self.approval_info.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(ApproveTokenError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

pub type ApproveTokenResult = Result<Nat, ApproveTokenError>;

#[derive(CandidType, Serialize, Clone)]
pub enum ApproveTokenError {
    InvalidSpender,
    Unauthorized,
    NonExistingTokenId,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

#[derive(CandidType, Deserialize)]
pub struct ApproveCollectionArg {
    pub approval_info: ApprovalInfo,
}

impl ApproveCollectionArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), ApproveCollectionError> {
        if self.approval_info.from_subaccount.is_some()
            || self.approval_info.spender.subaccount.is_some()
        {
            return Err(ApproveCollectionError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self.approval_info.spender.owner == ANONYMOUS
            || &self.approval_info.spender.owner == caller
        {
            return Err(ApproveCollectionError::InvalidSpender);
        }

        if let Some(created_at_time) = self.approval_info.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(ApproveCollectionError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
                return Err(ApproveCollectionError::TooOld);
            }
        }

        if let Some(expires_at) = self.approval_info.expires_at {
            if expires_at < now + settings.permitted_drift * SECOND {
                return Err(ApproveCollectionError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "approval expiration time is too close".to_string(),
                });
            }
        }

        if let Some(ref memo) = self.approval_info.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(ApproveCollectionError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

pub type ApproveCollectionResult = Result<Nat, ApproveCollectionError>;

#[derive(CandidType, Serialize, Clone)]
pub enum ApproveCollectionError {
    InvalidSpender,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

#[derive(CandidType, Deserialize)]
pub struct RevokeTokenApprovalArg {
    pub spender: Option<Account>, // null revokes matching approvals for all spenders
    pub from_subaccount: Option<Subaccount>, // null refers to the default subaccount
    pub token_id: Nat,
    pub memo: Option<ByteBuf>,
    pub created_at_time: Option<u64>,
}

impl RevokeTokenApprovalArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), RevokeTokenApprovalError> {
        if self.from_subaccount.is_some() || self.spender.map_or(false, |s| s.subaccount.is_some())
        {
            return Err(RevokeTokenApprovalError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self
            .spender
            .map_or(false, |s| s.owner == ANONYMOUS || &s.owner == caller)
        {
            return Err(RevokeTokenApprovalError::GenericError {
                error_code: Nat::from(0u64),
                message: "invalid spender".to_string(),
            });
        }

        if let Some(created_at_time) = self.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(RevokeTokenApprovalError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
                return Err(RevokeTokenApprovalError::TooOld);
            }
        }

        if let Some(ref memo) = self.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(RevokeTokenApprovalError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

pub type RevokeTokenApprovalResult = Result<Nat, RevokeTokenApprovalError>;

#[derive(CandidType, Serialize, Clone)]
pub enum RevokeTokenApprovalError {
    ApprovalDoesNotExist,
    Unauthorized,
    NonExistingTokenId,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

#[derive(CandidType, Deserialize)]
pub struct RevokeCollectionApprovalArg {
    pub spender: Option<Account>, // null revokes matching approvals for all spenders
    pub from_subaccount: Option<Subaccount>, // null refers to the default subaccount
    pub memo: Option<ByteBuf>,
    pub created_at_time: Option<u64>,
}

impl RevokeCollectionApprovalArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), RevokeCollectionApprovalError> {
        if self.from_subaccount.is_some() || self.spender.map_or(false, |s| s.subaccount.is_some())
        {
            return Err(RevokeCollectionApprovalError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self
            .spender
            .map_or(false, |s| s.owner == ANONYMOUS || &s.owner == caller)
        {
            return Err(RevokeCollectionApprovalError::GenericError {
                error_code: Nat::from(0u64),
                message: "invalid spender".to_string(),
            });
        }

        if let Some(created_at_time) = self.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(RevokeCollectionApprovalError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
                return Err(RevokeCollectionApprovalError::TooOld);
            }
        }

        if let Some(ref memo) = self.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(RevokeCollectionApprovalError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

pub type RevokeCollectionApprovalResult = Result<Nat, RevokeCollectionApprovalError>;

#[derive(CandidType, Serialize, Clone)]
pub enum RevokeCollectionApprovalError {
    ApprovalDoesNotExist,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

#[derive(CandidType, Deserialize)]
pub struct IsApprovedArg {
    pub spender: Account,
    pub from_subaccount: Option<Subaccount>, // should be None
    pub token_id: Nat,
}

#[derive(CandidType, Deserialize, Serialize)]
pub struct TokenApproval {
    pub token_id: Nat,
    pub approval_info: ApprovalInfo,
}

pub type CollectionApproval = ApprovalInfo;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TransferFromArg {
    pub spender_subaccount: Option<Subaccount>, // should be None
    pub from: Account,
    pub to: Account,
    pub token_id: Nat,
    pub memo: Option<ByteBuf>,
    pub created_at_time: Option<u64>,
}

impl TransferFromArg {
    pub fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), TransferFromError> {
        if self.spender_subaccount.is_some()
            || self.from.subaccount.is_some()
            || self.to.subaccount.is_some()
        {
            return Err(TransferFromError::GenericError {
                error_code: Nat::from(0u64),
                message: "subaccount is not supported".to_string(),
            });
        }

        if self.from.owner == ANONYMOUS || &self.from.owner == caller {
            return Err(TransferFromError::Unauthorized);
        }

        if self.to.owner == ANONYMOUS || self.to.owner == self.from.owner {
            return Err(TransferFromError::InvalidRecipient);
        }

        if let Some(ref memo) = self.memo {
            if memo.len() > settings.max_memo_size as usize {
                return Err(TransferFromError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }
        if let Some(created_at_time) = self.created_at_time {
            if created_at_time > now + settings.permitted_drift * SECOND {
                return Err(TransferFromError::CreatedInFuture {
                    ledger_time: now + settings.permitted_drift,
                });
            }
            if created_at_time < now - (settings.tx_window + settings.permitted_drift) * SECOND {
                return Err(TransferFromError::TooOld);
            }
        }
        Ok(())
    }
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub enum TransferFromError {
    NonExistingTokenId,
    InvalidRecipient,
    Unauthorized,
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    Duplicate { duplicate_of: Nat },
    GenericError { error_code: Nat, message: String },
    GenericBatchError { error_code: Nat, message: String },
}

pub type TransferFromResult = Result<Nat, TransferFromError>;

#[derive(CandidType, Serialize, Clone)]
pub struct Transaction {
    pub ts: Nat,    // in Nanoseconds
    pub op: String, // "7mint" | "7burn" | "7xfer" | "37appr" | "37appr_coll | "37revoke" | "37revoke_coll" | "37xfer"
    pub tid: Nat,
    pub from: Option<Account>,
    pub to: Option<Account>,
    pub spender: Option<Account>,
    pub exp: Option<Nat>,
    pub meta: Option<MetadataValue>,
    pub memo: Option<ByteBuf>,
}
