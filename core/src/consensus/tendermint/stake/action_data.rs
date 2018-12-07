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

use ccrypto::blake256;
use ckey::Address;
use cstate::{TopLevelState, TopState, TopStateView};
use ctypes::parcel::Error as ParcelError;
use primitives::H256;
use rlp::{RlpStream, UntrustedRlp};

use super::StakeResult;

const ACTION_DATA_KEY_PREFIX: &str = "TendermintStakeAction";

fn get_account_key(address: &Address) -> H256 {
    let mut rlp = RlpStream::new();
    rlp.begin_list(2).append(&ACTION_DATA_KEY_PREFIX).append(address);
    blake256(rlp.drain())
}

pub type StakeBalance = u64;

pub struct StakeAccount<'a> {
    pub address: &'a Address,
    pub balance: StakeBalance,
}

impl<'a> StakeAccount<'a> {
    pub fn load_from_state(state: &TopLevelState, address: &'a Address) -> StakeResult<StakeAccount<'a>> {
        let account_key = get_account_key(address);
        let action_data = state.action_data(&account_key)?;

        let balance = match action_data {
            Some(data) => UntrustedRlp::new(&data).as_val()?,
            None => StakeBalance::default(),
        };

        Ok(StakeAccount {
            address,
            balance,
        })
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StakeResult<()> {
        let account_key = get_account_key(self.address);
        let rlp = rlp::encode(&self.balance);
        state.update_action_data(&account_key, rlp.into_vec())?;
        Ok(())
    }

    pub fn subtract_balance(&mut self, amount: u64) -> Result<(), ParcelError> {
        if self.balance < amount {
            return Err(ParcelError::InsufficientBalance {
                address: *self.address,
                cost: amount,
                balance: self.balance,
            })
        }
        self.balance -= amount;
        Ok(())
    }

    pub fn add_balance(&mut self, amount: u64) -> Result<(), ParcelError> {
        self.balance += amount;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cstate::tests::helpers;

    #[test]
    fn default_balance_is_zero() {
        let state = helpers::get_temp_state();
        let address = Address::random();
        let account = StakeAccount::load_from_state(&state, &address).unwrap();
        assert_eq!(account.address, &address);
        assert_eq!(account.balance, 0);
    }

    #[test]
    fn balance_add() {
        let mut state = helpers::get_temp_state();
        let address = Address::random();
        {
            let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
            account.add_balance(100).unwrap();
            account.save_to_state(&mut state).unwrap();
        }
        let account = StakeAccount::load_from_state(&state, &address).unwrap();
        assert_eq!(account.balance, 100);
    }

    #[test]
    fn balance_subtract_error_on_low() {
        let mut state = helpers::get_temp_state();
        let address = Address::random();
        {
            let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
            account.add_balance(100).unwrap();
            account.save_to_state(&mut state).unwrap();
        }
        {
            let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
            let result = account.subtract_balance(110);
            assert!(result.is_err());
            assert_eq!(
                result,
                Err(ParcelError::InsufficientBalance {
                    address,
                    cost: 110,
                    balance: 100,
                }
                .into())
            );
        }
        let account = StakeAccount::load_from_state(&state, &address).unwrap();
        assert_eq!(account.balance, 100);
    }

    #[test]
    fn balance_subtract() {
        let mut state = helpers::get_temp_state();
        let address = Address::random();

        let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
        account.add_balance(100).unwrap();
        account.save_to_state(&mut state).unwrap();

        let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
        let result = account.subtract_balance(90);
        assert!(result.is_ok());
        account.save_to_state(&mut state).unwrap();

        let account = StakeAccount::load_from_state(&state, &address).unwrap();
        assert_eq!(account.balance, 10);
    }
}
