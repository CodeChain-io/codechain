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

#[cfg(test)]
use std::collections::btree_map;
use std::collections::{btree_set, BTreeMap, BTreeSet};

use ckey::Address;
use cstate::{ActionData, ActionDataKeyBuilder, StateResult, TopLevelState, TopState, TopStateView};
use ctypes::errors::RuntimeError;
use primitives::H256;
use rlp::{Decodable, Encodable, Rlp, RlpStream};

use super::CUSTOM_ACTION_HANDLER_ID;

fn get_account_key(address: &Address) -> H256 {
    ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 2).append(&"Account").append(address).into_key()
}

lazy_static! {
    pub static ref STAKEHOLDER_ADDRESSES_KEY: H256 =
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"StakeholderAddresses").into_key();
}

fn get_delegation_key(address: &Address) -> H256 {
    ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 2).append(&"Delegation").append(address).into_key()
}

pub type StakeQuantity = u64;

pub struct StakeAccount<'a> {
    pub address: &'a Address,
    pub balance: StakeQuantity,
}

impl<'a> StakeAccount<'a> {
    pub fn load_from_state(state: &TopLevelState, address: &'a Address) -> StateResult<StakeAccount<'a>> {
        let account_key = get_account_key(address);
        let action_data = state.action_data(&account_key)?;

        let balance = match action_data {
            Some(data) => Rlp::new(&data).as_val(),
            None => StakeQuantity::default(),
        };

        Ok(StakeAccount {
            address,
            balance,
        })
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let account_key = get_account_key(self.address);
        let rlp = rlp::encode(&self.balance);
        state.update_action_data(&account_key, rlp.into_vec())?;
        Ok(())
    }

    pub fn subtract_balance(&mut self, amount: u64) -> Result<(), RuntimeError> {
        if self.balance < amount {
            return Err(RuntimeError::InsufficientBalance {
                address: *self.address,
                cost: amount,
                balance: self.balance,
            })
        }
        self.balance -= amount;
        Ok(())
    }

    pub fn add_balance(&mut self, amount: u64) -> Result<(), RuntimeError> {
        self.balance += amount;
        Ok(())
    }
}

pub struct Stakeholders(BTreeSet<Address>);

impl Stakeholders {
    pub fn load_from_state(state: &TopLevelState) -> StateResult<Stakeholders> {
        let action_data = state.action_data(&*STAKEHOLDER_ADDRESSES_KEY)?;
        let addresses = decode_set(action_data.as_ref());
        Ok(Stakeholders(addresses))
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        state.update_action_data(&*STAKEHOLDER_ADDRESSES_KEY, encode_set(&self.0))?;
        Ok(())
    }

    #[cfg(test)]
    pub fn contains(&self, address: &Address) -> bool {
        self.0.contains(address)
    }

    pub fn update(&mut self, account: &StakeAccount) {
        if account.balance > 0 {
            self.0.insert(*account.address);
        } else {
            self.0.remove(account.address);
        }
    }

    pub fn iter(&self) -> btree_set::Iter<Address> {
        self.0.iter()
    }
}

pub struct Delegation<'a> {
    pub delegator: &'a Address,
    delegatees: BTreeMap<Address, StakeQuantity>,
}

impl<'a> Delegation<'a> {
    pub fn load_from_state(state: &TopLevelState, delegator: &'a Address) -> StateResult<Delegation<'a>> {
        let key = get_delegation_key(delegator);
        let action_data = state.action_data(&key)?;
        let delegatees = decode_map(action_data.as_ref());

        Ok(Delegation {
            delegator,
            delegatees,
        })
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let key = get_delegation_key(self.delegator);
        let encoded = encode_map(&self.delegatees);
        state.update_action_data(&key, encoded)?;
        Ok(())
    }

    pub fn add_quantity(&mut self, delegatee: Address, quantity: StakeQuantity) -> StateResult<()> {
        *self.delegatees.entry(delegatee).or_insert(0) += quantity;
        Ok(())
    }

    #[cfg(test)]
    pub fn get_quantity(&self, delegatee: &Address) -> StakeQuantity {
        self.delegatees.get(delegatee).cloned().unwrap_or(0)
    }

    #[cfg(test)]
    pub fn iter(&self) -> btree_map::Iter<Address, StakeQuantity> {
        self.delegatees.iter()
    }

    pub fn sum(&self) -> u64 {
        self.delegatees.values().sum()
    }
}

fn decode_set<V>(data: Option<&ActionData>) -> BTreeSet<V>
where
    V: Ord + Decodable, {
    let mut result = BTreeSet::new();
    if let Some(rlp) = data.map(|x| Rlp::new(x)) {
        for record in rlp.iter() {
            let value: V = record.as_val();
            result.insert(value);
        }
    }
    result
}

fn encode_set<V>(set: &BTreeSet<V>) -> Vec<u8>
where
    V: Ord + Encodable, {
    let mut rlp = RlpStream::new();
    rlp.begin_list(set.len());
    for value in set.iter() {
        rlp.append(value);
    }
    rlp.drain().into_vec()
}

fn decode_map<K, V>(data: Option<&ActionData>) -> BTreeMap<K, V>
where
    K: Ord + Decodable,
    V: Decodable, {
    let mut result = BTreeMap::new();
    if let Some(rlp) = data.map(|x| Rlp::new(x)) {
        for record in rlp.iter() {
            let key: K = record.val_at(0);
            let value: V = record.val_at(1);
            assert_eq!(2, record.item_count());
            result.insert(key, value);
        }
    }
    result
}

fn encode_map<K, V>(map: &BTreeMap<K, V>) -> Vec<u8>
where
    K: Ord + Encodable,
    V: Encodable, {
    let mut rlp = RlpStream::new();
    rlp.begin_list(map.len());
    for (key, value) in map.iter() {
        let mut record = rlp.begin_list(2);
        record.append(key);
        record.append(value);
    }
    rlp.drain().into_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cstate::tests::helpers;
    use rand::{Rng, SeedableRng};
    use rand_xorshift::XorShiftRng;
    use std::collections::HashMap;

    fn rng() -> XorShiftRng {
        let seed: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7];
        XorShiftRng::from_seed(seed)
    }

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
                Err(RuntimeError::InsufficientBalance {
                    address,
                    cost: 110,
                    balance: 100,
                })
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

    #[test]
    fn stakeholders_track() {
        let mut rng = rng();
        let mut state = helpers::get_temp_state();
        let addresses: Vec<_> = (1..100).map(|_| Address::random()).collect();
        let accounts: Vec<_> = addresses
            .iter()
            .map(|address| StakeAccount {
                address,
                balance: rng.gen_range(1, 100),
            })
            .collect();

        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            stakeholders.update(account);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        assert!(addresses.iter().all(|address| stakeholders.contains(address)));
    }

    #[test]
    fn stakeholders_untrack() {
        let mut rng = rng();
        let mut state = helpers::get_temp_state();
        let addresses: Vec<_> = (1..100).map(|_| Address::random()).collect();
        let mut accounts: Vec<_> = addresses
            .iter()
            .map(|address| StakeAccount {
                address,
                balance: rng.gen_range(1, 100),
            })
            .collect();

        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            stakeholders.update(account);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        for account in &mut accounts {
            if rand::random() {
                account.balance = 0;
            }
        }
        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            stakeholders.update(account);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            let tracked = stakeholders.contains(account.address);
            let has_balance = account.balance > 0;
            assert!(tracked && has_balance || !tracked && !has_balance);
        }
    }

    #[test]
    fn initial_delegation_is_empty() {
        let state = helpers::get_temp_state();

        let delegatee = Address::random();
        let delegation = Delegation::load_from_state(&state, &delegatee).unwrap();
        assert_eq!(delegation.delegator, &delegatee);
        assert_eq!(delegation.iter().count(), 0);
    }

    #[test]
    fn delegation_add() {
        let mut rng = rng();
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatees: Vec<_> = (0..10).map(|_| Address::random()).collect();
        let delegation_amount: HashMap<&Address, StakeQuantity> =
            delegatees.iter().map(|address| (address, rng.gen_range(0, 100))).collect();

        // Do delegate
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        for delegatee in delegatees.iter() {
            delegation.add_quantity(*delegatee, delegation_amount[delegatee]).unwrap()
        }
        delegation.save_to_state(&mut state).unwrap();

        // assert
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.iter().count(), delegatees.len());
        for delegatee in delegatees.iter() {
            assert_eq!(delegation.get_quantity(delegatee), delegation_amount[delegatee]);
        }
    }
}
