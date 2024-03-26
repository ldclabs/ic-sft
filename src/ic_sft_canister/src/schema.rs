use crate::{store::Settings, ANONYMOUS, SECOND};
use candid::{Nat, Principal};
use ic_sft_types::{
    ApproveCollectionArg, ApproveCollectionError, ApproveTokenArg, ApproveTokenError,
    RevokeCollectionApprovalArg, RevokeCollectionApprovalError, RevokeTokenApprovalArg,
    RevokeTokenApprovalError, TransferArg, TransferError, TransferFromArg, TransferFromError,
};
use std::{convert::From, string::ToString};

pub trait Validate {
    type Error;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error>;
}

impl Validate for TransferArg {
    type Error = TransferError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
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

impl Validate for ApproveTokenArg {
    type Error = ApproveTokenError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
                return Err(ApproveTokenError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Validate for ApproveCollectionArg {
    type Error = ApproveCollectionError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
                return Err(ApproveCollectionError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Validate for RevokeTokenApprovalArg {
    type Error = RevokeTokenApprovalError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
                return Err(RevokeTokenApprovalError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Validate for RevokeCollectionApprovalArg {
    type Error = RevokeCollectionApprovalError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
                return Err(RevokeCollectionApprovalError::GenericError {
                    error_code: Nat::from(0u64),
                    message: "memo size is too large".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Validate for TransferFromArg {
    type Error = TransferFromError;
    fn validate(
        &self,
        now: u64,
        caller: &Principal,
        settings: &Settings,
    ) -> Result<(), Self::Error> {
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
            if memo.0.len() > settings.max_memo_size as usize {
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
