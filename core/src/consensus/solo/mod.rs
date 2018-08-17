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

use ctypes::machine::{Header, LiveBlock, Parcels, WithBalances};

use self::params::SoloParams;
use super::super::consensus::EngineType;
use super::super::SignedParcel;
use super::{ConsensusEngine, Seal};

/// A consensus engine which does not provide any consensus mechanism.
pub struct Solo<M> {
    params: SoloParams,
    machine: M,
}

impl<M> Solo<M> {
    /// Returns new instance of Solo over the given state machine.
    pub fn new(params: SoloParams, machine: M) -> Self {
        Solo {
            params,
            machine,
        }
    }
}

impl<M: WithBalances> ConsensusEngine<M> for Solo<M>
where
    M::LiveBlock: Parcels<Parcel = SignedParcel>,
{
    fn name(&self) -> &str {
        "Solo"
    }

    fn machine(&self) -> &M {
        &self.machine
    }

    fn seals_internally(&self) -> Option<bool> {
        Some(true)
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Solo
    }

    fn generate_seal(&self, block: &M::LiveBlock, _parent: &M::Header) -> Seal {
        if block.parcels().is_empty() {
            Seal::None
        } else {
            Seal::Regular(Vec::new())
        }
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> {
        Ok(())
    }

    fn on_close_block(&self, block: &mut M::LiveBlock) -> Result<(), M::Error> {
        let author = *LiveBlock::header(&*block).author();
        let total_reward = block.parcels().iter().fold(self.params.block_reward, |sum, parcel| sum + parcel.fee);
        self.machine.add_balance(block, &author, &total_reward)
    }
}

#[cfg(test)]
mod tests {
    use primitives::H520;

    use super::super::super::block::{IsBlock, OpenBlock};
    use super::super::super::header::Header;
    use super::super::super::scheme::Scheme;
    use super::super::super::tests::helpers::get_temp_state_db;
    use super::super::Seal;

    #[test]
    fn seal() {
        let scheme = Scheme::new_test_solo();
        let engine = &*scheme.engine;
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::new(engine, db, &genesis_header, Default::default(), vec![], false).unwrap();
        let parent_parcels_root = genesis_header.parcels_root().clone();
        let parent_invoices_root = genesis_header.invoices_root().clone();
        let b = b.close_and_lock(parent_parcels_root, parent_invoices_root);
        if let Seal::Regular(seal) = engine.generate_seal(b.block(), &genesis_header) {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn fail_to_verify() {
        let engine = Scheme::new_test_solo().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_block_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_unordered(&header).is_ok());
    }
}
