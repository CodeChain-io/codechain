// Copyright 2018 Kodebox, Inc.
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

use ckey::Address;
use cstate::{ActionHandler, ActionHandlerResult, TopLevelState};
use ctypes::invoice::Invoice;
use rlp::UntrustedRlp;

use self::action_data::{StakeAccount, Stakeholders};
use self::actions::Action;
pub use self::distribute::fee_distribute;

const CUSTOM_ACTION_HANDLER_ID: u64 = 2;

pub type StakeResult<T> = ActionHandlerResult<T>;

pub struct Stake {
    genesis_stakes: HashMap<Address, u64>,
}

impl Stake {
    pub fn new(genesis_stakes: HashMap<Address, u64>) -> Stake {
        Stake {
            genesis_stakes,
        }
    }
}

impl ActionHandler for Stake {
    fn handler_id(&self) -> u64 {
        CUSTOM_ACTION_HANDLER_ID
    }

    fn init(&self, state: &mut TopLevelState) -> ActionHandlerResult<()> {
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

    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, sender: &Address) -> ActionHandlerResult<Invoice> {
        let action = UntrustedRlp::new(bytes).as_val()?;
        match action {
            Action::TransferCCS {
                address,
                quantity,
            } => transfer_ccs(state, sender, &address, quantity),
        }
    }
}

fn transfer_ccs(
    state: &mut TopLevelState,
    sender: &Address,
    receiver: &Address,
    quantity: u64,
) -> StakeResult<Invoice> {
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

pub fn get_stakes(state: &TopLevelState) -> StakeResult<HashMap<Address, u64>> {
    let stakeholders = Stakeholders::load_from_state(state)?;
    let mut result = HashMap::new();
    for stakeholder in stakeholders.iter() {
        let account = StakeAccount::load_from_state(state, stakeholder)?;
        result.insert(*stakeholder, account.balance);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cstate::tests::helpers;

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
            Stake::new(genesis_stakes)
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
            Stake::new(genesis_stakes)
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
}
