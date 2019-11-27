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

use std::cmp::Ordering;
use std::collections::btree_map::{BTreeMap, Entry};
use std::collections::btree_set::{self, BTreeSet};
use std::collections::{btree_map, HashMap, HashSet};
use std::ops::Deref;
use std::vec;

use ckey::{public_to_address, Address, Public};
use cstate::{ActionData, ActionDataKeyBuilder, StateResult, TopLevelState, TopState, TopStateView};
use ctypes::errors::RuntimeError;
use primitives::{Bytes, H256};
use rlp::{decode_list, encode_list, Decodable, Encodable, Rlp, RlpStream};

use super::CUSTOM_ACTION_HANDLER_ID;

pub fn get_account_key(address: &Address) -> H256 {
    ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 2).append(&"Account").append(address).into_key()
}

lazy_static! {
    pub static ref STAKEHOLDER_ADDRESSES_KEY: H256 =
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"StakeholderAddresses").into_key();
    pub static ref CANDIDATES_KEY: H256 =
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"Candidates").into_key();
    pub static ref JAIL_KEY: H256 = ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"Jail").into_key();
    pub static ref BANNED_KEY: H256 =
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"Banned").into_key();
    pub static ref VALIDATORS_KEY: H256 =
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"Validators").into_key();
}

pub fn get_delegation_key(address: &Address) -> H256 {
    ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 2).append(&"Delegation").append(address).into_key()
}

pub fn get_intermediate_rewards_key() -> H256 {
    ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"IntermediateRewards").into_key()
}

pub type StakeQuantity = u64;
pub type Deposit = u64;

pub struct StakeAccount<'a> {
    pub address: &'a Address,
    pub balance: StakeQuantity,
}

impl<'a> StakeAccount<'a> {
    pub fn load_from_state(state: &TopLevelState, address: &'a Address) -> StateResult<StakeAccount<'a>> {
        let account_key = get_account_key(address);
        let action_data = state.action_data(&account_key)?;

        let balance = match action_data {
            Some(data) => Rlp::new(&data).as_val().unwrap(),
            None => StakeQuantity::default(),
        };

        Ok(StakeAccount {
            address,
            balance,
        })
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let account_key = get_account_key(self.address);
        if self.balance != 0 {
            let rlp = rlp::encode(&self.balance);
            state.update_action_data(&account_key, rlp)?;
        } else {
            state.remove_action_data(&account_key);
        }
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
        let key = *STAKEHOLDER_ADDRESSES_KEY;
        if !self.0.is_empty() {
            state.update_action_data(&key, encode_set(&self.0))?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    fn delegatees(state: &TopLevelState) -> StateResult<HashMap<Address, u64>> {
        let stakeholders = Stakeholders::load_from_state(state)?;
        let mut result = HashMap::new();
        for stakeholder in stakeholders.iter() {
            let delegation = Delegation::load_from_state(state, stakeholder)?;
            for (delegatee, quantity) in delegation.iter() {
                *result.entry(*delegatee).or_default() += *quantity;
            }
        }
        Ok(result)
    }


    #[cfg(test)]
    pub fn contains(&self, address: &Address) -> bool {
        self.0.contains(address)
    }

    pub fn update_by_increased_balance(&mut self, account: &StakeAccount) {
        if account.balance > 0 {
            self.0.insert(*account.address);
        }
    }

    pub fn update_by_decreased_balance(&mut self, account: &StakeAccount, delegation: &Delegation) {
        assert!(account.address == delegation.delegator);
        if account.balance == 0 && delegation.sum() == 0 {
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
        if !self.delegatees.is_empty() {
            let encoded = encode_map(&self.delegatees);
            state.update_action_data(&key, encoded)?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    pub fn add_quantity(&mut self, delegatee: Address, quantity: StakeQuantity) -> StateResult<()> {
        if quantity == 0 {
            return Ok(())
        }
        *self.delegatees.entry(delegatee).or_insert(0) += quantity;
        Ok(())
    }

    pub fn subtract_quantity(&mut self, delegatee: Address, quantity: StakeQuantity) -> StateResult<()> {
        if quantity == 0 {
            return Ok(())
        }

        if let Entry::Occupied(mut entry) = self.delegatees.entry(delegatee) {
            match entry.get().cmp(&quantity) {
                Ordering::Greater => {
                    *entry.get_mut() -= quantity;
                    return Ok(())
                }
                Ordering::Equal => {
                    entry.remove();
                    return Ok(())
                }
                Ordering::Less => {}
            }
        }

        Err(RuntimeError::FailedToHandleCustomAction("Cannot subtract more than that is delegated to".to_string())
            .into())
    }

    pub fn get_quantity(&self, delegatee: &Address) -> StakeQuantity {
        self.delegatees.get(delegatee).cloned().unwrap_or(0)
    }

    pub fn iter(&self) -> btree_map::Iter<Address, StakeQuantity> {
        self.delegatees.iter()
    }

    pub fn sum(&self) -> u64 {
        self.delegatees.values().sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, RlpDecodable, RlpEncodable)]
pub struct Validator {
    weight: StakeQuantity,
    delegation: StakeQuantity,
    deposit: Deposit,
    pubkey: Public,
}

impl Validator {
    pub fn new_for_test(delegation: StakeQuantity, deposit: Deposit, pubkey: Public) -> Self {
        Self {
            weight: delegation,
            delegation,
            deposit,
            pubkey,
        }
    }

    fn new(delegation: StakeQuantity, deposit: Deposit, pubkey: Public) -> Self {
        Self {
            weight: delegation,
            delegation,
            deposit,
            pubkey,
        }
    }

    fn reset(&mut self) {
        self.weight = self.delegation;
    }

    pub fn pubkey(&self) -> &Public {
        &self.pubkey
    }

    pub fn delegation(&self) -> StakeQuantity {
        self.delegation
    }
}

#[derive(Debug)]
pub struct Validators(Vec<Validator>);
impl Validators {
    pub fn from_vector_to_test(vec: Vec<Validator>) -> Self {
        Validators(vec)
    }

    pub fn load_from_state(state: &TopLevelState) -> StateResult<Self> {
        let key = &*VALIDATORS_KEY;
        let validators = state.action_data(&key)?.map(|data| decode_list(&data)).unwrap_or_default();

        Ok(Validators(validators))
    }

    pub fn elect(state: &TopLevelState) -> StateResult<Self> {
        let (delegation_threshold, max_num_of_validators, min_num_of_validators, min_deposit) = {
            let metadata = state.metadata()?.expect("Metadata must exist");
            let common_params = metadata.params().expect("CommonParams must exist in the metadata when elect");
            (
                common_params.delegation_threshold(),
                common_params.max_num_of_validators(),
                common_params.min_num_of_validators(),
                common_params.min_deposit(),
            )
        };
        assert!(max_num_of_validators >= min_num_of_validators);

        let delegatees = Stakeholders::delegatees(&state)?;
        // Step 1 & 2.
        let mut validators = Candidates::prepare_validators(&state, min_deposit, &delegatees)?;
        // validators are now sorted in descending order of (delegation, deposit, priority)
        validators.reverse();

        let banned = Banned::load_from_state(&state)?;
        for validator in &validators {
            let address = public_to_address(&validator.pubkey);
            assert!(!banned.is_banned(&address), "{} is banned address", address);
        }

        // Step 3
        validators.truncate(max_num_of_validators);

        if validators.len() < min_num_of_validators {
            cerror!(
                ENGINE,
                "There must be something wrong. {}, {} < {}",
                "validators.len() < min_num_of_validators",
                validators.len(),
                min_num_of_validators
            );
        }
        // Step 4 & 5
        let (minimum, rest) = validators.split_at(min_num_of_validators.min(validators.len()));
        let over_threshold = rest.iter().filter(|c| c.delegation >= delegation_threshold);

        let mut result: Vec<_> = minimum.iter().chain(over_threshold).cloned().collect();
        result.reverse(); // Ascending order of (delegation, deposit, priority)
        Ok(Self(result))
    }


    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let key = &*VALIDATORS_KEY;
        if !self.is_empty() {
            state.update_action_data(&key, encode_list(&self.0).to_vec())?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    pub fn update_weight(&mut self, block_author: &Address) {
        let min_delegation = self.min_delegation();
        for Validator {
            weight,
            pubkey,
            ..
        } in self.0.iter_mut().rev()
        {
            if public_to_address(pubkey) == *block_author {
                // block author
                *weight = weight.saturating_sub(min_delegation);
                break
            }
            // neglecting validators
            *weight = weight.saturating_sub(min_delegation * 2);
        }
        if self.0.iter().all(|validator| validator.weight == 0) {
            self.0.iter_mut().for_each(Validator::reset);
        }
        self.0.sort_unstable();
    }

    pub fn remove(&mut self, target: &Address) {
        self.0.retain(
            |Validator {
                 pubkey,
                 ..
             }| public_to_address(pubkey) != *target,
        );
    }

    pub fn delegation(&self, pubkey: &Public) -> Option<StakeQuantity> {
        self.0.iter().find(|validator| validator.pubkey == *pubkey).map(|&validator| validator.delegation)
    }

    fn min_delegation(&self) -> StakeQuantity {
        self.0.iter().map(|&validator| validator.delegation).min().expect("There must be at least one validators")
    }
}

impl Deref for Validators {
    type Target = Vec<Validator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Validators> for Vec<Validator> {
    fn from(val: Validators) -> Self {
        val.0
    }
}

impl IntoIterator for Validators {
    type Item = Validator;
    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub mod v0 {
    use std::mem;

    use super::*;

    #[derive(Default, Debug, PartialEq)]
    pub struct IntermediateRewards {
        pub(super) previous: BTreeMap<Address, u64>,
        pub(super) current: BTreeMap<Address, u64>,
    }

    impl IntermediateRewards {
        pub fn load_from_state(state: &TopLevelState) -> StateResult<Self> {
            let key = get_intermediate_rewards_key();
            let action_data = state.action_data(&key)?;
            let (previous, current) = decode_map_tuple(action_data.as_ref());

            Ok(Self {
                previous,
                current,
            })
        }

        pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
            let key = get_intermediate_rewards_key();
            if self.previous.is_empty() && self.current.is_empty() {
                state.remove_action_data(&key);
            } else {
                let encoded = encode_map_tuple(&self.previous, &self.current);
                state.update_action_data(&key, encoded)?;
            }
            Ok(())
        }

        pub fn add_quantity(&mut self, address: Address, quantity: StakeQuantity) {
            if quantity == 0 {
                return
            }
            *self.current.entry(address).or_insert(0) += quantity;
        }

        pub fn drain_previous(&mut self) -> BTreeMap<Address, u64> {
            let mut new = BTreeMap::new();
            mem::swap(&mut new, &mut self.previous);
            new
        }

        pub fn move_current_to_previous(&mut self) {
            assert!(self.previous.is_empty());
            mem::swap(&mut self.previous, &mut self.current);
        }
    }
}

pub mod v1 {
    use std::mem;

    use super::*;

    #[derive(Default, Debug, PartialEq)]
    pub struct IntermediateRewards {
        pub(super) current: BTreeMap<Address, u64>,
        pub(super) calculated: BTreeMap<Address, u64>,
    }

    impl IntermediateRewards {
        pub fn load_from_state(state: &TopLevelState) -> StateResult<Self> {
            let key = get_intermediate_rewards_key();
            let action_data = state.action_data(&key)?;
            let (current, calculated) = decode_map_tuple(action_data.as_ref());

            Ok(Self {
                current,
                calculated,
            })
        }

        pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
            let key = get_intermediate_rewards_key();
            if self.current.is_empty() && self.calculated.is_empty() {
                state.remove_action_data(&key);
            } else {
                let encoded = encode_map_tuple(&self.current, &self.calculated);
                state.update_action_data(&key, encoded)?;
            }
            Ok(())
        }

        pub fn add_quantity(&mut self, address: Address, quantity: StakeQuantity) {
            if quantity == 0 {
                return
            }
            *self.current.entry(address).or_insert(0) += quantity;
        }

        pub fn update_calculated(&mut self, rewards: BTreeMap<Address, u64>) {
            self.calculated = rewards;
        }

        pub fn drain_current(&mut self) -> BTreeMap<Address, u64> {
            let mut new = BTreeMap::new();
            mem::swap(&mut new, &mut self.current);
            new
        }

        pub fn drain_calculated(&mut self) -> BTreeMap<Address, u64> {
            let mut new = BTreeMap::new();
            mem::swap(&mut new, &mut self.calculated);
            new
        }
    }
}

pub struct Candidates(Vec<Candidate>);
#[derive(Clone, Debug, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct Candidate {
    pub pubkey: Public,
    pub deposit: Deposit,
    pub nomination_ends_at: u64,
    pub metadata: Bytes,
}

impl Candidates {
    pub fn load_from_state(state: &TopLevelState) -> StateResult<Candidates> {
        let key = *CANDIDATES_KEY;
        let candidates = state.action_data(&key)?.map(|data| decode_list::<Candidate>(&data)).unwrap_or_default();
        Ok(Candidates(candidates))
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let key = *CANDIDATES_KEY;
        if !self.0.is_empty() {
            let encoded = encode_iter(self.0.iter());
            state.update_action_data(&key, encoded)?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    // Sorted list of validators in ascending order of (delegation, deposit, priority).
    fn prepare_validators(
        state: &TopLevelState,
        min_deposit: Deposit,
        delegations: &HashMap<Address, StakeQuantity>,
    ) -> StateResult<Vec<Validator>> {
        let Candidates(candidates) = Self::load_from_state(state)?;
        let mut result = Vec::new();
        for candidate in candidates.into_iter().filter(|c| c.deposit >= min_deposit) {
            let address = public_to_address(&candidate.pubkey);
            if let Some(delegation) = delegations.get(&address).cloned() {
                result.push(Validator::new(delegation, candidate.deposit, candidate.pubkey));
            }
        }
        // Candidates are sorted in low priority: low index, high priority: high index
        // so stable sorting with the key (delegation, deposit) preserves its priority order.
        // ascending order of (delegation, deposit, priority)
        result.sort_by_key(|v| (v.delegation, v.deposit));
        Ok(result)
    }

    pub fn get_candidate(&self, account: &Address) -> Option<&Candidate> {
        self.0.iter().find(|c| public_to_address(&c.pubkey) == *account)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(test)]
    pub fn get_index(&self, account: &Address) -> Option<usize> {
        self.0.iter().position(|c| public_to_address(&c.pubkey) == *account)
    }

    pub fn add_deposit(&mut self, pubkey: &Public, quantity: Deposit, nomination_ends_at: u64, metadata: Bytes) {
        if let Some(index) = self.0.iter().position(|c| c.pubkey == *pubkey) {
            let candidate = &mut self.0[index];
            candidate.deposit += quantity;
            if candidate.nomination_ends_at < nomination_ends_at {
                candidate.nomination_ends_at = nomination_ends_at;
            }
            candidate.metadata = metadata;
        } else {
            self.0.push(Candidate {
                pubkey: *pubkey,
                deposit: quantity,
                nomination_ends_at,
                metadata,
            });
        };
        self.reprioritize(&[public_to_address(pubkey)]);
    }

    pub fn renew_candidates(
        &mut self,
        validators: &Validators,
        nomination_ends_at: u64,
        inactive_validators: &[Address],
        banned: &Banned,
    ) {
        let to_renew: HashSet<_> = (validators.iter())
            .map(|validator| validator.pubkey)
            .filter(|pubkey| !inactive_validators.contains(&public_to_address(pubkey)))
            .collect();

        for candidate in self.0.iter_mut().filter(|c| to_renew.contains(&c.pubkey)) {
            let address = public_to_address(&candidate.pubkey);
            assert!(!banned.is_banned(&address), "{} is banned address", address);
            candidate.nomination_ends_at = nomination_ends_at;
        }

        let to_reprioritize: Vec<_> =
            self.0.iter().filter(|c| to_renew.contains(&c.pubkey)).map(|c| public_to_address(&c.pubkey)).collect();

        self.reprioritize(&to_reprioritize);
    }

    pub fn drain_expired_candidates(&mut self, term_index: u64) -> Vec<Candidate> {
        let (expired, retained): (Vec<_>, Vec<_>) = self.0.drain(..).partition(|c| c.nomination_ends_at <= term_index);
        self.0 = retained;
        expired
    }

    pub fn remove(&mut self, address: &Address) -> Option<Candidate> {
        if let Some(index) = self.0.iter().position(|c| public_to_address(&c.pubkey) == *address) {
            Some(self.0.remove(index))
        } else {
            None
        }
    }

    fn reprioritize(&mut self, targets: &[Address]) {
        let mut renewed = Vec::new();
        for target in targets {
            let position = (self.0.iter())
                .position(|c| public_to_address(&c.pubkey) == *target)
                .expect("Reprioritize target should be a candidate");
            renewed.push(self.0.remove(position));
        }
        self.0.append(&mut renewed);
    }
}

pub struct Jail(BTreeMap<Address, Prisoner>);
#[derive(Clone, Debug, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct Prisoner {
    pub address: Address,
    pub deposit: Deposit,
    pub custody_until: u64,
    pub released_at: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReleaseResult {
    NotExists,
    InCustody,
    Released(Prisoner),
}

impl Jail {
    pub fn load_from_state(state: &TopLevelState) -> StateResult<Jail> {
        let key = *JAIL_KEY;
        let prisoner = state.action_data(&key)?.map(|data| decode_list::<Prisoner>(&data)).unwrap_or_default();
        let indexed = prisoner.into_iter().map(|c| (c.address, c)).collect();
        Ok(Jail(indexed))
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let key = *JAIL_KEY;
        if !self.0.is_empty() {
            let encoded = encode_iter(self.0.values());
            state.update_action_data(&key, encoded)?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    pub fn get_prisoner(&self, address: &Address) -> Option<&Prisoner> {
        self.0.get(address)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn add(&mut self, candidate: Candidate, custody_until: u64, released_at: u64) {
        assert!(custody_until <= released_at);
        let address = public_to_address(&candidate.pubkey);
        self.0.insert(address, Prisoner {
            address,
            deposit: candidate.deposit,
            custody_until,
            released_at,
        });
    }

    pub fn remove(&mut self, address: &Address) -> Option<Prisoner> {
        self.0.remove(address)
    }

    pub fn try_release(&mut self, address: &Address, term_index: u64) -> ReleaseResult {
        match self.0.entry(*address) {
            Entry::Occupied(entry) => {
                if entry.get().custody_until < term_index {
                    ReleaseResult::Released(entry.remove())
                } else {
                    ReleaseResult::InCustody
                }
            }
            _ => ReleaseResult::NotExists,
        }
    }

    pub fn drain_released_prisoners(&mut self, term_index: u64) -> Vec<Prisoner> {
        let (released, retained): (Vec<_>, Vec<_>) =
            self.0.values().cloned().partition(|c| c.released_at <= term_index);
        self.0 = retained.into_iter().map(|c| (c.address, c)).collect();
        released
    }
}

pub struct Banned(BTreeSet<Address>);
impl Banned {
    pub fn load_from_state(state: &TopLevelState) -> StateResult<Banned> {
        let key = *BANNED_KEY;
        let action_data = state.action_data(&key)?;
        Ok(Banned(decode_set(action_data.as_ref())))
    }

    pub fn save_to_state(&self, state: &mut TopLevelState) -> StateResult<()> {
        let key = *BANNED_KEY;
        if !self.0.is_empty() {
            let encoded = encode_set(&self.0);
            state.update_action_data(&key, encoded)?;
        } else {
            state.remove_action_data(&key);
        }
        Ok(())
    }

    pub fn add(&mut self, address: Address) {
        self.0.insert(address);
    }

    pub fn is_banned(&self, address: &Address) -> bool {
        self.0.contains(address)
    }
}

fn decode_set<V>(data: Option<&ActionData>) -> BTreeSet<V>
where
    V: Ord + Decodable, {
    let mut result = BTreeSet::new();
    if let Some(rlp) = data.map(|x| Rlp::new(x)) {
        for record in rlp.iter() {
            let value: V = record.as_val().unwrap();
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
    rlp.drain()
}

fn decode_map<K, V>(data: Option<&ActionData>) -> BTreeMap<K, V>
where
    K: Ord + Decodable,
    V: Decodable, {
    if let Some(rlp) = data.map(|x| Rlp::new(x)) {
        decode_map_impl(rlp)
    } else {
        Default::default()
    }
}

fn decode_map_impl<K, V>(rlp: Rlp) -> BTreeMap<K, V>
where
    K: Ord + Decodable,
    V: Decodable, {
    let mut result = BTreeMap::new();
    for record in rlp.iter() {
        let key: K = record.val_at(0).unwrap();
        let value: V = record.val_at(1).unwrap();
        assert_eq!(Ok(2), record.item_count());
        result.insert(key, value);
    }
    result
}

fn encode_map<K, V>(map: &BTreeMap<K, V>) -> Vec<u8>
where
    K: Ord + Encodable,
    V: Encodable, {
    let mut rlp = RlpStream::new();
    encode_map_impl(&mut rlp, map);
    rlp.drain()
}

fn encode_map_impl<K, V>(rlp: &mut RlpStream, map: &BTreeMap<K, V>)
where
    K: Ord + Encodable,
    V: Encodable, {
    rlp.begin_list(map.len());
    for (key, value) in map.iter() {
        let record = rlp.begin_list(2);
        record.append(key);
        record.append(value);
    }
}

fn decode_map_tuple<K, V>(data: Option<&ActionData>) -> (BTreeMap<K, V>, BTreeMap<K, V>)
where
    K: Ord + Decodable,
    V: Decodable, {
    if let Some(rlp) = data.map(|x| Rlp::new(x)) {
        assert_eq!(Ok(2), rlp.item_count());
        let map0 = decode_map_impl(rlp.at(0).unwrap());
        let map1 = decode_map_impl(rlp.at(1).unwrap());
        (map0, map1)
    } else {
        Default::default()
    }
}

fn encode_map_tuple<K, V>(map0: &BTreeMap<K, V>, map1: &BTreeMap<K, V>) -> Vec<u8>
where
    K: Ord + Encodable,
    V: Encodable, {
    let mut rlp = RlpStream::new();
    rlp.begin_list(2);

    encode_map_impl(&mut rlp, map0);
    encode_map_impl(&mut rlp, map1);

    rlp.drain()
}

fn encode_iter<'a, V, I>(iter: I) -> Vec<u8>
where
    V: 'a + Encodable,
    I: ExactSizeIterator<Item = &'a V> + Clone, {
    let mut rlp = RlpStream::new();
    rlp.begin_list(iter.clone().count());
    for value in iter {
        rlp.append(value);
    }
    rlp.drain()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cstate::tests::helpers;
    use rand::{Rng, SeedableRng};
    use rand_xorshift::XorShiftRng;

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
    fn balance_subtract_all_should_remove_entry_from_db() {
        let mut state = helpers::get_temp_state();
        let address = Address::random();

        let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
        account.add_balance(100).unwrap();
        account.save_to_state(&mut state).unwrap();

        let mut account = StakeAccount::load_from_state(&state, &address).unwrap();
        let result = account.subtract_balance(100);
        assert!(result.is_ok());
        account.save_to_state(&mut state).unwrap();

        let data = state.action_data(&get_account_key(&address)).unwrap();
        assert_eq!(data, None);
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
            stakeholders.update_by_increased_balance(account);
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
            stakeholders.update_by_increased_balance(account);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &mut accounts {
            if rand::random() {
                account.balance = 0;
            }
            let delegation = Delegation::load_from_state(&state, account.address).unwrap();
            stakeholders.update_by_decreased_balance(account, &delegation);
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
    fn stakeholders_doesnt_untrack_if_delegation_exists() {
        let mut state = helpers::get_temp_state();
        let addresses: Vec<_> = (1..100).map(|_| Address::random()).collect();
        let mut accounts: Vec<_> = addresses
            .iter()
            .map(|address| StakeAccount {
                address,
                balance: 100,
            })
            .collect();

        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            stakeholders.update_by_increased_balance(account);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        let mut stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &mut accounts {
            // like self-delegate
            let mut delegation = Delegation::load_from_state(&state, account.address).unwrap();
            delegation.add_quantity(*account.address, account.balance).unwrap();
            account.balance = 0;
            stakeholders.update_by_decreased_balance(account, &delegation);
        }
        stakeholders.save_to_state(&mut state).unwrap();

        let stakeholders = Stakeholders::load_from_state(&state).unwrap();
        for account in &accounts {
            assert!(stakeholders.contains(account.address));
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

    #[test]
    fn delegation_zero_add_should_not_be_included() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatee1 = Address::random();
        let delegatee2 = Address::random();

        // Do delegate
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.add_quantity(delegatee1, 100).unwrap();
        delegation.add_quantity(delegatee2, 0).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        let delegated = delegation.iter().collect::<Vec<_>>();
        assert_eq!(&delegated, &[(&delegatee1, &100)]);
    }

    #[test]
    fn delegation_can_subtract() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatee = Address::random();

        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.add_quantity(delegatee, 100).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        // Do subtract
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.subtract_quantity(delegatee, 30).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        // Assert
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&delegatee), 70);
    }

    #[test]
    fn delegation_cannot_subtract_mor_than_delegated() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatee = Address::random();

        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.add_quantity(delegatee, 100).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        // Do subtract
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert!(delegation.subtract_quantity(delegatee, 130).is_err());
    }

    #[test]
    fn delegation_empty_removed_from_state() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatee = Address::random();

        // Do delegate
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.add_quantity(delegatee, 0).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        let result = state.action_data(&get_delegation_key(&delegator)).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn delegation_became_empty_removed_from_state() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let delegator = Address::random();
        let delegatee = Address::random();

        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.add_quantity(delegatee, 100).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        // Do subtract
        let mut delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        delegation.subtract_quantity(delegatee, 100).unwrap();
        delegation.save_to_state(&mut state).unwrap();

        // Assert
        let result = state.action_data(&get_delegation_key(&delegator)).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn load_and_save_intermediate_rewards_v0() {
        let mut state = helpers::get_temp_state();
        let rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        rewards.save_to_state(&mut state).unwrap();
    }

    #[test]
    fn add_quantity_v0() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut state = helpers::get_temp_state();
        let mut origin_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        origin_rewards.add_quantity(address1, 1);
        origin_rewards.add_quantity(address2, 2);
        origin_rewards.save_to_state(&mut state).unwrap();
        let recovered_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(origin_rewards, recovered_rewards);
    }

    #[test]
    fn drain_v0() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut state = helpers::get_temp_state();
        let mut origin_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        origin_rewards.add_quantity(address1, 1);
        origin_rewards.add_quantity(address2, 2);
        origin_rewards.save_to_state(&mut state).unwrap();
        let mut recovered_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(origin_rewards, recovered_rewards);
        let _drained = recovered_rewards.drain_previous();
        recovered_rewards.save_to_state(&mut state).unwrap();
        let mut final_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(BTreeMap::new(), final_rewards.previous);
        let current = final_rewards.current.clone();
        final_rewards.move_current_to_previous();
        assert_eq!(BTreeMap::new(), final_rewards.current);
        assert_eq!(current, final_rewards.previous);
    }

    #[test]
    fn save_v0_and_load_v1_intermediate_rewards() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut state = helpers::get_temp_state();
        let mut origin_rewards = v0::IntermediateRewards::load_from_state(&state).unwrap();
        origin_rewards.add_quantity(address1, 1);
        origin_rewards.add_quantity(address2, 2);
        origin_rewards.save_to_state(&mut state).unwrap();
        let recovered_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(origin_rewards.previous, recovered_rewards.current);
        assert_eq!(origin_rewards.current, recovered_rewards.calculated);
    }

    #[test]
    fn load_and_save_intermediate_rewards_v1() {
        let mut state = helpers::get_temp_state();
        let rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        rewards.save_to_state(&mut state).unwrap();
    }

    #[test]
    fn add_quantity_v1() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut state = helpers::get_temp_state();
        let mut origin_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        origin_rewards.add_quantity(address1, 1);
        origin_rewards.add_quantity(address2, 2);
        origin_rewards.save_to_state(&mut state).unwrap();
        let recovered_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(origin_rewards, recovered_rewards);
    }

    #[test]
    fn drain_v1() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut state = helpers::get_temp_state();
        let mut origin_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        origin_rewards.add_quantity(address1, 1);
        origin_rewards.add_quantity(address2, 2);
        origin_rewards.save_to_state(&mut state).unwrap();
        let mut recovered_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(origin_rewards, recovered_rewards);
        recovered_rewards.drain_current();
        recovered_rewards.save_to_state(&mut state).unwrap();
        let mut final_rewards = v1::IntermediateRewards::load_from_state(&state).unwrap();
        assert_eq!(BTreeMap::new(), final_rewards.current);
        final_rewards.drain_calculated();
        assert_eq!(BTreeMap::new(), final_rewards.calculated);
    }

    #[test]
    fn candidates_deposit_add() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);
        let deposits = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        for deposit in deposits.iter() {
            let mut candidates = Candidates::load_from_state(&state).unwrap();
            candidates.add_deposit(&pubkey, *deposit, 0, b"".to_vec());
            candidates.save_to_state(&mut state).unwrap();
        }

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(candidate.unwrap().deposit, 55);
    }

    #[test]
    fn candidates_metadata() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 10, 0, b"metadata".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(&candidate.unwrap().metadata, b"metadata");
    }

    #[test]
    fn candidates_update_metadata() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 10, 0, b"metadata".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 10, 0, b"metadata-updated".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(&candidate.unwrap().metadata, b"metadata-updated");
    }

    #[test]
    fn candidates_deposit_can_be_zero() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);
        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 0, 10, b"".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(candidate.unwrap().deposit, 0);
        assert_eq!(candidate.unwrap().nomination_ends_at, 10, "Can be a candidate with 0 deposit");
    }

    #[test]
    fn candidates_update_metadata_with_zero_additional_deposit() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 10, 0, b"metadata".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        candidates.add_deposit(&pubkey, 0, 0, b"metadata-updated".to_vec());
        candidates.save_to_state(&mut state).unwrap();

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(&candidate.unwrap().metadata, b"metadata-updated");
    }

    #[test]
    fn candidates_deposit_should_update_nomination_ends_at() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = Public::random();
        let account = public_to_address(&pubkey);
        let deposit_and_nomination_ends_at = [(10, 11), (20, 22), (30, 33), (0, 44)];

        for (deposit, nomination_ends_at) in &deposit_and_nomination_ends_at {
            let mut candidates = Candidates::load_from_state(&state).unwrap();
            candidates.add_deposit(&pubkey, *deposit, *nomination_ends_at, b"".to_vec());
            candidates.save_to_state(&mut state).unwrap();
        }

        // Assert
        let candidates = Candidates::load_from_state(&state).unwrap();
        let candidate = candidates.get_candidate(&account);
        assert_ne!(candidate, None);
        assert_eq!(candidate.unwrap().deposit, 60);
        assert_eq!(
            candidate.unwrap().nomination_ends_at,
            44,
            "nomination_ends_at should be updated incrementally, and including zero deposit"
        );
    }

    #[test]
    fn candidates_can_remove_expired_deposit() {
        let mut state = helpers::get_temp_state();

        let pubkey0 = 0.into();
        let pubkey1 = 1.into();
        let pubkey2 = 2.into();
        let pubkey3 = 3.into();

        // Prepare
        let candidates_prepared = [
            Candidate {
                pubkey: pubkey0,
                deposit: 20,
                nomination_ends_at: 11,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey1,
                deposit: 30,
                nomination_ends_at: 22,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey2,
                deposit: 40,
                nomination_ends_at: 33,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey3,
                deposit: 50,
                nomination_ends_at: 44,
                metadata: b"".to_vec(),
            },
        ];

        for Candidate {
            pubkey,
            deposit,
            nomination_ends_at,
            metadata,
        } in &candidates_prepared
        {
            let mut candidates = Candidates::load_from_state(&state).unwrap();
            candidates.add_deposit(&pubkey, *deposit, *nomination_ends_at, metadata.clone());
            candidates.save_to_state(&mut state).unwrap();
        }

        // Remove Expired
        let mut candidates = Candidates::load_from_state(&state).unwrap();
        let mut expired = candidates.drain_expired_candidates(22);
        candidates.save_to_state(&mut state).unwrap();

        // Assert
        expired.sort_unstable_by_key(|c| c.pubkey);
        let mut prepared_expired = candidates_prepared[0..=1].to_vec();
        prepared_expired.sort_unstable_by_key(|c| c.pubkey);
        assert_eq!(expired, prepared_expired);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates.get_candidate(&public_to_address(&candidates_prepared[2].pubkey)),
            Some(&candidates_prepared[2])
        );
        assert_eq!(
            candidates.get_candidate(&public_to_address(&candidates_prepared[3].pubkey)),
            Some(&candidates_prepared[3])
        );
    }

    #[test]
    fn candidates_expire_all_cleanup_state() {
        let mut state = helpers::get_temp_state();

        let pubkey0 = 0.into();
        let pubkey1 = 1.into();
        let pubkey2 = 2.into();
        let pubkey3 = 3.into();

        // Prepare
        let candidates_prepared = [
            Candidate {
                pubkey: pubkey0,
                deposit: 20,
                nomination_ends_at: 11,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey1,
                deposit: 30,
                nomination_ends_at: 22,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey2,
                deposit: 40,
                nomination_ends_at: 33,
                metadata: b"".to_vec(),
            },
            Candidate {
                pubkey: pubkey3,
                deposit: 50,
                nomination_ends_at: 44,
                metadata: b"".to_vec(),
            },
        ];

        for Candidate {
            pubkey,
            deposit,
            nomination_ends_at,
            metadata,
        } in &candidates_prepared
        {
            let mut candidates = Candidates::load_from_state(&state).unwrap();
            candidates.add_deposit(&pubkey, *deposit, *nomination_ends_at, metadata.clone());
            candidates.save_to_state(&mut state).unwrap();
        }

        // Remove Expired
        let mut candidates = Candidates::load_from_state(&state).unwrap();
        let mut expired = candidates.drain_expired_candidates(99);
        candidates.save_to_state(&mut state).unwrap();

        expired.sort_unstable_by_key(|c| c.pubkey);
        let mut prepared_expired = candidates_prepared[0..4].to_vec();
        prepared_expired.sort_unstable_by_key(|c| c.pubkey);

        // Assert
        assert_eq!(expired, prepared_expired);
        let result = state.action_data(&*CANDIDATES_KEY).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn jail_try_free_not_existing() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = 1.into();
        let address = public_to_address(&pubkey);
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.save_to_state(&mut state).unwrap();

        let mut jail = Jail::load_from_state(&state).unwrap();
        let freed = jail.try_release(&Address::from(1000), 5);
        assert_eq!(freed, ReleaseResult::NotExists);
        assert_eq!(jail.len(), 1);
        assert_ne!(jail.get_prisoner(&address), None);
    }

    #[test]
    fn jail_try_release_none_until_custody() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = 1.into();
        let address = public_to_address(&pubkey);
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.save_to_state(&mut state).unwrap();

        let mut jail = Jail::load_from_state(&state).unwrap();
        let released = jail.try_release(&address, 10);
        assert_eq!(released, ReleaseResult::InCustody);
        assert_eq!(jail.len(), 1);
        assert_ne!(jail.get_prisoner(&address), None);
    }

    #[test]
    fn jail_try_release_prisoner_after_custody() {
        let mut state = helpers::get_temp_state();

        // Prepare
        let pubkey = 1.into();
        let address = public_to_address(&pubkey);
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.save_to_state(&mut state).unwrap();

        let mut jail = Jail::load_from_state(&state).unwrap();
        let released = jail.try_release(&address, 11);
        jail.save_to_state(&mut state).unwrap();

        // Assert
        assert_eq!(
            released,
            ReleaseResult::Released(Prisoner {
                address,
                deposit: 100,
                custody_until: 10,
                released_at: 20,
            })
        );
        assert_eq!(jail.len(), 0);
        assert_eq!(jail.get_prisoner(&address), None);

        let result = state.action_data(&*JAIL_KEY).unwrap();
        assert_eq!(result, None, "Should clean the state if all prisoners are released");
    }

    #[test]
    fn jail_keep_prisoners_until_kick_at() {
        let mut state = helpers::get_temp_state();

        let pubkey1 = 1.into();
        let pubkey2 = 2.into();
        let address1 = public_to_address(&pubkey1);
        let address2 = public_to_address(&pubkey2);

        // Prepare
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey: pubkey1,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.add(
            Candidate {
                pubkey: pubkey2,
                deposit: 200,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            15,
            25,
        );
        jail.save_to_state(&mut state).unwrap();

        // Kick
        let mut jail = Jail::load_from_state(&state).unwrap();
        let released = jail.drain_released_prisoners(19);
        jail.save_to_state(&mut state).unwrap();

        // Assert
        assert_eq!(released, Vec::new());
        assert_eq!(jail.len(), 2);
        assert_ne!(jail.get_prisoner(&address1), None);
        assert_ne!(jail.get_prisoner(&address2), None);
    }

    #[test]
    fn jail_partially_kick_prisoners() {
        let mut state = helpers::get_temp_state();

        let pubkey1 = 1.into();
        let pubkey2 = 2.into();
        let address1 = public_to_address(&pubkey1);
        let address2 = public_to_address(&pubkey2);

        // Prepare
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey: pubkey1,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.add(
            Candidate {
                pubkey: pubkey2,
                deposit: 200,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            15,
            25,
        );
        jail.save_to_state(&mut state).unwrap();

        // Kick
        let mut jail = Jail::load_from_state(&state).unwrap();
        let released = jail.drain_released_prisoners(20);
        jail.save_to_state(&mut state).unwrap();

        // Assert
        assert_eq!(released, vec![Prisoner {
            address: address1,
            deposit: 100,
            custody_until: 10,
            released_at: 20,
        }]);
        assert_eq!(jail.len(), 1);
        assert_eq!(jail.get_prisoner(&address1), None);
        assert_ne!(jail.get_prisoner(&address2), None);
    }

    #[test]
    fn jail_kick_all_prisoners() {
        let mut state = helpers::get_temp_state();

        let pubkey1 = 1.into();
        let pubkey2 = 2.into();
        let address1 = public_to_address(&pubkey1);
        let address2 = public_to_address(&pubkey2);

        // Prepare
        let mut jail = Jail::load_from_state(&state).unwrap();
        jail.add(
            Candidate {
                pubkey: pubkey1,
                deposit: 100,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            10,
            20,
        );
        jail.add(
            Candidate {
                pubkey: pubkey2,
                deposit: 200,
                nomination_ends_at: 0,
                metadata: b"".to_vec(),
            },
            15,
            25,
        );
        jail.save_to_state(&mut state).unwrap();

        // Kick
        let mut jail = Jail::load_from_state(&state).unwrap();
        let released = jail.drain_released_prisoners(25);
        jail.save_to_state(&mut state).unwrap();

        // Assert
        assert_eq!(released, vec![
            Prisoner {
                address: address1,
                deposit: 100,
                custody_until: 10,
                released_at: 20,
            },
            Prisoner {
                address: address2,
                deposit: 200,
                custody_until: 15,
                released_at: 25,
            }
        ]);
        assert_eq!(jail.len(), 0);
        assert_eq!(jail.get_prisoner(&address1), None);
        assert_eq!(jail.get_prisoner(&address2), None);

        let result = state.action_data(&*JAIL_KEY).unwrap();
        assert_eq!(result, None, "Should clean the state if all prisoners are released");
    }

    #[test]
    fn empty_ban_save_clean_state() {
        let mut state = helpers::get_temp_state();
        let banned = Banned::load_from_state(&state).unwrap();
        banned.save_to_state(&mut state).unwrap();

        let result = state.action_data(&*BANNED_KEY).unwrap();
        assert_eq!(result, None, "Should clean the state if there are no banned accounts");
    }

    #[test]
    fn added_to_ban_is_banned() {
        let mut state = helpers::get_temp_state();

        let address = Address::from(1);
        let innocent = Address::from(2);

        let mut banned = Banned::load_from_state(&state).unwrap();
        banned.add(address);
        banned.save_to_state(&mut state).unwrap();

        let banned = Banned::load_from_state(&state).unwrap();
        assert!(banned.is_banned(&address));
        assert!(!banned.is_banned(&innocent));
    }

    #[test]
    fn latest_deposit_higher_priority() {
        let mut state = helpers::get_temp_state();
        let pubkeys = (0..10).map(|_| Public::random()).collect::<Vec<_>>();

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        for _ in 0..10 {
            // Random pre-fill
            let i = rand::thread_rng().gen_range(0, pubkeys.len());
            let pubkey = &pubkeys[i];
            candidates.add_deposit(pubkey, 0, 0, Bytes::new());
        }
        // Inserting pubkey in this order, they'll get sorted.
        for pubkey in &pubkeys {
            candidates.add_deposit(pubkey, 10, 0, Bytes::new());
        }
        candidates.save_to_state(&mut state).unwrap();

        let candidates = Candidates::load_from_state(&state).unwrap();
        let results: Vec<_> = pubkeys.iter().map(|pubkey| candidates.get_index(&public_to_address(&pubkey))).collect();
        // TODO assert!(results.is_sorted(), "Should be sorted in the insertion order");
        for i in 0..results.len() - 1 {
            assert!(results[i] < results[i + 1], "Should be sorted in the insertion order");
        }
    }

    #[test]
    fn renew_doesnt_change_relative_priority() {
        let mut state = helpers::get_temp_state();
        let pubkeys = (0..10).map(|_| Public::random()).collect::<Vec<_>>();

        let mut candidates = Candidates::load_from_state(&state).unwrap();
        for pubkey in &pubkeys {
            candidates.add_deposit(pubkey, 10, 0, Bytes::new());
        }
        candidates.save_to_state(&mut state).unwrap();

        let dummy_validators = Validators(
            pubkeys[0..5]
                .iter()
                .map(|pubkey| Validator {
                    pubkey: *pubkey,
                    deposit: 0,
                    delegation: 0,
                    weight: 0,
                })
                .collect(),
        );
        let dummy_banned = Banned::load_from_state(&state).unwrap();
        candidates.renew_candidates(&dummy_validators, 0, &[], &dummy_banned);

        let indexes: Vec<_> =
            pubkeys.iter().map(|pubkey| candidates.get_index(&public_to_address(pubkey)).unwrap()).collect();
        assert_eq!(indexes, vec![5, 6, 7, 8, 9, 0, 1, 2, 3, 4]);
    }
}
