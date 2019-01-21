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

use ckey::Address;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

const ACTION_TAG_TRANSFER_CCS: u8 = 1;

#[derive(Debug)]
pub enum Action {
    TransferCCS {
        address: Address,
        quantity: u64,
    },
}

impl Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::TransferCCS {
                address,
                quantity,
            } => s.begin_list(3).append(&ACTION_TAG_TRANSFER_CCS).append(address).append(quantity),
        };
    }
}

impl Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        match tag {
            ACTION_TAG_TRANSFER_CCS => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Ok(Action::TransferCCS {
                    address: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected Tendermint Stake Action Type")),
        }
    }
}
