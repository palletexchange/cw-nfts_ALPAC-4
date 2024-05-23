use cosmwasm_schema::write_api;
use cosmwasm_std::Empty;

use cw1155::{Cw1155ExecuteMsg, Cw1155InstantiateMsg, Cw1155QueryMsg};

type Extension = Empty;

fn main() {
    write_api! {
        instantiate: Cw1155InstantiateMsg,
        execute: Cw1155ExecuteMsg<Extension>,
        query: Cw1155QueryMsg<Empty>,
    }
}
