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

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use ckey::Address;
use cstate::{ActionHandler, StateResult, TopLevelState};
use ctypes::errors::RuntimeError;
use ctypes::invoice::Invoice;
use rlp::{Decodable, UntrustedRlp};

use self::action_data::{StakeAccount, Stakeholders};
use self::actions::Action;
pub use self::distribute::fee_distribute;
use consensus::tendermint::stake::action_data::Delegation;
use consensus::ValidatorSet;

const CUSTOM_ACTION_HANDLER_ID: u64 = 2;

pub struct Stake {
    genesis_stakes: HashMap<Address, u64>,
    validators: Arc<ValidatorSet>,
}

impl Stake {
    pub fn new(genesis_stakes: HashMap<Address, u64>, validators: Arc<ValidatorSet>) -> Stake {
        Stake {
            genesis_stakes,
            validators,
        }
    }
}

impl ActionHandler for Stake {
    fn handler_id(&self) -> u64 {
        CUSTOM_ACTION_HANDLER_ID
    }

    fn init(&self, state: &mut TopLevelState) -> StateResult<()> {
        let mut stakeholders = Stakeholders::load_from_state(state)?;
        for (address, amount) in self.genesis_stakes.iter() {
            if *amount > 0 {
                let account = StakeAccount {
                    address,
                    balance: *amount,
                };
                stakeholders.update(&account);
                account.save_to_state(state)?;
            }
            stakeholders.save_to_state(state)?;
        }
        Ok(())
    }

    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, sender: &Address) -> StateResult<Invoice> {
        let action = Action::decode(&UntrustedRlp::new(bytes))
            .map_err(|err| RuntimeError::FailedToHandleCustomAction(err.to_string()))?;
        match action {
            Action::TransferCCS {
                address,
                quantity,
            } => transfer_ccs(state, sender, &address, quantity),
            Action::DelegateCCS {
                address,
                quantity,
            } => delegate_ccs(state, sender, &address, quantity, self.validators.deref()),
        }
    }
}

fn transfer_ccs(
    state: &mut TopLevelState,
    sender: &Address,
    receiver: &Address,
    quantity: u64,
) -> StateResult<Invoice> {
    let mut stakeholders = Stakeholders::load_from_state(state)?;
    let mut sender_account = StakeAccount::load_from_state(state, sender)?;
    let mut receiver_account = StakeAccount::load_from_state(state, receiver)?;

    sender_account.subtract_balance(quantity)?;
    receiver_account.add_balance(quantity)?;

    stakeholders.update(&sender_account);
    stakeholders.update(&receiver_account);

    stakeholders.save_to_state(state)?;
    sender_account.save_to_state(state)?;
    receiver_account.save_to_state(state)?;

    Ok(Invoice::Success)
}

fn delegate_ccs(
    state: &mut TopLevelState,
    sender: &Address,
    delegatee: &Address,
    quantity: u64,
    validators: &ValidatorSet,
) -> StateResult<Invoice> {
    // TODO: remove parent hash from validator set.
    if !validators.contains_address(&Default::default(), delegatee) {
        return Err(RuntimeError::FailedToHandleCustomAction("Cannot delegate to non-validator".into()).into())
    }
    let mut delegator = StakeAccount::load_from_state(state, sender)?;
    let mut delegation = Delegation::load_from_state(state, &sender)?;

    delegator.subtract_balance(quantity)?;
    delegation.add_quantity(*delegatee, quantity)?;

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;
    Ok(Invoice::Success)
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

#[cfg(test)]
mod tests {
    use super::*;
    use ckey::{public_to_address, Public};
    use consensus::validator_set::new_validator_set;
    use cstate::tests::helpers;
    use rlp::Encodable;

    #[test]
    fn genesis_stakes() {
        let address1 = Address::random();
        let address2 = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(address1, 100);
            Stake::new(genesis_stakes, new_validator_set(Vec::new()))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account1.balance, 100);
        assert_eq!(account2.balance, 0);
        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
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
            Stake::new(genesis_stakes, new_validator_set(Vec::new()))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let result = transfer_ccs(&mut state, &address1, &address2, 10);
        assert_eq!(Ok(Invoice::Success), result);

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account1.balance, 90);
        assert_eq!(account2.balance, 10);
        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
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
            Stake::new(genesis_stakes, new_validator_set(Vec::new()))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        transfer_ccs(&mut state, &address1, &address2, 100).unwrap();

        let account1 = StakeAccount::load_from_state(&state, &address1).unwrap();
        let account2 = StakeAccount::load_from_state(&state, &address2).unwrap();
        assert_eq!(account1.balance, 0);
        assert_eq!(account2.balance, 100);
        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert!(!stakeholders.contains(&address1));
        assert!(stakeholders.contains(&address2));
    }

    #[test]
    fn delegate() {
        let delegatee_public = Public::random();
        let delegatee = public_to_address(&delegatee_public);
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes, new_validator_set(vec![delegatee_public]))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert_eq!(result, Ok(Invoice::Success));

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 60);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 40);

        // Should not be touched
        let delegatee_account = StakeAccount::load_from_state(&state, &delegatee).unwrap();
        let delegation_untouched = Delegation::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegatee_account.balance, 100);
        assert_eq!(delegation_untouched.iter().count(), 0);
    }

    #[test]
    fn delegate_only_to_validator() {
        let delegatee_public = Public::random();
        let delegatee = public_to_address(&delegatee_public);
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes, new_validator_set(Vec::new()))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn delegate_too_much() {
        let delegatee_public = Public::random();
        let delegatee = public_to_address(&delegatee_public);
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes, new_validator_set(vec![delegatee_public]))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 200,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }

    #[test]
    fn can_transfer_within_non_delegated_tokens() {
        let delegatee_public = Public::random();
        let delegatee = public_to_address(&delegatee_public);
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes, new_validator_set(vec![delegatee_public]))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());
    }

    #[test]
    fn cannot_transfer_over_non_delegated_tokens() {
        let delegatee_public = Public::random();
        let delegatee = public_to_address(&delegatee_public);
        let delegator = Address::random();

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes, new_validator_set(vec![delegatee_public]))
        };
        assert_eq!(Ok(()), stake.init(&mut state));

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_ok());

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 100,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator);
        assert!(result.is_err());
    }
}
