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

mod hit;

use std::convert::From;
use std::sync::Arc;

use ckey::Address;
use cmerkle::TrieError;
use ctypes::invoice::Invoice;
use ctypes::parcel::Error as ParcelError;
use rlp::DecoderError;

use crate::{StateError, TopLevelState};

pub trait ActionHandler: Send + Sync {
    fn handler_id(&self) -> u64;
    fn init(&self, state: &mut TopLevelState) -> ActionHandlerResult<()>;
    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, sender: &Address) -> ActionHandlerResult<Invoice>;
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

impl From<ParcelError> for ActionHandlerError {
    fn from(error: ParcelError) -> Self {
        ActionHandlerError::StateError(StateError::Parcel(error))
    }
}

pub use self::hit::HitHandler;
