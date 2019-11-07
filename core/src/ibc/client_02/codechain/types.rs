// Copyright 2019 Kodebox, Inc.
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

use rlp;

use ckey::Public;
use encoded;
use ibc;
use ibc::client_02 as client;
use ibc::client_02::{type_path, Header as ClientHeader, Kind, KIND_CODECHAIN};
use ibc::commitment_23 as commitment;
use ibc::KVStore;
use primitives::H256;
use rlp::{DecoderError, UntrustedRlp};

pub type ValidatorSet = Vec<Public>;

pub struct ConsensusState {
    height: u64,
    root: commitment::merkle::Root,
    next_validator_set: ValidatorSet,
}

impl rlp::Encodable for ConsensusState {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(3);
        s.append(&self.height);
        s.append(&self.root);
        s.append_list(&self.next_validator_set);
    }
}

impl rlp::Decodable for ConsensusState {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 3 {
            return Err(DecoderError::RlpIncorrectListLen {
                expected: 3,
                got: item_count,
            })
        }
        Ok(ConsensusState {
            height: rlp.val_at(0)?,
            root: rlp.val_at(1)?,
            next_validator_set: rlp.list_at(2)?,
        })
    }
}

impl ConsensusState {
    fn update(&mut self, header: &self::Header) {
        self.height = header.get_height();
        self.root = commitment::merkle::Root::new(header.raw.state_root())
    }
}

impl client::ConsensusState for ConsensusState {
    fn kind(&self) -> u8 {
        client::KIND_CODECHAIN
    }

    fn get_height(&self) -> u64 {
        self.height
    }

    fn get_root(&self) -> &dyn commitment::Root {
        &self.root
    }

    fn check_validity_and_update_state(&mut self, header: &[u8]) -> Result<(), String> {
        let header = self::Header::new(header.to_vec());

        header.verify_basic()?;
        header.verify_signature(&self.next_validator_set)?;

        self.update(&header);

        Ok(())
    }

    fn check_misbehaviour_and_update_state(&mut self) -> bool {
        unimplemented!()
    }

    fn encode(&self) -> Vec<u8> {
        rlp::encode(self).into_vec()
    }
}

pub struct Header {
    raw: encoded::Header,
}

impl Header {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            raw: encoded::Header::new(bytes),
        }
    }

    fn verify_basic(&self) -> Result<(), String> {
        // TODO
        Ok(())
    }

    fn verify_signature(&self, _validator_set: &[Public]) -> Result<(), String> {
        // TODO
        Ok(())
    }
}

impl client::Header for Header {
    fn kind(&self) -> u8 {
        client::KIND_CODECHAIN
    }
    fn get_height(&self) -> u64 {
        self.raw.number()
    }
    fn encode(&self) -> &[u8] {
        self.raw.rlp().as_raw()
    }
}

pub struct State {
    id: String,
}

impl State {
    pub fn new(id: &str, ctx: &mut dyn ibc::Context) -> Self {
        let s = State {
            id: id.to_owned(),
        };
        s.set_type(ctx);
        s
    }

    pub fn find(id: &str) -> Self {
        State {
            id: id.to_owned(),
        }
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> Kind {
        KIND_CODECHAIN
    }

    fn set_type(&self, ctx: &mut dyn ibc::Context) {
        let kv_store = ctx.get_kv_store();
        let path = type_path(self.id());
        KVStore::set(kv_store, &path, &[self.kind()]);
    }
}

impl client::State for State {
    fn get_consensus_state(&self, ctx: &mut dyn ibc::Context) -> Box<dyn client::ConsensusState> {
        let kv_store = ctx.get_kv_store();
        let bytes = kv_store.get(&client::consensus_state_path(self.id()));
        let rlp = UntrustedRlp::new(&bytes);
        let consensus_state: ConsensusState = rlp.as_val().expect("data from DB");
        Box::new(consensus_state)
    }

    fn set_consensus_state(&self, ctx: &mut dyn ibc::Context, cs: &dyn client::ConsensusState) {
        let kv_store = ctx.get_kv_store();
        let data = cs.encode();
        let path = client::consensus_state_path(self.id());
        KVStore::set(kv_store, &path, &data);
    }

    fn get_root(&self, ctx: &mut dyn ibc::Context, block_height: u64) -> Result<Box<dyn commitment::Root>, String> {
        let kv_store = ctx.get_kv_store();
        let path = client::root_path(self.id(), block_height);
        let bytes = KVStore::get(kv_store, &path);
        let rlp = UntrustedRlp::new(&bytes);
        let raw_hash: H256 = rlp.as_val().map_err(|err| format!("ibc get_root: {}", err.to_string()))?;
        Ok(Box::new(commitment::merkle::Root::new(raw_hash)))
    }

    fn set_root(&self, ctx: &mut dyn ibc::Context, block_height: u64, root: &dyn commitment::Root) {
        let kv_store = ctx.get_kv_store();
        let path = client::root_path(self.id(), block_height);
        KVStore::set(kv_store, &path, &root.encode());
    }

    fn exists(&self, ctx: &mut dyn ibc::Context) -> bool {
        let kv_store = ctx.get_kv_store();
        KVStore::has(kv_store, &client::consensus_state_path(self.id()))
    }

    fn update(&self, ctx: &mut dyn ibc::Context, header: &[u8]) -> Result<(), String> {
        if !self.exists(ctx) {
            return Err("client not exist".to_owned())
        }

        let mut consensus_state = self.get_consensus_state(ctx);
        consensus_state.check_validity_and_update_state(header)?;

        self.set_consensus_state(ctx, consensus_state.as_ref());
        self.set_root(ctx, consensus_state.get_height(), consensus_state.get_root());

        Ok(())
    }
}
