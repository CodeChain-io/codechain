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
use ibc::client_02::{type_path, Kind, KIND_CODECHAIN};
use ibc::commitment_23 as commitment;
use ibc::KVStore;
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

    fn check_validity_and_update_state(&mut self) -> Result<(), String> {
        unimplemented!()
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

impl client::Header for Header {
    fn kind(&self) -> u8 {
        client::KIND_CODECHAIN
    }
    fn get_height(&self) -> u64 {
        self.raw.number()
    }
}

pub struct State {
    id: String,
}

impl State {
    pub fn new(id: &str, ctx: &dyn ibc::Context) -> Self {
        let s = State {
            id: id.to_owned(),
        };
        s.set_type(ctx);
        s
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> Kind {
        KIND_CODECHAIN
    }

    fn set_type(&self, ctx: &dyn ibc::Context) {
        let kv_store = ctx.get_kv_store();
        let path = type_path(self.id());
        KVStore::set(kv_store, &path, &[self.kind()]);
    }
}

impl client::State for State {
    fn get_consensus_state(&self, ctx: &dyn ibc::Context) -> Box<dyn client::ConsensusState> {
        unimplemented!()
    }

    fn set_consensus_state(&self, ctx: &dyn ibc::Context, cs: &dyn client::ConsensusState) {
        let kv_store = ctx.get_kv_store();
        let data = cs.encode();
        let path = client::consensus_state_path(self.id());
        KVStore::set(kv_store, &path, &data);
    }

    fn get_root(&self, ctx: &dyn ibc::Context, client_type: u8) -> Result<Box<dyn commitment::Root>, String> {
        unimplemented!()
    }

    fn set_root(&self, ctx: &dyn ibc::Context, block_height: u64, root: &dyn commitment::Root) {
        let kv_store = ctx.get_kv_store();
        let path = client::root_path(self.id(), block_height);
        KVStore::set(kv_store, &path, &root.encode());
    }

    fn exists(&self, ctx: &dyn ibc::Context) -> bool {
        let kv_store = ctx.get_kv_store();
        KVStore::has(kv_store, &client::consensus_state_path(self.id()))
    }

    // is, update, freeze, delete,
}
