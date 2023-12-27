use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONSTANTS: Item<Constants> = Item::new("constants");

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub count: i32,
    pub owner: String,
}

pub const PIGGY_BANK_ENTRY: Item<PiggyBankEntry> = Item::new("piggy_bank_entry");

#[cw_serde]
pub struct PiggyBankEntry {
    pub amount: Uint128,
    pub unlock: Expiration,
    pub rebase_at_lock: Decimal,
}

pub const LAST_PRICE: Item<Uint128> = Item::new("last_price");

pub const REBASES: Map<u64, Decimal> = Map::new("rebases");
