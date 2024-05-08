use crate::store;
use candid::Nat;
use ic_sft_types::{nat_to_u64, SftId};

// Returns a vector of `token_id`s of all semi-fungible tokens in the `token_id` Token, sorted by `token_id`.
#[ic_cdk::query]
pub fn sft_tokens_in(token_id: Nat, prev: Option<Nat>, take: Option<Nat>) -> Vec<Nat> {
    let take = store::collection::take_value(take.as_ref().map(nat_to_u64));

    store::holders::with(|r| {
        let id = SftId::from(&token_id);
        r.get(&id.0)
            .map(|hs| {
                let max_sid = hs.total();
                let start_sid = if let Some(ref prev) = prev {
                    SftId::from(prev).1
                } else {
                    1u32
                };
                let mut res: Vec<Nat> = Vec::with_capacity(take as usize);
                for sid in start_sid..=max_sid {
                    res.push(Nat::from(SftId(id.0, sid).to_u64()));
                    if res.len() as u16 >= take {
                        return res;
                    }
                }
                res
            })
            .unwrap_or_default()
    })
}
