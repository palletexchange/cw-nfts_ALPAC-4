use crate::msg::{EvolutionInfoResponse, EvolvedStageResponse, HoldsResponse};
use crate::state::{EVOLVED_META_DATA, EVOLVED_STAGE, HOLDS};
use crate::Metadata;
use cosmwasm_std::{Deps, Order, StdResult};

pub fn evolved_stage(deps: Deps, token_id: String) -> StdResult<EvolvedStageResponse> {
    let evolved_stage = EVOLVED_STAGE.load(deps.storage, &token_id)?;
    Ok(EvolvedStageResponse { evolved_stage })
}

pub fn evolution_info(
    deps: Deps,
    token_id: String,
    stage: u8,
) -> StdResult<EvolutionInfoResponse<Metadata>> {
    let evolution_info = EVOLVED_META_DATA.load(deps.storage, (&token_id, stage))?;
    Ok(EvolutionInfoResponse::<Metadata> { evolution_info })
}

pub fn holds(deps: Deps, token_id: String) -> StdResult<HoldsResponse> {
    let holds = HOLDS
        .prefix(&token_id)
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item.unwrap();
            v
        })
        .collect();
    Ok(HoldsResponse { holds })
}
