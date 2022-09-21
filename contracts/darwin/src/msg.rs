use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Binary;

use cw20::Cw20ReceiveMsg;

use cw721::Expiration;
use cw721_base::msg::{ExecuteMsg as Cw721ExecuteMsg, QueryMsg as CW721QueryMsg};

use crate::state::{EvolutionMetaData, Token};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Extension<T> {
    pub devolved_extension: T,
    pub evolved_extension: T,
    pub evolve_conditions: Vec<Token>,
}

/// This is like Cw721ExecuteMsg but we add a Mint command for an owner
/// to make this stand-alone. You will likely want to remove mint and
/// use other control logic in any contract that inherits this.
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DarwinExecuteMsg<T> {
    /// Transfer is a base message to move a token to another account without triggering actions
    TransferNft {
        recipient: String,
        token_id: String,
    },
    /// Send is a base message to transfer a token to a contract and trigger an action
    /// on the receiving contract.
    SendNft {
        contract: String,
        token_id: String,
        msg: Binary,
    },
    /// Allows operator to transfer / send the token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    Approve {
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted Approval
    Revoke {
        spender: String,
        token_id: String,
    },
    /// Allows operator to transfer / send any token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    ApproveAll {
        operator: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted ApproveAll permission
    RevokeAll {
        operator: String,
    },

    /// Mint a new NFT, can only be called by the contract minter
    Mint(DarwinMintMsg<T>),

    /// Burn an NFT the sender has access to
    Burn {
        token_id: String,
    },

    /// Evolve nft
    Evolve {
        token_id: String,
        /// Need to pick specific NFT for conditions that token_id is None
        selected_nfts: Option<Vec<Token>>,
    },

    Receive(Cw20ReceiveMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DarwinMintMsg<T> {
    /// Unique ID of the NFT
    pub token_id: String,
    /// The owner of the newly minter NFT
    pub owner: String,

    /// Set of data for each evolved stage
    pub evolution_data: Vec<EvolutionMetaData<T>>,
}

impl<T> From<DarwinExecuteMsg<T>> for Cw721ExecuteMsg<T>
where
    T: Serialize + DeserializeOwned + Clone,
{
    fn from(msg: DarwinExecuteMsg<T>) -> Cw721ExecuteMsg<T> {
        match msg {
            DarwinExecuteMsg::TransferNft {
                recipient,
                token_id,
            } => Cw721ExecuteMsg::TransferNft {
                recipient,
                token_id,
            },
            DarwinExecuteMsg::SendNft {
                contract,
                token_id,
                msg,
            } => Cw721ExecuteMsg::SendNft {
                contract,
                token_id,
                msg,
            },
            DarwinExecuteMsg::Approve {
                spender,
                token_id,
                expires,
            } => Cw721ExecuteMsg::Approve {
                spender,
                token_id,
                expires,
            },
            DarwinExecuteMsg::Revoke { spender, token_id } => {
                Cw721ExecuteMsg::Revoke { spender, token_id }
            }
            DarwinExecuteMsg::ApproveAll { operator, expires } => {
                Cw721ExecuteMsg::ApproveAll { operator, expires }
            }
            DarwinExecuteMsg::RevokeAll { operator } => Cw721ExecuteMsg::RevokeAll { operator },
            DarwinExecuteMsg::Burn { token_id } => Cw721ExecuteMsg::Burn { token_id },
            _ => panic!("cannot covert to Cw721ExecuteMsg"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Devolve NFT
    Devolve {
        token_id: String,
        /// Need to pick specific NFT for conditions that token_id is None
        selected_nfts: Option<Vec<Token>>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DarwinQueryMsg {
    /// Return the owner of the given token, error if token does not exist
    /// Return type: OwnerOfResponse
    OwnerOf {
        token_id: String,
        /// unset or false will filter out expired approvals, you must set to true to see them
        include_expired: Option<bool>,
    },
    /// List all operators that can access all of the owner's tokens.
    /// Return type: `OperatorsResponse`
    AllOperators {
        owner: String,
        /// unset or false will filter out expired approvals, you must set to true to see them
        include_expired: Option<bool>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Total number of tokens issued
    NumTokens {},

    /// With MetaData Extension.
    /// Returns top-level metadata about the contract: `ContractInfoResponse`
    ContractInfo {},
    /// With MetaData Extension.
    /// Returns metadata about one particular token, based on *ERC721 Metadata JSON Schema*
    /// but directly from the contract: `NftInfoResponse`
    NftInfo {
        token_id: String,
    },
    /// With MetaData Extension.
    /// Returns the result of both `NftInfo` and `OwnerOf` as one query as an optimization
    /// for clients: `AllNftInfo`
    AllNftInfo {
        token_id: String,
        /// unset or false will filter out expired approvals, you must set to true to see them
        include_expired: Option<bool>,
    },

    /// With Enumerable extension.
    /// Returns all tokens owned by the given address, [] if unset.
    /// Return type: TokensResponse.
    Tokens {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// With Enumerable extension.
    /// Requires pagination. Lists all token_ids controlled by the contract.
    /// Return type: TokensResponse.
    AllTokens {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    EvolutionInfo {
        token_id: String,
        stage: u8,
    },

    EvolvedStage {
        token_id: String,
    },

    Holds {
        token_id: String,
    },
}

impl From<DarwinQueryMsg> for CW721QueryMsg {
    fn from(msg: DarwinQueryMsg) -> CW721QueryMsg {
        match msg {
            DarwinQueryMsg::OwnerOf {
                token_id,
                include_expired,
            } => CW721QueryMsg::OwnerOf {
                token_id,
                include_expired,
            },
            DarwinQueryMsg::AllOperators {
                owner,
                include_expired,
                start_after,
                limit,
            } => CW721QueryMsg::AllOperators {
                owner,
                include_expired,
                start_after,
                limit,
            },
            DarwinQueryMsg::NumTokens {} => CW721QueryMsg::NumTokens {},
            DarwinQueryMsg::ContractInfo {} => CW721QueryMsg::ContractInfo {},
            DarwinQueryMsg::NftInfo { token_id } => CW721QueryMsg::NftInfo { token_id },
            DarwinQueryMsg::AllNftInfo {
                token_id,
                include_expired,
            } => CW721QueryMsg::AllNftInfo {
                token_id,
                include_expired,
            },
            DarwinQueryMsg::Tokens {
                owner,
                start_after,
                limit,
            } => CW721QueryMsg::Tokens {
                owner,
                start_after,
                limit,
            },
            DarwinQueryMsg::AllTokens { start_after, limit } => {
                CW721QueryMsg::AllTokens { start_after, limit }
            }
            _ => panic!("cannot covert {:?} to CW721QueryMsg", msg),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct EvolvedStageResponse {
    pub evolved_stage: u8,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct EvolutionInfoResponse<T> {
    pub evolution_info: EvolutionMetaData<T>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct HoldsResponse {
    pub holds: Vec<Token>,
}
