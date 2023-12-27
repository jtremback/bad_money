use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};

use crate::error::ContractError;
use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Constants, PiggyBankEntry, CONSTANTS, LAST_PRICE, PIGGY_BANK_ENTRY, REBASES};
use cosmwasm_std::{coin, Decimal, Uint128};
use cw_storage_plus::Bound;
use cw_utils::Expiration;
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
        ExecuteMsg::Increment {} => try_increment(deps, env, info),
        ExecuteMsg::Reset { count } => try_reset(deps, env, info, count),
        ExecuteMsg::DepositToPiggyBank {} => deposit_to_piggy_bank(deps, env, info),
        ExecuteMsg::Unlock {} => unlock_piggy_bank(deps, env, info),
        ExecuteMsg::WithdrawFromPiggyBank {} => withdraw_piggy_bank(deps, env, info),
        ExecuteMsg::Rebase {} => rebase(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => query_count(deps),
    }
}

pub fn deposit_to_piggy_bank(
    deps: DepsMut,
    _: Env,
    message_info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut piggy_bank_entry = PIGGY_BANK_ENTRY.may_load(deps.storage)?;

    let rebases = REBASES.range(deps.storage, None, None, cosmwasm_std::Order::Descending);

    let current_rebase = rebases
        .take(1)
        .last()
        .ok_or(ContractError::NoRebaseRecord {})?;

    if piggy_bank_entry.is_none() {
        piggy_bank_entry = Some(PiggyBankEntry {
            amount: Uint128::zero(),
            unlock: Expiration::Never {},
            rebase_at_lock: current_rebase?.1,
        });
    }

    let mut piggy_bank_entry = piggy_bank_entry.unwrap();
    piggy_bank_entry.amount += message_info.funds[0].amount;

    PIGGY_BANK_ENTRY.save(deps.storage, &piggy_bank_entry)?;

    Ok(Response::new().add_attribute("action", "deposit_to_piggy_bank"))
}

pub fn unlock_piggy_bank(
    deps: DepsMut,
    env: Env,
    _: MessageInfo,
) -> Result<Response, ContractError> {
    let mut piggy_bank_entry = PIGGY_BANK_ENTRY.load(deps.storage)?;

    match piggy_bank_entry.unlock {
        Expiration::Never {} => {
            piggy_bank_entry.unlock = Expiration::AtTime(env.block.time.plus_seconds(60 * 60 * 24));

            PIGGY_BANK_ENTRY.save(deps.storage, &piggy_bank_entry)?;

            Ok(Response::new())
        }
        _ => Err(ContractError::InvalidExpiration {}),
    }
}

pub fn withdraw_piggy_bank(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let piggy_bank_entry = PIGGY_BANK_ENTRY.load(deps.storage)?;

    match piggy_bank_entry.unlock {
        Expiration::Never {} => {
            // Error out if the expiration is set to "Never". This entry is not unlocking.
            return Err(ContractError::InvalidExpiration {});
        }
        _ => {}
    }

    if piggy_bank_entry.unlock.is_expired(&env.block) {
        // Error out if the expiration is not expired yet.
        return Err(ContractError::InvalidExpiration {});
    }

    let unlock_timestamp = match piggy_bank_entry.unlock {
        Expiration::AtTime(t) => t,
        _ => return Err(ContractError::InvalidExpiration {}),
    };

    let rebases = REBASES.range(
        deps.storage,
        None,
        Some(Bound::exclusive(unlock_timestamp.seconds())),
        cosmwasm_std::Order::Descending,
    );

    let last_rebase = rebases
        .take(1)
        .last()
        .ok_or(ContractError::NoRebaseRecord {})?;

    // Calculate the amount to mint. This takes the rebase multiplier which was in effect when their coins
    // unlocked.
    let amount_to_mint =
        piggy_bank_entry.amount * (last_rebase?.1 / piggy_bank_entry.rebase_at_lock);

    // Mint coins to the sender
    let res = Response::new()
        .add_message(MsgMint {
            sender: info.sender.to_string(),
            mint_to_address: info.sender.to_string(),
            amount: Some(coin(amount_to_mint.u128(), "TOKEN_DENOM").into()),
        })
        .add_attribute("action", "withdraw_piggy_bank")
        .add_attribute("amount", amount_to_mint);

    Ok(res)
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

fn try_increment(deps: DepsMut, _env: Env, _info: MessageInfo) -> Result<Response, ContractError> {
    let mut constant = CONSTANTS.load(deps.storage)?;
    constant.count += 1;
    CONSTANTS.save(deps.storage, &constant)?;
    Ok(Response::new().add_attribute("action", "increament"))
}

fn try_reset(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    count: i32,
) -> Result<Response, ContractError> {
    let mut constant = CONSTANTS.load(deps.storage)?;
    if constant.owner != info.sender {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }
    constant.count = count;
    CONSTANTS.save(deps.storage, &constant)?;
    Ok(Response::new().add_attribute("action", "COUNT reset successfully"))
}

pub fn query_count(_deps: Deps) -> StdResult<Binary> {
    let constant = CONSTANTS.load(_deps.storage)?;
    to_json_binary(
        &(CountResponse {
            count: constant.count,
        }),
    )
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, StdError, Timestamp, Uint128};

    use super::*;

    #[test]
    fn test_create() {
        let mut deps = mock_dependencies();

        let info = mock_info("anyone", &[]);
        instantiate(deps.as_mut(), mock_env(), info, InstantiateMsg { count: 1 }).unwrap();

        let sender = String::from("sender0001");
        let balance = coins(100, "tokens");

        // Cannot create, invalid ids
        let info = mock_info(&sender, &balance);
        for id in &["sh", "atomic_swap_id_too_long"] {
            let create = CreateMsg {
                id: id.to_string(),
                hash: real_hash(),
                recipient: String::from("rcpt0001"),
                expires: Expiration::AtHeight(123456),
            };
            let err = execute(
                deps.as_mut(),
                mock_env(),
                info.clone(),
                ExecuteMsg::Create(create.clone()),
            )
            .unwrap_err();
            assert_eq!(err, ContractError::InvalidId {});
        }

        // Cannot create, no funds
        let info = mock_info(&sender, &[]);
        let create = CreateMsg {
            id: "swap0001".to_string(),
            hash: real_hash(),
            recipient: "rcpt0001".into(),
            expires: Expiration::AtHeight(123456),
        };
        let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
        assert_eq!(err, ContractError::EmptyBalance {});

        // Cannot create, expired
        let info = mock_info(&sender, &balance);
        let create = CreateMsg {
            id: "swap0001".to_string(),
            hash: real_hash(),
            recipient: "rcpt0001".into(),
            expires: Expiration::AtTime(Timestamp::from_seconds(1)),
        };
        let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
        assert_eq!(err, ContractError::Expired {});

        // Cannot create, invalid hash
        let info = mock_info(&sender, &balance);
        let create = CreateMsg {
            id: "swap0001".to_string(),
            hash: "bu115h17".to_string(),
            recipient: "rcpt0001".into(),
            expires: Expiration::AtHeight(123456),
        };
        let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
        assert_eq!(
            err,
            ContractError::ParseError("Invalid character \'u\' at position 1".into())
        );

        // Can create, all valid
        let info = mock_info(&sender, &balance);
        let create = CreateMsg {
            id: "swap0001".to_string(),
            hash: real_hash(),
            recipient: "rcpt0001".into(),
            expires: Expiration::AtHeight(123456),
        };
        let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap();
        assert_eq!(0, res.messages.len());
        assert_eq!(("action", "create"), res.attributes[0]);

        // Cannot re-create (modify), already existing
        let new_balance = coins(1, "tokens");
        let info = mock_info(&sender, &new_balance);
        let create = CreateMsg {
            id: "swap0001".to_string(),
            hash: real_hash(),
            recipient: "rcpt0001".into(),
            expires: Expiration::AtHeight(123456),
        };
        let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Create(create)).unwrap_err();
        assert_eq!(err, ContractError::AlreadyExists {});
    }
}
