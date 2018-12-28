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

mod params;

use std::sync::{Arc, Weak};

use ckey::{public_to_address, recover, Address, Password, Public, SchnorrSignature, Signature};
use ctypes::machine::WithBalances;
use parking_lot::RwLock;
use primitives::H256;

use self::params::SimplePoAParams;
use super::signer::EngineSigner;
use super::validator_set::validator_list::ValidatorList;
use super::validator_set::ValidatorSet;
use super::{ConsensusEngine, ConstructedVerifier, EngineError, Seal};
use crate::account_provider::AccountProvider;
use crate::block::{ExecutedBlock, IsBlock};
use crate::client::EngineClient;
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::error::{BlockError, Error};
use crate::header::Header;

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
            validators: Box::new(ValidatorList::new(params.validators)),
            block_reward: params.block_reward,
        }
    }
}

struct EpochVerifier {
    list: ValidatorList,
}

impl super::epoch::EpochVerifier<CodeChainMachine> for EpochVerifier {
    fn verify_light(&self, header: &Header) -> Result<(), Error> {
        verify_external(header, &self.list)
    }
}

fn verify_external(header: &Header, validators: &ValidatorSet) -> Result<(), Error> {
    use rlp::UntrustedRlp;

    // Check if the signature belongs to a validator, can depend on parent state.
    let sig = UntrustedRlp::new(&header.seal()[0]).as_val::<Signature>()?;
    let signer = public_to_address(&recover(&sig, &header.bare_hash())?);

    if *header.author() != signer {
        return Err(EngineError::NotAuthorized(*header.author()).into())
    }

    if validators.contains_address(header.parent_hash(), &signer) {
        Ok(())
    } else {
        Err(BlockError::InvalidSeal.into())
    }
}

impl ConsensusEngine<CodeChainMachine> for SimplePoA {
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
        EngineType::InternalSealing
    }

    /// Attempt to seal the block internally.
    fn generate_seal(&self, block: &ExecutedBlock, _parent: &Header) -> Seal {
        let header = block.header();
        let author = header.author();
        if self.validators.contains_address(header.parent_hash(), author) {
            // account should be permanently unlocked, otherwise sealing will fail
            if let Ok(signature) = self.sign(header.bare_hash()) {
                return Seal::SimplePoA(signature)
            } else {
                ctrace!(ENGINE, "generate_seal: FAIL: accounts secret key unavailable");
            }
        }
        Seal::None
    }

    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        verify_external(header, &*self.validators)
    }

    fn genesis_epoch_data(&self, header: &Header) -> Result<Vec<u8>, String> {
        self.validators.genesis_epoch_data(header)
    }

    #[cfg(not(test))]
    fn signals_epoch_end(&self, _header: &Header) -> super::EpochChange {
        // don't bother signalling even though a contract might try.
        super::EpochChange::No
    }

    #[cfg(test)]
    fn signals_epoch_end(&self, header: &Header) -> super::EpochChange {
        // in test mode, always signal even though they don't be finalized.
        let first = header.number() == 0;
        self.validators.signals_epoch_end(first, header)
    }

    fn is_epoch_end(
        &self,
        chain_head: &Header,
        _chain: &super::Headers<Header>,
        _transition_store: &super::PendingTransitionStore,
    ) -> Option<Vec<u8>> {
        let first = chain_head.number() == 0;

        // finality never occurs so only apply immediate transitions.
        self.validators.is_epoch_end(first, chain_head)
    }

    fn epoch_verifier<'a>(&self, header: &Header, proof: &'a [u8]) -> ConstructedVerifier<'a, CodeChainMachine> {
        let first = header.number() == 0;

        match self.validators.epoch_set(first, &self.machine, header.number(), proof) {
            Ok((list, finalize)) => {
                let verifier = Box::new(EpochVerifier {
                    list,
                });

                // our epoch verifier will ensure no unverified verifier is ever verified.
                match finalize {
                    Some(finalize) => ConstructedVerifier::Unconfirmed(verifier, proof, finalize),
                    None => ConstructedVerifier::Trusted(verifier),
                }
            }
            Err(e) => ConstructedVerifier::Err(e),
        }
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let author = *block.header().author();
        let total_reward = self.block_reward(block.header().number())
            + self.block_fee(Box::new(block.parcels().to_owned().into_iter().map(Into::into)));
        self.machine.add_balance(block, &author, total_reward)
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        self.validators.register_client(client);
    }

    /// Register an account which signs consensus messages.
    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address, password: Option<Password>) {
        self.signer.write().set(ap, address, password);
    }

    fn sign(&self, hash: H256) -> Result<SchnorrSignature, Error> {
        self.signer.read().sign(hash).map_err(Into::into)
    }

    fn signer_public(&self) -> Option<Public> {
        self.signer.read().public().cloned()
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::block::OpenBlock;
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
        let b = OpenBlock::try_new(engine, db, &genesis_header, Default::default(), vec![], false).unwrap();
        let parent_parcels_root = *genesis_header.parcels_root();
        let parent_invoices_root = *genesis_header.invoices_root();
        let b = b.close_and_lock(parent_parcels_root, parent_invoices_root).unwrap();
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
