use crate::state::{Cw1155Contract, TokenInfo};
use crate::{CONTRACT_NAME, CONTRACT_VERSION};
use cosmwasm_std::{
    Addr, Binary, CustomMsg, DepsMut, Env, Event, MessageInfo, Order, Response, StdResult, Storage,
    SubMsg, Uint128,
};
use cw1155::{
    ApproveAllEvent, ApproveEvent, Balance, BurnEvent, Cw1155BatchReceiveMsg, Cw1155ContractError,
    Cw1155ExecuteMsg, Cw1155InstantiateMsg, Cw1155MintMsg, Cw1155ReceiveMsg, Expiration, MintEvent,
    RevokeAllEvent, RevokeEvent, TokenAmount, TokenApproval, TransferEvent,
};
use cw2::set_contract_version;
use serde::de::DeserializeOwned;
use serde::Serialize;

impl<'a, T, C, E, Q> Cw1155Contract<'a, T, C, E, Q>
where
    T: Serialize + DeserializeOwned + Clone,
    C: CustomMsg,
    E: CustomMsg,
    Q: CustomMsg,
{
    pub fn instantiate(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        msg: Cw1155InstantiateMsg,
    ) -> StdResult<Response> {
        // store contract version
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

        // store contract info
        let contract_info = cw721::ContractInfoResponse {
            name: msg.name,
            symbol: msg.symbol,
        };
        self.contract_info.save(deps.storage, &contract_info)?;

        // store minter
        let owner = match msg.minter {
            Some(owner) => deps.api.addr_validate(&owner)?,
            None => info.sender,
        };
        cw_ownable::initialize_owner(deps.storage, deps.api, Some(owner.as_ref()))?;

        // store total supply
        self.supply.save(deps.storage, &Uint128::zero())?;

        Ok(Response::default())
    }

    pub fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: Cw1155ExecuteMsg<T, E>,
    ) -> Result<Response, Cw1155ContractError> {
        let env = ExecuteEnv { deps, env, info };
        match msg {
            // cw1155
            Cw1155ExecuteMsg::SendBatch {
                from,
                to,
                batch,
                msg,
            } => self.send_batch(env, from, to, batch, msg),
            Cw1155ExecuteMsg::MintBatch { recipient, msgs } => {
                self.mint_batch(env, recipient, msgs)
            }
            Cw1155ExecuteMsg::BurnBatch { from, batch } => self.burn_batch(env, from, batch),
            Cw1155ExecuteMsg::ApproveAll { operator, expires } => {
                self.approve_all(env, operator, expires)
            }
            Cw1155ExecuteMsg::RevokeAll { operator } => self.revoke_all(env, operator),

            // cw721
            Cw1155ExecuteMsg::Send {
                from,
                to,
                token_id,
                amount,
                msg,
            } => self.send(env, from, to, token_id, amount, msg),
            Cw1155ExecuteMsg::Mint { recipient, msg } => self.mint(env, recipient, msg),
            Cw1155ExecuteMsg::Burn {
                from,
                token_id,
                amount,
            } => self.burn(env, from, token_id, amount),
            Cw1155ExecuteMsg::Approve {
                spender,
                token_id,
                amount,
                expires,
            } => self.approve_token(env, spender, token_id, amount, expires),
            Cw1155ExecuteMsg::Revoke {
                spender,
                token_id,
                amount,
            } => self.revoke_token(env, spender, token_id, amount),
            Cw1155ExecuteMsg::Extension { .. } => unimplemented!(),
        }
    }
}

/// To mitigate clippy::too_many_arguments warning
pub struct ExecuteEnv<'a> {
    deps: DepsMut<'a>,
    env: Env,
    info: MessageInfo,
}

// helper
impl<'a, T, C, E, Q> Cw1155Contract<'a, T, C, E, Q>
where
    T: Serialize + DeserializeOwned + Clone,
    C: CustomMsg,
    E: CustomMsg,
    Q: CustomMsg,
{
    pub fn mint(
        &self,
        env: ExecuteEnv,
        recipient: String,
        msg: Cw1155MintMsg<T>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let to = deps.api.addr_validate(&recipient)?;

        let mut rsp = Response::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            None,
            Some(to),
            vec![TokenAmount {
                token_id: msg.token_id.to_string(),
                amount: msg.amount,
            }],
        )?;
        rsp = rsp.add_event(event);

        // store token info if not exist (if it is the first mint)
        if !self.tokens.has(deps.storage, &msg.token_id) {
            let token_info = TokenInfo {
                token_uri: msg.token_uri,
                extension: msg.extension,
            };
            self.tokens.save(deps.storage, &msg.token_id, &token_info)?;
        }

        Ok(rsp)
    }

    pub fn mint_batch(
        &self,
        env: ExecuteEnv,
        recipient: String,
        msgs: Vec<Cw1155MintMsg<T>>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        cw_ownable::assert_owner(deps.storage, &info.sender)?;

        let to = deps.api.addr_validate(&recipient)?;

        let batch = msgs
            .iter()
            .map(|msg| {
                // store token info if not exist (if it is the first mint)
                if !self.tokens.has(deps.storage, &msg.token_id) {
                    let token_info = TokenInfo {
                        token_uri: msg.token_uri.clone(),
                        extension: msg.extension.clone(),
                    };
                    self.tokens.save(deps.storage, &msg.token_id, &token_info)?;
                }
                Ok(TokenAmount {
                    token_id: msg.token_id.to_string(),
                    amount: msg.amount,
                })
            })
            .collect::<StdResult<Vec<_>>>()?;

        let mut rsp = Response::default();
        let event = self.update_balances(&mut deps, &env, None, Some(to), batch)?;
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn send(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        to: String,
        token_id: String,
        amount: Uint128,
        msg: Option<Binary>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            env,
            info,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };
        let to = deps.api.addr_validate(&to)?;

        let balance_update =
            self.verify_approval(deps.storage, &env, &info, &from, &token_id, amount)?;

        let mut rsp = Response::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            Some(from.clone()),
            Some(to.clone()),
            vec![TokenAmount {
                token_id: token_id.to_string(),
                amount: balance_update.amount,
            }],
        )?;
        rsp = rsp.add_event(event);

        if let Some(msg) = msg {
            rsp.messages = vec![SubMsg::new(
                Cw1155ReceiveMsg {
                    operator: info.sender.to_string(),
                    from: Some(from.to_string()),
                    amount,
                    token_id,
                    msg,
                }
                .into_cosmos_msg(to)?,
            )]
        }

        Ok(rsp)
    }

    pub fn send_batch(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        to: String,
        batch: Vec<TokenAmount>,
        msg: Option<Binary>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            env,
            info,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };
        let to = deps.api.addr_validate(&to)?;

        let batch = self.verify_approvals(deps.storage, &env, &info, &from, batch)?;

        let mut rsp = Response::default();
        let event = self.update_balances(
            &mut deps,
            &env,
            Some(from.clone()),
            Some(to.clone()),
            batch.to_vec(),
        )?;
        rsp = rsp.add_event(event);

        if let Some(msg) = msg {
            rsp.messages = vec![SubMsg::new(
                Cw1155BatchReceiveMsg {
                    operator: info.sender.to_string(),
                    from: Some(from.to_string()),
                    batch,
                    msg,
                }
                .into_cosmos_msg(to)?,
            )]
        };

        Ok(rsp)
    }

    pub fn burn(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        token_id: String,
        amount: Uint128,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };

        // whoever can transfer these tokens can burn
        let balance_update =
            self.verify_approval(deps.storage, &env, &info, &from, &token_id, amount)?;

        let mut rsp = Response::default();

        let event = self.update_balances(
            &mut deps,
            &env,
            Some(from),
            None,
            vec![TokenAmount {
                token_id,
                amount: balance_update.amount,
            }],
        )?;
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn burn_batch(
        &self,
        env: ExecuteEnv,
        from: Option<String>,
        batch: Vec<TokenAmount>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv {
            mut deps,
            info,
            env,
        } = env;

        let from = if let Some(from) = from {
            deps.api.addr_validate(&from)?
        } else {
            info.sender.clone()
        };

        let batch = self.verify_approvals(deps.storage, &env, &info, &from, batch)?;

        let mut rsp = Response::default();
        let event = self.update_balances(&mut deps, &env, Some(from), None, batch)?;
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn approve_token(
        &self,
        env: ExecuteEnv,
        operator: String,
        token_id: String,
        amount: Option<Uint128>,
        expiration: Option<Expiration>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv { deps, info, env } = env;

        // reject expired data as invalid
        let expiration = expiration.unwrap_or_default();
        if expiration.is_expired(&env.block) {
            return Err(Cw1155ContractError::Expired {});
        }

        // get sender's token balance to get valid approval amount
        let balance = self
            .balances
            .load(deps.storage, (info.sender.clone(), token_id.to_string()))?;
        let approval_amount = amount.unwrap_or(Uint128::MAX).min(balance.amount);

        // store the approval
        let operator = deps.api.addr_validate(&operator)?;
        self.token_approves.save(
            deps.storage,
            (&token_id, &info.sender, &operator),
            &TokenApproval {
                amount: approval_amount,
                expiration,
            },
        )?;

        let mut rsp = Response::default();

        let event = ApproveEvent::new(&info.sender, &operator, &token_id, approval_amount).into();
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn approve_all(
        &self,
        env: ExecuteEnv,
        operator: String,
        expires: Option<Expiration>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv { deps, info, env } = env;

        // reject expired data as invalid
        let expires = expires.unwrap_or_default();
        if expires.is_expired(&env.block) {
            return Err(Cw1155ContractError::Expired {});
        }

        // set the operator for us
        let operator = deps.api.addr_validate(&operator)?;
        self.approves
            .save(deps.storage, (&info.sender, &operator), &expires)?;

        let mut rsp = Response::default();

        let event = ApproveAllEvent::new(&info.sender, &operator).into();
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn revoke_token(
        &self,
        env: ExecuteEnv,
        operator: String,
        token_id: String,
        amount: Option<Uint128>,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        let operator = deps.api.addr_validate(&operator)?;

        // get prev approval amount to get valid revoke amount
        let prev_approval = self
            .token_approves
            .load(deps.storage, (&token_id, &info.sender, &operator))?;
        let revoke_amount = amount.unwrap_or(Uint128::MAX).min(prev_approval.amount);

        // remove or update approval
        if revoke_amount == prev_approval.amount {
            self.token_approves
                .remove(deps.storage, (&token_id, &info.sender, &operator));
        } else {
            self.token_approves.update(
                deps.storage,
                (&token_id, &info.sender, &operator),
                |prev| -> StdResult<_> {
                    let mut new_approval = prev.unwrap();
                    new_approval.amount = new_approval.amount.checked_sub(revoke_amount)?;
                    Ok(new_approval)
                },
            )?;
        }

        let mut rsp = Response::default();

        let event = RevokeEvent::new(&info.sender, &operator, &token_id, revoke_amount).into();
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    pub fn revoke_all(
        &self,
        env: ExecuteEnv,
        operator: String,
    ) -> Result<Response, Cw1155ContractError> {
        let ExecuteEnv { deps, info, .. } = env;
        let operator_addr = deps.api.addr_validate(&operator)?;
        self.approves
            .remove(deps.storage, (&info.sender, &operator_addr));

        let mut rsp = Response::default();

        let event = RevokeAllEvent::new(&info.sender, &operator_addr).into();
        rsp = rsp.add_event(event);

        Ok(rsp)
    }

    /// When from is None: mint new tokens
    /// When to is None: burn tokens
    /// When both are Some: transfer tokens
    ///
    /// Make sure permissions are checked before calling this.
    fn update_balances(
        &self,
        deps: &mut DepsMut,
        env: &Env,
        from: Option<Addr>,
        to: Option<Addr>,
        tokens: Vec<TokenAmount>,
    ) -> Result<Event, Cw1155ContractError> {
        if let Some(from) = &from {
            for TokenAmount { token_id, amount } in tokens.iter() {
                self.balances.update(
                    deps.storage,
                    (from.clone(), token_id.to_string()),
                    |balance: Option<Balance>| -> StdResult<_> {
                        let mut new_balance = balance.unwrap();
                        new_balance.amount = new_balance.amount.checked_sub(*amount)?;
                        Ok(new_balance)
                    },
                )?;
            }
        }

        if let Some(to) = &to {
            for TokenAmount { token_id, amount } in tokens.iter() {
                self.balances.update(
                    deps.storage,
                    (to.clone(), token_id.to_string()),
                    |balance: Option<Balance>| -> StdResult<_> {
                        let mut new_balance: Balance = if let Some(balance) = balance {
                            balance
                        } else {
                            Balance {
                                owner: to.clone(),
                                amount: Uint128::zero(),
                                token_id: token_id.to_string(),
                            }
                        };

                        new_balance.amount = new_balance.amount.checked_add(*amount)?;
                        Ok(new_balance)
                    },
                )?;
            }
        }

        let event = if let Some(from) = &from {
            for TokenAmount { token_id, amount } in &tokens {
                // remove token approvals
                for (operator, approval) in self
                    .token_approves
                    .prefix((token_id, from))
                    .range(deps.storage, None, None, Order::Ascending)
                    .collect::<StdResult<Vec<_>>>()?
                {
                    if approval.is_expired(env) || approval.amount <= *amount {
                        self.token_approves
                            .remove(deps.storage, (token_id, from, &operator));
                    } else {
                        self.token_approves.update(
                            deps.storage,
                            (token_id, from, &operator),
                            |prev| -> StdResult<_> {
                                let mut new_approval = prev.unwrap();
                                new_approval.amount = new_approval.amount.checked_sub(*amount)?;
                                Ok(new_approval)
                            },
                        )?;
                    }
                }

                // decrement tokens if burning
                if to.is_none() {
                    self.decrement_tokens(deps.storage, token_id, amount)?;
                }
            }

            if let Some(to) = &to {
                // transfer
                TransferEvent::new(from, to, tokens).into()
            } else {
                // burn
                BurnEvent::new(from, tokens).into()
            }
        } else if let Some(to) = &to {
            // mint
            for TokenAmount { token_id, amount } in &tokens {
                self.increment_tokens(deps.storage, token_id, amount)?;
            }
            MintEvent::new(to, tokens).into()
        } else {
            panic!("Invalid transfer: from and to cannot both be None")
        };

        Ok(event)
    }

    /// returns valid token amount if the sender can execute or is approved to execute
    pub fn verify_approval(
        &self,
        storage: &dyn Storage,
        env: &Env,
        info: &MessageInfo,
        owner: &Addr,
        token_id: &str,
        amount: Uint128,
    ) -> Result<TokenAmount, Cw1155ContractError> {
        let operator = &info.sender;

        let owner_balance = self
            .balances
            .load(storage, (owner.clone(), token_id.to_string()))?;
        let mut balance_update = TokenAmount {
            token_id: token_id.to_string(),
            amount: owner_balance.amount.min(amount),
        };

        // owner or all operator can execute
        if owner == operator || self.verify_all_approval(storage, env, owner, operator) {
            return Ok(balance_update);
        }

        // token operator can execute up to approved amount
        if let Some(token_approval) =
            self.get_active_token_approval(storage, env, owner, operator, token_id)
        {
            balance_update.amount = balance_update.amount.min(token_approval.amount);
            return Ok(balance_update);
        }

        Err(Cw1155ContractError::Unauthorized {})
    }

    /// returns valid token amounts if the sender can execute or is approved to execute on all provided tokens
    pub fn verify_approvals(
        &self,
        storage: &dyn Storage,
        env: &Env,
        info: &MessageInfo,
        owner: &Addr,
        tokens: Vec<TokenAmount>,
    ) -> Result<Vec<TokenAmount>, Cw1155ContractError> {
        tokens
            .iter()
            .map(|TokenAmount { token_id, amount }| {
                self.verify_approval(storage, env, info, owner, token_id, *amount)
            })
            .collect()
    }

    pub fn verify_all_approval(
        &self,
        storage: &dyn Storage,
        env: &Env,
        owner: &Addr,
        operator: &Addr,
    ) -> bool {
        match self.approves.load(storage, (owner, operator)) {
            Ok(ex) => !ex.is_expired(&env.block),
            Err(_) => false,
        }
    }

    pub fn get_active_token_approval(
        &self,
        storage: &dyn Storage,
        env: &Env,
        owner: &Addr,
        operator: &Addr,
        token_id: &str,
    ) -> Option<TokenApproval> {
        match self
            .token_approves
            .load(storage, (token_id, owner, operator))
        {
            Ok(approval) => {
                if !approval.is_expired(env) {
                    Some(approval)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}
