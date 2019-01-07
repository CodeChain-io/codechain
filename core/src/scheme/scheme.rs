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
use ckey::{Address, NetworkId};
use cmerkle::TrieFactory;
use cstate::{
    ActionHandlerError, ActionHandlerResult, Metadata, MetadataAddress, Shard, ShardAddress, StateDB, StateResult,
    StateWithCache, TopLevelState,
};
use ctypes::transaction::Error as TransactionError;
use ctypes::ShardId;
use hashdb::{AsHashDB, HashDB};
use parking_lot::RwLock;
use primitives::{Bytes, H256, U256};
use rlp::{Encodable, Rlp, RlpStream};

use crate::blockchain::HeaderProvider;

use super::pod_state::{PodAccounts, PodShards};
use super::seal::Generic as GenericSeal;
use super::Genesis;
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::{BlakePoW, CodeChainEngine, Cuckoo, NullEngine, SimplePoA, Solo, Tendermint};
use crate::error::{Error, SchemeError};
use crate::header::Header;

#[derive(Debug, PartialEq, Default, RlpEncodable)]
pub struct CommonParams {
    /// Maximum size of extra data.
    pub max_extra_data_size: usize,
    /// Maximum size of metadata.
    pub max_metadata_size: usize,
    /// Maximum size of the content of text used in store/remove actions.
    pub max_text_content_size: usize,
    /// Network id.
    pub network_id: NetworkId,
    /// Minimum transaction cost.
    pub min_pay_transaction_cost: u64,
    pub min_set_regular_key_tranasction_cost: u64,
    pub min_create_shard_transaction_cost: u64,
    pub min_set_shard_owners_transaction_cost: u64,
    pub min_set_shard_users_transaction_cost: u64,
    pub min_wrap_ccc_transaction_cost: u64,
    pub min_custom_transaction_cost: u64,
    pub min_store_transaction_cost: u64,
    pub min_remove_transaction_cost: u64,
    pub min_asset_mint_cost: u64,
    pub min_asset_transfer_cost: u64,
    pub min_asset_scheme_change_cost: u64,
    pub min_asset_compose_cost: u64,
    pub min_asset_decompose_cost: u64,
    pub min_asset_unwrap_ccc_cost: u64,
    /// Maximum size of block body.
    pub max_body_size: usize,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: u64,
}

impl From<cjson::scheme::Params> for CommonParams {
    fn from(p: cjson::scheme::Params) -> Self {
        Self {
            max_extra_data_size: p.max_extra_data_size.into(),
            max_metadata_size: p.max_metadata_size.into(),
            max_text_content_size: p.max_text_content_size.into(),
            network_id: p.network_id,
            min_pay_transaction_cost: p.min_pay_cost.into(),
            min_set_regular_key_tranasction_cost: p.min_set_regular_key_cost.into(),
            min_create_shard_transaction_cost: p.min_create_shard_cost.into(),
            min_set_shard_owners_transaction_cost: p.min_set_shard_owners_cost.into(),
            min_set_shard_users_transaction_cost: p.min_set_shard_users_cost.into(),
            min_wrap_ccc_transaction_cost: p.min_wrap_ccc_cost.into(),
            min_custom_transaction_cost: p.min_custom_cost.into(),
            min_store_transaction_cost: p.min_store_cost.into(),
            min_remove_transaction_cost: p.min_remove_cost.into(),
            min_asset_mint_cost: p.min_mint_asset_cost.into(),
            min_asset_transfer_cost: p.min_transfer_asset_cost.into(),
            min_asset_scheme_change_cost: p.min_change_asset_scheme_cost.into(),
            min_asset_compose_cost: p.min_compose_asset_cost.into(),
            min_asset_decompose_cost: p.min_decompose_asset_cost.into(),
            min_asset_unwrap_ccc_cost: p.min_unwrap_ccc_cost.into(),
            max_body_size: p.max_body_size.into(),
            snapshot_period: p.snapshot_period.into(),
        }
    }
}

/// Parameters for a block chain; includes both those intrinsic to the design of the
/// chain and those to be interpreted by the active chain engine.
pub struct Scheme {
    /// User friendly scheme name
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

    /// May be prepopulated if we know this in advance.
    state_root_memo: RwLock<H256>,

    /// Genesis state as plain old data.
    genesis_accounts: PodAccounts,
    genesis_shards: PodShards,
}

// helper for formatting errors.
fn fmt_err<F: ::std::fmt::Display>(f: F) -> String {
    format!("Scheme json is invalid: {}", f)
}

macro_rules! load_bundled {
    ($e:expr) => {
        Scheme::load(include_bytes!(concat!("../../res/", $e, ".json")) as &[u8]).expect(concat!(
            "Chain scheme ",
            $e,
            " is invalid."
        ))
    };
}

impl Scheme {
    // create an instance of an CodeChain state machine, minus consensus logic.
    fn machine(_engine_scheme: &cjson::scheme::Engine, params: CommonParams) -> CodeChainMachine {
        CodeChainMachine::new(params)
    }

    /// Convert engine scheme into a arc'd Engine of the right underlying type.
    /// TODO avoid this hard-coded nastiness - use dynamic-linked plugin framework instead.
    fn engine(engine_scheme: cjson::scheme::Engine, params: CommonParams) -> Arc<CodeChainEngine> {
        let machine = Self::machine(&engine_scheme, params);

        match engine_scheme {
            cjson::scheme::Engine::Null(null) => Arc::new(NullEngine::new(null.params.into(), machine)),
            cjson::scheme::Engine::Solo(solo) => Arc::new(Solo::new(solo.params.into(), machine)),
            cjson::scheme::Engine::SimplePoA(simple_poa) => Arc::new(SimplePoA::new(simple_poa.params.into(), machine)),
            cjson::scheme::Engine::Tendermint(tendermint) => Tendermint::new(tendermint.params.into(), machine),
            cjson::scheme::Engine::Cuckoo(cuckoo) => Arc::new(Cuckoo::new(cuckoo.params.into(), machine)),
            cjson::scheme::Engine::BlakePoW(blake_pow) => Arc::new(BlakePoW::new(blake_pow.params.into(), machine)),
        }
    }

    fn initialize_state(&self, db: StateDB) -> StateResult<StateDB> {
        let root = BLAKE_NULL_RLP;
        let (db, root) = self.initialize_accounts(db, root)?;
        let (db, root) = self.initialize_shards(db, root)?;
        let (db, root) = match self.initialize_action_handlers(db, root) {
            Ok(x) => Ok(x),
            Err(ActionHandlerError::StateError(e)) => Err(e),
            Err(ActionHandlerError::DecoderError(_)) => unreachable!("DecoderError from genesis shouldn't be occured"),
        }?;

        *self.state_root_memo.write() = root;
        Ok(db)
    }

    fn initialize_accounts<DB: AsHashDB>(&self, mut db: DB, mut root: H256) -> StateResult<(DB, H256)> {
        // basic accounts in scheme.
        {
            let mut t = TrieFactory::create(db.as_hashdb_mut(), &mut root);

            for (address, account) in &*self.genesis_accounts {
                let r = t.insert(&**address, &account.rlp_bytes());
                debug_assert_eq!(Ok(None), r);
                r?;
            }
        }

        Ok((db, root))
    }

    fn initialize_shards<DB: AsHashDB>(&self, mut db: DB, mut root: H256) -> StateResult<(DB, H256)> {
        let mut shards = Vec::<(ShardAddress, Shard)>::with_capacity(self.genesis_shards.len());

        // Initialize shard-level tries
        for (shard_id, shard) in &*self.genesis_shards {
            let mut shard_root = BLAKE_NULL_RLP;
            let owners = shard.owners.clone();
            if owners.is_empty() {
                return Err(TransactionError::EmptyShardOwners(*shard_id).into())
            }
            let users = shard.users.clone();
            shards.push((ShardAddress::new(*shard_id), Shard::new(shard_root, owners, users)));
        }

        debug_assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<ShardId>());
        debug_assert!(shards.len() <= ::std::u16::MAX as usize, "{} <= {}", shards.len(), ::std::u16::MAX as usize);
        let global_metadata = Metadata::new(shards.len() as ShardId);

        // Initialize shards
        for (address, shard) in shards.into_iter() {
            let mut t = TrieFactory::from_existing(db.as_hashdb_mut(), &mut root)?;
            let r = t.insert(&*address, &shard.rlp_bytes());
            debug_assert_eq!(Ok(None), r);
            r?;
        }

        {
            let mut t = TrieFactory::from_existing(db.as_hashdb_mut(), &mut root)?;
            let address = MetadataAddress::new();

            let r = t.insert(&*address, &global_metadata.rlp_bytes());
            debug_assert_eq!(Ok(None), r);
            r?;
        }

        Ok((db, root))
    }

    fn initialize_action_handlers(&self, db: StateDB, root: H256) -> ActionHandlerResult<(StateDB, H256)> {
        // basic accounts in scheme.
        let mut top_level = TopLevelState::from_existing(db, root)?;
        for handler in self.engine.action_handlers() {
            handler.init(&mut top_level)?;
        }
        Ok(top_level.commit_and_into_db()?)
    }

    pub fn check_genesis_root(&self, db: &HashDB) -> bool {
        if db.keys().is_empty() {
            return true
        }
        db.contains(&self.state_root())
    }

    /// Ensure that the given state DB has the trie nodes in for the genesis state.
    pub fn ensure_genesis_state(&self, db: StateDB) -> Result<StateDB, Error> {
        if !self.check_genesis_root(db.as_hashdb()) {
            return Err(SchemeError::InvalidState.into())
        }

        if db.as_hashdb().contains(&self.state_root()) {
            return Ok(db)
        }

        Ok(self.initialize_state(db)?)
    }

    pub fn check_genesis_common_params<HP: HeaderProvider>(&self, chain: &HP) -> Result<(), Error> {
        let genesis_header = self.genesis_header();
        let genesis_header_hash = genesis_header.hash();
        let header =
            chain.block_header(&genesis_header_hash).ok_or_else(|| Error::Scheme(SchemeError::InvalidCommonParams))?;
        let extra_data = header.extra_data();
        let common_params_hash = blake256(&self.params().rlp_bytes()).to_vec();
        if extra_data != &common_params_hash {
            return Err(Error::Scheme(SchemeError::InvalidCommonParams))
        }
        Ok(())
    }

    /// Return the state root for the genesis state, memoising accordingly.
    pub fn state_root(&self) -> H256 {
        *self.state_root_memo.read()
    }

    /// Loads scheme from json file. Provide factories for executing contracts and ensuring
    /// storage goes to the right place.
    pub fn load<R>(reader: R) -> Result<Self, String>
    where
        R: Read, {
        cjson::scheme::Scheme::load(reader).map_err(fmt_err).and_then(|x| load_from(x).map_err(fmt_err))
    }

    /// Create a new test Scheme.
    pub fn new_test() -> Self {
        load_bundled!("null")
    }

    /// Create a new Scheme with Solo consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_solo() -> Self {
        load_bundled!("solo")
    }

    /// Create a new Scheme with SimplePoA consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_simple_poa() -> Self {
        load_bundled!("simple_poa")
    }

    /// Create a new Scheme with Tendermint consensus which does internal sealing (not requiring
    /// work).
    pub fn new_test_tendermint() -> Self {
        load_bundled!("tendermint")
    }

    /// Create a new Scheme with Cuckoo PoW consensus.
    pub fn new_test_cuckoo() -> Self {
        load_bundled!("cuckoo")
    }

    /// Create a new Scheme with Blake PoW consensus.
    pub fn new_test_blake_pow() -> Self {
        load_bundled!("blake_pow")
    }

    pub fn new_husky() -> Self {
        load_bundled!("husky")
    }

    pub fn new_saluki() -> Self {
        load_bundled!("saluki")
    }

    pub fn new_corgi() -> Self {
        load_bundled!("corgi")
    }

    /// Get common blockchain parameters.
    pub fn params(&self) -> &CommonParams {
        &self.engine.params()
    }

    /// Get the header of the genesis block.
    pub fn genesis_header(&self) -> Header {
        let mut header: Header = Default::default();
        header.set_parent_hash(self.parent_hash);
        header.set_timestamp(self.timestamp);
        header.set_number(0);
        header.set_author(self.author);
        header.set_transactions_root(self.transactions_root);
        header.set_extra_data(blake256(&self.params().rlp_bytes()).to_vec());
        header.set_state_root(self.state_root());
        header.set_invoices_root(self.invoices_root);
        header.set_score(self.score);
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

    pub fn genesis_accounts(&self) -> Vec<Address> {
        self.genesis_accounts.keys().cloned().collect()
    }
}

/// Load from JSON object.
fn load_from(s: cjson::scheme::Scheme) -> Result<Scheme, Error> {
    let g = Genesis::from(s.genesis);
    let GenericSeal(seal_rlp) = g.seal.into();
    let params = CommonParams::from(s.params);
    let engine = Scheme::engine(s.engine, params);

    let mut s = Scheme {
        name: s.name.clone(),
        engine,
        data_dir: s.data_dir.unwrap_or(s.name),
        nodes: s.nodes.unwrap_or_else(Vec::new),
        parent_hash: g.parent_hash,
        transactions_root: g.transactions_root,
        invoices_root: g.invoices_root,
        author: g.author,
        score: g.score,
        timestamp: g.timestamp,
        extra_data: g.extra_data,
        seal_rlp,
        state_root_memo: RwLock::new(Default::default()), // will be overwritten right after.
        genesis_accounts: s.accounts.into(),
        genesis_shards: s.shards.into(),
    };

    // use memoized state root if provided.
    match g.state_root {
        Some(root) => *s.state_root_memo.get_mut() = root,
        None => {
            let db = StateDB::new_with_memorydb();
            let _ = s.initialize_state(db)?;
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
        let scheme = Scheme::new_test();
        let common_params = scheme.params();
        let hash_of_common_params = H256::blake(&common_params.rlp_bytes()).to_vec();

        let genesis_header = scheme.genesis_header();
        let result = genesis_header.extra_data();
        assert_eq!(&hash_of_common_params, result);
    }
}
