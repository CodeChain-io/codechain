// Copyright 2018-2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod action_data;
mod actions;
mod distribute;

use std::collections::btree_map::BTreeMap;
use std::collections::HashMap;

use ccrypto::Blake;
use ckey::{public_to_address, recover, Address, Signature};
use cstate::{ActionHandler, StateResult, TopLevelState, TopState};
use ctypes::errors::{RuntimeError, SyntaxError};
use ctypes::util::unexpected::Mismatch;
use ctypes::{CommonParams, Header};
use primitives::H256;
use rlp::{Decodable, UntrustedRlp};

use self::action_data::{Candidates, Delegation, IntermediateRewards, Jail, ReleaseResult, StakeAccount, Stakeholders};
use self::actions::Action;
pub use self::distribute::fee_distribute;

const CUSTOM_ACTION_HANDLER_ID: u64 = 2;

pub struct Stake {
    genesis_stakes: HashMap<Address, u64>,
    enable_delegations: bool,
}

impl Stake {
    #[cfg(not(test))]
    pub fn new(genesis_stakes: HashMap<Address, u64>) -> Stake {
        Stake {
            genesis_stakes,
            enable_delegations: parse_env_var_enable_delegations(),
        }
    }

    #[cfg(test)]
    pub fn new(genesis_stakes: HashMap<Address, u64>) -> Stake {
        Stake {
            genesis_stakes,
            enable_delegations: true,
        }
    }
}

#[cfg(not(test))]
fn parse_env_var_enable_delegations() -> bool {
    let var = std::env::var("ENABLE_DELEGATIONS");
    match var.as_ref().map(|x| x.trim()) {
        Ok(value) => value.parse::<bool>().unwrap(),
        Err(std::env::VarError::NotPresent) => false,
        Err(err) => unreachable!("{:?}", err),
    }
}

impl ActionHandler for Stake {
    fn name(&self) -> &'static str {
        "stake handler"
    }

    fn handler_id(&self) -> u64 {
        CUSTOM_ACTION_HANDLER_ID
    }

    fn init(&self, state: &mut TopLevelState) -> StateResult<()> {
        let mut stakeholders = Stakeholders::load_from_state(state)?;
        for (address, amount) in self.genesis_stakes.iter() {
            let account = StakeAccount {
                address,
                balance: *amount,
            };
            account.save_to_state(state)?;
            stakeholders.update_by_increased_balance(&account);
        }
        stakeholders.save_to_state(state)?;
        Ok(())
    }

    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, sender: &Address) -> StateResult<()> {
        let action = Action::decode(&UntrustedRlp::new(bytes)).expect("Verification passed");
        match action {
            Action::TransferCCS {
                address,
                quantity,
            } => transfer_ccs(state, sender, &address, quantity),
            Action::DelegateCCS {
                address,
                quantity,
            } => {
                if self.enable_delegations {
                    delegate_ccs(state, sender, &address, quantity)
                } else {
                    Err(RuntimeError::FailedToHandleCustomAction("DelegateCCS is disabled".to_string()).into())
                }
            }
            Action::Revoke {
                address,
                quantity,
            } => {
                if self.enable_delegations {
                    revoke(state, sender, &address, quantity)
                } else {
                    Err(RuntimeError::FailedToHandleCustomAction("Revoke is disabled".to_string()).into())
                }
            }
            Action::SelfNominate {
                deposit,
                ..
            } => {
                if self.enable_delegations {
                    self_nominate(state, sender, deposit, 0, 0)
                } else {
                    Err(RuntimeError::FailedToHandleCustomAction("SelfNominate is disabled".to_string()).into())
                }
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => change_params(state, metadata_seq, *params, &signatures),
        }
    }

    fn verify(&self, bytes: &[u8]) -> Result<(), SyntaxError> {
        let action = Action::decode(&UntrustedRlp::new(bytes))
            .map_err(|err| SyntaxError::InvalidCustomAction(err.to_string()))?;
        match action {
            Action::TransferCCS {
                ..
            } => Ok(()),
            Action::DelegateCCS {
                ..
            } => Ok(()),
            Action::Revoke {
                ..
            } => Ok(()),
            Action::SelfNominate {
                ..
            } => {
                // FIXME: Metadata size limit
                Ok(())
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => {
                let action = Action::ChangeParams {
                    metadata_seq,
                    params,
                    signatures: vec![],
                };
                let encoded_action = H256::blake(rlp::encode(&action));
                for signature in signatures {
                    // XXX: Signature recovery is an expensive job. Should we do it twice?
                    recover(&signature, &encoded_action).map_err(|err| {
                        SyntaxError::InvalidCustomAction(format!("Cannot decode the signature: {}", err))
                    })?;
                }
                Ok(())
            }
        }
    }

    fn on_close_block(
        &self,
        _state: &mut TopLevelState,
        _header: &Header,
        _parent_header: &Header,
        _parent_common_params: &CommonParams,
    ) -> StateResult<()> {
        Ok(())
    }
}

fn transfer_ccs(state: &mut TopLevelState, sender: &Address, receiver: &Address, quantity: u64) -> StateResult<()> {
    let mut stakeholders = Stakeholders::load_from_state(state)?;
    let mut sender_account = StakeAccount::load_from_state(state, sender)?;
    let mut receiver_account = StakeAccount::load_from_state(state, receiver)?;
    let sender_delegations = Delegation::load_from_state(state, sender)?;

    sender_account.subtract_balance(quantity)?;
    receiver_account.add_balance(quantity)?;

    stakeholders.update_by_decreased_balance(&sender_account, &sender_delegations);
    stakeholders.update_by_increased_balance(&receiver_account);

    stakeholders.save_to_state(state)?;
    sender_account.save_to_state(state)?;
    receiver_account.save_to_state(state)?;

    Ok(())
}

fn delegate_ccs(state: &mut TopLevelState, sender: &Address, delegatee: &Address, quantity: u64) -> StateResult<()> {
    // TODO: remove parent hash from validator set.
    // TODO: handle banned account
    // TODO: handle jailed account
    let candidates = Candidates::load_from_state(state)?;
    let jail = Jail::load_from_state(state)?;
    if candidates.get_candidate(delegatee).is_none() && jail.get_prisoner(delegatee).is_none() {
        return Err(
            RuntimeError::FailedToHandleCustomAction("Can delegate to who is a candidate or a prisoner".into()).into()
        )
    }
    let mut delegator = StakeAccount::load_from_state(state, sender)?;
    let mut delegation = Delegation::load_from_state(state, &sender)?;

    delegator.subtract_balance(quantity)?;
    delegation.add_quantity(*delegatee, quantity)?;
    // delegation does not touch stakeholders

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;
    Ok(())
}

fn revoke(state: &mut TopLevelState, sender: &Address, delegatee: &Address, quantity: u64) -> StateResult<()> {
    let mut delegator = StakeAccount::load_from_state(state, sender)?;
    let mut delegation = Delegation::load_from_state(state, &sender)?;

    delegator.add_balance(quantity)?;
    delegation.subtract_quantity(*delegatee, quantity)?;
    // delegation does not touch stakeholders

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;
    Ok(())
}

fn self_nominate(
    state: &mut TopLevelState,
    sender: &Address,
    deposit: u64,
    current_term: u64,
    nomination_ends_at: u64,
) -> StateResult<()> {
    // TODO: proper handling of get_current_term
    // TODO: proper handling of NOMINATE_EXPIRATION
    // TODO: check banned accounts

    let mut jail = Jail::load_from_state(&state)?;
    let total_deposit = match jail.try_release(sender, current_term) {
        ReleaseResult::InCustody => {
            return Err(RuntimeError::FailedToHandleCustomAction("Account is still in custody".to_string()).into())
        }
        ReleaseResult::NotExists => deposit,
        ReleaseResult::Released(prisoner) => {
            assert_eq!(&prisoner.address, sender);
            prisoner.deposit + deposit
        }
    };

    let mut candidates = Candidates::load_from_state(&state)?;
    state.sub_balance(sender, deposit)?;
    candidates.add_deposit(sender, total_deposit, nomination_ends_at);

    jail.save_to_state(state)?;
    candidates.save_to_state(state)?;
    Ok(())
}

pub fn get_stakes(state: &TopLevelState) -> StateResult<HashMap<Address, u64>> {
    let stakeholders = Stakeholders::load_from_state(state)?;
    let mut result = HashMap::new();
    for stakeholder in stakeholders.iter() {
        let account = StakeAccount::load_from_state(state, stakeholder)?;
        let delegation = Delegation::load_from_state(state, stakeholder)?;
        result.insert(*stakeholder, account.balance + delegation.sum());
    }
    Ok(result)
}

pub fn add_intermediate_rewards(state: &mut TopLevelState, address: Address, reward: u64) -> StateResult<()> {
    let mut rewards = IntermediateRewards::load_from_state(state)?;
    rewards.add_quantity(address, reward);
    rewards.save_to_state(state)?;
    Ok(())
}

pub fn drain_previous_rewards(state: &mut TopLevelState) -> StateResult<BTreeMap<Address, u64>> {
    let mut rewards = IntermediateRewards::load_from_state(state)?;
    let drained = rewards.drain_previous();
    rewards.save_to_state(state)?;
    Ok(drained)
}

pub fn move_current_to_previous_intermediate_rewards(state: &mut TopLevelState) -> StateResult<()> {
    let mut rewards = IntermediateRewards::load_from_state(state)?;
    rewards.move_current_to_previous();
    rewards.save_to_state(state)
}

fn change_params(
    state: &mut TopLevelState,
    metadata_seq: u64,
    params: CommonParams,
    signatures: &[Signature],
) -> StateResult<()> {
    // Update state first because the signature validation is more expensive.
    state.update_params(metadata_seq, params)?;

    let action = Action::ChangeParams {
        metadata_seq,
        params: params.into(),
        signatures: vec![],
    };
    let encoded_action = H256::blake(rlp::encode(&action));
    let stakes = get_stakes(state)?;
    let signed_stakes = signatures.iter().try_fold(0, |sum, signature| {
        let public = recover(signature, &encoded_action).unwrap_or_else(|err| {
            unreachable!("The transaction with an invalid signature cannot pass the verification: {}", err);
        });
        let address = public_to_address(&public);
        stakes.get(&address).map(|stake| sum + stake).ok_or_else(|| RuntimeError::SignatureOfInvalidAccount(address))
    })?;
    let total_stakes: u64 = stakes.values().sum();
    if total_stakes / 2 >= signed_stakes {
        return Err(RuntimeError::InsufficientStakes(Mismatch {
            expected: total_stakes,
            found: signed_stakes,
        })
        .into())
    }
    Ok(())
}

#[allow(dead_code)]
pub fn on_term_close(state: &mut TopLevelState, current_term: u64) -> StateResult<()> {
    // TODO: total_slash = slash_unresponsive(headers, pending_rewards)
    // TODO: pending_rewards.update(signature_reward(blocks, total_slash))

    let mut candidates = Candidates::load_from_state(state)?;
    let expired = candidates.drain_expired_candidates(current_term);
    for candidate in &expired {
        state.add_balance(&candidate.address, candidate.deposit)?;
    }
    candidates.save_to_state(state)?;

    // TODO: auto_withdraw(pending_rewards)

    let mut jailed = Jail::load_from_state(&state)?;
    let kicked = jailed.kick_prisoners(current_term);
    for prisoner in &kicked {
        state.add_balance(&prisoner.address, prisoner.deposit)?;
    }
    jailed.save_to_state(state)?;

    // Stakeholders list isn't changed while reverting.
    let reverted: Vec<_> = expired.iter().map(|c| c.address).chain(kicked.iter().map(|p| p.address)).collect();
    revert_delegations(state, &reverted)?;

    // TODO: validators, validator_order = elect()
    Ok(())
}

#[allow(dead_code)]
pub fn jail(state: &mut TopLevelState, address: &Address, custody_until: u64, kick_at: u64) -> StateResult<()> {
    let mut candidates = Candidates::load_from_state(state)?;
    let mut jail = Jail::load_from_state(state)?;

    let candidate = candidates.remove(address).expect("There should be a candidate to jail");
    jail.add(candidate, custody_until, kick_at);

    jail.save_to_state(state)?;
    candidates.save_to_state(state)?;
    Ok(())
}

fn revert_delegations(state: &mut TopLevelState, reverted_delegatees: &[Address]) -> StateResult<()> {
    let stakeholders = Stakeholders::load_from_state(state)?;
    for stakeholder in stakeholders.iter() {
        let mut balance = StakeAccount::load_from_state(state, stakeholder)?;
        let mut delegation = Delegation::load_from_state(state, stakeholder)?;

        for prisoner in reverted_delegatees.iter() {
            let quantity = delegation.get_quantity(prisoner);
            if quantity > 0 {
                delegation.subtract_quantity(*prisoner, quantity)?;
                balance.add_balance(quantity)?;
            }
        }
        delegation.save_to_state(state)?;
        balance.save_to_state(state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::action_data::get_account_key;
    use super::*;

    use consensus::stake::action_data::{get_delegation_key, Candidate, Prisoner};
    use cstate::tests::helpers;
    use cstate::TopStateView;
    use rlp::Encodable;

    #[test]
    fn genesis_stakes() {
        let address1 = Address::random();
        let address2 = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(address1, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        assert_eq!(account1.balance, 100);

        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account2.balance, 0);

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert_eq!(stakeholders.iter().len(), 1);
        assert!(stakeholders.contains(&address1));
        assert!(!stakeholders.contains(&address2));
    }

    #[test]
    fn balance_transfer_partial() {
        let address1 = Address::random();
        let address2 = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(address1, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        let result = transfer_ccs(&mut state, &address1, &address2, 10);
        assert_eq!(result, Ok(()));

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        assert_eq!(account1.balance, 90);

        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account2.balance, 10);

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert_eq!(stakeholders.iter().len(), 2);
        assert!(stakeholders.contains(&address1));
        assert!(stakeholders.contains(&address2));
    }

    #[test]
    fn balance_transfer_all() {
        let address1 = Address::random();
        let address2 = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(address1, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        let result = transfer_ccs(&mut state, &address1, &address2, 100);
        assert_eq!(result, Ok(()));

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        assert_eq!(account1.balance, 0);
        assert_eq!(state.action_data(&get_account_key(&address1)).unwrap(), None, "Should clear state");

        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account2.balance, 100);

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert_eq!(stakeholders.iter().len(), 1);
        assert!(!stakeholders.contains(&address1), "Not be a stakeholder anymore");
        assert!(stakeholders.contains(&address2));
    }

    #[test]
    fn delegate() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert_eq!(result, Ok(()));

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 60);

        let delegatee_account = StakeAccount::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegatee_account.balance, 100, "Shouldn't be touched");

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 40);

        let delegation_delegatee = Delegation::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegation_delegatee.iter().count(), 0, "Shouldn't be touched");

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert_eq!(stakeholders.iter().len(), 2);
        assert!(stakeholders.contains(&delegator));
        assert!(stakeholders.contains(&delegatee));
    }

    #[test]
    fn delegate_all() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 100,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert_eq!(result, Ok(()));

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 0);
        assert_eq!(state.action_data(&get_account_key(&delegator)).unwrap(), None, "Should clear state");

        let delegatee_account = StakeAccount::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegatee_account.balance, 100, "Shouldn't be touched");

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 100);

        let delegation_delegatee = Delegation::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegation_delegatee.iter().count(), 0, "Shouldn't be touched");

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert_eq!(stakeholders.iter().len(), 2);
        assert!(stakeholders.contains(&delegator), "Should still be a stakeholder after delegated all");
        assert!(stakeholders.contains(&delegatee));
    }

    #[test]
    fn delegate_only_to_candidate() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn delegate_too_much() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 200,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn can_transfer_within_non_delegated_tokens() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator).unwrap();

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());
    }

    #[test]
    fn cannot_transfer_over_non_delegated_tokens() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator).unwrap();

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 100,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn can_revoke_delegated_tokens() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 20,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50 + 20);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 50 - 20);
    }

    #[test]
    fn cannot_revoke_more_than_delegated_tokens() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 70,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 50);
    }

    #[test]
    fn revoke_all_should_clear_state() {
        let delegatee = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, 0, 0, 10).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100);
        assert_eq!(state.action_data(&get_delegation_key(&delegator)).unwrap(), None);
    }

    #[test]
    fn self_nominate_deposit_test() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let result = self_nominate(&mut state, &address, 0, 0, 5);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 1000);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                address,
                deposit: 0,
                nomination_ends_at: 5,
            }),
            "nomination_ends_at should be updated even if candidate deposits 0"
        );

        let result = self_nominate(&mut state, &address, 200, 0, 10);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                address,
                deposit: 200,
                nomination_ends_at: 10,
            })
        );

        let result = self_nominate(&mut state, &address, 0, 0, 15);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                address,
                deposit: 200,
                nomination_ends_at: 15,
            }),
            "nomination_ends_at should be updated even if candidate deposits 0"
        );
    }

    #[test]
    fn self_nominate_fail_with_insufficient_balance() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let result = self_nominate(&mut state, &address, 2000, 0, 5);
        assert!(result.is_err(), "Cannot self-nominate without a sufficient balance");
    }

    #[test]
    fn self_nominate_returns_deposits_after_expiration() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        self_nominate(&mut state, &address, 200, 0, 30).unwrap();

        let result = on_term_close(&mut state, 29);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800, "Should keep nomination before expiration");
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                address,
                deposit: 200,
                nomination_ends_at: 30,
            }),
            "Keep deposit before expiration",
        );

        let result = on_term_close(&mut state, 30);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 1000, "Return deposit after expiration");
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.get_candidate(&address), None, "Removed from candidates after expiration");
    }

    #[test]
    fn self_nominate_reverts_delegations_after_expiration() {
        let address = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        self_nominate(&mut state, &address, 0, 0, 30).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator).unwrap();

        let result = on_term_close(&mut state, 29);
        assert_eq!(result, Ok(()));

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100 - 40);
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 40, "Should keep delegation before expiration");

        let result = on_term_close(&mut state, 30);
        assert_eq!(result, Ok(()));

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100);
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 0, "Should revert before expiration");
    }

    #[test]
    fn jail_candidate() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        self_nominate(&mut state, &address, deposit, 0, 5).unwrap();

        let custody_until = 10;
        let kicked_at = 20;
        let result = jail(&mut state, &address, custody_until, kicked_at);
        assert!(result.is_ok());

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.get_candidate(&address), None, "The candidate is removed");

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(
            jail.get_prisoner(&address),
            Some(&Prisoner {
                address,
                deposit,
                custody_until,
                kicked_at,
            }),
            "The candidate become a prisoner"
        );

        assert_eq!(state.balance(&address).unwrap(), 1000 - deposit, "Deposited ccs is temporarily unavailable");
    }

    #[test]
    fn cannot_self_nominate_while_custody() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, deposit, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();

        for current_term in 0..=custody_until {
            let result = self_nominate(&mut state, &address, 0, current_term, current_term + nominate_expire);
            assert!(
                result.is_err(),
                "Shouldn't nominate while current_term({}) <= custody_until({})",
                current_term,
                custody_until
            );
            on_term_close(&mut state, current_term).unwrap();
        }
    }

    #[test]
    fn can_self_nominate_after_custody() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, deposit, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();
        for current_term in 0..=custody_until {
            on_term_close(&mut state, current_term).unwrap();
        }

        let current_term = custody_until + 1;
        let additional_deposit = 123;
        let result =
            self_nominate(&mut state, &address, additional_deposit, current_term, current_term + nominate_expire);
        assert!(result.is_ok());

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                deposit: deposit + additional_deposit,
                nomination_ends_at: current_term + nominate_expire,
                address,
            }),
            "The prisoner is become a candidate",
        );

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(jail.get_prisoner(&address), None, "The prisoner is removed");

        assert_eq!(state.balance(&address).unwrap(), 1000 - deposit - additional_deposit, "Deposit is accumulated");
    }

    #[test]
    fn jail_kicked_after() {
        let address = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, deposit, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();

        for current_term in 0..kicked_at {
            on_term_close(&mut state, current_term).unwrap();

            let candidates = Candidates::load_from_state(&state).unwrap();
            assert_eq!(candidates.get_candidate(&address), None);

            let jail = Jail::load_from_state(&state).unwrap();
            assert!(jail.get_prisoner(&address).is_some());
        }

        on_term_close(&mut state, kicked_at).unwrap();

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.get_candidate(&address), None, "A prisoner should not become a candidate");

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(jail.get_prisoner(&address), None, "A prisoner should be kicked");

        assert_eq!(state.balance(&address).unwrap(), 1000, "Balance should be restored after being kicked");
    }

    #[test]
    fn can_delegate_until_kicked() {
        let address = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, deposit, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();

        for current_term in 0..=kicked_at {
            let action = Action::DelegateCCS {
                address,
                quantity: 1,
            };
            let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
            assert!(result.is_ok());

            on_term_close(&mut state, current_term).unwrap();
        }

        let action = Action::DelegateCCS {
            address,
            quantity: 1,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn kick_reverts_delegations() {
        let address = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, deposit, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator).unwrap();

        for current_term in 0..=kicked_at {
            on_term_close(&mut state, current_term).unwrap();
        }

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 0, "Delegation should be reverted");

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100, "Delegation should be reverted");
    }

    #[test]
    fn self_nomination_before_kick_preserves_delegations() {
        let address = Address::random();
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let nominate_expire = 5;
        let custody_until = 10;
        let kicked_at = 20;
        self_nominate(&mut state, &address, 0, 0, nominate_expire).unwrap();
        jail(&mut state, &address, custody_until, kicked_at).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator).unwrap();
        for current_term in 0..custody_until {
            on_term_close(&mut state, current_term).unwrap();
        }

        let current_term = custody_until + 1;
        let result = self_nominate(&mut state, &address, 0, current_term, current_term + nominate_expire);
        assert!(result.is_ok());

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 40, "Delegation should be preserved");

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100 - 40, "Delegation should be preserved");
    }
}
