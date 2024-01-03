use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};

use crate::error::ContractError;
use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Constants, PiggyBankEntry, Unlock, CONSTANTS, LAST_PRICE, PIGGY_BANK_ENTRIES, REBASES, UNLOCKS,
};
use cosmwasm_std::{coin, Decimal, Uint128};
use cw_storage_plus::Bound;
use osmosis_std::types::osmosis::tokenfactory::v1beta1::MsgMint;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = Constants {
        count: msg.count,
        owner: _info.sender.to_string(),
    };
    CONSTANTS.save(deps.storage, &state)?;
    Ok(Response::new()
        .add_attribute("action", "initialisation")
        .add_attribute("sender", _info.sender.clone()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => deposit(deps, env, info),
        ExecuteMsg::Unlock { amount } => unlock(deps, env, info, amount),
        ExecuteMsg::Withdraw {} => withdraw(deps, env, info),
        ExecuteMsg::Rebase {} => rebase(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => query_count(deps),
    }
}

pub fn deposit(
    deps: DepsMut,
    _: Env,
    message_info: MessageInfo,
) -> Result<Response, ContractError> {
    let last_rebase = REBASES
        .range(deps.storage, None, None, cosmwasm_std::Order::Descending)
        .take(1)
        .last()
        .ok_or(ContractError::NoRebaseRecord {})??
        .1;

    match PIGGY_BANK_ENTRIES.may_load(deps.storage, message_info.sender.clone())? {
        // If pbe does not exist, create pbe with amount being deposited
        None => {
            let piggy_bank_entry = PiggyBankEntry {
                amount: message_info.funds[0].amount,
                rebase_at_lock: last_rebase,
            };
            PIGGY_BANK_ENTRIES.save(
                deps.storage,
                message_info.sender.clone(),
                &piggy_bank_entry,
            )?;
        }
        // If pbe exists, get amount as if withdrawn at current time and add to amount being deposited,
        // overwriting pbe with current rebase_at_lock
        Some(piggy_bank_entry) => {
            let amount = calc_withdraw(
                piggy_bank_entry.rebase_at_lock,
                last_rebase,
                piggy_bank_entry.amount,
            );
            let piggy_bank_entry = PiggyBankEntry {
                amount: amount,
                rebase_at_lock: last_rebase,
            };
            PIGGY_BANK_ENTRIES.save(
                deps.storage,
                message_info.sender.clone(),
                &piggy_bank_entry,
            )?;
        }
    };

    Ok(Response::new().add_attribute("action", "deposit"))
}

pub fn unlock(
    deps: DepsMut,
    env: Env,
    message_info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut piggy_bank_entry = PIGGY_BANK_ENTRIES
        .may_load(deps.storage, message_info.sender.clone())?
        .ok_or(ContractError::NoPiggyBankEntry {})?; // Get the piggy bank entry for the sender (if it exists)

    // See if there is enough in the piggy bank to unlock
    if piggy_bank_entry.amount < amount {
        return Err(ContractError::InsufficientFunds {});
    }

    // Subtract the amount from the piggy bank
    piggy_bank_entry.amount = piggy_bank_entry.amount - amount;

    // If the piggy bank entry is empty, remove the entry, otherwise save it
    if piggy_bank_entry.amount.is_zero() {
        PIGGY_BANK_ENTRIES.remove(deps.storage, message_info.sender.clone());
    } else {
        PIGGY_BANK_ENTRIES.save(deps.storage, message_info.sender.clone(), &piggy_bank_entry)?;
    }

    // If there already is an unlock entry for this time
    // (unlikely but needs to be handled, for example if someone did a bunch one block), then add to it
    // Otherwise, create a new unlock entry
    let unlock = match UNLOCKS.may_load(
        deps.storage,
        (message_info.sender.clone(), env.block.time.seconds()),
    )? {
        Some(unlock) => Unlock {
            amount: unlock.amount + amount,
            time: unlock.time,
            rebase_at_lock: piggy_bank_entry.rebase_at_lock,
        },
        None => Unlock {
            amount: amount,
            time: env.block.time,
            rebase_at_lock: piggy_bank_entry.rebase_at_lock,
        },
    };

    // Save the unlock entry
    UNLOCKS.save(
        deps.storage,
        (message_info.sender, env.block.time.seconds()),
        &unlock,
    )?;

    Ok(Response::new().add_attribute("action", "unlock"))
}

// This withdraws all unlocks which can be withdrawn
pub fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Get all unlock entries which are before the current time
    let total_amount_to_mint = UNLOCKS
        .prefix(info.sender.clone())
        .range(
            deps.storage,
            None,
            Some(Bound::inclusive(env.block.time.seconds())),
            cosmwasm_std::Order::Ascending,
        )
        .fold(
            Ok(Uint128::zero()),
            |acc: Result<Uint128, ContractError>, unlock| {
                let (unlock_time, unlock) = unlock?;

                // Get the last rebase multiplier before the unlock timestamp
                let rebase_at_unlock = REBASES
                    .range(
                        deps.storage,
                        None,
                        Some(Bound::exclusive(unlock_time)),
                        cosmwasm_std::Order::Descending,
                    )
                    .take(1)
                    .last()
                    .ok_or(ContractError::NoRebaseRecord {})??
                    .1;

                // Calculate the amount to mint using rebase multipliers
                let amount_to_mint =
                    calc_withdraw(unlock.rebase_at_lock, rebase_at_unlock, unlock.amount);

                // Add the amount_to_mint to the accumulator
                acc.map(|acc| acc + amount_to_mint)
            },
        )?;

    Ok(Response::new()
        .add_message(MsgMint {
            sender: info.sender.to_string(),
            mint_to_address: info.sender.to_string(),
            amount: Some(coin(total_amount_to_mint.u128(), "TOKEN_DENOM").into()),
        })
        .add_attribute("action", "withdraw_piggy_bank")
        .add_attribute("amount", total_amount_to_mint))
}

// Withdraw the unlock at a specific time. Normally 'withdraw' should be used instead of this, but it is
// an escape hatch in case the number of unlocks to withdraw somehow exceeds the gas limit.
pub fn withdraw_at_time(
    deps: DepsMut,
    _env: Env,
    message_info: MessageInfo,
    unlock_time: u64,
) -> Result<Response, ContractError> {
    // Get the unlock entry at the specified time
    let unlock = UNLOCKS.load(deps.storage, (message_info.sender.clone(), unlock_time))?;

    // Get the rebase multiplier at the unlock timestamp
    let rebase_at_unlock = REBASES
        .range(
            deps.storage,
            None,
            Some(Bound::exclusive(unlock_time)),
            cosmwasm_std::Order::Descending,
        )
        .take(1)
        .last()
        .ok_or(ContractError::NoRebaseRecord {})??
        .1;

    // Calculate the amount to mint using rebase multipliers
    let amount_to_mint = calc_withdraw(unlock.rebase_at_lock, rebase_at_unlock, unlock.amount);

    Ok(Response::new()
        .add_message(MsgMint {
            sender: message_info.sender.clone().to_string(),
            mint_to_address: message_info.sender.to_string(),
            amount: Some(coin(amount_to_mint.u128(), "TOKEN_DENOM").into()),
        })
        .add_attribute("action", "withdraw_piggy_bank")
        .add_attribute("amount", amount_to_mint))
}

// Calculate the amount to withdraw. This takes the rebase multiplier which was in effect when their coins
// unlocked.
fn calc_withdraw(rebase_at_lock: Decimal, rebase_at_unlock: Decimal, amount: Uint128) -> Uint128 {
    amount * (rebase_at_unlock / rebase_at_lock)
}

fn oracle_price() -> Uint128 {
    Uint128::new(1000000)
}

pub fn rebase(deps: DepsMut, env: Env, _: MessageInfo) -> Result<Response, ContractError> {
    let last_price = LAST_PRICE.load(deps.storage)?;
    let current_price = oracle_price();
    let rebase_multiplier = Decimal::from_ratio(current_price, last_price);

    let timestamp = env.block.time;
    REBASES.save(deps.storage, timestamp.seconds(), &rebase_multiplier)?;

    Ok(Response::new())
}

pub fn query_count(_deps: Deps) -> StdResult<Binary> {
    let constant = CONSTANTS.load(_deps.storage)?;
    to_json_binary(
        &(CountResponse {
            count: constant.count,
        }),
    )
}

// #[cfg(test)]
// mod tests {
//     use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
//     use cosmwasm_std::{coins, from_binary, StdError, Timestamp, Uint128};
//     use osmosis_std::types::cosmos::gov::v1::Deposit;

//     use super::*;

//     #[test]
//     fn test_piggy_bank() {
//         let mut deps = mock_dependencies();

//         let sender = String::from("sender0001");
//         let balance = coins(100, "tokens");

//         let info = mock_info("anyone", &[coin(100, "bm")]);

//         execute(deps, mock_env(), info, ExecuteMsg::DepositToPiggyBank {}).unwrap();
//     }
//     fn test_create() {
//         let mut deps = mock_dependencies();

//         let info = mock_info("anyone", &[]);
//         instantiate(deps.as_mut(), mock_env(), info, InstantiateMsg { count: 1 }).unwrap();

//         let sender = String::from("sender0001");
//         let balance = coins(100, "tokens");

//         // Cannot create, invalid ids
//         let info = mock_info(&sender, &balance);
//         for id in &["sh", "atomic_swap_id_too_long"] {
//             let create = CreateMsg {
//                 id: id.to_string(),
//                 hash: real_hash(),
//                 recipient: String::from("rcpt0001"),
//                 expires: Expiration::AtHeight(123456),
//             };
//             let err = execute(
//                 deps.as_mut(),
//                 mock_env(),
//                 info.clone(),
//                 ExecuteMsg::Create(create.clone()),
//             )
//             .unwrap_err();
//             assert_eq!(err, ContractError::InvalidId {});
//         }

//         // Cannot create, no funds
//         let info = mock_info(&sender, &[]);
//         let create = CreateMsg {
//             id: "swap0001".to_string(),
//             hash: real_hash(),
//             recipient: "rcpt0001".into(),
//             expires: Expiration::AtHeight(123456),
//         };
//         let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
//         assert_eq!(err, ContractError::EmptyBalance {});

//         // Cannot create, expired
//         let info = mock_info(&sender, &balance);
//         let create = CreateMsg {
//             id: "swap0001".to_string(),
//             hash: real_hash(),
//             recipient: "rcpt0001".into(),
//             expires: Expiration::AtTime(Timestamp::from_seconds(1)),
//         };
//         let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
//         assert_eq!(err, ContractError::Expired {});

//         // Cannot create, invalid hash
//         let info = mock_info(&sender, &balance);
//         let create = CreateMsg {
//             id: "swap0001".to_string(),
//             hash: "bu115h17".to_string(),
//             recipient: "rcpt0001".into(),
//             expires: Expiration::AtHeight(123456),
//         };
//         let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
//         assert_eq!(
//             err,
//             ContractError::ParseError("Invalid character \'u\' at position 1".into())
//         );

//         // Can create, all valid
//         let info = mock_info(&sender, &balance);
//         let create = CreateMsg {
//             id: "swap0001".to_string(),
//             hash: real_hash(),
//             recipient: "rcpt0001".into(),
//             expires: Expiration::AtHeight(123456),
//         };
//         let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap();
//         assert_eq!(0, res.messages.len());
//         assert_eq!(("action", "create"), res.attributes[0]);

//         // Cannot re-create (modify), already existing
//         let new_balance = coins(1, "tokens");
//         let info = mock_info(&sender, &new_balance);
//         let create = CreateMsg {
//             id: "swap0001".to_string(),
//             hash: real_hash(),
//             recipient: "rcpt0001".into(),
//             expires: Expiration::AtHeight(123456),
//         };
//         let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
//         assert_eq!(err, ContractError::AlreadyExists {});
//     }
// }
