use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONSTANTS: Item<Constants> = Item::new("constants");

#[cw_serde]
pub struct Constants {
    pub count: i32,
    pub owner: String,
}

pub const PIGGY_BANK_ENTRIES: Map<Addr, PiggyBankEntry> = Map::new("piggy_bank_entries");

#[cw_serde]
pub struct PiggyBankEntry {
    pub amount: Uint128,
    pub rebase_at_lock: Decimal,
}

pub const LAST_PRICE: Item<Uint128> = Item::new("last_price");

pub const REBASES: Map<u64, Decimal> = Map::new("rebases");

pub const UNLOCKS: Map<(Addr, u64), Unlock> = Map::new("unlocks");

#[cw_serde]
pub struct Unlock {
    pub amount: Uint128,
    pub time: Timestamp,
    pub rebase_at_lock: Decimal,
}
