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

use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

/// A full epoch transition.
#[derive(Debug, Clone)]
pub struct Transition {
    /// Block hash at which the transition occurred.
    pub block_hash: H256,
    /// Block number at which the transition occurred.
    pub block_number: u64,
    /// "transition/epoch" proof from the engine combined with a finality proof.
    pub proof: Vec<u8>,
}

impl Encodable for Transition {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3).append(&self.block_hash).append(&self.block_number).append(&self.proof);
    }
}

impl Decodable for Transition {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 3 {
            return Err(DecoderError::RlpInvalidLength {
                expected: 3,
                got: item_count,
            })
        }
        Ok(Transition {
            block_hash: rlp.val_at(0)?,
            block_number: rlp.val_at(1)?,
            proof: rlp.val_at(2)?,
        })
    }
}
