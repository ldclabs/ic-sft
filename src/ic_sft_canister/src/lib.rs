mod store;
mod types;

mod api_init;
mod api_sft_manage;
mod api_sft_query;
mod api_sft_update;

use candid::{Nat, Principal};
use hmac::{Hmac, Mac};
use icrc_ledger_types::icrc1::account::Account;
use serde_bytes::ByteBuf;
use sha3::{Digest, Sha3_256};
use std::{borrow::Cow, collections::BTreeSet};

use types::*;

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

pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub fn mac_256(key: &[u8], add: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha3_256>::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(add);
    mac.finalize().into_bytes().into()
}

pub fn to_cbor_bytes(obj: &impl serde::Serialize) -> Cow<[u8]> {
    let mut buf = vec![];
    ciborium::into_writer(obj, &mut buf).expect("Failed to encode in CBOR format");
    Cow::Owned(buf)
}

ic_cdk::export_candid!();
