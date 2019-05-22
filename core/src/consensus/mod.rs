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

mod blake_pow;
mod cuckoo;
mod null_engine;
mod signer;
mod simple_poa;
mod solo;
mod stake;
mod tendermint;
mod validator_set;
mod vote_collector;

pub use self::blake_pow::BlakePoW;
pub use self::cuckoo::Cuckoo;
pub use self::null_engine::NullEngine;
pub use self::simple_poa::SimplePoA;
pub use self::solo::Solo;
pub use self::tendermint::{Tendermint, TendermintParams, TimeGapParams};
pub use self::validator_set::validator_list::ValidatorList;
pub use self::validator_set::ValidatorSet;

use std::fmt;
use std::sync::{Arc, Weak};

use ckey::{Address, SchnorrSignature};
use cnetwork::NetworkService;
use cstate::ActionHandler;
use ctypes::errors::SyntaxError;
use ctypes::transaction::Action;
use ctypes::util::unexpected::{Mismatch, OutOfBounds};
use ctypes::CommonParams;
use primitives::{Bytes, H256, U256};

use self::tendermint::types::{BitSet, View};
use crate::account_provider::AccountProvider;
use crate::block::{ExecutedBlock, SealedBlock};
use crate::client::EngineClient;
use crate::codechain_machine::CodeChainMachine;
use crate::encoded;
use crate::error::Error;
use crate::header::Header;
use crate::transaction::UnverifiedTransaction;
use crate::views::HeaderView;
use Client;

pub enum Seal {
    Solo,
    SimplePoA(SchnorrSignature),
    Tendermint {
        prev_view: View,
        cur_view: View,
        precommits: Vec<SchnorrSignature>,
        precommit_bitset: BitSet,
    },
    None,
}

impl Seal {
    pub fn seal_fields(&self) -> Option<Vec<Bytes>> {
        match self {
            Seal::None => None,
            Seal::Solo => Some(Vec::new()),
            Seal::SimplePoA(signature) => Some(vec![::rlp::encode(signature).into_vec()]),
            Seal::Tendermint {
                prev_view,
                cur_view,
                precommits,
                precommit_bitset,
            } => Some(vec![
                ::rlp::encode(prev_view).into_vec(),
                ::rlp::encode(cur_view).into_vec(),
                ::rlp::encode_list(precommits).into_vec(),
                ::rlp::encode(precommit_bitset).into_vec(),
            ]),
        }
    }
}

/// Engine type.
#[derive(Debug, PartialEq, Eq)]
pub enum EngineType {
    PoA,
    PBFT,
    PoW,
    Solo,
}

impl EngineType {
    pub fn need_signer_key(&self) -> bool {
        match self {
            EngineType::PoA => true,
            EngineType::PBFT => true,
            EngineType::Solo => false,
            EngineType::PoW => false,
        }
    }

    pub fn ignore_reseal_min_period(&self) -> bool {
        match self {
            EngineType::PoA => false,
            EngineType::PBFT => true,
            EngineType::Solo => false,
            EngineType::PoW => false,
        }
    }

    pub fn ignore_reseal_on_transaction(&self) -> bool {
        match self {
            EngineType::PoA => false,
            EngineType::PBFT => true,
            EngineType::Solo => false,
            EngineType::PoW => false,
        }
    }
}

/// A consensus mechanism for the chain.
pub trait ConsensusEngine: Sync + Send {
    /// The name of this engine.
    fn name(&self) -> &str;

    /// Get access to the underlying state machine.
    fn machine(&self) -> &CodeChainMachine;

    /// The number of additional header fields required for this engine.
    fn seal_fields(&self, _header: &Header) -> usize {
        0
    }

    /// None means that it requires external input (e.g. PoW) to seal a block.
    /// Some(true) means the engine is currently prime for seal generation (i.e. node is the current validator).
    /// Some(false) means that the node might seal internally but is not qualified now.
    fn seals_internally(&self) -> Option<bool> {
        None
    }

    /// The type of this engine.
    fn engine_type(&self) -> EngineType;

    /// Attempt to seal the block internally.
    ///
    /// If `Some` is returned, then you get a valid seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which None will
    /// be returned.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn generate_seal(&self, _block: &ExecutedBlock, _parent: &Header) -> Seal {
        Seal::None
    }

    fn proposal_generated(&self, _sealed_block: &SealedBlock) {}

    /// Verify a locally-generated seal of a header.
    ///
    /// If this engine seals internally,
    /// no checks have to be done here, since all internally generated seals
    /// should be valid.
    ///
    /// Externally-generated seals (e.g. PoW) will need to be checked for validity.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Phase 1 quick block verification. Only does checks that are cheap. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_header_basic(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Phase 2 verification. Perform costly checks such as transaction signatures. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Phase 3 verification. Check block information against parent. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_family(&self, _header: &Header, _parent: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Phase 4 verification. Verify block header against potentially external data.
    /// Should only be called when `register_client` has been called previously.
    fn verify_block_external(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Populate a header's fields based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    fn populate_from_parent(&self, _header: &mut Header, _parent: &Header) {}

    /// Called when the step is not changed in time
    fn on_timeout(&self, _token: usize) {}

    /// Stops any services that the may hold the Engine and makes it safe to drop.
    fn stop(&self) {}

    /// Block transformation functions, after the transactions.
    fn on_close_block(&self, _block: &mut ExecutedBlock, _parent_common_params: &CommonParams) -> Result<(), Error> {
        Ok(())
    }

    /// Add Client which can be used for sealing, potentially querying the state and sending messages.
    fn register_client(&self, _client: Weak<EngineClient>) {}

    /// Handle any potential consensus messages;
    /// updating consensus state and potentially issuing a new one.
    fn handle_message(&self, _message: &[u8]) -> Result<(), EngineError> {
        Err(EngineError::UnexpectedMessage)
    }

    /// Find out if the block is a proposal block and should not be inserted into the DB.
    /// Takes a header of a fully verified block.
    fn is_proposal(&self, _verified_header: &Header) -> bool {
        false
    }

    /// Called when proposal block is verified.
    /// Consensus many hold the verified proposal block until it should be imported.
    fn on_verified_proposal(&self, _verified_block_data: encoded::Block) {}

    /// Register an account which signs consensus messages.
    fn set_signer(&self, _ap: Arc<AccountProvider>, _address: Address) {}

    fn register_network_extension_to_service(&self, _: &NetworkService) {}

    fn register_time_gap_config_to_worker(&self, _time_gap_params: TimeGapParams) {}

    fn score_to_target(&self, _score: &U256) -> U256 {
        U256::zero()
    }

    fn block_reward(&self, block_number: u64) -> u64;

    fn block_fee(&self, transactions: Box<Iterator<Item = UnverifiedTransaction>>) -> u64 {
        transactions.map(|tx| tx.fee).sum()
    }

    fn recommended_confirmation(&self) -> u32;

    fn register_chain_notify(&self, _: &Client) {}

    fn get_best_block_from_best_proposal_header(&self, header: &HeaderView) -> H256 {
        header.hash()
    }

    fn can_change_canon_chain(&self, _header: &HeaderView) -> bool {
        true
    }

    fn action_handlers(&self) -> &[Arc<ActionHandler>] {
        &[]
    }

    fn find_action_handler_for(&self, id: u64) -> Option<&ActionHandler> {
        self.action_handlers().iter().find(|handler| handler.handler_id() == id).map(AsRef::as_ref)
    }
}

/// Voting errors.
#[derive(Debug)]
pub enum EngineError {
    /// Precommit signatures or author field does not belong to an authority.
    BlockNotAuthorized(Address),
    /// The signature cannot be verified with the signer of the message.
    MessageWithInvalidSignature {
        height: u64,
        signer_index: usize,
        address: Address,
    },
    /// The vote for the future height couldn't be verified
    FutureMessage {
        future_height: u64,
        current_height: u64,
    },
    /// The validator on the given height and index is exist(index >= validator set size)
    ValidatorNotExist {
        height: u64,
        index: usize,
    },
    PrevBlockNotExist {
        height: u64,
    },
    /// The same author issued different votes at the same step.
    DoubleVote(Address),
    /// The received block is from an incorrect proposer.
    NotProposer(Mismatch<Address>),
    /// Message was not expected.
    UnexpectedMessage,
    /// Seal field has an unexpected size.
    BadSealFieldSize(OutOfBounds<usize>),
    /// Malformed consensus message.
    MalformedMessage(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::EngineError::*;
        let msg = match self {
            BlockNotAuthorized(address) => format!("Signer {} is not authorized.", address),
            MessageWithInvalidSignature {
                height,
                signer_index,
                address,
            } => format!("The {}th validator({}) on height {} is not authorized.", signer_index, address, height),
            FutureMessage {
                future_height,
                current_height,
            } => format!("The message is from height {} but the current height is {}", future_height, current_height),
            ValidatorNotExist {
                height,
                index,
            } => format!("The {}th validator on height {} does not exist. (out of bound)", index, height),
            PrevBlockNotExist {
                height,
            } => format!("The previous block of height {} does not exist.", height),
            DoubleVote(address) => format!("Author {} issued too many blocks.", address),
            NotProposer(mis) => format!("Author is not a current proposer: {}", mis),
            UnexpectedMessage => "This Engine should not be fed messages.".into(),
            BadSealFieldSize(oob) => format!("Seal field has an unexpected length: {}", oob),
            MalformedMessage(msg) => format!("Received malformed consensus message: {}", msg),
        };

        f.write_fmt(format_args!("Engine error ({})", msg))
    }
}

/// Common type alias for an engine coupled with an CodeChain-like state machine.
pub trait CodeChainEngine: ConsensusEngine {
    /// Additional verification for transactions in blocks.
    fn verify_transaction_with_params(
        &self,
        tx: &UnverifiedTransaction,
        common_params: &CommonParams,
    ) -> Result<(), Error> {
        if let Action::Custom {
            handler_id,
            bytes,
        } = &tx.action
        {
            let handler = self
                .find_action_handler_for(*handler_id)
                .ok_or_else(|| SyntaxError::InvalidCustomAction(format!("{} is an invalid handler id", handler_id)))?;
            handler.verify(bytes)?;
        }
        self.machine().verify_transaction_with_params(tx, common_params)
    }
}

// convenience wrappers for existing functions.
impl<T> CodeChainEngine for T where T: ConsensusEngine {}
