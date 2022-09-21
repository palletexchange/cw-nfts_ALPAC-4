use cosmwasm_schema::cw_serde;

use cw20::Cw20ReceiveMsg;
use cw721::CustomMsg;

use crate::state::{EvolutionMetaData, Token};
use crate::Metadata;

#[cw_serde]
pub struct Extension<T> {
    pub devolved_extension: T,
    pub evolved_extension: T,
    pub evolve_conditions: Vec<Token>,
}

/// This is like Cw721ExecuteMsg but we add a Mint command for an owner
/// to make this stand-alone. You will likely want to remove mint and
/// use other control logic in any contract that inherits this.
#[cw_serde]
pub enum DarwinExecuteMsg<T> {
    /// Mint a new NFT, can only be called by the contract minter
    Mint(DarwinMintMsg<T>),

    /// Evolve nft
    Evolve {
        token_id: String,
        /// Need to pick specific NFT for conditions that token_id is None
        selected_nfts: Option<Vec<Token>>,
    },

    Receive(Cw20ReceiveMsg),
}

impl CustomMsg for DarwinExecuteMsg<Metadata> {}

#[cw_serde]
pub struct DarwinMintMsg<T> {
    /// Unique ID of the NFT
    pub token_id: String,
    /// The owner of the newly minter NFT
    pub owner: String,

    /// Set of data for each evolved stage
    pub evolution_data: Vec<EvolutionMetaData<T>>,
}

#[cw_serde]
pub enum Cw20HookMsg {
    /// Devolve NFT
    Devolve {
        token_id: String,
        /// Need to pick specific NFT for conditions that token_id is None
        selected_nfts: Option<Vec<Token>>,
    },
}

#[cw_serde]
pub enum DarwinQueryMsg {
    EvolutionInfo { token_id: String, stage: u8 },

    EvolvedStage { token_id: String },

    Holds { token_id: String },
}

impl CustomMsg for DarwinQueryMsg {}

#[cw_serde]
pub struct EvolvedStageResponse {
    pub evolved_stage: u8,
}

#[cw_serde]
pub struct EvolutionInfoResponse<T> {
    pub evolution_info: EvolutionMetaData<T>,
}

#[cw_serde]
pub struct HoldsResponse {
    pub holds: Vec<Token>,
}
