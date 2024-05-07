use crate::Memo;
use candid::{CandidType, Nat};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};
use serde::{Deserialize, Serialize};
use std::string::ToString;

#[derive(CandidType, Deserialize, Serialize)]
pub struct ApprovalInfo {
    pub spender: Account,
    pub from_subaccount: Option<Subaccount>, // should be None
    pub expires_at: Option<u64>,
    pub created_at_time: Option<u64>, // as nanoseconds since the UNIX epoch in the UTC timezone
    pub memo: Option<Memo>,
}

#[derive(CandidType, Deserialize)]
pub struct ApproveTokenArg {
    pub token_id: Nat,
    pub approval_info: ApprovalInfo,
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
    pub memo: Option<Memo>,
    pub created_at_time: Option<u64>,
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
    pub memo: Option<Memo>,
    pub created_at_time: Option<u64>,
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
    pub memo: Option<Memo>,
    pub created_at_time: Option<u64>,
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
