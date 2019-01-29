// Copyright 2018-2019 Kodebox, Inc.
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

mod hit;

use std::convert::From;
use std::sync::Arc;

use ccrypto::blake256;
use ckey::Address;
use cmerkle::TrieError;
use ctypes::errors::RuntimeError;
use ctypes::invoice::Invoice;
use primitives::H256;
use rlp::{DecoderError, Encodable, RlpStream};

use super::TopStateView;
use crate::{StateError, TopLevelState};

pub trait ActionHandler: Send + Sync {
    fn handler_id(&self) -> u64;
    fn init(&self, state: &mut TopLevelState) -> ActionHandlerResult<()>;
    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, sender: &Address) -> ActionHandlerResult<Invoice>;

    fn query(&self, key_fragment: &[u8], state: &TopLevelState) -> ActionHandlerResult<Option<Vec<u8>>> {
        let key = ActionDataKeyBuilder::key_from_fragment(self.handler_id(), key_fragment);
        let some_action_data = state.action_data(&key)?.map(Vec::from);
        Ok(some_action_data)
    }
}

pub trait FindActionHandler {
    fn find_action_handler_for(&self, _id: u64) -> Option<&Arc<ActionHandler>> {
        None
    }
}

pub type ActionHandlerResult<T> = Result<T, ActionHandlerError>;

#[derive(Debug, PartialEq)]
pub enum ActionHandlerError {
    DecoderError(DecoderError),
    StateError(StateError),
}

impl From<DecoderError> for ActionHandlerError {
    fn from(error: DecoderError) -> Self {
        ActionHandlerError::DecoderError(error)
    }
}

impl From<StateError> for ActionHandlerError {
    fn from(error: StateError) -> Self {
        ActionHandlerError::StateError(error)
    }
}

impl From<TrieError> for ActionHandlerError {
    fn from(error: TrieError) -> Self {
        ActionHandlerError::StateError(StateError::Trie(error))
    }
}

impl From<RuntimeError> for ActionHandlerError {
    fn from(error: RuntimeError) -> Self {
        ActionHandlerError::StateError(StateError::Runtime(error))
    }
}

pub struct ActionDataKeyBuilder {
    rlp: RlpStream,
}

impl ActionDataKeyBuilder {
    fn prepare(handler_id: u64) -> ActionDataKeyBuilder {
        let mut rlp = RlpStream::new_list(3);
        rlp.append(&"ActionData");
        rlp.append(&handler_id);
        ActionDataKeyBuilder {
            rlp,
        }
    }

    pub fn key_from_fragment(handler_id: u64, key_fragment: &[u8]) -> H256 {
        let mut builder = Self::prepare(handler_id);
        builder.rlp.append_raw(&key_fragment, 1);
        builder.into_key()
    }

    pub fn new(handler_id: u64, fragment_length: usize) -> ActionDataKeyBuilder {
        let mut builder = Self::prepare(handler_id);
        builder.rlp.begin_list(fragment_length);
        builder
    }

    pub fn append<E>(mut self, e: &E) -> ActionDataKeyBuilder
    where
        E: Encodable, {
        self.rlp.append(e);
        self
    }

    pub fn into_key(self) -> H256 {
        blake256(self.rlp.as_raw())
    }
}

pub use self::hit::HitHandler;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_data_key_builder_raw_fragment_and_list_are_same() {
        let key1 =
            ActionDataKeyBuilder::new(1, 3).append(&"key").append(&"fragment").append(&"has trailing list").into_key();

        let mut rlp = RlpStream::new_list(3);
        rlp.append(&"key").append(&"fragment").append(&"has trailing list");
        let key2 = ActionDataKeyBuilder::key_from_fragment(1, rlp.as_raw());
        assert_eq!(key1, key2);
    }
}
