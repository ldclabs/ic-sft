mod store;
mod types;
mod utils;

mod api_init;
mod api_sft_manage;
mod api_sft_query;
mod api_sft_update;

use candid::{Nat, Principal};
use icrc_ledger_types::icrc1::account::Account;
use num_traits::cast::ToPrimitive;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use types::*;

pub const SECOND: u64 = 1_000_000_000;
pub static ANONYMOUS: Principal = Principal::anonymous();

pub fn nat_to_u64(nat: &Nat) -> u64 {
    nat.0.to_u64().unwrap_or(0)
}

fn is_controller() -> Result<(), String> {
    if ic_cdk::api::is_controller(&ic_cdk::caller()) {
        Ok(())
    } else {
        Err("User is not a controller".to_string())
    }
}

fn is_authenticated() -> Result<(), String> {
    if ic_cdk::caller() == ANONYMOUS {
        Err("Anonymous user is not allowed".to_string())
    } else {
        Ok(())
    }
}

ic_cdk::export_candid!();
