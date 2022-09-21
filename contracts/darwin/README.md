# Darwin, the evolve extension of Cw721

Builds on top of the metadata pattern in `cw721-metadata-onchain`.

All of the CW-721 logic and behaviour you would expect for an NFT is implemented as normal.

New and Changed msgs from CW-721 are described below.

## Mint msg changed
```rust
/// Mint a new NFT, can only be called by the contract minter
Mint(DarwinMintMsg<T>),

pub struct DarwinMintMsg<T> {
    /// Unique ID of the NFT
    pub token_id: String,
    /// The owner of the newly minter NFT
    pub owner: String,

    /// Set of data for each evolved stage
    pub evolution_data: Vec<EvolutionMetaData<T>>,
}

pub struct EvolutionMetaData<T> {
    pub token_uri: Option<String>,
    pub extension: T,
    pub evolution_conditions: Vec<Token>,
    pub evolution_fee: EvolutionFee,
}

pub struct EvolutionFee {
    pub fee_token: Addr,
    pub evolve_fee_amount: Uint128,
    pub devolve_fee_amount: Uint128,
    pub fee_recipient: Addr,
}
```

`evolution_conditions` is a list of tokens required to execute evolve. `TokenInfo` initially sets the data of `evolution_data[0]`

## New Execute msgs

```rust
/// Evolve nft
Evolve {
    token_id: String,
    /// Need to pick specific NFT for conditions that token_id is None
    selected_nfts: Option<Vec<Token>>,
},

Receive(Cw20ReceiveMsg),

pub enum Cw20HookMsg {
    /// Devolve NFT
    Devolve {
        token_id: String,
        /// Need to pick specific NFT for conditions that token_id is None
        selected_nfts: Option<Vec<Token>>,
    },
}
```

`Evolve` is execute function that evolve NFT. `TokenInfo` will change to the next `stage` data. Msg sender need to do `IncreaseAllowance` for CW-20 of `fee_token` and `evolution_conditions` and do `Approve` for CW-721 of `evolution_conditions` to Darwin Nft contract. Also need to add native of `evolution_conditions` to `funds` when execute msg.

`Devolve` is execute function that devolve NFT. `TokenInfo` will change to former `stage` data. The tokens used for evolution are returned to the owner of NFT.

`selected_nfts` is required to select CW-721 tokens to use for evolution/withdraw(when devolve) if the token_ids of some CW-721 tokens in `evolution_condition` are `None` have not been determined.


## New query msgs

```rust
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
```


## New query responses
```rust
pub struct EvolutionInfoResponse<T> {
    pub evolution_info: EvolutionMetaData<T>,
}

pub struct EvolvedStageResponse {
    pub evolved_stage: u8,
}

pub struct HoldsResponse {
    pub holds: Vec<Token>,
}

```

`EvolutionInfo` is a query to get `EvolutionInfo` which is include `token_uri`, `extension` (onchain metadata), `evolution_condition` and `evolution_fee`. Return type: `EvolutionInfoResponse`

`EvolvedStage` is a query to get `stage` of the token. Return type: `EvolvedStageResponse`

`Holds` is a query to get tokens deposited when it evolved. Return type: `HoldsResponse`