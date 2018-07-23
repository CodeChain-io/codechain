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
use ctypes::invoice::Invoice;
use ctypes::parcel::Outcome;
use primitives::{Bytes, H256};
use rlp::{self, Decodable, DecoderError, Encodable, UntrustedRlp};
use trie::TrieMut;

use super::super::{StateResult, TopLevelState, TopState, TopStateInfo};
use super::ActionHandler;

const ACTION_ID: u8 = 0;

pub struct HitAction {
    increase: u8,
}

impl Decodable for HitAction {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        if rlp.val_at::<u8>(0)? != ACTION_ID {
            return Err(DecoderError::Custom("Unknown message id detected"))
        }
        Ok(Self {
            increase: rlp.val_at(1)?,
        })
    }
}

pub struct HitHandler {}

impl HitHandler {
    pub fn new() -> Self {
        Self {}
    }

    fn address(&self) -> H256 {
        let mut hash: H256 = blake256(&b"metadata hit");
        hash[0] = b'M';
        hash
    }
}

impl ActionHandler for HitHandler {
    fn init(&self, state: &mut TrieMut) -> StateResult<()> {
        let r = state.insert(&self.address(), &0u32.rlp_bytes());
        debug_assert_eq!(Ok(None), r);
        r?;
        Ok(())
    }

    fn is_target(&self, bytes: &Bytes) -> bool {
        HitAction::decode(&UntrustedRlp::new(bytes)).is_ok()
    }

    /// `bytes` must be valid encoding of HitAction
    fn execute(&self, bytes: &Bytes, state: &mut TopLevelState) -> Option<StateResult<Outcome>> {
        HitAction::decode(&UntrustedRlp::new(bytes)).ok().map(|action| {
            let prev_counter: u32 = rlp::decode(&state.action_data(&self.address())?);
            let increase = action.increase as u32;
            state.update_action_data(&self.address(), (prev_counter + increase).rlp_bytes().to_vec())?;
            Ok(Outcome::Single {
                invoice: Invoice::Success,
                error: None,
            })
        })
    }
}
