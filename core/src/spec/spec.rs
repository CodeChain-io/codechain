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

use std::io::Read;
use std::sync::Arc;

use cbytes::Bytes;
use cjson;
use ctypes::{H256, U256, Address};

use super::Genesis;
use super::seal::Generic as GenericSeal;
use super::super::consensus::{ConsensusEngine, Solo, SoloAuthority, Tendermint};
use super::super::codechain_machine::CodeChainMachine;
use super::super::error::Error;

/// Parameters for a block chain; includes both those intrinsic to the design of the
/// chain and those to be interpreted by the active chain engine.
pub struct Spec {
    /// User friendly spec name
    pub name: String,
    /// What engine are we using for this?
    pub engine: Arc<ConsensusEngine<CodeChainMachine>>,
    /// Name of the subdir inside the main data dir to use for chain data and settings.
    pub data_dir: String,

    /// Known nodes on the network in enode format.
    pub nodes: Vec<String>,

    /// The genesis block's parent hash field.
    pub parent_hash: H256,
    /// The genesis block's author field.
    pub author: Address,
    /// The genesis block's timestamp field.
    pub timestamp: u64,
    /// Transactions root of the genesis block. Should be KECCAK_NULL_RLP.
    pub transactions_root: H256,
    /// Each seal field, expressed as RLP, concatenated.
    pub seal_rlp: Bytes,
}

// helper for formatting errors.
fn fmt_err<F: ::std::fmt::Display>(f: F) -> String {
    format!("Spec json is invalid: {}", f)
}

impl Spec {
    // create an instance of an CodeChain state machine, minus consensus logic.
    fn machine(
        _engine_spec: &cjson::spec::Engine,
    ) -> CodeChainMachine {
        CodeChainMachine::new()
    }

    /// Convert engine spec into a arc'd Engine of the right underlying type.
    /// TODO avoid this hard-coded nastiness - use dynamic-linked plugin framework instead.
    fn engine(
        engine_spec: cjson::spec::Engine,
    ) -> Arc<ConsensusEngine<CodeChainMachine>> {
        let machine = Self::machine(&engine_spec);

        match engine_spec {
            cjson::spec::Engine::Solo => Arc::new(Solo::new(machine)),
            cjson::spec::Engine::SoloAuthority(solo_authority) => Arc::new(SoloAuthority::new(solo_authority.params.into(), machine)),
            cjson::spec::Engine::Tendermint(tendermint) => Tendermint::new(tendermint.params.into(), machine)
                .expect("Failed to start the Tendermint consensus engine."),
        }
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<'a, R>(reader: R) -> Result<Self, String>
        where
            R: Read,
    {
        cjson::spec::Spec::load(reader).map_err(fmt_err).and_then(
            |x| {
                load_from(x).map_err(fmt_err)
            },
        )
    }
}

/// Load from JSON object.
fn load_from(s: cjson::spec::Spec) -> Result<Spec, Error> {
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();

    let s = Spec {
        name: s.name.clone().into(),
        engine: Spec::engine(s.engine),
        data_dir: s.data_dir.unwrap_or(s.name).into(),
        nodes: s.nodes.unwrap_or_else(Vec::new),
        parent_hash: g.parent_hash,
        transactions_root: g.transactions_root,
        author: g.author,
        timestamp: g.timestamp,
        seal_rlp,
    };

    Ok(s)
}

