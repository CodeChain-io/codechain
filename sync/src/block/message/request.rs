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

use ctypes::{BlockHash, BlockNumber};
use primitives::H256;
use rlp::{DecoderError, Encodable, Rlp, RlpStream};

#[derive(Clone, Debug, PartialEq)]
pub enum RequestMessage {
    Headers {
        start_number: BlockNumber,
        max_count: u64,
    },
    Bodies(Vec<BlockHash>),
    StateChunk(BlockHash, Vec<H256>),
}

impl Encodable for RequestMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            RequestMessage::Headers {
                start_number,
                max_count,
            } => {
                s.begin_list(2);
                s.append(start_number);
                s.append(max_count);
            }
            RequestMessage::Bodies(hashes) => {
                s.append_list(hashes);
            }
            RequestMessage::StateChunk(block_hash, merkle_roots) => {
                s.begin_list(2);
                s.append(block_hash);
                s.append_list(merkle_roots);
            }
        };
    }
}

impl RequestMessage {
    pub fn message_id(&self) -> u8 {
        match self {
            RequestMessage::Headers {
                ..
            } => super::MESSAGE_ID_GET_HEADERS,
            RequestMessage::Bodies(..) => super::MESSAGE_ID_GET_BODIES,
            RequestMessage::StateChunk {
                ..
            } => super::MESSAGE_ID_GET_STATE_CHUNK,
        }
    }

    pub fn decode(id: u8, rlp: &Rlp) -> Result<Self, DecoderError> {
        let message = match id {
            super::MESSAGE_ID_GET_HEADERS => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                RequestMessage::Headers {
                    start_number: rlp.val_at(0)?,
                    max_count: rlp.val_at(1)?,
                }
            }
            super::MESSAGE_ID_GET_BODIES => RequestMessage::Bodies(rlp.as_list()?),
            super::MESSAGE_ID_GET_STATE_CHUNK => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                RequestMessage::StateChunk(rlp.val_at(0)?, rlp.list_at(1)?)
            }
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        };

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use primitives::H256;
    use rlp::{Encodable, Rlp};

    use super::RequestMessage;

    pub fn decode_bytes(id: u8, bytes: &[u8]) -> RequestMessage {
        let rlp = Rlp::new(bytes);
        RequestMessage::decode(id, &rlp).unwrap()
    }

    #[test]
    fn request_headers_message_rlp() {
        let message = RequestMessage::Headers {
            start_number: 100,
            max_count: 100,
        };
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn request_bodies_message_rlp() {
        let message = RequestMessage::Bodies(vec![H256::default().into()]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn request_state_chunk_message_rlp() {
        let message = RequestMessage::StateChunk(H256::default().into(), vec![H256::default()]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }
}
