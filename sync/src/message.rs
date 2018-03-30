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

use ctypes::{H256, U256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

const STATUS_MESSAGE_ID: u8 = 0x00;

#[derive(Debug, PartialEq)]
pub enum Message {
    Status {
        total_score: U256,
        best_hash: H256,
        genesis_hash: H256,
    },
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);
        // add message id
        s.append(match self {
            &Message::Status {..} => &STATUS_MESSAGE_ID,
        });
        // add body as rlp
        match self {
            &Message::Status { total_score, best_hash, genesis_hash } => {
                s.begin_list(3);
                s.append(&total_score);
                s.append(&best_hash);
                s.append(&genesis_hash);
            },
        };
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 { return Err(DecoderError::RlpIncorrectListLen); }
        let id = rlp.val_at(0)?;
        let message = rlp.at(1)?;
        Ok(match id {
            STATUS_MESSAGE_ID => {
                if message.item_count()? != 3 { return Err(DecoderError::RlpIncorrectListLen); }
                Message::Status {
                    total_score: message.val_at(0)?,
                    best_hash: message.val_at(1)?,
                    genesis_hash: message.val_at(2)?,
                }
            },
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        })
    }
}

#[cfg(test)]
mod tests {
    use ctypes::{H256, U256};
    use rlp::Encodable;

    use super::Message;

    #[test]
    fn test_status_message_rlp() {
        let message = Message::Status {
            total_score: U256::default(),
            best_hash: H256::default(),
            genesis_hash: H256::default(),
        };
        assert_eq!(message, ::rlp::decode(message.rlp_bytes().as_ref()));
    }
}
