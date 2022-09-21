use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::msg::{Cw20HookMsg, DarwinExecuteMsg as ExecuteMsg};
use crate::state::{
    gen_holds_key, EvolutionMetaData, Token, EVOLVED_META_DATA, EVOLVED_STAGE, HOLDS, MAX_CONDITION,
};
use crate::Metadata;
use cosmwasm_std::{
    from_binary, to_binary, CosmosMsg, DepsMut, Empty, Env, MessageInfo, Response, StdError,
    StdResult, Storage, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw721::Cw721ExecuteMsg;

use cw721::ContractInfoResponse;
use cw721_base::{state::TokenInfo, ContractError, Cw721Contract, InstantiateMsg, MintMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:darwin";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Extension<T> {
    pub devolved_extension: T,
    pub evolved_extension: T,
    pub evolve_conditions: Vec<Token>,
}

pub fn instantiate<T: Serialize + DeserializeOwned + Clone>(
    contract: Cw721Contract<T, Empty>,
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };

    contract.contract_info.save(deps.storage, &info)?;
    let minter = deps.api.addr_validate(&msg.minter)?;
    contract.minter.save(deps.storage, &minter)?;

    Ok(Response::default())
}

pub fn execute(
    contract: Cw721Contract<Metadata, Empty>,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Metadata>,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::<Metadata>::Evolve {
            token_id,
            selected_nfts,
        } => evolve(
            contract,
            deps,
            env,
            info,
            token_id,
            selected_nfts.unwrap_or_default(),
        ),
        ExecuteMsg::<Metadata>::Mint(msg) => {
            let len = msg.evolution_data.len();
            for index in 0..len {
                add_meta_data(
                    deps.storage,
                    &msg.token_id,
                    index as u8,
                    &msg.evolution_data[index],
                )?;
            }
            EVOLVED_STAGE.save(deps.storage, &msg.token_id, &0u8)?;

            let cw721_base_mint_msg = MintMsg::<Metadata> {
                owner: msg.owner,
                token_id: msg.token_id,
                token_uri: msg.evolution_data[0].token_uri.clone(),
                extension: msg.evolution_data[0].extension.clone(),
            };

            contract.mint(deps, env, info, cw721_base_mint_msg)
        }
        ExecuteMsg::<Metadata>::Receive(msg) => receive(contract, deps, env, info, msg),
        _ => contract.execute(deps, env, info, msg.into()),
    }
}

pub fn evolve(
    contract: Cw721Contract<Metadata, Empty>,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: String,
    selected_nfts: Vec<Token>,
) -> Result<Response, ContractError> {
    let owner = contract.tokens.load(deps.storage, &token_id)?.owner;

    if owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let mut selected_nfts = selected_nfts;

    let evolved_stage = EVOLVED_STAGE.load(deps.storage, &token_id)?;

    // check is last stage
    let new_meatadata = EVOLVED_META_DATA.load(deps.storage, (&token_id, evolved_stage + 1))?;

    let evolved_metadata = EVOLVED_META_DATA.load(deps.storage, (&token_id, evolved_stage))?;

    let evolve_conditions = evolved_metadata.evolution_conditions;

    let mut extract_msgs: Vec<CosmosMsg> = vec![];

    for condition in evolve_conditions {
        if let Token::Cw20 {
            contract_address,
            amount,
        } = condition.clone()
        {
            // extract tokens
            extract_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount,
                })?,
                funds: vec![],
            }));

            // update hold
            deposit(deps.storage, &token_id, condition)?;
        } else if let Token::NativeToken { .. } = condition {
            // check funds
            assert_sent_native_token_balance(&condition, &info)?;

            // update hold
            deposit(deps.storage, &token_id, condition)?;
        } else if let Token::Cw721 {
            contract_address,
            token_id: condition_token_id,
        } = condition
        {
            let mut extract_token_id: Option<String> = None;
            // if condition token id is not none
            if let Some(condition_token_id) = condition_token_id {
                extract_token_id = Some(condition_token_id);
            } else {
                let len = selected_nfts.len();
                for index in 0..len {
                    if let Token::Cw721 {
                        contract_address: selected_contract_address,
                        token_id,
                    } = selected_nfts[index].clone()
                    {
                        if contract_address == selected_contract_address {
                            selected_nfts.remove(index);
                            extract_token_id = token_id;
                            break;
                        }
                    }
                }
            }

            if extract_token_id.is_none() {
                return Err(ContractError::Std(StdError::generic_err(
                    "Need to decide token id which will use for evolve",
                )));
            }

            extract_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_address.to_string(),
                msg: to_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: env.contract.address.to_string(),
                    token_id: extract_token_id.clone().unwrap(),
                })?,
                funds: vec![],
            }));

            // update hold
            deposit(
                deps.storage,
                &token_id,
                Token::Cw721 {
                    contract_address,
                    token_id: extract_token_id,
                },
            )?;
        }
    }

    EVOLVED_STAGE.save(deps.storage, &token_id, &(evolved_stage + 1))?;

    // extract fee
    extract_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: evolved_metadata.evolution_fee.fee_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
            owner: info.sender.to_string(),
            recipient: evolved_metadata.evolution_fee.fee_recipient.to_string(),
            amount: evolved_metadata.evolution_fee.evolve_fee_amount,
        })?,
        funds: vec![],
    }));

    // update nft info
    contract
        .tokens
        .update(deps.storage, &token_id, |old| match old {
            Some(token) => Ok(TokenInfo::<Metadata> {
                owner: token.owner,
                approvals: token.approvals,
                token_uri: new_meatadata.token_uri,
                extension: new_meatadata.extension,
            }),
            None => Err(ContractError::Std(StdError::generic_err("Token not found"))),
        })?;

    Ok(Response::new()
        .add_attribute("action", "evolve")
        .add_attribute("token_id", token_id)
        .add_attribute("evolved_stage", format!("{}", evolved_stage + 1))
        .add_messages(extract_msgs))
}

pub fn receive(
    contract: Cw721Contract<Metadata, Empty>,
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let contract_address = info.sender;
    let amount = msg.amount;
    let sender = deps.api.addr_validate(&msg.sender)?;
    let msg = from_binary::<Cw20HookMsg>(&msg.msg)?;

    match msg {
        Cw20HookMsg::Devolve {
            token_id,
            selected_nfts,
        } => {
            let mut selected_nfts = selected_nfts.unwrap_or_default();
            let owner = contract.tokens.load(deps.storage, &token_id)?.owner;

            if owner != sender {
                return Err(ContractError::Unauthorized {});
            }

            let evolved_stage = EVOLVED_STAGE.load(deps.storage, &token_id)?;

            if evolved_stage == 0 {
                return Err(ContractError::Std(StdError::generic_err(
                    "Can not devolve stage 0",
                )));
            }

            let evolved_metadata =
                EVOLVED_META_DATA.load(deps.storage, (&token_id, evolved_stage))?;

            if contract_address != evolved_metadata.evolution_fee.fee_token
                || amount != evolved_metadata.evolution_fee.devolve_fee_amount
            {
                return Err(ContractError::Std(StdError::generic_err(
                    "Evolve fee token mismatch",
                )));
            }

            let fee_send_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: evolved_metadata.evolution_fee.fee_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: evolved_metadata.evolution_fee.fee_recipient.to_string(),
                    amount: evolved_metadata.evolution_fee.devolve_fee_amount,
                })?,
                funds: vec![],
            });

            let new_meatadata =
                EVOLVED_META_DATA.load(deps.storage, (&token_id, (evolved_stage - 1)))?;

            let EvolutionMetaData::<Metadata> {
                token_uri,
                extension,
                evolution_conditions,
                ..
            } = new_meatadata;

            // update nft info
            contract
                .tokens
                .update(deps.storage, &token_id, |old| match old {
                    Some(token) => Ok(TokenInfo::<Metadata> {
                        owner: token.owner,
                        approvals: token.approvals,
                        token_uri,
                        extension,
                    }),
                    None => Err(ContractError::Std(StdError::generic_err("Token not found"))),
                })?;

            let mut withdraw_msgs: Vec<CosmosMsg> = vec![];
            for mut withdraw_token in evolution_conditions {
                if let Token::Cw721 {
                    contract_address,
                    token_id: _,
                } = withdraw_token.clone()
                {
                    let len = selected_nfts.len();
                    for index in 0..len {
                        if let Token::Cw721 {
                            contract_address: selected_contract_address,
                            token_id: _,
                        } = selected_nfts[index].clone()
                        {
                            if contract_address == selected_contract_address {
                                withdraw_token = selected_nfts.remove(index);
                                break;
                            }
                        }
                    }
                }
                withdraw_msgs.push(
                    withdraw(deps.storage, &token_id, withdraw_token)?
                        .into_send_msg(sender.to_string())?,
                )
            }

            EVOLVED_STAGE.save(deps.storage, &token_id, &(evolved_stage - 1))?;

            Ok(Response::new()
                .add_attribute("action", "devolve")
                .add_attribute("token_id", token_id)
                .add_message(fee_send_msg)
                .add_messages(withdraw_msgs))
        }
    }
}

fn add_meta_data(
    storage: &mut dyn Storage,
    token_id: &String,
    stage: u8,
    metadata: &EvolutionMetaData<Metadata>,
) -> Result<(), ContractError> {
    let conditions_length = metadata.evolution_conditions.len();
    if conditions_length > MAX_CONDITION as usize {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Length of the evolution conditions exceed max condition({})",
            MAX_CONDITION
        ))));
    }

    // check condition uniqueness
    let mut token_types: Vec<String> = vec![];
    for condition in metadata.evolution_conditions.clone() {
        let token_type = match condition {
            Token::Cw20 {
                contract_address, ..
            } => "20".to_string() + &contract_address.to_string(),
            Token::Cw721 {
                contract_address,
                token_id,
            } => {
                "721".to_string()
                    + &contract_address.to_string()
                    + &token_id.unwrap_or_else(|| "".to_string())
            }
            Token::NativeToken { denom, .. } => "native".to_string() + &denom.to_string(),
        };

        if !token_types.iter().any(|item| item == &token_type) {
            token_types.push(token_type);
        } else {
            return Err(ContractError::Std(StdError::generic_err(
                "Duplicated token type",
            )));
        }
    }

    EVOLVED_META_DATA.save(storage, (token_id, stage), metadata)?;

    Ok(())
}

fn deposit(
    storage: &mut dyn Storage,
    token_id: &str,
    deposit_token: Token,
) -> Result<(), StdError> {
    let key = gen_holds_key(token_id, deposit_token.clone())?;

    let deposit_token_ = deposit_token.clone(); // to avoid moved

    if let Token::Cw20 {
        contract_address,
        amount,
    } = deposit_token_
    {
        let hold = HOLDS.may_load(storage, key.clone())?;

        if let Some(hold) = hold {
            if let Token::Cw20 {
                contract_address: _,
                amount: hold_amount,
            } = hold
            {
                let updated_hold = Token::Cw20 {
                    contract_address,
                    amount: amount + hold_amount,
                };
                HOLDS.save(storage, key, &updated_hold)?;
            } else {
                return Err(StdError::generic_err("Wrong token type"));
            }
        } else {
            HOLDS.save(storage, key, &deposit_token)?;
        }
    } else if let Token::NativeToken { denom, amount } = deposit_token_ {
        let hold = HOLDS.may_load(storage, key.clone())?;

        if let Some(hold) = hold {
            if let Token::NativeToken {
                denom: _,
                amount: hold_amount,
            } = hold
            {
                let updated_hold = Token::NativeToken {
                    denom,
                    amount: amount + hold_amount,
                };
                HOLDS.save(storage, key, &updated_hold)?;
            } else {
                return Err(StdError::generic_err("Wrong token type"));
            }
        } else {
            HOLDS.save(storage, key, &deposit_token)?;
        }
    } else if let Token::Cw721 {
        contract_address: _,
        token_id,
    } = deposit_token_
    {
        if token_id.is_none() {
            return Err(StdError::generic_err("Can't deposit without token_id"));
        }
        HOLDS.save(storage, key, &deposit_token)?;
    }
    Ok(())
}

fn withdraw(
    storage: &mut dyn Storage,
    token_id: &str,
    withdraw_token: Token,
) -> Result<Token, StdError> {
    let key = gen_holds_key(token_id, withdraw_token.clone())?;

    let withdraw_token_ = withdraw_token.clone(); // to avoid moved

    if let Token::Cw20 {
        contract_address,
        amount,
    } = withdraw_token_
    {
        let hold = HOLDS.load(storage, key.clone())?;

        if let Token::Cw20 {
            contract_address: _,
            amount: hold_amount,
        } = hold
        {
            if hold_amount == amount {
                HOLDS.remove(storage, key.clone())
            }

            let updated_hold = Token::Cw20 {
                contract_address,
                amount: hold_amount.checked_sub(amount)?,
            };

            HOLDS.save(storage, key, &updated_hold)?;
        } else {
            return Err(StdError::generic_err("Wrong token type"));
        }
    } else if let Token::NativeToken { denom, amount } = withdraw_token_ {
        let hold = HOLDS.load(storage, key.clone())?;

        if let Token::NativeToken {
            denom: _,
            amount: hold_amount,
        } = hold
        {
            if hold_amount == amount {
                HOLDS.remove(storage, key.clone())
            }

            let updated_hold = Token::NativeToken {
                denom,
                amount: hold_amount.checked_sub(amount)?,
            };

            HOLDS.save(storage, key, &updated_hold)?;
        } else {
            return Err(StdError::generic_err("Wrong token type"));
        }
    } else if let Token::Cw721 {
        contract_address: _,
        token_id,
    } = withdraw_token_
    {
        if token_id.is_none() {
            return Err(StdError::generic_err("Can't deposit without token_id"));
        }
        HOLDS.remove(storage, key)
    }

    Ok(withdraw_token)
}

fn assert_sent_native_token_balance(token: &Token, message_info: &MessageInfo) -> StdResult<()> {
    if let Token::NativeToken { denom, amount } = token {
        match message_info.funds.iter().find(|x| x.denom == *denom) {
            Some(coin) => {
                if *amount == coin.amount {
                    Ok(())
                } else {
                    Err(StdError::generic_err(
                        "Native token balance mismatch between the argument and the transferred",
                    ))
                }
            }
            None => {
                if amount.is_zero() {
                    Ok(())
                } else {
                    Err(StdError::generic_err(
                        "Native token balance mismatch between the argument and the transferred",
                    ))
                }
            }
        }
    } else {
        Ok(())
    }
}
