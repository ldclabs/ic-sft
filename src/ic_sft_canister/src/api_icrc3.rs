use crate::{store, utils::to_cbor_bytes};
use icrc_ledger_types::icrc3::{
    archive::{GetArchivesArgs, GetArchivesResult},
    blocks::{GetBlocksRequest, GetBlocksResult, ICRC3DataCertificate, SupportedBlockType},
};
use serde_bytes::ByteBuf;

static ICRC7_URL: &str = "https://github.com/dfinity/ICRC/blob/main/ICRCs/ICRC-7/ICRC-7.md";
static ICRC37_URL: &str = "https://github.com/dfinity/ICRC/blob/main/ICRCs/ICRC-37/ICRC-37.md";

#[ic_cdk::query]
pub fn icrc3_supported_block_types() -> Vec<SupportedBlockType> {
    vec![
        SupportedBlockType {
            block_type: "7mint".to_string(),
            url: ICRC7_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "7burn".to_string(),
            url: ICRC7_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "7xfer".to_string(),
            url: ICRC7_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "7update".to_string(),
            url: ICRC7_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "37approve".to_string(),
            url: ICRC37_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "37approve_coll".to_string(),
            url: ICRC37_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "37revoke".to_string(),
            url: ICRC37_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "37revoke_coll".to_string(),
            url: ICRC37_URL.to_string(),
        },
        SupportedBlockType {
            block_type: "37xfer".to_string(),
            url: ICRC37_URL.to_string(),
        },
    ]
}

#[ic_cdk::query]
pub fn icrc3_get_tip_certificate() -> Option<ICRC3DataCertificate> {
    let certificate = ByteBuf::from(ic_cdk::api::data_certificate()?);
    let hash_tree = store::collection::with(|r| r.hash_tree());
    let buf = to_cbor_bytes(&hash_tree);
    Some(ICRC3DataCertificate {
        certificate,
        hash_tree: ByteBuf::from(buf),
    })
}

#[ic_cdk::query]
pub fn icrc3_get_archives(_args: GetArchivesArgs) -> GetArchivesResult {
    vec![] // TODO: implement
}

#[ic_cdk::query]
pub fn icrc3_get_blocks(args: Vec<GetBlocksRequest>) -> GetBlocksResult {
    store::blocks::get_blocks(args)
}
