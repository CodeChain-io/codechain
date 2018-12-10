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

use ccrypto::blake256;
use ckey::Address;
use cmerkle::TrieMut;
use ctypes::invoice::Invoice;
use primitives::H256;
use rlp::{self, Decodable, Encodable, UntrustedRlp};

use super::{ActionHandler, ActionHandlerResult};
use crate::{StateResult, TopLevelState, TopState, TopStateView};

const ACTION_ID: u64 = 1;

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

    fn address(&self) -> H256 {
        let mut hash: H256 = blake256(&b"metadata hit");
        hash[0] = b'M';
        hash
    }
}

impl ActionHandler for HitHandler {
    fn handler_id(&self) -> u64 {
        ACTION_ID
    }

    fn init(&self, state: &mut TrieMut) -> StateResult<()> {
        let r = state.insert(&self.address(), &1u32.rlp_bytes());
        debug_assert_eq!(Ok(None), r);
        r?;
        Ok(())
    }

    /// `bytes` must be valid encoding of HitAction
    fn execute(&self, bytes: &[u8], state: &mut TopLevelState, _sender: &Address) -> ActionHandlerResult {
        let action = HitAction::decode(&UntrustedRlp::new(bytes))?;
        let action_data = state.action_data(&self.address())?.unwrap_or_default();
        let prev_counter: u32 = rlp::decode(&*action_data);
        let increase = u32::from(action.increase);
        state.update_action_data(&self.address(), (prev_counter + increase).rlp_bytes().to_vec())?;
        Ok(Invoice::Success)
    }
}
