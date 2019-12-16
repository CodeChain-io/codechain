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

use std::collections::btree_map::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::iter::Iterator;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Weak};

use ckey::{public_to_address, Address};
use cnetwork::NetworkService;
use crossbeam_channel as crossbeam;
use cstate::{ActionHandler, TopStateView};
use ctypes::{BlockHash, CommonParams, Header};
use num_rational::Ratio;

use super::super::stake;
use super::super::{ConsensusEngine, EngineError, Seal};
use super::network::TendermintExtension;
pub use super::params::{TendermintParams, TimeoutParams};
use super::types::TendermintSealView;
use super::worker;
use super::{ChainNotify, Tendermint, SEAL_FIELDS};
use crate::account_provider::AccountProvider;
use crate::block::*;
use crate::client::{Client, ConsensusClient};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::tendermint::params::TimeGapParams;
use crate::consensus::{EngineType, ValidatorSet};
use crate::encoded;
use crate::error::Error;
use crate::views::HeaderView;
use crate::BlockId;
use rlp::Encodable;

#[derive(Default)]
struct WorkInfo {
    proposed: usize,
    missed: usize,
    signed: u64,
}

impl ConsensusEngine for Tendermint {
    fn name(&self) -> &str {
        "Tendermint"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine.as_ref()
    }

    /// (consensus view, proposal signature, authority signatures)
    fn seal_fields(&self, _header: &Header) -> usize {
        SEAL_FIELDS
    }

    /// Should this node participate.
    fn seals_internally(&self) -> Option<bool> {
        Some(self.has_signer.load(AtomicOrdering::SeqCst))
    }

    fn engine_type(&self) -> EngineType {
        EngineType::PBFT
    }

    /// Attempt to seal generate a proposal seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which case
    /// `Seal::None` will be returned.
    fn generate_seal(&self, _block: Option<&ExecutedBlock>, parent: &Header) -> Seal {
        let (result, receiver) = crossbeam::bounded(1);
        let parent_hash = parent.hash();
        self.inner
            .send(worker::Event::GenerateSeal {
                block_number: parent.number() + 1,
                parent_hash,
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    /// Called when the node is the leader and a proposal block is generated from the miner.
    /// This writes the proposal information and go to the prevote step.
    fn proposal_generated(&self, sealed_block: &SealedBlock) {
        self.inner.send(worker::Event::ProposalGenerated(Box::from(sealed_block.clone()))).unwrap();
    }

    fn verify_header_basic(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::VerifyHeaderBasic {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::VerifyBlockExternal {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn populate_from_parent(&self, header: &mut Header, _parent: &Header) {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::CalculateScore {
                block_number: header.number(),
                result,
            })
            .unwrap();
        let score = receiver.recv().unwrap();
        header.set_score(score);
    }

    /// Equivalent to a timeout: to be used for tests.
    fn on_timeout(&self, token: usize) {
        self.inner.send(worker::Event::OnTimeout(token)).unwrap();
    }

    fn stop(&self) {}

    fn on_close_block(
        &self,
        block: &mut ExecutedBlock,
        term_common_params: Option<&CommonParams>,
    ) -> Result<(), Error> {
        let client = self.client().ok_or(EngineError::CannotOpenBlock)?;

        let parent_hash = *block.header().parent_hash();
        let parent = client.block_header(&parent_hash.into()).expect("Parent header must exist").decode();
        let parent_common_params = client.common_params(parent_hash.into()).expect("CommonParams of parent must exist");
        let author = *block.header().author();
        let block_number = block.header().number();

        let (total_reward, total_min_fee) = {
            let transactions = block.transactions();
            let block_reward = self.block_reward(block_number);
            let total_min_fee: u64 = transactions.iter().map(|tx| tx.fee).sum();
            let min_fee =
                transactions.iter().map(|tx| CodeChainMachine::min_cost(&parent_common_params, &tx.action)).sum();
            (block_reward + total_min_fee, min_fee)
        };
        assert!(total_reward >= total_min_fee, "{} >= {}", total_reward, total_min_fee);
        let stakes = stake::get_stakes(block.state()).expect("Cannot get Stake status");

        let mut distributor = stake::fee_distribute(total_min_fee, &stakes);
        for (address, share) in &mut distributor {
            self.machine.add_balance(block, &address, share)?
        }

        let block_author_reward = total_reward - total_min_fee + distributor.remaining_fee();

        let metadata = block.state().metadata()?.expect("Metadata must exist");
        let term = metadata.current_term_id();
        let term_seconds = match term {
            0 => parent_common_params.term_seconds(),
            _ => term_common_params.expect("TermCommonParams should exist").term_seconds(),
        };

        match term {
            0 => {
                self.machine.add_balance(block, &author, block_author_reward)?;
            }
            _ => {
                stake::add_intermediate_rewards(block.state_mut(), author, block_author_reward)?;
            }
        }

        if !is_term_changed(block.header(), &parent, term_seconds) {
            return Ok(())
        }

        let inactive_validators = match term {
            0 => Vec::new(),
            _ => {
                let rewards = stake::drain_previous_rewards(block.state_mut())?;
                let start_of_the_current_term = metadata.last_term_finished_block_num() + 1;

                if term > 1 {
                    let banned = stake::Banned::load_from_state(block.state())?;
                    let start_of_the_current_term_header = if block_number == start_of_the_current_term {
                        encoded::Header::new(block.header().clone().rlp_bytes().to_vec())
                    } else {
                        client.block_header(&start_of_the_current_term.into()).unwrap()
                    };

                    let pending_rewards = calculate_pending_rewards_of_the_term(
                        &*client,
                        &*self.validators,
                        rewards,
                        start_of_the_current_term_header,
                        &banned,
                    )?;

                    for (address, reward) in pending_rewards {
                        self.machine.add_balance(block, &address, reward)?;
                    }
                }

                stake::move_current_to_previous_intermediate_rewards(block.state_mut())?;

                let validators = stake::Validators::load_from_state(block.state())?
                    .into_iter()
                    .map(|val| public_to_address(val.pubkey()))
                    .collect();
                inactive_validators(&*client, start_of_the_current_term, block.header(), validators)
            }
        };

        stake::on_term_close(block.state_mut(), block_number, &inactive_validators)?;

        Ok(())
    }

    fn register_client(&self, client: Weak<dyn ConsensusClient>) {
        *self.client.write() = Some(Weak::clone(&client));
        self.stake.register_resources(client, Arc::downgrade(&self.validators));
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::IsProposal {
                block_number: header.number(),
                block_hash: header.hash(),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address) {
        self.has_signer.store(true, AtomicOrdering::SeqCst);
        self.inner
            .send(worker::Event::SetSigner {
                ap,
                address,
            })
            .unwrap();
    }

    fn register_network_extension_to_service(&self, service: &NetworkService) {
        let timeouts = self.timeouts;

        let inner = self.inner.clone();
        let extension = service.register_extension(move |api| TendermintExtension::new(inner, timeouts, api));
        let client = Arc::downgrade(&self.client().unwrap());
        self.extension_initializer.send((extension, client)).unwrap();

        let (result, receiver) = crossbeam::bounded(1);
        self.inner.send(worker::Event::Restore(result)).unwrap();
        receiver.recv().unwrap();
    }

    fn register_time_gap_config_to_worker(&self, time_gap_params: TimeGapParams) {
        self.external_params_initializer.send(time_gap_params).unwrap();
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn register_chain_notify(&self, client: &Client) {
        client.add_notify(Arc::downgrade(&self.chain_notify) as Weak<dyn ChainNotify>);
    }

    fn get_best_block_from_best_proposal_header(&self, header: &HeaderView) -> BlockHash {
        header.parent_hash()
    }


    fn can_change_canon_chain(
        &self,
        _new_header_hash: BlockHash,
        parent_hash_of_new_header: BlockHash,
        grandparent_hash_of_new_header: BlockHash,
        prev_best_hash: BlockHash,
    ) -> bool {
        parent_hash_of_new_header == prev_best_hash || grandparent_hash_of_new_header == prev_best_hash
    }

    fn action_handlers(&self) -> &[Arc<dyn ActionHandler>] {
        &self.action_handlers
    }

    fn possible_authors(&self, block_number: Option<u64>) -> Result<Option<Vec<Address>>, EngineError> {
        let client = self.client().ok_or(EngineError::CannotOpenBlock)?;
        let block_hash = match block_number {
            None => {
                client.block_header(&BlockId::Latest).expect("latest block must exist").hash() // the latest block
            }
            Some(block_number) => {
                assert_ne!(0, block_number);
                client.block_header(&(block_number - 1).into()).ok_or(EngineError::CannotOpenBlock)?.hash() // the parent of the given block number
            }
        };
        Ok(Some(self.validators.addresses(&block_hash)))
    }
}

fn is_term_changed(header: &Header, parent: &Header, term_seconds: u64) -> bool {
    if term_seconds == 0 {
        return false
    }

    let current_term_period = header.timestamp() / term_seconds;
    let parent_term_period = parent.timestamp() / term_seconds;

    current_term_period != parent_term_period
}

fn inactive_validators(
    client: &dyn ConsensusClient,
    start_of_the_current_term: u64,
    current_block: &Header,
    mut validators: HashSet<Address>,
) -> Vec<Address> {
    validators.remove(current_block.author());
    let hash = *current_block.parent_hash();
    let mut header = client.block_header(&hash.into()).expect("Header of the parent must exist");
    while start_of_the_current_term <= header.number() {
        validators.remove(&header.author());
        header = client.block_header(&header.parent_hash().into()).expect("Header of the parent must exist");
    }

    validators.into_iter().collect()
}

// Aggregate the validators' work info of a term
fn aggregate_work_info(
    chain: &dyn ConsensusClient,
    validators: &dyn ValidatorSet,
    start_of_the_next_term_header: encoded::Header,
) -> Result<HashMap<Address, WorkInfo>, Error> {
    let mut work_info = HashMap::<Address, WorkInfo>::new();

    let start_of_the_current_term = {
        let end_of_the_last_term =
            chain.last_term_finished_block_num((start_of_the_next_term_header.number() - 2).into()).unwrap();

        end_of_the_last_term + 1
    };
    let mut header = start_of_the_next_term_header;
    let mut parent_validators = {
        let parent_header = chain.block_header(&header.parent_hash().into()).unwrap();
        validators.addresses(&parent_header.parent_hash())
    };
    while start_of_the_current_term != header.number() {
        for index in TendermintSealView::new(&header.seal()).bitset()?.true_index_iter() {
            let signer = *parent_validators.get(index).expect("The seal must be the signature of the validator");
            work_info.entry(signer).or_default().signed += 1;
        }

        header = chain.block_header(&header.parent_hash().into()).unwrap();
        parent_validators = {
            // The seal of the current block has the signatures of the parent block.
            // It needs the hash of the grand parent block to find the validators of the parent block.
            let parent_header = chain.block_header(&header.parent_hash().into()).unwrap();
            validators.addresses(&parent_header.parent_hash())
        };

        let author = header.author();
        let info = work_info.entry(author).or_default();
        info.proposed += 1;
        info.missed += parent_validators.len() - TendermintSealView::new(&header.seal()).bitset()?.count();
    }

    Ok(work_info)
}

fn calculate_pending_rewards_of_the_term(
    chain: &dyn ConsensusClient,
    validators: &dyn ValidatorSet,
    rewards: BTreeMap<Address, u64>,
    start_of_the_next_term_header: encoded::Header,
    banned: &stake::Banned,
) -> Result<HashMap<Address, u64>, Error> {
    // XXX: It's okay because we don't have a plan to increasing the maximum number of validators.
    //      However, it's better to use the correct number.
    const MAX_NUM_OF_VALIDATORS: usize = 30;
    let work_info = aggregate_work_info(chain, validators, start_of_the_next_term_header)?;
    let mut pending_rewards = HashMap::<Address, u64>::with_capacity(MAX_NUM_OF_VALIDATORS);

    let mut reduced_rewards = 0;

    // Penalty disloyal validators
    let number_of_blocks_in_term: usize = work_info.values().map(|info| info.proposed).sum();
    for (address, intermediate_reward) in rewards {
        if banned.is_banned(&address) {
            reduced_rewards += intermediate_reward;
            continue
        }
        let number_of_signatures = work_info.get(&address).unwrap().signed;
        let final_block_rewards =
            final_rewards(intermediate_reward, number_of_signatures, u64::try_from(number_of_blocks_in_term).unwrap());
        reduced_rewards += intermediate_reward - final_block_rewards;
        pending_rewards.insert(address, final_block_rewards);
    }

    // Give additional rewards
    give_additional_rewards(reduced_rewards, work_info, |address, reward| {
        let prev = pending_rewards.entry(*address).or_default();
        *prev += reward;
        Ok(())
    })?;

    Ok(pending_rewards)
}

/// reward = floor(intermediate_rewards * (a * number_of_signatures / number_of_blocks_in_term + b) / 10)
fn final_rewards(intermediate_reward: u64, number_of_signatures: u64, number_of_blocks_in_term: u64) -> u64 {
    let (a, b) = if number_of_signatures * 3 >= number_of_blocks_in_term * 2 {
        // number_of_signatures / number_of_blocks_in_term >= 2 / 3
        // x * 3/10 + 7/10
        (3, 7)
    } else if number_of_signatures * 2 >= number_of_blocks_in_term {
        // number_of_signatures / number_of_blocks_in_term >= 1 / 2
        // x * 48/10 - 23/10
        (48, -23)
    } else if number_of_signatures * 3 >= number_of_blocks_in_term {
        // number_of_signatures / number_of_blocks_in_term >= 1 / 3
        // x * 6/10 - 2/10
        (6, -2)
    } else {
        // 1 / 3 > number_of_signatures / number_of_blocks_in_term
        // 0
        assert!(
            number_of_blocks_in_term > 3 * number_of_signatures,
            "number_of_signatures / number_of_blocks_in_term = {}",
            (number_of_signatures as f64) / (number_of_blocks_in_term as f64)
        );
        (0, 0)
    };
    let numerator = i128::from(intermediate_reward)
        * (a * i128::from(number_of_signatures) + b * i128::from(number_of_blocks_in_term));
    assert!(numerator >= 0);
    let denominator = 10 * i128::from(number_of_blocks_in_term);
    // Rust's division rounds towards zero.
    u64::try_from(numerator / denominator).unwrap()
}

fn give_additional_rewards<F: FnMut(&Address, u64) -> Result<(), Error>>(
    mut reduced_rewards: u64,
    work_info: HashMap<Address, WorkInfo>,
    mut f: F,
) -> Result<(), Error> {
    let sorted_validators = work_info
        .into_iter()
        .map(|(address, info)| (address, Ratio::new(info.missed, info.proposed)))
        // When one sees the Ratio crate's Order trait implementation, he/she can easily realize that the
        // comparing routine is erroneous. It inversely compares denominators when the numerators are the same.
        // That's problematic when it makes comparisons between ratios such as Ratio{numer: 0, denom:2} and Ratio{numer: 0, denom: 3}.
        // But Ratio::new() calls Ratio::reduce() from within so that the numerator and the denominator are
        // divided equally by the gcd. Therefore the comparison between above two instances would never happen and only
        // comparisons between the reduced ones like Ratio{numer:0, denom:1} and Ratio{numer:0, denom: 1} should occur.
        .fold(BTreeMap::<Ratio<usize>, Vec<Address>>::new(), |mut map, (address, average_missed)| {
            map.entry(average_missed).or_default().push(address);
            map
        });
    for validators in sorted_validators.values() {
        let reward = reduced_rewards / (u64::try_from(validators.len()).unwrap() + 1);
        if reward == 0 {
            break
        }
        for validator in validators {
            f(validator, reward)?;
            reduced_rewards -= reward;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use super::*;

    #[test]
    fn test_final_rewards() {
        let intermediate_reward = 1000;
        {
            let number_of_signatures = 300;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(1000, final_rewards);
        }

        {
            let number_of_signatures = 250;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(950, final_rewards);
        }

        {
            let number_of_signatures = 200;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(900, final_rewards);
        }

        {
            let number_of_signatures = 175;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(500, final_rewards);
        }

        {
            let number_of_signatures = 150;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(100, final_rewards);
        }

        {
            let number_of_signatures = 125;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(50, final_rewards);
        }

        {
            let number_of_signatures = 100;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(0, final_rewards);
        }

        {
            let number_of_signatures = 0;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(0, final_rewards);
        }
    }

    #[test]
    fn final_rewards_are_rounded_towards_zero() {
        let intermediate_reward = 4321;
        {
            let number_of_signatures = 300;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(4321, final_rewards);
        }

        {
            let number_of_signatures = 250;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(4104, final_rewards);
        }

        {
            let number_of_signatures = 200;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(3888, final_rewards);
        }

        {
            let number_of_signatures = 175;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(2160, final_rewards);
        }

        {
            let number_of_signatures = 150;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(432, final_rewards);
        }

        {
            let number_of_signatures = 125;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(216, final_rewards);
        }

        {
            let number_of_signatures = 100;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(0, final_rewards);
        }

        {
            let number_of_signatures = 0;
            let number_of_blocks_in_term = 300;
            let final_rewards = final_rewards(intermediate_reward, number_of_signatures, number_of_blocks_in_term);
            assert_eq!(0, final_rewards);
        }
    }

    #[test]
    fn test_additional_rewards() {
        let reduced_rewards = 100;
        let addr00 = Address::random();
        let addr10 = Address::random();
        let addr11 = Address::random();
        let addr12 = Address::random();
        let addr20 = Address::random();
        let addr21 = Address::random();
        let work_info = HashMap::from_iter(
            vec![
                (addr00, WorkInfo {
                    proposed: 30,
                    missed: 28,
                    signed: 0,
                }),
                (addr10, WorkInfo {
                    proposed: 60,
                    missed: 59,
                    signed: 0,
                }),
                (addr11, WorkInfo {
                    proposed: 120,
                    missed: 118,
                    signed: 0,
                }),
                (addr12, WorkInfo {
                    proposed: 120,
                    missed: 118,
                    signed: 0,
                }),
                (addr20, WorkInfo {
                    proposed: 60,
                    missed: 60,
                    signed: 0,
                }),
                (addr21, WorkInfo {
                    proposed: 120,
                    missed: 120,
                    signed: 0,
                }),
            ]
            .into_iter(),
        );

        let mut result = HashMap::with_capacity(7);
        give_additional_rewards(reduced_rewards, work_info, |address, reward| {
            assert_eq!(None, result.insert(*address, reward));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            result,
            HashMap::from_iter(
                vec![(addr00, 50), (addr10, 12), (addr11, 12), (addr12, 12), (addr20, 4), (addr21, 4)].into_iter()
            )
        );
    }
}
