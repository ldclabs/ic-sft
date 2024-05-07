use candid::{CandidType, Principal};
use ciborium::{from_reader, into_writer};
use ic_stable_structures::{storable::Bound, Storable};
use icrc_ledger_types::{
    icrc::generic_value::{Hash, ICRC3Map as Map, Value as OldValue},
    icrc1::{account::Account, transfer::Memo},
    icrc3::blocks::ICRC3GenericBlock,
};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{borrow::Cow, convert::From, ops::Deref, string::ToString};

use crate::{nat_to_u64, Metadata, Value};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Block(ICRC3GenericBlock);

impl AsRef<ICRC3GenericBlock> for Block {
    #[inline]
    fn as_ref(&self) -> &ICRC3GenericBlock {
        &self.0
    }
}

impl Deref for Block {
    type Target = ICRC3GenericBlock;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Map> for Block {
    fn from(value: Map) -> Self {
        Self(Value::Map(value))
    }
}

impl TryFrom<Value> for Block {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Map(_) => Ok(Self(value)),
            _ => Err("block must be a map value".to_string()),
        }
    }
}

impl Storable for Block {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Block data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Block data")
    }
}

impl Block {
    pub fn new(phash: Option<Hash>, tx: Transaction) -> Self {
        let mut block = Map::new();
        if let Some(phash) = phash {
            block.insert("phash".to_string(), Value::Blob(ByteBuf::from(phash)));
        }
        block.insert("btype".to_string(), Value::Text(tx.op));
        block.insert("ts".to_string(), Value::Nat(tx.ts.into()));

        let mut val = Map::new();
        val.insert("tid".to_string(), Value::Nat(tx.tid.into()));
        if let Some(from) = tx.from {
            val.insert("from".to_string(), OldValue::from(from).into());
        }
        if let Some(to) = tx.to {
            val.insert("to".to_string(), OldValue::from(to).into());
        }
        if let Some(spender) = tx.spender {
            val.insert("spender".to_string(), OldValue::from(spender).into());
        }
        if let Some(exp) = tx.exp {
            val.insert("exp".to_string(), Value::Nat(exp.into()));
        }
        if let Some(meta) = tx.meta {
            val.insert("meta".to_string(), Value::Map(meta));
        }
        if let Some(memo) = tx.memo {
            val.insert("memo".to_string(), Value::Blob(memo.0));
        }
        if let Some(created_at_time) = tx.created_at_time {
            val.insert("ts".to_string(), Value::Nat(created_at_time.into()));
        }
        block.insert("tx".to_string(), Value::Map(val));
        Self(Value::Map(block))
    }

    pub fn into_inner(self) -> Value {
        self.0
    }

    pub fn into_map(self) -> Map {
        match self.0 {
            Value::Map(map) => map,
            _ => unreachable!(),
        }
    }

    pub fn hash(self) -> Hash {
        self.0.hash()
    }
}

#[derive(CandidType, Default, Serialize, Clone)]
pub struct Transaction {
    pub ts: u64,    // in nanoseconds
    pub op: String, // "7mint" | "7burn" | "7xfer" | "37approve" | "37approve_coll | "37revoke" | "37revoke_coll" | "37xfer"
    pub tid: u64,
    pub from: Option<Account>,
    pub to: Option<Account>,
    pub spender: Option<Account>,
    pub exp: Option<u64>, // in nanoseconds
    pub meta: Option<Map>,
    pub memo: Option<Memo>,
    pub created_at_time: Option<u64>, // in nanoseconds
}

impl TryFrom<Block> for Transaction {
    type Error = String;
    fn try_from(blk: Block) -> Result<Self, Self::Error> {
        let mut tx = Transaction::default();
        let map = blk.into_map();
        let ts = map.get("ts").ok_or("missing ts field")?;
        let ts = OldValue::from(ts.to_owned()).as_nat()?;
        tx.ts = nat_to_u64(&ts);

        let op = map.get("btype").ok_or("missing btype field")?;
        tx.op = OldValue::from(op.to_owned()).as_text()?;

        let map = map.get("tx").ok_or("missing tx field")?;
        let map = OldValue::from(map.to_owned()).as_map()?;

        let tid = map.get("tid").ok_or("missing tid field")?;
        let tid = tid.to_owned().as_nat()?;
        tx.tid = nat_to_u64(&tid);

        if let Some(from) = map.get("from") {
            tx.from = Some(Account::try_from(from.to_owned())?);
        }
        if let Some(to) = map.get("to") {
            tx.to = Some(Account::try_from(to.to_owned())?);
        }
        if let Some(spender) = map.get("spender") {
            tx.spender = Some(Account::try_from(spender.to_owned())?);
        }
        if let Some(exp) = map.get("exp") {
            tx.exp = Some(nat_to_u64(&exp.to_owned().as_nat()?));
        }
        if let Some(ts) = map.get("ts") {
            tx.created_at_time = Some(nat_to_u64(&ts.to_owned().as_nat()?));
        }
        if let Some(memo) = map.get("memo") {
            tx.memo = Some(Memo(memo.to_owned().as_blob()?));
        }
        if let Some(meta) = map.get("meta") {
            tx.meta = Some(
                meta.to_owned()
                    .as_map()?
                    .into_iter()
                    .map(|(k, v)| (k, Value::from(v)))
                    .collect(),
            );
        }
        Ok(tx)
    }
}

impl Transaction {
    pub fn mint(
        now_ns: u64,
        tid: u64,
        from: Option<Principal>,
        to: Principal,
        meta: Metadata,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "7mint".to_string(),
            tid,
            from: from.map(|owner| Account {
                owner,
                subaccount: None,
            }),
            to: Some(Account {
                owner: to,
                subaccount: None,
            }),
            meta: Some(meta),
            memo,
            ..Default::default()
        }
    }

    pub fn burn(
        now_ns: u64,
        tid: u64,
        from: Principal,
        to: Option<Principal>,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "7burn".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            to: to.map(|owner| Account {
                owner,
                subaccount: None,
            }),
            memo,
            ..Default::default()
        }
    }

    pub fn transfer(
        now_ns: u64,
        tid: u64,
        from: Principal,
        to: Principal,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "7xfer".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            to: Some(Account {
                owner: to,
                subaccount: None,
            }),
            memo,
            ..Default::default()
        }
    }

    pub fn update(
        now_ns: u64,
        tid: u64,
        from: Principal,
        meta: Metadata,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "7update".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            meta: Some(meta),
            memo,
            ..Default::default()
        }
    }

    pub fn approve(
        now_ns: u64,
        tid: u64,
        from: Principal,
        spender: Principal,
        exp_sec: Option<u64>,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "37approve".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            spender: Some(Account {
                owner: spender,
                subaccount: None,
            }),
            exp: exp_sec,
            memo,
            ..Default::default()
        }
    }

    pub fn approve_collection(
        now_ns: u64,
        from: Principal,
        spender: Principal,
        exp_sec: Option<u64>,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "37approve_coll".to_string(),
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            spender: Some(Account {
                owner: spender,
                subaccount: None,
            }),
            exp: exp_sec,
            memo,
            ..Default::default()
        }
    }

    pub fn revoke(
        now_ns: u64,
        tid: u64,
        from: Principal,
        spender: Option<Principal>,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "37revoke".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            spender: spender.map(|owner| Account {
                owner,
                subaccount: None,
            }),
            memo,
            ..Default::default()
        }
    }

    pub fn revoke_collection(
        now_ns: u64,
        from: Principal,
        spender: Option<Principal>,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "37revoke_coll".to_string(),
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            spender: spender.map(|owner| Account {
                owner,
                subaccount: None,
            }),
            memo,
            ..Default::default()
        }
    }

    pub fn transfer_from(
        now_ns: u64,
        tid: u64,
        from: Principal,
        to: Principal,
        spender: Principal,
        memo: Option<Memo>,
    ) -> Self {
        Transaction {
            ts: now_ns,
            op: "37xfer".to_string(),
            tid,
            from: Some(Account {
                owner: from,
                subaccount: None,
            }),
            to: Some(Account {
                owner: to,
                subaccount: None,
            }),
            spender: Some(Account {
                owner: spender,
                subaccount: None,
            }),
            memo,
            ..Default::default()
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use candid::Principal;

    #[test]
    fn block_works() {
        let tx = Transaction {
            ts: 1,
            op: "7mint".to_string(),
            tid: 1,
            from: Some(Account {
                owner: Principal::anonymous(),
                subaccount: None,
            }),
            to: Some(Account {
                owner: Principal::anonymous(),
                subaccount: None,
            }),
            spender: None,
            exp: None,
            meta: None,
            memo: None,
            created_at_time: None,
        };

        let block = Block::new(Some([0; 32]), tx);
        println!("{:?}", block.into_inner().hash());
    }
}
