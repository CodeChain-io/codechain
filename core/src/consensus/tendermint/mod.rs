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

mod backup;
mod chain_notify;
mod engine;
mod message;
mod network;
mod params;
pub mod types;
mod worker;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use crossbeam_channel as crossbeam;
use cstate::ActionHandler;
use ctimer::TimerToken;
use parking_lot::RwLock;
use primitives::H256;

use self::chain_notify::TendermintChainNotify;
pub use self::params::{TendermintParams, TimeoutParams};
use self::types::{Height, Step, View};
use super::stake;
use super::validator_set::ValidatorSet;
use crate::client::EngineClient;
use crate::codechain_machine::CodeChainMachine;
use ChainNotify;

/// Timer token representing the consensus step timeouts.
const ENGINE_TIMEOUT_TOKEN_NONCE_BASE: TimerToken = 23;
/// Timer token for empty proposal blocks.
const ENGINE_TIMEOUT_EMPTY_PROPOSAL: TimerToken = 22;
/// Timer token for broadcasting step state.
const ENGINE_TIMEOUT_BROADCAST_STEP_STATE: TimerToken = 21;

/// Unit: second
const ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL: i64 = 1;

pub type BlockHash = H256;

/// ConsensusEngine using `Tendermint` consensus algorithm
pub struct Tendermint {
    client: RwLock<Option<Weak<EngineClient>>>,
    extension_initializer: crossbeam::Sender<(crossbeam::Sender<network::Event>, Weak<EngineClient>)>,
    timeouts: TimeoutParams,
    join: Option<JoinHandle<()>>,
    quit_tendermint: crossbeam::Sender<()>,
    inner: crossbeam::Sender<worker::Event>,
    /// Set used to determine the current validators.
    validators: Arc<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
    /// codechain machine descriptor
    machine: Arc<CodeChainMachine>,
    /// Action handlers for this consensus method
    action_handlers: Vec<Arc<ActionHandler>>,
    /// Chain notify
    chain_notify: Arc<TendermintChainNotify>,
    has_signer: AtomicBool,
}

impl Drop for Tendermint {
    fn drop(&mut self) {
        self.quit_tendermint.send(()).unwrap();
        if let Some(handler) = self.join.take() {
            handler.join().unwrap();
        }
    }
}

impl Tendermint {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Arc<Self> {
        let stake = stake::Stake::new(our_params.genesis_stakes, Arc::clone(&our_params.validators));
        let timeouts = our_params.timeouts;
        let validators = Arc::clone(&our_params.validators);
        let machine = Arc::new(machine);

        let (join, extension_initializer, inner, quit_tendermint) = worker::spawn(our_params.validators);
        let action_handlers: Vec<Arc<ActionHandler>> = vec![Arc::new(stake)];
        let chain_notify = Arc::new(TendermintChainNotify::new(inner.clone()));

        Arc::new(Tendermint {
            client: Default::default(),
            extension_initializer,
            timeouts,
            join: Some(join),
            quit_tendermint,
            inner,
            validators,
            block_reward: our_params.block_reward,
            machine,
            action_handlers,
            chain_notify,
            has_signer: false.into(),
        })
    }
}

const SEAL_FIELDS: usize = 4;

#[cfg(test)]
mod tests {
    use ccrypto::blake256;
    use ckey::Address;
    use primitives::Bytes;

    use super::message::{message_info_rlp, VoteStep};
    use super::types::BitSet;
    use crate::account_provider::AccountProvider;
    use crate::block::{ClosedBlock, IsBlock, OpenBlock};
    use crate::client::TestBlockChainClient;
    use crate::consensus::{CodeChainEngine, EngineError, Seal};
    use crate::error::BlockError;
    use crate::error::Error;
    use crate::header::Header;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    /// Accounts inserted with "0" and "1" are validators. First proposer is "0".
    fn setup() -> (Scheme, Arc<AccountProvider>, Arc<TestBlockChainClient>) {
        let tap = AccountProvider::transient_provider();
        let scheme = Scheme::new_test_tendermint();
        let test = TestBlockChainClient::new_with_scheme(Scheme::new_test_tendermint());

        let test_client: Arc<TestBlockChainClient> = Arc::new(test);
        let engine_client = Arc::clone(&test_client) as Arc<EngineClient>;
        scheme.engine.register_client(Arc::downgrade(&engine_client));
        (scheme, tap, test_client)
    }

    fn propose_default(scheme: &Scheme, proposer: Address) -> (ClosedBlock, Vec<Bytes>) {
        let db = get_temp_state_db();
        let db = scheme.ensure_genesis_state(db).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(scheme.engine.as_ref(), db, &genesis_header, proposer, vec![], false).unwrap();
        let b = b.close(*genesis_header.transactions_root()).unwrap();
        if let Some(seal) = scheme.engine.generate_seal(b.block(), &genesis_header).seal_fields() {
            (b, seal)
        } else {
            panic!()
        }
    }

    fn insert_and_unlock(tap: &Arc<AccountProvider>, acc: &str) -> Address {
        let addr = tap.insert_account(blake256(acc).into(), &acc.into()).unwrap();
        tap.unlock_account_permanently(addr, acc.into()).unwrap();
        addr
    }

    fn insert_and_register(tap: &Arc<AccountProvider>, engine: &CodeChainEngine, acc: &str) -> Address {
        let addr = insert_and_unlock(tap, acc);
        engine.set_signer(tap.clone(), addr);
        addr
    }

    #[test]
    fn has_valid_metadata() {
        let engine = Scheme::new_test_tendermint().engine;
        assert!(!engine.name().is_empty());
    }

    #[test]
    #[ignore] // FIXME
    fn verification_fails_on_short_seal() {
        let engine = Scheme::new_test_tendermint().engine;
        let header = Header::default();

        let verify_result = engine.verify_block_basic(&header);

        match verify_result {
            Err(Error::Block(BlockError::InvalidSealArity(_))) => {}
            Err(err) => {
                panic!("should be block seal-arity mismatch error (got {:?})", err);
            }
            _ => {
                panic!("Should be error, got Ok");
            }
        }
    }

    #[test]
    #[ignore] // FIXME
    fn generate_seal() {
        let (scheme, tap, _c) = setup();

        let proposer = insert_and_register(&tap, scheme.engine.as_ref(), "1");

        let (b, seal) = propose_default(&scheme, proposer);
        assert!(b.lock().try_seal(scheme.engine.as_ref(), seal).is_ok());
    }

    #[test]
    #[ignore] // FIXME
    fn parent_block_existence_checking() {
        let (spec, tap, _c) = setup();
        let engine = spec.engine;

        let mut header = Header::default();
        header.set_number(4);
        let proposer = insert_and_unlock(&tap, "0");
        header.set_author(proposer);
        header.set_parent_hash(Default::default());

        let vote_info = message_info_rlp(VoteStep::new(3, 0, Step::Precommit), Some(*header.parent_hash()));
        let signature2 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature2],
            precommit_bitset: BitSet::new_with_indices(&[2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        println!(".....");
        assert!(engine.verify_block_external(&header).is_err());
    }

    #[test]
    #[ignore] // FIXME
    fn seal_signatures_checking() {
        let (spec, tap, c) = setup();
        let engine = spec.engine;

        let validator0 = insert_and_unlock(&tap, "0");
        let validator1 = insert_and_unlock(&tap, "1");
        let validator2 = insert_and_unlock(&tap, "2");
        let validator3 = insert_and_unlock(&tap, "3");

        let block1_hash = c.add_block_with_author(Some(validator1), 1, 1);

        let mut header = Header::default();
        header.set_number(2);
        let proposer = validator2;
        header.set_author(proposer);
        header.set_parent_hash(block1_hash);

        let vote_info = message_info_rlp(VoteStep::new(1, 0, Step::Precommit), Some(*header.parent_hash()));
        let signature2 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature2],
            precommit_bitset: BitSet::new_with_indices(&[2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // One good signature is not enough.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::BadSealFieldSize(_))) => {}
            _ => panic!(),
        }

        let voter = validator3;
        let signature3 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();
        let voter = validator0;
        let signature0 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature2, signature3],
            precommit_bitset: BitSet::new_with_indices(&[0, 2, 3]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        assert!(engine.verify_block_external(&header).is_ok());

        let bad_voter = insert_and_unlock(&tap, "101");
        let bad_signature = tap.get_account(&bad_voter, None).unwrap().sign_schnorr(&blake256(vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature2, bad_signature],
            precommit_bitset: BitSet::new_with_indices(&[0, 2, 3]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // Two good and one bad signature.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::BlockNotAuthorized(_))) => {}
            _ => panic!(),
        };
        engine.stop();
    }
}
