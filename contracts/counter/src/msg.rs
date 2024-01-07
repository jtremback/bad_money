use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_utils::Duration;

#[cw_serde]
pub struct InstantiateMsg {
    pub rebase_interval: Duration,
    pub unlock_interval: Duration,
    pub denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Unlock { amount: Uint128 },
    Withdraw {},
    Rebase {},
}

#[cw_serde]
pub enum QueryMsg {
    GetUnlocks { address: Addr },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct CountResponse {
    pub count: i32,
}
