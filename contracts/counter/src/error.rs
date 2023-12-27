use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Invalid expiration")]
    InvalidExpiration {},

    #[error("No rebase record")]
    NoRebaseRecord {},
}
