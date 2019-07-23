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

mod params;

use std::sync::{Arc, Weak};

use ckey::{public_to_address, recover, Address, Signature};
use ctypes::{CommonParams, Header};
use parking_lot::RwLock;

use self::params::SimplePoAParams;
use super::signer::EngineSigner;
use super::validator_set::validator_list::RoundRobinValidator;
use super::validator_set::ValidatorSet;
use super::{ConsensusEngine, EngineError, Seal};
use crate::account_provider::AccountProvider;
use crate::block::ExecutedBlock;
use crate::client::ConsensusClient;
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::error::{BlockError, Error};

pub struct SimplePoA {
    machine: CodeChainMachine,
    signer: RwLock<EngineSigner>,
    validators: Box<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
}

impl SimplePoA {
    /// Create a new instance of SimplePoA engine
    pub fn new(params: SimplePoAParams, machine: CodeChainMachine) -> Self {
        SimplePoA {
            machine,
            signer: Default::default(),
            // If you want to change the type of validator set, please fix possible_authors first.
            validators: Box::new(RoundRobinValidator::new(params.validators)),
            block_reward: params.block_reward,
        }
    }
}

fn verify_external(header: &Header, validators: &ValidatorSet) -> Result<(), Error> {
    use rlp::UntrustedRlp;

    // Check if the signature belongs to a validator, can depend on parent state.
    let sig = UntrustedRlp::new(&header.seal()[0]).as_val::<Signature>()?;
    let signer = public_to_address(&recover(&sig, &header.bare_hash())?);

    if *header.author() != signer {
        return Err(EngineError::BlockNotAuthorized(*header.author()).into())
    }

    if validators.contains_address(header.parent_hash(), &signer) {
        Ok(())
    } else {
        Err(BlockError::InvalidSeal.into())
    }
}

impl ConsensusEngine for SimplePoA {
    fn name(&self) -> &str {
        "SimplePoA"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine
    }

    // One field - the signature
    fn seal_fields(&self, _header: &Header) -> usize {
        1
    }

    fn seals_internally(&self) -> Option<bool> {
        Some(self.signer.read().is_some())
    }

    fn engine_type(&self) -> EngineType {
        EngineType::PoA
    }

    /// Attempt to seal the block internally.
    fn generate_seal(&self, block: &ExecutedBlock, _parent: &Header) -> Seal {
        let header = block.header();
        let author = header.author();
        if self.validators.contains_address(header.parent_hash(), author) {
            // account should be permanently unlocked, otherwise sealing will fail
            if let Ok(signature) = self.signer.read().sign(header.bare_hash()) {
                return Seal::SimplePoA(signature)
            } else {
                ctrace!(ENGINE, "generate_seal: FAIL: accounts secret key unavailable");
            }
        }
        Seal::None
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        verify_external(header, &*self.validators)
    }

    fn on_close_block(
        &self,
        block: &mut ExecutedBlock,
        _parent_header: &Header,
        _parent_common_params: &CommonParams,
        _term_common_params: Option<&CommonParams>,
    ) -> Result<(), Error> {
        let author = *block.header().author();
        let total_reward = self.block_reward(block.header().number())
            + self.block_fee(Box::new(block.transactions().to_owned().into_iter().map(Into::into)));
        self.machine.add_balance(block, &author, total_reward)
    }

    fn register_client(&self, client: Weak<ConsensusClient>) {
        self.validators.register_client(client);
    }

    /// Register an account which signs consensus messages.
    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address) {
        self.signer.write().set(ap, address);
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn possible_authors(&self, _block_number: Option<u64>) -> Result<Option<Vec<Address>>, EngineError> {
        // TODO: It works because the round robin validator doesn't use the parent hash.
        let parent = 0.into();
        Ok(Some(self.validators.addresses(&parent)))
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{IsBlock, OpenBlock};
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    #[test]
    fn has_valid_metadata() {
        let engine = Scheme::new_test_simple_poa().engine;
        assert!(!engine.name().is_empty());
    }

    #[test]
    fn fail_to_verify_signature_when_seal_is_invalid() {
        let engine = Scheme::new_test_simple_poa().engine;
        let mut header: Header = Header::default();
        header.set_seal(vec![::rlp::encode(&Signature::default()).into_vec()]);

        let verify_result = engine.verify_block_external(&header);
        assert!(verify_result.is_err());
    }

    #[test]
    fn generate_seal() {
        let scheme = Scheme::new_test_simple_poa();
        let engine = &*scheme.engine;
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(engine, db, &genesis_header, Default::default(), vec![]).unwrap();
        let parent_common_params = CommonParams::default_for_test();
        let term_common_params = CommonParams::default_for_test();
        let b = b.close_and_lock(&genesis_header, &parent_common_params, Some(&term_common_params)).unwrap();
        if let Some(seal) = engine.generate_seal(b.block(), &genesis_header).seal_fields() {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn seals_internally() {
        let engine = Scheme::new_test_simple_poa().engine;
        assert!(!engine.seals_internally().unwrap());
    }
}
