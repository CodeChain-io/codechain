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

use std::sync::Weak;

use ckeys::{Signature, Private, Network, public_to_address};
use ctypes::{Address, H256, H520};
use parking_lot::RwLock;

use super::{ConsensusEngine, EngineError, Seal, ConstructedVerifier};
use super::signer::EngineSigner;
use super::validator_set::ValidatorSet;
use super::validator_set::validator_list::ValidatorList;
use super::super::block::{ExecutedBlock, IsBlock};
use super::super::client::EngineClient;
use super::super::codechain_machine::CodeChainMachine;
use super::super::error::{BlockError, Error};
use super::super::header::Header;

pub struct SoloAuthority {
    machine: CodeChainMachine,
    signer: RwLock<EngineSigner>,
    validators: Box<ValidatorSet>,
}

impl SoloAuthority {
    /// Create a new instance of SoloAuthority engine
    pub fn new(machine: CodeChainMachine, validators: Box<ValidatorSet>) -> Self {
        SoloAuthority {
            machine,
            signer: Default::default(),
            validators,
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
    let sig = Signature(UntrustedRlp::new(&header.seal()[0]).as_val::<H520>()?.into());
    let signer = public_to_address(&sig.recover(&header.bare_hash())?);

    if *header.author() != signer {
        return Err(EngineError::NotAuthorized(header.author().clone()).into())
    }

    match validators.contains(header.parent_hash(), &signer) {
        false => Err(BlockError::InvalidSeal.into()),
        true => Ok(())
    }
}

impl ConsensusEngine<CodeChainMachine> for SoloAuthority {
    fn name(&self) -> &str { "SoloAuthority" }

    fn machine(&self) -> &CodeChainMachine { &self.machine }

    // One field - the signature
    fn seal_fields(&self, _header: &Header) -> usize { 1 }

    fn seals_internally(&self) -> Option<bool> {
        Some(self.signer.read().is_some())
    }

    /// Attempt to seal the block internally.
    fn generate_seal(&self, block: &ExecutedBlock, _parent: &Header) -> Seal {
        let header = block.header();
        let author = header.author();
        if self.validators.contains(header.parent_hash(), author) {
            // account should be permanently unlocked, otherwise sealing will fail
            if let Ok(signature) = self.sign(header.bare_hash()) {
                return Seal::Regular(vec![::rlp::encode(&(&H520::from(signature) as &[u8])).into_vec()]);
            } else {
                trace!(target: "soloauthority", "generate_seal: FAIL: accounts secret key unavailable");
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
    fn signals_epoch_end(&self, _header: &Header)
        -> super::EpochChange
    {
        // don't bother signalling even though a contract might try.
        super::EpochChange::No
    }

    #[cfg(test)]
    fn signals_epoch_end(&self, header: &Header)
        -> super::EpochChange
    {
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
                let verifier = Box::new(EpochVerifier { list });

                // our epoch verifier will ensure no unverified verifier is ever verified.
                match finalize {
                    Some(finalize) => ConstructedVerifier::Unconfirmed(verifier, proof, finalize),
                    None => ConstructedVerifier::Trusted(verifier),
                }
            }
            Err(e) => ConstructedVerifier::Err(e),
        }
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        self.validators.register_client(client);
    }

    /// Register an account which signs consensus messages.
    fn set_signer(&self, address: Address, private: Private) {
        self.signer.write().set(address, private);
    }

    fn sign(&self, hash: H256) -> Result<Signature, Error> {
        self.signer.read().sign(hash).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use ckeys::{Network, Random, Generator};
    use ctypes::H520;

    use super::SoloAuthority;
    use super::super::{ConsensusEngine, Seal};
    use super::super::signer::EngineSigner;
    use super::super::validator_set::validator_list::ValidatorList;
    use super::super::super::block::{OpenBlock, IsBlock};
    use super::super::super::codechain_machine::CodeChainMachine;
    use super::super::super::header::Header;

    fn new_test_authority() -> SoloAuthority {
        let machine = CodeChainMachine::new();
        let mut random = Random {};
        let key_pair = random.generate().unwrap();
        let address = key_pair.address();
        let mut signer = EngineSigner::default();
        signer.set(address.clone(), key_pair.private().clone());
        let validators = Box::new(ValidatorList::new(vec![address.clone()]));
        SoloAuthority::new(machine, validators)
    }

    fn genesis_header() -> Header {
        Header::default()
    }

    #[test]
    fn has_valid_metadata() {
        let engine = new_test_authority();
        assert!(!engine.name().is_empty());
    }

    #[test]
    fn can_do_signature_verification_fail() {
        let engine = new_test_authority();
        let mut header: Header = Header::default();
        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        let verify_result = engine.verify_block_external(&header);
        assert!(verify_result.is_err());
    }

    #[test]
    fn can_generate_seal() {
        let engine = new_test_authority();
        let genesis_header = genesis_header();
        let b = OpenBlock::new(&engine, &genesis_header, Default::default(), false).unwrap();
        let b = b.close_and_lock();
        if let Seal::Regular(seal) = engine.generate_seal(b.block(), &genesis_header) {
            assert!(b.try_seal(&engine, seal).is_ok());
        }
    }

    #[test]
    fn seals_internally() {
        let engine = new_test_authority();
        assert!(!engine.seals_internally().unwrap());
    }
}
