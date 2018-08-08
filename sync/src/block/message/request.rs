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

use ctypes::BlockNumber;
use primitives::H256;
use rlp::{DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Clone, Debug, PartialEq)]
pub enum RequestMessage {
    Headers {
        start_number: BlockNumber,
        max_count: u64,
    },
    Bodies(Vec<H256>),
    StateHead(H256),
    StateChunk {
        block_hash: H256,
        tree_root: H256,
    },
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
            RequestMessage::StateHead(block_hash) => {
                s.begin_list(1);
                s.append(block_hash);
            }
            RequestMessage::StateChunk {
                block_hash,
                tree_root,
            } => {
                s.begin_list(2);
                s.append(block_hash);
                s.append(tree_root);
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
            RequestMessage::StateHead(..) => super::MESSAGE_ID_GET_STATE_HEAD,
            RequestMessage::StateChunk {
                ..
            } => super::MESSAGE_ID_GET_STATE_CHUNK,
        }
    }

    pub fn decode(id: u8, rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let message = match id {
            super::MESSAGE_ID_GET_HEADERS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                RequestMessage::Headers {
                    start_number: rlp.val_at(0)?,
                    max_count: rlp.val_at(1)?,
                }
            }
            super::MESSAGE_ID_GET_BODIES => RequestMessage::Bodies(rlp.as_list()?),
            super::MESSAGE_ID_GET_STATE_HEAD => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                RequestMessage::StateHead(rlp.val_at(0)?)
            }
            super::MESSAGE_ID_GET_STATE_CHUNK => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                RequestMessage::StateChunk {
                    block_hash: rlp.val_at(0)?,
                    tree_root: rlp.val_at(1)?,
                }
            }
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        };

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use primitives::H256;
    use rlp::{Encodable, UntrustedRlp};

    use super::RequestMessage;

    pub fn decode_bytes(id: u8, bytes: &[u8]) -> RequestMessage {
        let rlp = UntrustedRlp::new(bytes);
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
        let message = RequestMessage::Bodies(vec![H256::default()]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn request_state_head_message_rlp() {
        let message = RequestMessage::StateHead(H256::default());
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn request_state_chunk_message_rlp() {
        let message = RequestMessage::StateChunk {
            block_hash: H256::default(),
            tree_root: H256::default(),
        };
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }
}
