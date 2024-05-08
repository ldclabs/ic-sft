mod api_icrc10;
mod api_icrc37;
mod api_icrc7;
mod api_init;
mod api_sft_manage;
mod api_sft_query;
mod api_sft_update;
mod schema;
mod store;
mod utils;

use candid::{Nat, Principal};
use ic_sft_types::*;
use icrc_ledger_types::icrc1::account::Account;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

pub const SECOND: u64 = 1_000_000_000;
pub static ANONYMOUS: Principal = Principal::anonymous();

fn is_controller() -> Result<(), String> {
    if ic_cdk::api::is_controller(&ic_cdk::caller()) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

fn is_authenticated() -> Result<(), String> {
    if ic_cdk::caller() == ANONYMOUS {
        Err("anonymous user is not allowed".to_string())
    } else {
        Ok(())
    }
}

ic_cdk::export_candid!();
