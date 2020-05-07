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
pub mod vote_collector;
mod vote_regression_checker;
mod worker;

use self::chain_notify::TendermintChainNotify;
pub use self::message::{ConsensusMessage, VoteOn, VoteStep};
pub use self::params::{TendermintParams, TimeGapParams, TimeoutParams};
pub use self::types::{Height, Step, View};
pub use super::{stake, ValidatorSet};
use crate::client::ConsensusClient;
use crate::codechain_machine::CodeChainMachine;
use crate::ChainNotify;
use crossbeam_channel as crossbeam;
use cstate::ActionHandler;
use ctimer::TimerToken;
use parking_lot::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

/// Timer token representing the consensus step timeouts.
const ENGINE_TIMEOUT_TOKEN_NONCE_BASE: TimerToken = 23;
/// Timer token for empty proposal blocks.
const ENGINE_TIMEOUT_EMPTY_PROPOSAL: TimerToken = 22;
/// Timer token for broadcasting step state.
const ENGINE_TIMEOUT_BROADCAST_STEP_STATE: TimerToken = 21;

/// Unit: second
const ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL: u64 = 1;

/// ConsensusEngine using `Tendermint` consensus algorithm
pub struct Tendermint {
    client: RwLock<Option<Weak<dyn ConsensusClient>>>,
    external_params_initializer: crossbeam::Sender<TimeGapParams>,
    extension_initializer: crossbeam::Sender<(crossbeam::Sender<network::Event>, Weak<dyn ConsensusClient>)>,
    timeouts: TimeoutParams,
    join: Option<JoinHandle<()>>,
    quit_tendermint: crossbeam::Sender<()>,
    inner: crossbeam::Sender<worker::Event>,
    validators: Arc<dyn ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
    /// codechain machine descriptor
    machine: Arc<CodeChainMachine>,
    /// Action handlers for this consensus method
    action_handlers: Vec<Arc<dyn ActionHandler>>,
    /// stake object to register client data later
    stake: Arc<stake::Stake>,
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
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Arc<Self> {
        let validators = Arc::clone(&our_params.validators);
        let stake = Arc::new(stake::Stake::new(our_params.genesis_stakes));
        let timeouts = our_params.timeouts;
        let machine = Arc::new(machine);

        let (join, external_params_initializer, extension_initializer, inner, quit_tendermint) =
            worker::spawn(our_params.validators);
        let action_handlers: Vec<Arc<dyn ActionHandler>> = vec![stake.clone()];
        let chain_notify = Arc::new(TendermintChainNotify::new(inner.clone()));

        Arc::new(Tendermint {
            client: Default::default(),
            external_params_initializer,
            extension_initializer,
            timeouts,
            join: Some(join),
            quit_tendermint,
            inner,
            validators,
            block_reward: our_params.block_reward,
            machine,
            action_handlers,
            stake,
            chain_notify,
            has_signer: false.into(),
        })
    }

    fn client(&self) -> Option<Arc<dyn ConsensusClient>> {
        self.client.read().as_ref()?.upgrade()
    }
}

const SEAL_FIELDS: usize = 4;

#[cfg(test)]
mod tests {
    use ccrypto::blake256;
    use ckey::{public_to_address, sign_schnorr, Address, KeyPair, Private};
    use ctypes::{CommonParams, Header};
    use primitives::Bytes;
    use std::str::FromStr;

    use super::super::BitSet;
    use super::message::VoteStep;
    use crate::account_provider::AccountProvider;
    use crate::block::{ClosedBlock, OpenBlock};
    use crate::client::TestBlockChainClient;
    use crate::consensus::{CodeChainEngine, Seal};
    use crate::error::BlockError;
    use crate::error::Error;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    /// Accounts inserted with "0" and "1" are validators. First proposer is "0".
    fn setup() -> (Scheme, Arc<AccountProvider>, Arc<TestBlockChainClient>) {
        let tap = AccountProvider::transient_provider();
        let scheme = Scheme::new_test_tendermint();
        let test = TestBlockChainClient::new_with_scheme(Scheme::new_test_tendermint());

        let test_client: Arc<TestBlockChainClient> = Arc::new(test);
        let consensus_client = Arc::clone(&test_client) as Arc<dyn ConsensusClient>;
        scheme.engine.register_client(Arc::downgrade(&consensus_client));
        (scheme, tap, test_client)
    }

    fn propose_default(scheme: &Scheme, proposer: Address) -> (ClosedBlock, Vec<Bytes>) {
        let db = get_temp_state_db();
        let db = scheme.ensure_genesis_state(db).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(scheme.engine.as_ref(), db, &genesis_header, proposer, vec![]).unwrap();
        let seal = scheme.engine.generate_seal(None, &genesis_header).seal_fields().unwrap();
        let term_common_params = CommonParams::default_for_test();
        let b = b.close(&genesis_header, Some(&term_common_params)).unwrap();
        (b, seal)
    }

    fn insert_and_unlock(tap: &Arc<AccountProvider>, acc: &str) -> Address {
        let addr = tap.insert_account(blake256(acc).into(), &acc.into()).unwrap();
        tap.unlock_account_permanently(addr, acc.into()).unwrap();
        addr
    }

    fn insert_and_register(tap: &Arc<AccountProvider>, engine: &dyn CodeChainEngine, acc: &str) -> Address {
        let addr = insert_and_unlock(tap, acc);
        engine.set_signer(tap.clone(), addr);
        addr
    }

    #[test]
    fn serialize_deserialize_test() {
        let key_pair = {
            let serialized_priv_key = "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd";
            let private_key = Private::from_str(&serialized_priv_key).unwrap();
            KeyPair::from_private(private_key).unwrap()
        };

        let mut header = Header::default();
        header.set_number(4);
        header.set_author(public_to_address(key_pair.public()));

        let precommit_bitset = {
            let mut bitset = BitSet::new();
            bitset.set(2);
            bitset
        };
        let signature = {
            let height = 3;
            let view = 0;
            let step = Step::Precommit;
            let vote_on = VoteOn {
                step: VoteStep::new(height, view, step),
                block_hash: Some(*header.parent_hash()),
            };
            sign_schnorr(key_pair.private(), &vote_on.hash()).unwrap()
        };
        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature],
            precommit_bitset,
        };
        header.set_seal(seal.seal_fields().unwrap());

        let encoded = rlp::encode(&header);
        let decoded: Header = rlp::decode(&encoded).unwrap();

        assert_eq!(header.hash(), decoded.hash());
    }

    #[test]
    #[ignore] // FIXME
    fn verification_fails_on_short_seal() {
        let engine = Scheme::new_test_tendermint().engine;
        let header = Header::default();

        let verify_result = engine.verify_header_basic(&header);

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

        let vote_on = VoteOn {
            step: VoteStep::new(3, 0, Step::Precommit),
            block_hash: Some(*header.parent_hash()),
        };
        let signature2 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&vote_on.hash()).unwrap();

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
}
