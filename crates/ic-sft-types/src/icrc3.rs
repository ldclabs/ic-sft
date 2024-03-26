use candid::CandidType;
use icrc_ledger_types::{
    icrc::generic_value::{Hash, Map, Value},
    icrc1::{account::Account, transfer::Memo},
};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{convert::From, ops::Deref, string::ToString};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Block(Value);

impl AsRef<Value> for Block {
    #[inline]
    fn as_ref(&self) -> &Value {
        &self.0
    }
}

impl Deref for Block {
    type Target = Value;
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
            Value::Map(map) => Ok(Self(Value::Map(map))),
            _ => Err("block must be a map value".to_string()),
        }
    }
}

impl Block {
    pub fn new(phash: Hash, tx: Transaction) -> Self {
        let mut block = Map::new();
        block.insert("phash".to_string(), Value::Blob(ByteBuf::from(phash)));
        block.insert("btype".to_string(), Value::Text(tx.op));
        block.insert("ts".to_string(), Value::Nat(tx.ts.into()));

        let mut val = Map::new();
        val.insert("tid".to_string(), Value::Nat(tx.tid.into()));
        if let Some(from) = tx.from {
            val.insert("from".to_string(), account_value(from));
        }
        if let Some(to) = tx.to {
            val.insert("to".to_string(), account_value(to));
        }
        if let Some(spender) = tx.spender {
            val.insert("spender".to_string(), account_value(spender));
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
}

#[derive(CandidType, Serialize, Clone)]
pub struct Transaction {
    pub ts: u64,    // in Nanoseconds
    pub op: String, // "7mint" | "7burn" | "7xfer" | "37appr" | "37appr_coll | "37revoke" | "37revoke_coll" | "37xfer"
    pub tid: u64,
    pub from: Option<Account>,
    pub to: Option<Account>,
    pub spender: Option<Account>,
    pub exp: Option<u64>,
    pub meta: Option<Map>,
    pub memo: Option<Memo>,
    pub created_at_time: Option<u64>,
}

// should be replaced with a generic function in icrc_ledger_types::icrc
fn account_value(Account { owner, subaccount }: Account) -> Value {
    let mut parts = vec![Value::blob(owner.as_slice())];
    if let Some(subaccount) = subaccount {
        parts.push(Value::blob(subaccount.as_slice()));
    }
    Value::Array(parts)
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

        let block = Block::new([0; 32], tx);
        println!("{:?}", block.hash());
    }
}
