use crate::{
    mac_256, store,
    types::{InitArg, SECOND},
};
use std::time::Duration;

#[ic_cdk::init]
pub fn init(args: InitArg) {
    let now = ic_cdk::api::time() / SECOND;
    store::collection::with_mut(|r| {
        r.symbol = args.symbol;
        r.name = args.name;
        r.description = args.description;
        r.logo = args.logo;
        r.assets_origin = args.assets_origin;
        r.supply_cap = args.supply_cap;
        r.created_at = now;
        r.updated_at = now;
        r.settings.max_query_batch_size = args.max_query_batch_size.unwrap_or(100);
        r.settings.max_update_batch_size = args.max_update_batch_size.unwrap_or(100);
        r.settings.default_take_value = args.default_take_value.unwrap_or(20);
        r.settings.max_take_value = args.max_take_value.unwrap_or(200);
        r.settings.max_memo_size = args.max_memo_size.unwrap_or(32);
        r.settings.atomic_batch_transfers = args.atomic_batch_transfers.unwrap_or(false);
        r.settings.tx_window = args.tx_window.unwrap_or(60 * 60);
        r.settings.permitted_drift = args.permitted_drift.unwrap_or(2 * 60);
    });

    store::collection::save();

    ic_cdk_timers::set_timer(Duration::from_nanos(0), || ic_cdk::spawn(load_secret()));
}

#[ic_cdk::pre_upgrade]
pub fn pre_upgrade() {
    store::collection::save();
}

#[ic_cdk::post_upgrade]
pub fn post_upgrade() {
    store::collection::load();

    ic_cdk_timers::set_timer(Duration::from_nanos(0), || ic_cdk::spawn(load_secret()));
}

async fn load_secret() {
    // can't be used in `init` and `post_upgrade`
    let rr = ic_cdk::api::management_canister::main::raw_rand()
        .await
        .expect("Failed to get random bytes");

    store::signing::set_secret(mac_256(&rr.0, b"SIGNING_SECRET"));
}
