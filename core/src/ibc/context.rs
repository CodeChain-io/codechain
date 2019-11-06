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
use super::kv_store;
use cstate::{ActionDataKeyBuilder, TopState};
use cstate::{TopLevelState, TopStateView};
use ibc::custom_action_handler::CUSTOM_ACTION_HANDLER_ID;
use ibc::KVStore;
use primitives::H256;

pub trait Context {
    fn get_kv_store(&mut self) -> &mut dyn kv_store::KVStore;
}

pub struct TopLevelContext<'a> {
    kv_store: TopLevelKVStore<'a>,
}

impl<'a> TopLevelContext<'a> {
    pub fn new(state: &'a mut TopLevelState) -> Self {
        TopLevelContext {
            kv_store: TopLevelKVStore {
                state,
            },
        }
    }
}

impl<'a> Context for TopLevelContext<'a> {
    fn get_kv_store(&mut self) -> &mut dyn KVStore {
        &mut self.kv_store
    }
}

pub struct TopLevelKVStore<'a> {
    state: &'a mut TopLevelState,
}

impl<'a> TopLevelKVStore<'a> {
    fn key(path: &str) -> H256 {
        let key_builder = ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1);
        key_builder.append(&path.as_bytes()).into_key()
    }
}

impl<'a> kv_store::KVStore for TopLevelKVStore<'a> {
    fn get(&self, path: &str) -> Vec<u8> {
        let key = TopLevelKVStore::key(path);
        self.state.action_data(&key).expect("Get key").expect("Data empty").into()
    }

    fn has(&self, path: &str) -> bool {
        let key = TopLevelKVStore::key(path);
        self.state.action_data(&key).expect("Get key").is_some()
    }

    fn set(&mut self, path: &str, value: &[u8]) {
        let key = TopLevelKVStore::key(path);
        self.state.update_action_data(&key, value.to_vec()).expect("Set in IBC KVStore")
    }
}
