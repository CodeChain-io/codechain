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

use super::new_state;
use super::types::{ConsensusState, State};
use ibc;

pub struct Manager {}

impl Manager {
    pub fn new() -> Self {
        Manager {}
    }

    pub fn create(
        &self,
        ctx: &mut dyn ibc::Context,
        id: &str,
        cs: &dyn ConsensusState,
    ) -> Result<Box<dyn State>, String> {
        let state = new_state(id, ctx, cs.kind());
        if state.exists(ctx) {
            return Err("Create client on already existing id".to_owned())
        }
        state.set_root(ctx, cs.get_height(), cs.get_root());
        state.set_consensus_state(ctx, cs);
        Ok(state)
    }
}
