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

pub mod codechain;
mod manager;
mod types;

use ibc;
use primitives::Bytes;

pub use self::manager::Manager;
pub use self::types::{ConsensusState, Header, Kind, State, KIND_CODECHAIN};

use super::context::Context;

pub fn create(context: impl Context, consensus_state: impl ConsensusState) {}

pub fn check_validity_and_update_state(header: impl Header) {}

pub fn check_misbehaviour_and_update_state(bytes: Bytes) {}


pub fn new_state(id: &str, ctx: &dyn ibc::Context, client_type: Kind) -> Box<dyn State> {
    if client_type == KIND_CODECHAIN {
        Box::new(codechain::State::new(id, ctx))
    } else {
        panic!("Invalid client type");
    }
}

pub fn path(id: &str) -> String {
    format!("clients/{}", id)
}

pub fn consensus_state_path(id: &str) -> String {
    format!("{}/consensusState", path(id))
}

pub fn root_path(id: &str, block_number: u64) -> String {
    format!("{}/roots/{}", path(id), block_number)
}

pub fn type_path(id: &str) -> String {
    format!("{}/type", path(id))
}
