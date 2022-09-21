use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_binary, Addr, BankMsg, Coin, CosmosMsg, StdError, StdResult, Uint128, WasmMsg,
};
use cw_storage_plus::Map;
use std::fmt;

use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::Metadata;

pub const MAX_CONDITION: u8 = 10u8;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Token {
    Cw20 {
        contract_address: Addr,
        amount: Uint128,
    },
    NativeToken {
        denom: String,
        amount: Uint128,
    },
    Cw721 {
        contract_address: Addr,
        token_id: Option<String>,
    },
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Cw20 {
                contract_address,
                amount,
            } => write!(f, "{}{}", amount, contract_address),
            Token::NativeToken { denom, amount } => write!(f, "{}{}", amount, denom),
            Token::Cw721 {
                contract_address,
                token_id,
            } => write!(f, "{}:{:?}", contract_address, token_id),
        }
    }
}

impl Token {
    pub fn into_send_msg(&self, recipient: String) -> StdResult<CosmosMsg> {
        match self {
            Token::Cw20 {
                contract_address,
                amount,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient,
                    amount: *amount,
                })?,
                funds: vec![],
            })),
            Token::NativeToken { denom, amount } => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient,
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount: *amount,
                }],
            })),
            Token::Cw721 {
                contract_address,
                token_id,
            } => {
                if token_id.is_none() {
                    return Err(StdError::generic_err("Can not make key without token id"));
                }

                Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_address.to_string(),
                    msg: to_binary(&Cw721ExecuteMsg::TransferNft {
                        recipient,
                        token_id: token_id.clone().unwrap(),
                    })?,
                    funds: vec![],
                }))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EvolutionMetaData<T> {
    pub token_uri: Option<String>,
    pub extension: T,
    pub evolution_conditions: Vec<Token>,
    pub evolution_fee: EvolutionFee,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EvolutionFee {
    pub fee_token: Addr,
    pub evolve_fee_amount: Uint128,
    pub devolve_fee_amount: Uint128,
    pub fee_recipient: Addr,
}

pub const EVOLVED_STAGE: Map<&str, u8> = Map::new("evolved_stage");
pub const EVOLVED_META_DATA: Map<(&str, u8), EvolutionMetaData<Metadata>> =
    Map::new("evolved_meta_data");

// deposits before locked
pub const HOLDS: Map<(&str, Vec<u8>), Token> = Map::new("holds");

pub fn gen_holds_key(token_id: &str, token: Token) -> StdResult<(&str, Vec<u8>)> {
    match token {
        Token::Cw20 {
            contract_address,
            amount: _,
        } => {
            let contract_bytes = contract_address.as_bytes();
            let key = [vec![0x01], contract_bytes.to_vec()].concat();

            Ok((token_id, key))
        }
        Token::NativeToken { denom, amount: _ } => {
            let denom_bytes = denom.as_bytes();
            let key = [vec![0x02], denom_bytes.to_vec()].concat();

            Ok((token_id, key))
        }
        Token::Cw721 {
            contract_address,
            token_id: hold_token_id,
        } => {
            if hold_token_id.is_none() {
                return Err(StdError::generic_err("Can not make key without token id"));
            }

            let key = [
                vec![0x03],
                contract_address.as_bytes().to_vec(),
                vec![0x00],
                hold_token_id.unwrap().as_bytes().to_vec(),
            ]
            .concat();

            Ok((token_id, key))
        }
    }
}
