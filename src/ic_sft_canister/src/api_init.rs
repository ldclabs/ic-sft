use crate::{store, SECOND};
use ic_sft_types::InitArg;
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
        r.settings.max_update_batch_size = args.max_update_batch_size.unwrap_or(20);
        r.settings.default_take_value = args.default_take_value.unwrap_or(10);
        r.settings.max_take_value = args.max_take_value.unwrap_or(100);
        r.settings.max_memo_size = args.max_memo_size.unwrap_or(32);
        r.settings.atomic_batch_transfers = args.atomic_batch_transfers.unwrap_or(false);
        r.settings.tx_window = args.tx_window.unwrap_or(2 * 60 * 60);
        r.settings.permitted_drift = args.permitted_drift.unwrap_or(2 * 60);
        r.settings.max_approvals_per_token_or_collection =
            args.max_approvals_per_token_or_collection.unwrap_or(10);
        r.settings.max_revoke_approvals = args.max_revoke_approvals.unwrap_or(10);
    });

    store::collection::save();
    ic_cdk_timers::set_timer(Duration::from_nanos(0), || {
        ic_cdk::spawn(store::keys::load())
    });
}

#[ic_cdk::pre_upgrade]
pub fn pre_upgrade() {
    store::collection::save();
    store::keys::save();
}

#[ic_cdk::post_upgrade]
pub fn post_upgrade() {
    store::collection::load();

    ic_cdk_timers::set_timer(Duration::from_nanos(0), || {
        ic_cdk::spawn(store::keys::load())
    });
}
