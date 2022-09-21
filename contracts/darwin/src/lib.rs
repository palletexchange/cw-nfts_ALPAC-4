pub mod execute;
pub mod msg;
pub mod query;
pub mod state;

use cosmwasm_std::to_binary;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::execute::{execute as darwin_execute, instantiate as darwin_instantiate, Extension};
pub use crate::msg::{DarwinExecuteMsg, DarwinQueryMsg};
use crate::query::{evolution_info, evolved_stage, holds};
use cosmwasm_std::Empty;
use cw721_base::Cw721Contract;
pub use cw721_base::{ContractError, InstantiateMsg, MintMsg, MinterResponse};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Trait {
    pub display_type: Option<String>,
    pub trait_type: String,
    pub value: String,
}

// see: https://docs.opensea.io/docs/metadata-standards
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Metadata {
    pub image: Option<String>,
    pub image_data: Option<String>,
    pub external_url: Option<String>,
    pub description: Option<String>,
    pub name: Option<String>,
    pub attributes: Option<Vec<Trait>>,
    pub background_color: Option<String>,
    pub animation_url: Option<String>,
    pub youtube_url: Option<String>,
    /// This is how much the minter takes as a cut when sold
    /// royalties are owed on this token if it is Some
    pub royalty_percentage: Option<u64>,
    /// The payment address, may be different to or the same
    /// as the minter addr
    /// question: how do we validate this?
    pub royalty_payment_address: Option<String>,
}

pub type MintExtension = Option<Extension<Metadata>>;

pub type DarwinContract<'a> = Cw721Contract<'a, Metadata, Empty>;
pub type ExecuteMsg = DarwinExecuteMsg<Metadata>;

#[cfg(not(feature = "library"))]
pub mod entry {
    use super::*;

    use cosmwasm_std::entry_point;
    use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};

    #[entry_point]
    pub fn instantiate(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: InstantiateMsg,
    ) -> StdResult<Response> {
        let darwin_contract = DarwinContract::default();
        darwin_instantiate(darwin_contract, deps, env, info, msg)
    }

    #[entry_point]
    pub fn execute(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        let darwin_contract = DarwinContract::default();
        darwin_execute(darwin_contract, deps, env, info, msg)
    }

    #[entry_point]
    pub fn query(deps: Deps, env: Env, msg: DarwinQueryMsg) -> StdResult<Binary> {
        let darwin_contract = DarwinContract::default();
        match msg {
            DarwinQueryMsg::EvolutionInfo { token_id, stage } => {
                to_binary(&evolution_info(deps, token_id, stage)?)
            }
            DarwinQueryMsg::EvolvedStage { token_id } => to_binary(&evolved_stage(deps, token_id)?),
            DarwinQueryMsg::Holds { token_id } => to_binary(&holds(deps, token_id)?),
            _ => darwin_contract.query(deps, env, msg.into()),
        }
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
    pub struct MigrateMsg {}

    #[entry_point]
    pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
        Ok(Response::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{Cw20HookMsg, DarwinMintMsg};
    use crate::state::{EvolutionFee, EvolutionMetaData, Token, MAX_CONDITION};

    use cosmwasm_std::{Addr, Coin, CosmosMsg, StdError, SubMsg, Uint128, WasmMsg};

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
    use cw721::{Cw721ExecuteMsg, Cw721Query};

    const CREATOR: &str = "creator";

    #[test]
    fn mint_test() {
        let mut deps = mock_dependencies();
        let contract = DarwinContract::default();

        let info = mock_info(CREATOR, &[]);
        let init_msg = InstantiateMsg {
            name: "SpaceShips".to_string(),
            symbol: "SPACE".to_string(),
            minter: CREATOR.to_string(),
        };
        entry::instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg).unwrap();

        let token_id = "Enterprise";
        // Duplicated token type
        let mint_msg = DarwinMintMsg::<Metadata> {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            evolution_data: vec![EvolutionMetaData::<Metadata> {
                token_uri: Some(
                    "https://starships.example.com/Starship/Enterprise(level1).json".into(),
                ),
                extension: Metadata {
                    name: Some("Enterprise(level1)".to_string()),
                    ..Metadata::default()
                },
                evolution_conditions: vec![
                    Token::NativeToken {
                        denom: "native".to_string(),
                        amount: Uint128::new(100000000u128),
                    },
                    Token::NativeToken {
                        denom: "native".to_string(),
                        amount: Uint128::new(200000000u128),
                    },
                ],
                evolution_fee: EvolutionFee {
                    fee_token: Addr::unchecked("space"),
                    evolve_fee_amount: Uint128::new(0u128),
                    devolve_fee_amount: Uint128::new(0u128),
                    fee_recipient: Addr::unchecked(CREATOR.to_string()),
                },
            }],
        };

        let exec_msg = ExecuteMsg::Mint(mint_msg);
        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), exec_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err("Duplicated token type")),
        );

        let token_id = "Enterprise";
        let mut evolution_conditions: Vec<Token> = vec![];
        for i in 0..(MAX_CONDITION + 1) {
            evolution_conditions.push(Token::Cw20 {
                contract_address: Addr::unchecked("token".to_string() + &i.to_string()),
                amount: Uint128::new(100000000u128),
            })
        }
        // exceed max condition count
        let mint_msg = DarwinMintMsg::<Metadata> {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            evolution_data: vec![EvolutionMetaData::<Metadata> {
                token_uri: Some(
                    "https://starships.example.com/Starship/Enterprise(level1).json".into(),
                ),
                extension: Metadata {
                    name: Some("Enterprise(level1)".to_string()),
                    ..Metadata::default()
                },
                evolution_conditions,
                evolution_fee: EvolutionFee {
                    fee_token: Addr::unchecked("space"),
                    evolve_fee_amount: Uint128::new(0u128),
                    devolve_fee_amount: Uint128::new(0u128),
                    fee_recipient: Addr::unchecked(CREATOR.to_string()),
                },
            }],
        };

        let exec_msg = ExecuteMsg::Mint(mint_msg);
        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), exec_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(format!(
                "Length of the evolution conditions exceed max condition({})",
                MAX_CONDITION,
            ))),
        );

        let token_id = "Enterprise";
        let mint_msg = DarwinMintMsg::<Metadata> {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            evolution_data: vec![
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level1).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level1)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![
                        Token::NativeToken {
                            denom: "native".to_string(),
                            amount: Uint128::new(100000000u128),
                        },
                        Token::Cw20 {
                            contract_address: Addr::unchecked("fuel"),
                            amount: Uint128::new(100000000u128),
                        },
                    ],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(10000000u128),
                        devolve_fee_amount: Uint128::new(0u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level2).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level2)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![
                        Token::NativeToken {
                            denom: "native".to_string(),
                            amount: Uint128::new(200000000u128),
                        },
                        Token::Cw20 {
                            contract_address: Addr::unchecked("fuel"),
                            amount: Uint128::new(200000000u128),
                        },
                    ],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(20000000u128),
                        devolve_fee_amount: Uint128::new(20000000u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level3).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level3)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(0u128),
                        devolve_fee_amount: Uint128::new(30000000u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
            ],
        };

        let exec_msg = ExecuteMsg::Mint(mint_msg.clone());
        entry::execute(deps.as_mut(), mock_env(), info, exec_msg).unwrap();

        let res = contract.nft_info(deps.as_ref(), token_id.into()).unwrap();
        let evolved_stage = evolved_stage(deps.as_ref(), token_id.into()).unwrap();
        let evolution_info_0 = evolution_info(deps.as_ref(), token_id.into(), 0).unwrap();
        let evolution_info_1 = evolution_info(deps.as_ref(), token_id.into(), 1).unwrap();
        let evolution_info_2 = evolution_info(deps.as_ref(), token_id.into(), 2).unwrap();
        assert_eq!(res.token_uri, mint_msg.evolution_data[0].token_uri);
        assert_eq!(res.extension, mint_msg.evolution_data[0].extension);
        assert_eq!(evolved_stage.evolved_stage, 0);
        assert_eq!(evolution_info_0.evolution_info, mint_msg.evolution_data[0]);
        assert_eq!(evolution_info_1.evolution_info, mint_msg.evolution_data[1]);
        assert_eq!(evolution_info_2.evolution_info, mint_msg.evolution_data[2]);
    }

    #[test]
    fn evolve_devolve_test() {
        let mut deps = mock_dependencies();
        let contract = DarwinContract::default();

        let info = mock_info(CREATOR, &[]);
        let init_msg = InstantiateMsg {
            name: "SpaceShips".to_string(),
            symbol: "SPACE".to_string(),
            minter: CREATOR.to_string(),
        };
        entry::instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg).unwrap();

        let token_id = "Enterprise";
        let mint_msg = DarwinMintMsg::<Metadata> {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            evolution_data: vec![
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level1).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level1)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![
                        Token::NativeToken {
                            denom: "native".to_string(),
                            amount: Uint128::new(100000000u128),
                        },
                        Token::Cw20 {
                            contract_address: Addr::unchecked("fuel"),
                            amount: Uint128::new(100000000u128),
                        },
                    ],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(10000000u128),
                        devolve_fee_amount: Uint128::new(0u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level2).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level2)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![
                        Token::NativeToken {
                            denom: "native".to_string(),
                            amount: Uint128::new(200000000u128),
                        },
                        Token::Cw20 {
                            contract_address: Addr::unchecked("fuel"),
                            amount: Uint128::new(200000000u128),
                        },
                        Token::Cw721 {
                            contract_address: Addr::unchecked("SpaceShip Engine"),
                            token_id: Some("Enterprise Engine".to_string()),
                        },
                    ],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(20000000u128),
                        devolve_fee_amount: Uint128::new(20000000u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level3).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level3)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(0u128),
                        devolve_fee_amount: Uint128::new(30000000u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
            ],
        };

        let exec_msg = ExecuteMsg::Mint(mint_msg.clone());
        entry::execute(deps.as_mut(), mock_env(), info, exec_msg).unwrap();

        let evolve_msg = ExecuteMsg::Evolve {
            token_id: token_id.into(),
            selected_nfts: None,
        };

        // error case. give wrong amount of native token
        let info = mock_info(
            "john",
            &[Coin {
                denom: "native".to_string(),
                amount: Uint128::new(10000u128),
            }],
        );

        let err = entry::execute(deps.as_mut(), mock_env(), info, evolve_msg.clone()).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "Native token balance mismatch between the argument and the transferred"
            )),
        );

        let info = mock_info(
            "john",
            &[Coin {
                denom: "native".to_string(),
                amount: Uint128::new(100000000u128),
            }],
        );

        let res = entry::execute(deps.as_mut(), mock_env(), info.clone(), evolve_msg).unwrap();

        let contract_address = &mock_env().contract.address.to_string();

        assert_eq!(
            res.messages,
            vec![
                // get cw20 conditions tokens
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: Addr::unchecked("fuel").to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: contract_address.to_string(),
                        amount: Uint128::new(100000000u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                // send
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: mint_msg.evolution_data[0]
                        .evolution_fee
                        .fee_token
                        .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: mint_msg.evolution_data[0]
                            .evolution_fee
                            .fee_recipient
                            .to_string(),
                        amount: mint_msg.evolution_data[0].evolution_fee.evolve_fee_amount,
                    })
                    .unwrap(),
                    funds: vec![],
                })),
            ],
        );

        // check holds
        let token_holds = holds(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(
            token_holds.holds,
            vec![
                Token::Cw20 {
                    contract_address: Addr::unchecked("fuel"),
                    amount: Uint128::new(100000000u128),
                },
                Token::NativeToken {
                    denom: "native".to_string(),
                    amount: Uint128::new(100000000u128),
                },
            ]
        );

        // check evolved stage
        let stage = evolved_stage(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(stage.evolved_stage, 1);

        // check token_info
        let token_info = contract.nft_info(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(mint_msg.evolution_data[1].extension, token_info.extension);
        assert_eq!(mint_msg.evolution_data[1].token_uri, token_info.token_uri);

        let evolve_msg = ExecuteMsg::Evolve {
            token_id: token_id.into(),
            selected_nfts: None,
        };

        let info = mock_info(
            "john",
            &[Coin {
                denom: "native".to_string(),
                amount: Uint128::new(200000000u128),
            }],
        );

        let res = entry::execute(deps.as_mut(), mock_env(), info.clone(), evolve_msg).unwrap();

        let contract_address = &mock_env().contract.address.to_string();

        assert_eq!(
            res.messages,
            vec![
                // get cw20 conditions tokens
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: Addr::unchecked("fuel").to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: contract_address.to_string(),
                        amount: Uint128::new(200000000u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                // get cw721 conditions tokens
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: Addr::unchecked("SpaceShip Engine").to_string(),
                    msg: to_binary(&Cw721ExecuteMsg::TransferNft {
                        recipient: contract_address.to_string(),
                        token_id: "Enterprise Engine".to_string(),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                // send
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: mint_msg.evolution_data[1]
                        .evolution_fee
                        .fee_token
                        .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: mint_msg.evolution_data[1]
                            .evolution_fee
                            .fee_recipient
                            .to_string(),
                        amount: mint_msg.evolution_data[1].evolution_fee.evolve_fee_amount,
                    })
                    .unwrap(),
                    funds: vec![],
                }))
            ],
        );

        // check holds
        let token_holds = holds(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(
            token_holds.holds,
            vec![
                Token::Cw20 {
                    contract_address: Addr::unchecked("fuel"),
                    amount: Uint128::new(300000000u128),
                },
                Token::NativeToken {
                    denom: "native".to_string(),
                    amount: Uint128::new(300000000u128),
                },
                Token::Cw721 {
                    contract_address: Addr::unchecked("SpaceShip Engine"),
                    token_id: Some("Enterprise Engine".to_string()),
                },
            ]
        );

        // check evolved stage
        let stage = evolved_stage(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(stage.evolved_stage, 2);

        // check token_info
        let token_info = contract.nft_info(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(mint_msg.evolution_data[2].extension, token_info.extension);
        assert_eq!(mint_msg.evolution_data[2].token_uri, token_info.token_uri);

        // devolve test
        let devolve_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            amount: Uint128::new(30000000u128),
            sender: "john".to_string(),
            msg: to_binary(&Cw20HookMsg::Devolve {
                token_id: token_id.into(),
                selected_nfts: None,
            })
            .unwrap(),
        });

        let info = mock_info("space", &[]);

        let res = entry::execute(deps.as_mut(), mock_env(), info, devolve_msg).unwrap();

        assert_eq!(
            res.messages,
            vec![
                // send fee
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: mint_msg.evolution_data[2]
                        .evolution_fee
                        .fee_token
                        .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: mint_msg.evolution_data[2]
                            .evolution_fee
                            .fee_recipient
                            .to_string(),
                        amount: mint_msg.evolution_data[2].evolution_fee.devolve_fee_amount,
                    })
                    .unwrap(),
                    funds: vec![],
                })),
                // send conditions tokens
                SubMsg::new(
                    mint_msg.evolution_data[1].evolution_conditions[0]
                        .into_send_msg("john".to_string())
                        .unwrap()
                ),
                SubMsg::new(
                    mint_msg.evolution_data[1].evolution_conditions[1]
                        .into_send_msg("john".to_string())
                        .unwrap()
                ),
                SubMsg::new(
                    mint_msg.evolution_data[1].evolution_conditions[2]
                        .into_send_msg("john".to_string())
                        .unwrap()
                ),
            ],
        );

        // check holds
        let token_holds = holds(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(
            token_holds.holds,
            vec![
                Token::Cw20 {
                    contract_address: Addr::unchecked("fuel"),
                    amount: Uint128::new(100000000u128),
                },
                Token::NativeToken {
                    denom: "native".to_string(),
                    amount: Uint128::new(100000000u128),
                },
            ]
        );

        // check evolved stage
        let stage = evolved_stage(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(stage.evolved_stage, 1);

        // check token_info
        let token_info = contract.nft_info(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(mint_msg.evolution_data[1].extension, token_info.extension);
        assert_eq!(mint_msg.evolution_data[1].token_uri, token_info.token_uri);
    }

    #[test]
    fn evolve_devolve_test_for_none_token_id() {
        let mut deps = mock_dependencies();

        let info = mock_info(CREATOR, &[]);
        let init_msg = InstantiateMsg {
            name: "SpaceShips".to_string(),
            symbol: "SPACE".to_string(),
            minter: CREATOR.to_string(),
        };
        entry::instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg).unwrap();

        let token_id = "Enterprise";
        let mint_msg = DarwinMintMsg::<Metadata> {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            evolution_data: vec![
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level1).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level1)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![Token::Cw721 {
                        contract_address: Addr::unchecked("SpaceShip Engine"),
                        token_id: None,
                    }],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(10000000u128),
                        devolve_fee_amount: Uint128::new(0u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
                EvolutionMetaData::<Metadata> {
                    token_uri: Some(
                        "https://starships.example.com/Starship/Enterprise(level3).json".into(),
                    ),
                    extension: Metadata {
                        name: Some("Enterprise(level2)".to_string()),
                        ..Metadata::default()
                    },
                    evolution_conditions: vec![],
                    evolution_fee: EvolutionFee {
                        fee_token: Addr::unchecked("space"),
                        evolve_fee_amount: Uint128::new(0u128),
                        devolve_fee_amount: Uint128::new(10000000u128),
                        fee_recipient: Addr::unchecked(CREATOR.to_string()),
                    },
                },
            ],
        };

        let exec_msg = ExecuteMsg::Mint(mint_msg.clone());
        entry::execute(deps.as_mut(), mock_env(), info, exec_msg).unwrap();

        let info = mock_info(
            "john",
            &[Coin {
                denom: "native".to_string(),
                amount: Uint128::new(100000000u128),
            }],
        );

        // error case1. try evolve without selected_nfts;
        let evolve_msg = ExecuteMsg::Evolve {
            token_id: token_id.into(),
            selected_nfts: None,
        };

        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), evolve_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "Need to decide token id which will use for evolve"
            )),
        );

        // error case2. try evolve wrong selected_nfts;
        let evolve_msg = ExecuteMsg::Evolve {
            token_id: token_id.into(),
            selected_nfts: Some(vec![Token::Cw721 {
                contract_address: Addr::unchecked("Fake SpaceShip Engine"),
                token_id: Some("Tier1 Engine".to_string()),
            }]),
        };

        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), evolve_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "Need to decide token id which will use for evolve"
            )),
        );

        let evolve_msg = ExecuteMsg::Evolve {
            token_id: token_id.into(),
            selected_nfts: Some(vec![Token::Cw721 {
                contract_address: Addr::unchecked("SpaceShip Engine"),
                token_id: Some("Tier1 Engine".to_string()),
            }]),
        };

        let res = entry::execute(deps.as_mut(), mock_env(), info.clone(), evolve_msg).unwrap();

        assert_eq!(
            res.messages,
            vec![
                // get cw721 conditions tokens
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: Addr::unchecked("SpaceShip Engine").to_string(),
                    msg: to_binary(&Cw721ExecuteMsg::TransferNft {
                        recipient: mock_env().contract.address.to_string(),
                        token_id: "Tier1 Engine".to_string(),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                // send
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: mint_msg.evolution_data[0]
                        .evolution_fee
                        .fee_token
                        .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: mint_msg.evolution_data[0]
                            .evolution_fee
                            .fee_recipient
                            .to_string(),
                        amount: mint_msg.evolution_data[0].evolution_fee.evolve_fee_amount,
                    })
                    .unwrap(),
                    funds: vec![],
                }))
            ],
        );

        // check holds
        let token_holds = holds(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(
            token_holds.holds,
            vec![Token::Cw721 {
                contract_address: Addr::unchecked("SpaceShip Engine"),
                token_id: Some("Tier1 Engine".to_string()),
            }]
        );

        // devolve test
        let info = mock_info("space", &[]);
        // error case1. try devolve without selected_nfts;
        let devolve_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            amount: Uint128::new(10000000u128),
            sender: "john".to_string(),
            msg: to_binary(&Cw20HookMsg::Devolve {
                token_id: token_id.into(),
                selected_nfts: None,
            })
            .unwrap(),
        });

        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), devolve_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err("Can not make key without token id")),
        );

        // error case2. try devolve with wrong selected_nfts;
        let devolve_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            amount: Uint128::new(10000000u128),
            sender: "john".to_string(),
            msg: to_binary(&Cw20HookMsg::Devolve {
                token_id: token_id.into(),
                selected_nfts: Some(vec![Token::Cw721 {
                    contract_address: Addr::unchecked("Fake SpaceShip Engine"),
                    token_id: Some("Tier1 Engine".to_string()),
                }]),
            })
            .unwrap(),
        });

        let err = entry::execute(deps.as_mut(), mock_env(), info.clone(), devolve_msg).unwrap_err();

        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err("Can not make key without token id")),
        );

        let devolve_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            amount: Uint128::new(10000000u128),
            sender: "john".to_string(),
            msg: to_binary(&Cw20HookMsg::Devolve {
                token_id: token_id.into(),
                selected_nfts: Some(vec![Token::Cw721 {
                    contract_address: Addr::unchecked("SpaceShip Engine"),
                    token_id: Some("Tier1 Engine".to_string()),
                }]),
            })
            .unwrap(),
        });

        let res = entry::execute(deps.as_mut(), mock_env(), info, devolve_msg).unwrap();

        assert_eq!(
            res.messages,
            vec![
                // send fee
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: mint_msg.evolution_data[1]
                        .evolution_fee
                        .fee_token
                        .to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: mint_msg.evolution_data[1]
                            .evolution_fee
                            .fee_recipient
                            .to_string(),
                        amount: mint_msg.evolution_data[1].evolution_fee.devolve_fee_amount,
                    })
                    .unwrap(),
                    funds: vec![],
                })),
                // send conditions tokens
                SubMsg::new(
                    (Token::Cw721 {
                        contract_address: Addr::unchecked("SpaceShip Engine"),
                        token_id: Some("Tier1 Engine".to_string()),
                    })
                    .into_send_msg("john".to_string())
                    .unwrap(),
                ),
            ],
        );

        // check holds
        let token_holds = holds(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(token_holds.holds, vec![]);
    }
}
