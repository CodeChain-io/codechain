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
use std::sync::{Arc, Weak};

use crate::client::ConsensusClient;
use ccrypto::Blake;
use ckey::{public_to_address, recover, Address, Public, Signature};
use cstate::{ActionHandler, StateResult, TopLevelState, TopState, TopStateView};
use ctypes::errors::{RuntimeError, SyntaxError};
use ctypes::util::unexpected::Mismatch;
use ctypes::{CommonParams, Header};
use parking_lot::RwLock;
use primitives::{Bytes, H256};
use rlp::{Decodable, Rlp};

pub use self::action_data::{Banned, Validator, Validators};
use self::action_data::{Candidates, Delegation, Jail, ReleaseResult, StakeAccount, Stakeholders};
pub use self::actions::Action;
pub use self::distribute::fee_distribute;
use super::ValidatorSet;

pub const CUSTOM_ACTION_HANDLER_ID: u64 = 2;

pub struct Stake {
    genesis_stakes: HashMap<Address, u64>,
    client: RwLock<Option<Weak<dyn ConsensusClient>>>,
    validators: RwLock<Option<Weak<dyn ValidatorSet>>>,
}

impl Stake {
    pub fn new(genesis_stakes: HashMap<Address, u64>) -> Stake {
        Stake {
            genesis_stakes,
            client: Default::default(),
            validators: Default::default(),
        }
    }
    pub fn register_resources(&self, client: Weak<dyn ConsensusClient>, validators: Weak<dyn ValidatorSet>) {
        *self.client.write() = Some(Weak::clone(&client));
        *self.validators.write() = Some(Weak::clone(&validators));
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

    fn execute(
        &self,
        bytes: &[u8],
        state: &mut TopLevelState,
        fee_payer: &Address,
        sender_public: &Public,
    ) -> StateResult<()> {
        let action = Action::decode(&Rlp::new(bytes)).expect("Verification passed");
        match action {
            Action::TransferCCS {
                address,
                quantity,
            } => transfer_ccs(state, fee_payer, &address, quantity),
            Action::DelegateCCS {
                address,
                quantity,
            } => delegate_ccs(state, fee_payer, &address, quantity),
            Action::Revoke {
                address,
                quantity,
            } => revoke(state, fee_payer, &address, quantity),
            Action::Redelegate {
                prev_delegatee,
                next_delegatee,
                quantity,
            } => redelegate(state, fee_payer, &prev_delegatee, &next_delegatee, quantity),
            Action::SelfNominate {
                deposit,
                metadata,
            } => {
                let (current_term, nomination_ends_at) = {
                    let metadata = state.metadata()?.expect("Metadata must exist");
                    const DEFAULT_NOMINATION_EXPIRATION: u64 = 24;
                    let current_term = metadata.current_term_id();
                    let expiration = metadata
                        .params()
                        .map(CommonParams::nomination_expiration)
                        .unwrap_or(DEFAULT_NOMINATION_EXPIRATION);
                    let nomination_ends_at = current_term + expiration;
                    (current_term, nomination_ends_at)
                };
                self_nominate(state, fee_payer, sender_public, deposit, current_term, nomination_ends_at, metadata)
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => change_params(state, metadata_seq, *params, &signatures),
            Action::ReportDoubleVote {
                message1,
                ..
            } => {
                let validator_set =
                    self.validators.read().as_ref().and_then(Weak::upgrade).expect("ValidatorSet must be initialized");
                let client = self.client.read().as_ref().and_then(Weak::upgrade).expect("Client must be initialized");
                let parent_hash =
                    client.block_header(&(message1.height() - 1).into()).expect("Parent header verified").hash();
                let malicious_user_public = validator_set.get(&parent_hash, message1.signer_index());

                ban(state, sender_public, public_to_address(&malicious_user_public))
            }
        }
    }

    fn verify(&self, bytes: &[u8], current_params: &CommonParams) -> Result<(), SyntaxError> {
        let action =
            Action::decode(&Rlp::new(bytes)).map_err(|err| SyntaxError::InvalidCustomAction(err.to_string()))?;
        let client: Option<Arc<dyn ConsensusClient>> = self.client.read().as_ref().and_then(Weak::upgrade);
        let validators: Option<Arc<dyn ValidatorSet>> = self.validators.read().as_ref().and_then(Weak::upgrade);
        action.verify(current_params, client, validators)
    }

    fn on_close_block(&self, _state: &mut TopLevelState, _header: &Header) -> StateResult<()> {
        Ok(())
    }
}

fn transfer_ccs(state: &mut TopLevelState, fee_payer: &Address, receiver: &Address, quantity: u64) -> StateResult<()> {
    let mut stakeholders = Stakeholders::load_from_state(state)?;
    let mut sender_account = StakeAccount::load_from_state(state, fee_payer)?;
    let mut receiver_account = StakeAccount::load_from_state(state, receiver)?;
    let sender_delegations = Delegation::load_from_state(state, fee_payer)?;

    sender_account.subtract_balance(quantity)?;
    receiver_account.add_balance(quantity)?;

    stakeholders.update_by_decreased_balance(&sender_account, &sender_delegations);
    stakeholders.update_by_increased_balance(&receiver_account);

    stakeholders.save_to_state(state)?;
    sender_account.save_to_state(state)?;
    receiver_account.save_to_state(state)?;

    ctrace!(ENGINE, "Transferred CCS fee_payer: {}, receiver: {}, quantity: {}", fee_payer, receiver, quantity);
    Ok(())
}

fn delegate_ccs(state: &mut TopLevelState, fee_payer: &Address, delegatee: &Address, quantity: u64) -> StateResult<()> {
    let candidates = Candidates::load_from_state(state)?;
    if candidates.get_candidate(delegatee).is_none() {
        return Err(RuntimeError::FailedToHandleCustomAction("Can delegate to who is a candidate".into()).into())
    }

    let banned = Banned::load_from_state(state)?;
    let jailed = Jail::load_from_state(state)?;
    assert!(!banned.is_banned(&delegatee), "A candidate must not be banned");
    assert_eq!(None, jailed.get_prisoner(delegatee), "A candidate must not be jailed");

    let mut delegator = StakeAccount::load_from_state(state, fee_payer)?;
    let mut delegation = Delegation::load_from_state(state, &fee_payer)?;

    delegator.subtract_balance(quantity)?;
    delegation.add_quantity(*delegatee, quantity)?;
    // delegation does not touch stakeholders

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;

    ctrace!(ENGINE, "Delegated CCS. delegator: {}, delegatee: {}, quantity: {}", fee_payer, delegatee, quantity);
    Ok(())
}

fn revoke(state: &mut TopLevelState, fee_payer: &Address, delegatee: &Address, quantity: u64) -> StateResult<()> {
    let mut delegator = StakeAccount::load_from_state(state, fee_payer)?;
    let mut delegation = Delegation::load_from_state(state, &fee_payer)?;

    delegator.add_balance(quantity)?;
    delegation.subtract_quantity(*delegatee, quantity)?;
    // delegation does not touch stakeholders

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;

    ctrace!(ENGINE, "Revoked CCS. delegator: {}, delegatee: {}, quantity: {}", fee_payer, delegatee, quantity);
    Ok(())
}

fn redelegate(
    state: &mut TopLevelState,
    fee_payer: &Address,
    prev_delegatee: &Address,
    next_delegatee: &Address,
    quantity: u64,
) -> StateResult<()> {
    let candidates = Candidates::load_from_state(state)?;
    if candidates.get_candidate(next_delegatee).is_none() {
        return Err(RuntimeError::FailedToHandleCustomAction("Can delegate to who is a candidate".into()).into())
    }

    let banned = Banned::load_from_state(state)?;
    let jailed = Jail::load_from_state(state)?;
    assert!(!banned.is_banned(&next_delegatee), "A candidate must not be banned");
    assert_eq!(None, jailed.get_prisoner(next_delegatee), "A candidate must not be jailed");

    let delegator = StakeAccount::load_from_state(state, fee_payer)?;
    let mut delegation = Delegation::load_from_state(state, &fee_payer)?;

    delegation.subtract_quantity(*prev_delegatee, quantity)?;
    delegation.add_quantity(*next_delegatee, quantity)?;

    delegation.save_to_state(state)?;
    delegator.save_to_state(state)?;

    ctrace!(
        ENGINE,
        "Redelegated CCS. delegator: {}, prev_delegatee: {}, next_delegatee: {}, quantity: {}",
        fee_payer,
        prev_delegatee,
        next_delegatee,
        quantity
    );
    Ok(())
}

fn self_nominate(
    state: &mut TopLevelState,
    fee_payer: &Address,
    sender_public: &Public,
    deposit: u64,
    current_term: u64,
    nomination_ends_at: u64,
    metadata: Bytes,
) -> StateResult<()> {
    if public_to_address(sender_public) != *fee_payer {
        return Err(RuntimeError::FailedToHandleCustomAction("Cannot Self-nominate with regular key".to_string()).into())
    }

    let blacklist = Banned::load_from_state(state)?;
    if blacklist.is_banned(&fee_payer) {
        return Err(RuntimeError::FailedToHandleCustomAction("Account is blacklisted".to_string()).into())
    }

    let mut jail = Jail::load_from_state(&state)?;
    let total_deposit = match jail.try_release(fee_payer, current_term) {
        ReleaseResult::InCustody => {
            return Err(RuntimeError::FailedToHandleCustomAction("Account is still in custody".to_string()).into())
        }
        ReleaseResult::NotExists => deposit,
        ReleaseResult::Released(prisoner) => {
            assert_eq!(&prisoner.address, fee_payer);
            prisoner.deposit + deposit
        }
    };

    let mut candidates = Candidates::load_from_state(&state)?;
    state.sub_balance(fee_payer, deposit)?;
    candidates.add_deposit(sender_public, total_deposit, nomination_ends_at, metadata);

    jail.save_to_state(state)?;
    candidates.save_to_state(state)?;

    ctrace!(
        ENGINE,
        "Self-nominated. nominee: {}, deposit: {}, current_term: {}, ends_at: {}",
        fee_payer,
        deposit,
        current_term,
        nomination_ends_at
    );
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

pub fn get_validators(state: &TopLevelState) -> StateResult<Validators> {
    Validators::load_from_state(state)
}

pub mod v0 {
    use super::action_data::v0::IntermediateRewards;
    use super::*;

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
}

pub mod v1 {
    use super::action_data::v1::IntermediateRewards;
    use super::*;

    pub fn add_intermediate_rewards(state: &mut TopLevelState, address: Address, reward: u64) -> StateResult<()> {
        let mut rewards = IntermediateRewards::load_from_state(state)?;
        rewards.add_quantity(address, reward);
        rewards.save_to_state(state)?;
        Ok(())
    }

    pub fn drain_current_rewards(state: &mut TopLevelState) -> StateResult<BTreeMap<Address, u64>> {
        let mut rewards = IntermediateRewards::load_from_state(state)?;
        let drained = rewards.drain_current();
        rewards.save_to_state(state)?;
        Ok(drained)
    }

    pub fn update_calculated_rewards(state: &mut TopLevelState, values: HashMap<Address, u64>) -> StateResult<()> {
        let mut rewards = IntermediateRewards::load_from_state(state)?;
        rewards.update_calculated(values.into_iter().collect());
        rewards.save_to_state(state)
    }

    pub fn drain_calculated_rewards(state: &mut TopLevelState) -> StateResult<BTreeMap<Address, u64>> {
        let mut rewards = IntermediateRewards::load_from_state(state)?;
        let drained = rewards.drain_calculated();
        rewards.save_to_state(state)?;
        Ok(drained)
    }
}

pub fn update_validator_weights(state: &mut TopLevelState, block_author: &Address) -> StateResult<()> {
    let mut validators = Validators::load_from_state(state)?;
    validators.update_weight(block_author);
    validators.save_to_state(state)
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

    ctrace!(ENGINE, "ChangeParams. params: {:?}", params);
    Ok(())
}

pub fn on_term_close(
    state: &mut TopLevelState,
    last_term_finished_block_num: u64,
    inactive_validators: &[Address],
) -> StateResult<()> {
    let metadata = state.metadata()?.expect("The metadata must exist");
    let current_term = metadata.current_term_id();
    ctrace!(ENGINE, "on_term_close. current_term: {}", current_term);

    let (nomination_expiration, custody_until, kick_at) = {
        let metadata = metadata.params().expect(
            "Term close events can be called after the ChangeParams called, \
             so the metadata always has CommonParams",
        );
        let nomination_expiration = metadata.nomination_expiration();
        assert_ne!(0, nomination_expiration);
        let custody_period = metadata.custody_period();
        assert_ne!(0, custody_period);
        let release_period = metadata.release_period();
        assert_ne!(0, release_period);
        (nomination_expiration, current_term + custody_period, current_term + release_period)
    };

    let expired = update_candidates(state, current_term, nomination_expiration, inactive_validators)?;
    let released = release_jailed_prisoners(state, current_term)?;

    let reverted: Vec<_> = expired.into_iter().chain(released).collect();
    revert_delegations(state, &reverted)?;

    jail(state, inactive_validators, custody_until, kick_at)?;

    let validators = Validators::elect(state)?;
    validators.save_to_state(state)?;

    state.increase_term_id(last_term_finished_block_num)?;
    Ok(())
}

fn update_candidates(
    state: &mut TopLevelState,
    current_term: u64,
    nomination_expiration: u64,
    inactive_validators: &[Address],
) -> StateResult<Vec<Address>> {
    let banned = Banned::load_from_state(state)?;

    let mut candidates = Candidates::load_from_state(state)?;
    let nomination_ends_at = current_term + nomination_expiration;

    let current_validators = Validators::load_from_state(state)?;
    candidates.renew_candidates(&current_validators, nomination_ends_at, &inactive_validators, &banned);

    let expired = candidates.drain_expired_candidates(current_term);
    for candidate in &expired {
        let address = public_to_address(&candidate.pubkey);
        state.add_balance(&address, candidate.deposit)?;
        ctrace!(ENGINE, "on_term_close::expired. candidate: {}, deposit: {}", address, candidate.deposit);
    }
    candidates.save_to_state(state)?;
    Ok(expired.into_iter().map(|c| public_to_address(&c.pubkey)).collect())
}

fn release_jailed_prisoners(state: &mut TopLevelState, current_term: u64) -> StateResult<Vec<Address>> {
    let mut jailed = Jail::load_from_state(&state)?;
    let released = jailed.drain_released_prisoners(current_term);
    for prisoner in &released {
        state.add_balance(&prisoner.address, prisoner.deposit)?;
        ctrace!(ENGINE, "on_term_close::released. prisoner: {}, deposit: {}", prisoner.address, prisoner.deposit);
    }
    jailed.save_to_state(state)?;
    Ok(released.into_iter().map(|p| p.address).collect())
}

pub fn jail(state: &mut TopLevelState, addresses: &[Address], custody_until: u64, kick_at: u64) -> StateResult<()> {
    if addresses.is_empty() {
        return Ok(())
    }
    let mut candidates = Candidates::load_from_state(state)?;
    let mut jail = Jail::load_from_state(state)?;

    for address in addresses {
        let candidate = candidates.remove(address).expect("There should be a candidate to jail");
        ctrace!(ENGINE, "on_term_close::jail. candidate: {}, deposit: {}", address, candidate.deposit);
        jail.add(candidate, custody_until, kick_at);
    }

    jail.save_to_state(state)?;
    candidates.save_to_state(state)?;
    Ok(())
}

pub fn ban(state: &mut TopLevelState, informant: &Public, criminal: Address) -> StateResult<()> {
    let mut banned = Banned::load_from_state(state)?;
    if banned.is_banned(&criminal) {
        return Err(RuntimeError::FailedToHandleCustomAction("Account is already banned".to_string()).into())
    }

    let mut candidates = Candidates::load_from_state(state)?;
    let mut jailed = Jail::load_from_state(state)?;
    let mut validators = Validators::load_from_state(state)?;

    let deposit = match (candidates.remove(&criminal), jailed.remove(&criminal)) {
        (Some(_), Some(_)) => unreachable!("A candidate that are jailed cannot exist"),
        (Some(candidate), _) => candidate.deposit,
        (_, Some(jailed)) => jailed.deposit,
        _ => 0,
    };
    // confiscate criminal's deposit and give the same deposit amount to the informant.
    state.add_balance(&public_to_address(informant), deposit)?;

    jailed.remove(&criminal);
    banned.add(criminal);
    validators.remove(&criminal);

    jailed.save_to_state(state)?;
    banned.save_to_state(state)?;
    candidates.save_to_state(state)?;
    validators.save_to_state(state)?;

    // Revert delegations
    revert_delegations(state, &[criminal])?;

    Ok(())
}

fn revert_delegations(state: &mut TopLevelState, reverted_delegatees: &[Address]) -> StateResult<()> {
    // Stakeholders list isn't changed while reverting.

    let stakeholders = Stakeholders::load_from_state(state)?;
    for stakeholder in stakeholders.iter() {
        let mut delegator = StakeAccount::load_from_state(state, stakeholder)?;
        let mut delegation = Delegation::load_from_state(state, stakeholder)?;

        for delegatee in reverted_delegatees {
            let quantity = delegation.get_quantity(delegatee);
            if quantity > 0 {
                delegation.subtract_quantity(*delegatee, quantity)?;
                delegator.add_balance(quantity)?;
                ctrace!(
                    ENGINE,
                    "revert_delegation delegator: {}, delegatee: {}, quantity: {}",
                    stakeholder,
                    delegatee,
                    quantity
                );
            }
        }
        delegation.save_to_state(state)?;
        delegator.save_to_state(state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::action_data::get_account_key;
    use super::*;

    use crate::consensus::stake::action_data::{get_delegation_key, Candidate, Prisoner};
    use cstate::tests::helpers;
    use cstate::TopStateView;
    use rlp::Encodable;

    fn metadata_for_election() -> TopLevelState {
        let mut state = helpers::get_temp_state_with_metadata();
        state.metadata().unwrap().unwrap().set_params(CommonParams::default_for_test());
        let mut params = CommonParams::default_for_test();
        params.set_dynamic_validator_params_for_test(30, 10, 3, 20, 30, 4, 1000, 10000, 100);
        assert_eq!(Ok(()), state.update_params(0, params));
        state
    }

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
        let delegatee_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
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
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 100,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
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
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

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
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn delegate_too_much() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 200,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn can_transfer_within_non_delegated_tokens() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());
    }

    #[test]
    fn cannot_transfer_over_non_delegated_tokens() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        let action = Action::TransferCCS {
            address: delegatee,
            quantity: 100,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn can_revoke_delegated_tokens() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 20,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50 + 20);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 50 - 20);
    }

    #[test]
    fn cannot_revoke_more_than_delegated_tokens() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 70,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&delegatee), 50);
    }

    #[test]
    fn revoke_all_should_clear_state() {
        let delegatee_pubkey = Public::random();
        let delegatee = public_to_address(&delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegatee, 100);
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &delegatee, &delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Revoke {
            address: delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100);
        assert_eq!(state.action_data(&get_delegation_key(&delegator)).unwrap(), None);
    }

    #[test]
    fn can_redelegate_tokens() {
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);
        let next_delegatee_pubkey = Public::random();
        let next_delegatee = public_to_address(&next_delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();
        self_nominate(&mut state, &next_delegatee, &next_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee,
            quantity: 20,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50);
        assert_eq!(delegation.iter().count(), 2);
        assert_eq!(delegation.get_quantity(&prev_delegatee), 50 - 20);
        assert_eq!(delegation.get_quantity(&next_delegatee), 20);
    }

    #[test]
    fn cannot_redelegate_more_than_delegated_tokens() {
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);
        let next_delegatee_pubkey = Public::random();
        let next_delegatee = public_to_address(&next_delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();
        self_nominate(&mut state, &next_delegatee, &next_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee,
            quantity: 70,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 100 - 50);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&prev_delegatee), 50);
        assert_eq!(delegation.get_quantity(&next_delegatee), 0);
    }

    #[test]
    fn redelegate_all_should_clear_state() {
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);
        let next_delegatee_pubkey = Public::random();
        let next_delegatee = public_to_address(&next_delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();
        self_nominate(&mut state, &next_delegatee, &next_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert_eq!(Ok(()), result);

        let delegator_account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegator_account.balance, 50);
        assert_eq!(delegation.iter().count(), 1);
        assert_eq!(delegation.get_quantity(&prev_delegatee), 0);
        assert_eq!(delegation.get_quantity(&next_delegatee), 50);
    }

    #[test]
    fn redelegate_only_to_candidate() {
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);
        let next_delegatee_pubkey = Public::random();
        let next_delegatee = public_to_address(&next_delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_ok());

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee,
            quantity: 50,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn cannot_redelegate_to_banned_account() {
        let informant_pubkey = Public::random();
        let criminal_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let criminal = public_to_address(&criminal_pubkey);
        let delegator = public_to_address(&delegator_pubkey);
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&criminal, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();
        self_nominate(&mut state, &criminal, &criminal_pubkey, 100, 0, 10, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: criminal,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();
        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 2);

        assert_eq!(Ok(()), ban(&mut state, &informant_pubkey, criminal));

        let banned = Banned::load_from_state(&state).unwrap();
        assert!(banned.is_banned(&criminal));

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 1);

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee: criminal,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn cannot_redelegate_to_jailed_account() {
        let jail_pubkey = Public::random();
        let jail_address = public_to_address(&jail_pubkey);
        let prev_delegatee_pubkey = Public::random();
        let prev_delegatee = public_to_address(&prev_delegatee_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&jail_address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();
        self_nominate(&mut state, &prev_delegatee, &prev_delegatee_pubkey, 0, 0, 10, b"".to_vec()).unwrap();

        let deposit = 200;
        self_nominate(&mut state, &jail_address, &jail_pubkey, deposit, 0, 5, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address: prev_delegatee,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 2);

        let custody_until = 10;
        let released_at = 20;
        let result = jail(&mut state, &[jail_address], custody_until, released_at);
        assert!(result.is_ok());

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 1);

        let action = Action::Redelegate {
            prev_delegatee,
            next_delegatee: jail_address,
            quantity: 40,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn self_nominate_deposit_test() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let result = self_nominate(&mut state, &address, &address_pubkey, 0, 0, 5, b"metadata1".to_vec());
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 1000);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                pubkey: address_pubkey,
                deposit: 0,
                nomination_ends_at: 5,
                metadata: b"metadata1".to_vec(),
            }),
            "nomination_ends_at should be updated even if candidate deposits 0"
        );

        let result = self_nominate(&mut state, &address, &address_pubkey, 200, 0, 10, b"metadata2".to_vec());
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                pubkey: address_pubkey,
                deposit: 200,
                nomination_ends_at: 10,
                metadata: b"metadata2".to_vec(),
            })
        );

        let result = self_nominate(&mut state, &address, &address_pubkey, 0, 0, 15, b"metadata3".to_vec());
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800);
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                pubkey: address_pubkey,
                deposit: 200,
                nomination_ends_at: 15,
                metadata: b"metadata3".to_vec(),
            }),
            "nomination_ends_at should be updated even if candidate deposits 0"
        );
    }

    #[test]
    fn self_nominate_fail_with_insufficient_balance() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let result = self_nominate(&mut state, &address, &address_pubkey, 2000, 0, 5, b"".to_vec());
        assert!(result.is_err(), "Cannot self-nominate without a sufficient balance");
    }

    fn increase_term_id_until(state: &mut TopLevelState, term_id: u64) {
        let mut block_num = state.metadata().unwrap().unwrap().last_term_finished_block_num() + 1;
        while state.metadata().unwrap().unwrap().current_term_id() != term_id {
            assert_eq!(Ok(()), state.increase_term_id(block_num));
            block_num += 1;
        }
    }

    #[test]
    fn self_nominate_returns_deposits_after_expiration() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = metadata_for_election();
        increase_term_id_until(&mut state, 29);
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        self_nominate(&mut state, &address, &address_pubkey, 200, 0, 30, b"".to_vec()).unwrap();

        let result = on_term_close(&mut state, pseudo_term_to_block_num_calculator(29), &[]);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 800, "Should keep nomination before expiration");
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                pubkey: address_pubkey,
                deposit: 200,
                nomination_ends_at: 30,
                metadata: b"".to_vec(),
            }),
            "Keep deposit before expiration",
        );

        let result = on_term_close(&mut state, pseudo_term_to_block_num_calculator(30), &[]);
        assert_eq!(result, Ok(()));

        assert_eq!(state.balance(&address).unwrap(), 1000, "Return deposit after expiration");
        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.get_candidate(&address), None, "Removed from candidates after expiration");
    }

    #[test]
    fn self_nominate_reverts_delegations_after_expiration() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);
        let delegator_pubkey = Public::random();
        let delegator = public_to_address(&address_pubkey);

        let mut state = metadata_for_election();
        increase_term_id_until(&mut state, 29);
        state.add_balance(&address, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        self_nominate(&mut state, &address, &address_pubkey, 0, 0, 30, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        let result = on_term_close(&mut state, pseudo_term_to_block_num_calculator(29), &[]);
        assert_eq!(result, Ok(()));

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100 - 40);
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 40, "Should keep delegation before expiration");

        let result = on_term_close(&mut state, pseudo_term_to_block_num_calculator(30), &[]);
        assert_eq!(result, Ok(()));

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100);
        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 0, "Should revert before expiration");
    }

    #[test]
    fn jail_candidate() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, 5, b"".to_vec()).unwrap();

        let custody_until = 10;
        let released_at = 20;
        let result = jail(&mut state, &[address], custody_until, released_at);
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
                released_at,
            }),
            "The candidate become a prisoner"
        );

        assert_eq!(state.balance(&address).unwrap(), 1000 - deposit, "Deposited ccs is temporarily unavailable");
    }

    #[test]
    fn cannot_self_nominate_while_custody() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = metadata_for_election();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, nominate_expire, b"".to_vec()).unwrap();
        jail(&mut state, &[address], custody_until, released_at).unwrap();

        for current_term in 0..=custody_until {
            let result = self_nominate(
                &mut state,
                &address,
                &address_pubkey,
                0,
                current_term,
                current_term + nominate_expire,
                b"".to_vec(),
            );
            assert!(
                result.is_err(),
                "Shouldn't nominate while current_term({}) <= custody_until({})",
                current_term,
                custody_until
            );
            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();
        }
    }

    #[test]
    fn can_self_nominate_after_custody() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = metadata_for_election();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, nominate_expire, b"metadata-before".to_vec())
            .unwrap();
        jail(&mut state, &[address], custody_until, released_at).unwrap();
        for current_term in 0..=custody_until {
            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();
        }

        let current_term = custody_until + 1;
        let additional_deposit = 123;
        let result = self_nominate(
            &mut state,
            &address,
            &address_pubkey,
            additional_deposit,
            current_term,
            current_term + nominate_expire,
            b"metadata-after".to_vec(),
        );
        assert!(result.is_ok());

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(
            candidates.get_candidate(&address),
            Some(&Candidate {
                deposit: deposit + additional_deposit,
                nomination_ends_at: current_term + nominate_expire,
                pubkey: address_pubkey,
                metadata: "metadata-after".into()
            }),
            "The prisoner is become a candidate",
        );

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(jail.get_prisoner(&address), None, "The prisoner is removed");

        assert_eq!(state.balance(&address).unwrap(), 1000 - deposit - additional_deposit, "Deposit is accumulated");
    }

    #[test]
    fn jail_released_after() {
        let address_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);

        let mut state = metadata_for_election();
        state.add_balance(&address, 1000).unwrap();

        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();

        // TODO: change with stake.execute()
        let deposit = 200;
        let nominate_expire = 5;
        let custody_until = 10;
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, nominate_expire, b"".to_vec()).unwrap();
        jail(&mut state, &[address], custody_until, released_at).unwrap();

        for current_term in 0..released_at {
            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();

            let candidates = Candidates::load_from_state(&state).unwrap();
            assert_eq!(candidates.get_candidate(&address), None);

            let jail = Jail::load_from_state(&state).unwrap();
            assert!(jail.get_prisoner(&address).is_some());
        }

        on_term_close(&mut state, pseudo_term_to_block_num_calculator(released_at), &[]).unwrap();

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.get_candidate(&address), None, "A prisoner should not become a candidate");

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(jail.get_prisoner(&address), None, "A prisoner should be released");

        assert_eq!(state.balance(&address).unwrap(), 1000, "Balance should be restored after being released");
    }

    #[test]
    fn cannot_delegate_until_released() {
        let address_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = metadata_for_election();
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
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, nominate_expire, b"".to_vec()).unwrap();
        jail(&mut state, &[address], custody_until, released_at).unwrap();

        for current_term in 0..=released_at {
            let action = Action::DelegateCCS {
                address,
                quantity: 1,
            };
            let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
            assert_ne!(Ok(()), result);

            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();
        }

        let action = Action::DelegateCCS {
            address,
            quantity: 1,
        };
        let result = stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn kick_reverts_delegations() {
        let address_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = metadata_for_election();
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
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, deposit, 0, nominate_expire, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        jail(&mut state, &[address], custody_until, released_at).unwrap();

        for current_term in 0..=released_at {
            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();
        }

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 0, "Delegation should be reverted");

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100, "Delegation should be reverted");
    }

    #[test]
    fn self_nomination_before_kick_preserves_delegations() {
        let address_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let address = public_to_address(&address_pubkey);
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = metadata_for_election();
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
        let released_at = 20;
        self_nominate(&mut state, &address, &address_pubkey, 0, 0, nominate_expire, b"".to_vec()).unwrap();

        let action = Action::DelegateCCS {
            address,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        jail(&mut state, &[address], custody_until, released_at).unwrap();

        for current_term in 0..custody_until {
            on_term_close(&mut state, pseudo_term_to_block_num_calculator(current_term), &[]).unwrap();
        }

        let current_term = custody_until + 1;
        let result = self_nominate(
            &mut state,
            &address,
            &address_pubkey,
            0,
            current_term,
            current_term + nominate_expire,
            b"".to_vec(),
        );
        assert!(result.is_ok());

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&address), 40, "Delegation should be preserved");

        let account = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account.balance, 100 - 40, "Delegation should be preserved");
    }

    #[test]
    fn test_ban() {
        let informant_pubkey = Public::random();
        let criminal_pubkey = Public::random();
        let delegator_pubkey = Public::random();
        let criminal = public_to_address(&criminal_pubkey);
        let delegator = public_to_address(&delegator_pubkey);

        let mut state = helpers::get_temp_state();
        state.add_balance(&criminal, 1000).unwrap();

        let stake = {
            let mut genesis_stakes = HashMap::new();
            genesis_stakes.insert(delegator, 100);
            Stake::new(genesis_stakes)
        };
        stake.init(&mut state).unwrap();

        let deposit = 100;
        self_nominate(&mut state, &criminal, &criminal_pubkey, deposit, 0, 10, b"".to_vec()).unwrap();
        let action = Action::DelegateCCS {
            address: criminal,
            quantity: 40,
        };
        stake.execute(&action.rlp_bytes(), &mut state, &delegator, &delegator_pubkey).unwrap();

        assert_eq!(Ok(()), ban(&mut state, &informant_pubkey, criminal));

        let banned = Banned::load_from_state(&state).unwrap();
        assert!(banned.is_banned(&criminal));

        let candidates = Candidates::load_from_state(&state).unwrap();
        assert_eq!(candidates.len(), 0);

        assert_eq!(state.balance(&criminal).unwrap(), 900, "Should lose deposit");

        let delegation = Delegation::load_from_state(&state, &delegator).unwrap();
        assert_eq!(delegation.get_quantity(&criminal), 0, "Delegation should be reverted");

        let account_delegator = StakeAccount::load_from_state(&state, &delegator).unwrap();
        assert_eq!(account_delegator.balance, 100, "Delegation should be reverted");
    }

    #[test]
    fn ban_should_remove_prisoner_from_jail() {
        let informant_pubkey = Public::random();
        let criminal_pubkey = Public::random();
        let criminal = public_to_address(&criminal_pubkey);

        let mut state = helpers::get_temp_state();
        let stake = Stake::new(HashMap::new());
        stake.init(&mut state).unwrap();
        assert_eq!(Ok(()), state.add_balance(&criminal, 100));

        let deposit = 10;
        self_nominate(&mut state, &criminal, &criminal_pubkey, deposit, 0, 10, b"".to_vec()).unwrap();
        let custody_until = 10;
        let released_at = 20;
        jail(&mut state, &[criminal], custody_until, released_at).unwrap();

        assert_eq!(Ok(()), ban(&mut state, &informant_pubkey, criminal));

        let jail = Jail::load_from_state(&state).unwrap();
        assert_eq!(jail.get_prisoner(&criminal), None, "Should be removed from the jail");
    }

    fn pseudo_term_to_block_num_calculator(term_id: u64) -> u64 {
        term_id * 10 + 1
    }
}
