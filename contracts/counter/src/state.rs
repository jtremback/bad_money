use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;

pub const CONSTANTS: Item<Constants> = Item::new("constants");

#[cw_serde]
pub struct Constants {
    pub rebase_interval: Duration,
    pub unlock_interval: Duration,
    pub denom: String,
}

pub const LOCK_ENTRIES: Map<Addr, LockEntry> = Map::new("locks");

#[cw_serde]
pub struct LockEntry {
    pub amount: Uint128,
    pub rebase_at_lock: Decimal,
}

pub const LAST_PRICE: Item<Uint128> = Item::new("last_price");

pub const REBASES: Map<u64, Decimal> = Map::new("rebases");

pub const UNLOCK_ENTRIES: Map<(Addr, u64), UnlockEntry> = Map::new("unlocks");

#[cw_serde]
pub struct UnlockEntry {
    pub amount: Uint128,
    pub time: Timestamp,
    pub rebase_at_lock: Decimal,
}
