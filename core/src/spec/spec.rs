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
use ccrypto::BLAKE_NULL_RLP;
use cjson;
use ctypes::{Address, H256, U256};
use rlp::{Rlp, RlpStream};

use super::super::codechain_machine::CodeChainMachine;
use super::super::consensus::{CodeChainEngine, Solo, SoloAuthority, Tendermint};
use super::super::error::Error;
use super::super::header::Header;
use super::seal::Generic as GenericSeal;
use super::Genesis;

#[derive(Debug, PartialEq, Default)]
pub struct CommonParams {
    /// Account start nonce.
    pub account_start_nonce: U256,
    /// Network id.
    pub network_id: u64,
    /// Minimum transaction cost.
    pub min_transaction_cost: U256,
}

impl From<cjson::spec::Params> for CommonParams {
    fn from(p: cjson::spec::Params) -> Self {
        Self {
            account_start_nonce: p.account_start_nonce.map_or_else(U256::zero, Into::into),
            network_id: p.network_id.into(),
            min_transaction_cost: p.min_transaction_cost.into(),
        }
    }
}

/// Parameters for a block chain; includes both those intrinsic to the design of the
/// chain and those to be interpreted by the active chain engine.
pub struct Spec {
    /// User friendly spec name
    pub name: String,
    /// What engine are we using for this?
    pub engine: Arc<CodeChainEngine>,
    /// Name of the subdir inside the main data dir to use for chain data and settings.
    pub data_dir: String,

    /// Known nodes on the network in enode format.
    pub nodes: Vec<String>,

    /// The genesis block's parent hash field.
    pub parent_hash: H256,
    /// The genesis block's author field.
    pub author: Address,
    /// The genesis block's score field.
    pub score: U256,
    /// The genesis block's timestamp field.
    pub timestamp: u64,
    /// Transactions root of the genesis block. Should be BLAKE_NULL_RLP.
    pub transactions_root: H256,
    /// Invoices root of the genesis block. Should be BLAKE_NULL_RLP.
    pub invoices_root: H256,
    /// The genesis block's extra data field.
    pub extra_data: Bytes,
    /// Each seal field, expressed as RLP, concatenated.
    pub seal_rlp: Bytes,
}

// helper for formatting errors.
fn fmt_err<F: ::std::fmt::Display>(f: F) -> String {
    format!("Spec json is invalid: {}", f)
}

macro_rules! load_bundled {
    ($e:expr) => {
        Spec::load(include_bytes!(concat!("../../res/", $e, ".json")) as &[u8]).expect(concat!(
            "Chain spec ",
            $e,
            " is invalid."
        ))
    };
}

impl Spec {
    // create an instance of an CodeChain state machine, minus consensus logic.
    fn machine(_engine_spec: &cjson::spec::Engine, params: CommonParams) -> CodeChainMachine {
        CodeChainMachine::new(params)
    }

    /// Convert engine spec into a arc'd Engine of the right underlying type.
    /// TODO avoid this hard-coded nastiness - use dynamic-linked plugin framework instead.
    fn engine(engine_spec: cjson::spec::Engine, params: CommonParams) -> Arc<CodeChainEngine> {
        let machine = Self::machine(&engine_spec, params);

        match engine_spec {
            cjson::spec::Engine::Solo => Arc::new(Solo::new(machine)),
            cjson::spec::Engine::SoloAuthority(solo_authority) => {
                Arc::new(SoloAuthority::new(solo_authority.params.into(), machine))
            }
            cjson::spec::Engine::Tendermint(tendermint) => Tendermint::new(tendermint.params.into(), machine)
                .expect("Failed to start the Tendermint consensus engine."),
        }
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<'a, R>(reader: R) -> Result<Self, String>
    where
        R: Read, {
        cjson::spec::Spec::load(reader).map_err(fmt_err).and_then(|x| load_from(x).map_err(fmt_err))
    }

    /// Create a new Spec with Solo consensus which does internal sealing (not requiring
    /// work).
    pub fn new_solo() -> Self {
        load_bundled!("solo")
    }

    /// Create a new Spec with SoloAuthority consensus which does internal sealing (not requiring
    /// work).
    pub fn new_solo_authority() -> Self {
        load_bundled!("solo_authority")
    }

    /// Create a new Spec with Tendermint consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_tendermint() -> Self {
        load_bundled!("tendermint")
    }

    /// Get common blockchain parameters.
    pub fn params(&self) -> &CommonParams {
        &self.engine.params()
    }

    /// Get the header of the genesis block.
    pub fn genesis_header(&self) -> Header {
        let mut header: Header = Default::default();
        header.set_parent_hash(self.parent_hash.clone());
        header.set_timestamp(self.timestamp);
        header.set_number(0);
        header.set_author(self.author.clone());
        header.set_transactions_root(self.transactions_root.clone());
        header.set_extra_data(self.extra_data.clone());
        header.set_state_root(BLAKE_NULL_RLP);
        header.set_invoices_root(self.invoices_root.clone());
        header.set_score(self.score.clone());
        header.set_seal({
            let r = Rlp::new(&self.seal_rlp);
            r.iter().map(|f| f.as_raw().to_vec()).collect()
        });
        trace!(target: "spec", "Header hash is {}", header.hash());
        header
    }

    /// Compose the genesis block for this chain.
    pub fn genesis_block(&self) -> Bytes {
        let empty_list = RlpStream::new_list(0).out();
        let header = self.genesis_header();
        let mut ret = RlpStream::new_list(2);
        ret.append(&header);
        ret.append_raw(&empty_list, 1);
        ret.out()
    }
}

/// Load from JSON object.
fn load_from(s: cjson::spec::Spec) -> Result<Spec, Error> {
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();
    let params = CommonParams::from(s.params);

    let s = Spec {
        name: s.name.clone().into(),
        engine: Spec::engine(s.engine, params),
        data_dir: s.data_dir.unwrap_or(s.name).into(),
        nodes: s.nodes.unwrap_or_else(Vec::new),
        parent_hash: g.parent_hash,
        transactions_root: g.transactions_root,
        invoices_root: g.invoices_root,
        author: g.author,
        score: g.score,
        timestamp: g.timestamp,
        extra_data: g.extra_data,
        seal_rlp,
    };

    Ok(s)
}
