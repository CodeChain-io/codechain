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

use super::{ActionDataKeyBuilder, ActionHandler};
use crate::{StateResult, TopLevelState, TopState, TopStateView};
use ckey::{Address, Public};
use ctypes::errors::SyntaxError;
use ctypes::{CommonParams, Header};
use primitives::H256;
use rlp::{self, Decodable, Encodable, Rlp};

const CUSTOM_ACTION_HANDLER_ID: u64 = 1;

#[derive(RlpDecodable)]
pub struct HitAction {
    increase: u8,
}

#[derive(Clone, Default)]
pub struct HitHandler {}

impl HitHandler {
    pub fn new() -> Self {
        Self::default()
    }

    fn hit_count(&self) -> H256 {
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"hit count").into_key()
    }

    fn close_count(&self) -> H256 {
        ActionDataKeyBuilder::new(CUSTOM_ACTION_HANDLER_ID, 1).append(&"close count").into_key()
    }
}

impl ActionHandler for HitHandler {
    fn name(&self) -> &'static str {
        "hit handler"
    }

    fn handler_id(&self) -> u64 {
        CUSTOM_ACTION_HANDLER_ID
    }

    fn init(&self, state: &mut TopLevelState) -> StateResult<()> {
        let existing = state.action_data(&self.hit_count());
        debug_assert_eq!(Ok(None), existing);
        state.update_action_data(&self.hit_count(), 1u32.rlp_bytes().to_vec())?;
        state.update_action_data(&self.close_count(), 1u32.rlp_bytes().to_vec())?;
        Ok(())
    }

    /// `bytes` must be valid encoding of HitAction
    fn execute(
        &self,
        bytes: &[u8],
        state: &mut TopLevelState,
        _sender: &Address,
        _sender_pubkey: &Public,
    ) -> StateResult<()> {
        let address = self.hit_count();
        let action = HitAction::decode(&Rlp::new(bytes)).expect("Verification passed");
        let action_data = state.action_data(&address)?.unwrap_or_default();
        let prev_counter: u32 = rlp::decode(&*action_data).unwrap();
        let increase = u32::from(action.increase);
        state.update_action_data(&address, (prev_counter + increase).rlp_bytes().to_vec())?;
        Ok(())
    }

    fn verify(&self, bytes: &[u8], _params: &CommonParams) -> Result<(), SyntaxError> {
        HitAction::decode(&Rlp::new(bytes)).map_err(|err| SyntaxError::InvalidCustomAction(err.to_string()))?;
        Ok(())
    }

    fn on_close_block(&self, state: &mut TopLevelState, _header: &Header) -> StateResult<()> {
        let address = self.close_count();
        let action_data = state.action_data(&address)?.unwrap_or_default();
        let prev_counter: u32 = rlp::decode(&*action_data).unwrap();
        state.update_action_data(&address, (prev_counter + 1).rlp_bytes().to_vec())?;
        Ok(())
    }
}
