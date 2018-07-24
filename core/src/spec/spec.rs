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

use ccrypto::{blake256, BLAKE_NULL_RLP};
use cjson;
use ckey::Address;
use cstate::{
    ActionHandler, Backend, Metadata, MetadataAddress, Shard, ShardAddress, ShardMetadataAddress, StateDB, StateResult,
};
use ctypes::ShardId;
use hashdb::HashDB;
use parking_lot::RwLock;
use primitives::{Bytes, H256, U256};
use rlp::{Encodable, Rlp, RlpStream};
use trie::TrieFactory;

use super::super::blockchain::HeaderProvider;

use super::super::codechain_machine::CodeChainMachine;
use super::super::consensus::{BlakePoW, CodeChainEngine, Cuckoo, NullEngine, Solo, SoloAuthority, Tendermint};
use super::super::error::{Error, SpecError};
use super::super::header::Header;
use super::super::pod_state::{PodAccounts, PodShards};
use super::seal::Generic as GenericSeal;
use super::Genesis;

#[derive(Debug, PartialEq, Default, RlpEncodable)]
pub struct CommonParams {
    /// Maximum size of extra data.
    pub max_extra_data_size: usize,
    /// Maximum size of metadata.
    pub max_metadata_size: usize,
    /// Network id.
    pub network_id: u64,
    /// Minimum parcel cost.
    pub min_parcel_cost: U256,
    /// Maximum size of block body.
    pub max_body_size: usize,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: u64,
}

impl From<cjson::spec::Params> for CommonParams {
    fn from(p: cjson::spec::Params) -> Self {
        Self {
            max_extra_data_size: p.max_extra_data_size.into(),
            max_metadata_size: p.max_metadata_size.into(),
            network_id: p.network_id.into(),
            min_parcel_cost: p.min_parcel_cost.into(),
            max_body_size: p.max_body_size.into(),
            snapshot_period: p.snapshot_period.into(),
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
    /// Parcel root of the genesis block. Should be BLAKE_NULL_RLP.
    pub parcels_root: H256,
    /// Invoices root of the genesis block. Should be BLAKE_NULL_RLP.
    pub invoices_root: H256,
    /// The genesis block's extra data field.
    pub extra_data: Bytes,
    /// Each seal field, expressed as RLP, concatenated.
    pub seal_rlp: Bytes,

    /// May be prepopulated if we know this in advance.
    state_root_memo: RwLock<H256>,

    /// Genesis state as plain old data.
    genesis_accounts: PodAccounts,
    genesis_shards: PodShards,

    pub custom_handlers: Vec<Arc<ActionHandler>>,
}

// helper for formatting errors.
fn fmt_err<F: ::std::fmt::Display>(f: F) -> String {
    format!("Spec json is invalid: {}", f)
}

macro_rules! load_bundled {
    ($e:expr, $h:expr) => {
        Spec::load(include_bytes!(concat!("../../res/", $e, ".json")) as &[u8], $h).expect(concat!(
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
            cjson::spec::Engine::Null(null) => Arc::new(NullEngine::new(null.params.into(), machine)),
            cjson::spec::Engine::Solo(solo) => Arc::new(Solo::new(solo.params.into(), machine)),
            cjson::spec::Engine::SoloAuthority(solo_authority) => {
                Arc::new(SoloAuthority::new(solo_authority.params.into(), machine))
            }
            cjson::spec::Engine::Tendermint(tendermint) => Tendermint::new(tendermint.params.into(), machine),
            cjson::spec::Engine::Cuckoo(cuckoo) => Arc::new(Cuckoo::new(cuckoo.params.into(), machine)),
            cjson::spec::Engine::BlakePoW(blake_pow) => Arc::new(BlakePoW::new(blake_pow.params.into(), machine)),
        }
    }

    fn initialize_state<DB: Backend>(&self, trie_factory: &TrieFactory, db: DB) -> StateResult<DB> {
        let root = BLAKE_NULL_RLP;
        let (db, root) = self.initialize_accounts(trie_factory, db, root)?;
        let (db, root) = self.initialize_shards(trie_factory, db, root)?;
        let (db, root) = self.initialize_custom_actions(trie_factory, db, root)?;

        *self.state_root_memo.write() = root;
        Ok(db)
    }

    fn initialize_accounts<DB: Backend>(
        &self,
        trie_factory: &TrieFactory,
        mut db: DB,
        mut root: H256,
    ) -> StateResult<(DB, H256)> {
        // basic accounts in spec.
        {
            let mut t = trie_factory.create(db.as_hashdb_mut(), &mut root);

            for (address, account) in &*self.genesis_accounts {
                let r = t.insert(&**address, &account.rlp_bytes());
                debug_assert_eq!(Ok(None), r);
                r?;
            }
        }

        Ok((db, root))
    }

    fn initialize_shards<DB: Backend>(
        &self,
        trie_factory: &TrieFactory,
        mut db: DB,
        mut root: H256,
    ) -> StateResult<(DB, H256)> {
        let mut shard_roots = Vec::<(ShardId, H256)>::with_capacity(self.genesis_shards.len());

        // Initialize shard-level tries
        for (shard_id, shard) in &*self.genesis_shards {
            let mut shard_root = BLAKE_NULL_RLP;

            {
                let mut t = trie_factory.from_existing(db.as_hashdb_mut(), &mut shard_root)?;
                let address = ShardMetadataAddress::new(*shard_id);

                let r = t.insert(&*address, &shard.rlp_bytes());
                debug_assert_eq!(Ok(None), r);
                r?;
            }
            shard_roots.push((*shard_id, shard_root));
        }

        debug_assert_eq!(::std::mem::size_of::<u32>(), ::std::mem::size_of::<ShardId>());
        debug_assert!(
            shard_roots.len() <= ::std::u32::MAX as usize,
            "{} <= {}",
            shard_roots.len(),
            ::std::u32::MAX as usize
        );
        let global_metadata = Metadata::new(shard_roots.len() as ShardId);

        // Initialize shards
        for (shard_id, shard_root) in shard_roots.into_iter() {
            {
                let mut t = trie_factory.from_existing(db.as_hashdb_mut(), &mut root)?;
                let address = ShardAddress::new(shard_id);

                let shard = Shard::new(shard_root);
                let r = t.insert(&*address, &shard.rlp_bytes());
                debug_assert_eq!(Ok(None), r);
                r?;
            }
        }

        {
            let mut t = trie_factory.from_existing(db.as_hashdb_mut(), &mut root)?;
            let address = MetadataAddress::new();

            let r = t.insert(&*address, &global_metadata.rlp_bytes());
            debug_assert_eq!(Ok(None), r);
            r?;
        }

        Ok((db, root))
    }

    fn initialize_custom_actions<DB: Backend>(
        &self,
        trie_factory: &TrieFactory,
        mut db: DB,
        mut root: H256,
    ) -> StateResult<(DB, H256)> {
        // basic accounts in spec.
        {
            let mut t = trie_factory.from_existing(db.as_hashdb_mut(), &mut root)?;

            for handler in &self.custom_handlers {
                handler.init(t.as_mut())?;
            }
        }

        Ok((db, root))
    }

    pub fn check_genesis_root(&self, db: &HashDB) -> bool {
        if db.keys().is_empty() {
            return true
        }
        if db.contains(&self.state_root()) {
            true
        } else {
            false
        }
    }

    /// Ensure that the given state DB has the trie nodes in for the genesis state.
    pub fn ensure_genesis_state<DB: Backend>(&self, db: DB, trie_factory: &TrieFactory) -> Result<DB, Error> {
        if !self.check_genesis_root(db.as_hashdb()) {
            return Err(SpecError::InvalidState.into())
        }

        if db.as_hashdb().contains(&self.state_root()) {
            return Ok(db)
        }

        Ok(self.initialize_state(trie_factory, db)?)
    }

    pub fn check_genesis_common_params<HP: HeaderProvider>(&self, chain: &HP) -> Result<(), Error> {
        let genesis_header = self.genesis_header();
        let genesis_header_hash = genesis_header.hash();
        let header =
            chain.block_header(&genesis_header_hash).ok_or_else(|| Error::Spec(SpecError::InvalidCommonParams.into()))?;
        let extra_data = header.extra_data();
        let common_params_hash = blake256(&self.params().rlp_bytes()).to_vec();
        if extra_data != &common_params_hash {
            return Err(Error::Spec(SpecError::InvalidCommonParams.into()))
        }
        Ok(())
    }

    /// Return the state root for the genesis state, memoising accordingly.
    pub fn state_root(&self) -> H256 {
        self.state_root_memo.read().clone()
    }

    /// Loads spec from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<'a, R>(reader: R, handlers: Vec<Arc<ActionHandler>>) -> Result<Self, String>
    where
        R: Read, {
        cjson::spec::Spec::load(reader).map_err(fmt_err).and_then(|x| load_from(x, handlers).map_err(fmt_err))
    }

    /// Create a new test Spec.
    pub fn new_test(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("null", handlers)
    }

    /// Create a new Spec with Solo consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_solo(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("solo", handlers)
    }

    /// Create a new Spec with SoloAuthority consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_solo_authority(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("solo_authority", handlers)
    }

    /// Create a new Spec with Tendermint consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_tendermint(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("tendermint", handlers)
    }

    /// Create a new Spec with Cuckoo PoW consensus.
    pub fn new_test_cuckoo(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("cuckoo", handlers)
    }

    /// Create a new Spec with Blake PoW consensus.
    pub fn new_test_blake_pow(handlers: Vec<Arc<ActionHandler>>) -> Self {
        load_bundled!("blake_pow", handlers)
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
        header.set_parcels_root(self.parcels_root.clone());
        header.set_extra_data(blake256(&self.params().rlp_bytes()).to_vec());
        header.set_state_root(self.state_root());
        header.set_invoices_root(self.invoices_root.clone());
        header.set_score(self.score.clone());
        header.set_seal({
            let r = Rlp::new(&self.seal_rlp);
            r.iter().map(|f| f.as_raw().to_vec()).collect()
        });
        ctrace!(SPEC, "Genesis header is {:?}", header);
        ctrace!(SPEC, "Genesis header hash is {}", header.hash());
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
fn load_from(s: cjson::spec::Spec, handlers: Vec<Arc<ActionHandler>>) -> Result<Spec, Error> {
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();
    let params = CommonParams::from(s.params);

    let mut s = Spec {
        name: s.name.clone().into(),
        engine: Spec::engine(s.engine, params),
        data_dir: s.data_dir.unwrap_or(s.name).into(),
        nodes: s.nodes.unwrap_or_else(Vec::new),
        parent_hash: g.parent_hash,
        parcels_root: g.parcels_root,
        invoices_root: g.invoices_root,
        author: g.author,
        score: g.score,
        timestamp: g.timestamp,
        extra_data: g.extra_data,
        seal_rlp,
        state_root_memo: RwLock::new(Default::default()), // will be overwritten right after.
        genesis_accounts: s.accounts.into(),
        genesis_shards: s.shards.into(),

        custom_handlers: handlers,
    };

    // use memoized state root if provided.
    match g.state_root {
        Some(root) => *s.state_root_memo.get_mut() = root,
        None => {
            let db = StateDB::new_with_memorydb(0, s.custom_handlers.clone());
            let trie_factory = TrieFactory::new(Default::default());
            let _ = s.initialize_state(&trie_factory, db)?;
        }
    }

    Ok(s)
}

#[cfg(test)]
mod tests {
    use ccrypto::Blake;

    use super::*;

    #[test]
    fn extra_data_of_genesis_header_is_hash_of_common_params() {
        let spec = Spec::new_test(Vec::new());
        let common_params = spec.params();
        let hash_of_common_params = H256::blake(&common_params.rlp_bytes()).to_vec();

        let genesis_header = spec.genesis_header();
        let result = genesis_header.extra_data();
        assert_eq!(&hash_of_common_params, result);
    }
}
