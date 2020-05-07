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

use ctypes::BlockHash;
use primitives::U256;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

mod request;
mod response;

pub use self::request::RequestMessage;
pub use self::response::ResponseMessage;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum MessageID {
    Status = 0x01,
    GetHeaders = 0x02,
    Headers = 0x03,
    GetBodies = 0x04,
    Bodies = 0x05,
    GetStateHead = 0x06,
    StateHead = 0x07,
    GetStateChunk = 0x08,
    StateChunk = 0x09,
}

impl Encodable for MessageID {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_single_value(&(*self as u8));
    }
}

impl Decodable for MessageID {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.as_val()?;
        match tag {
            0x01u8 => Ok(MessageID::Status),
            0x02 => Ok(MessageID::GetHeaders),
            0x03 => Ok(MessageID::Headers),
            0x04 => Ok(MessageID::GetBodies),
            0x05 => Ok(MessageID::Bodies),
            0x06 => Ok(MessageID::GetStateHead),
            0x07 => Ok(MessageID::StateHead),
            0x08 => Ok(MessageID::GetStateChunk),
            0x09 => Ok(MessageID::StateChunk),
            _ => Err(DecoderError::Custom("Unexpected MessageID Value")),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Status {
        total_score: U256,
        best_hash: BlockHash,
        genesis_hash: BlockHash,
    },
    Request(u64, RequestMessage),
    Response(u64, ResponseMessage),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Status {
                total_score,
                best_hash,
                genesis_hash,
            } => {
                s.begin_list(2);
                s.append(&MessageID::Status);
                s.begin_list(3);
                s.append(total_score);
                s.append(best_hash);
                s.append(genesis_hash);
            }
            Message::Request(request_id, request) => {
                s.begin_list(3);
                s.append(&request.message_id());
                s.append(request_id);
                s.append(request);
            }
            Message::Response(response_id, response) => {
                s.begin_list(3);
                s.append(&response.message_id());
                s.append(response_id);
                s.append(response);
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let id = rlp.val_at(0)?;
        match id {
            MessageID::Status => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                let message = rlp.at(1)?;

                let message_item_count = message.item_count()?;
                if message_item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        expected: 3,
                        got: message_item_count,
                    })
                }

                Ok(Message::Status {
                    total_score: message.val_at(0)?,
                    best_hash: message.val_at(1)?,
                    genesis_hash: message.val_at(2)?,
                })
            }
            _ => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                let request_id = rlp.val_at(1)?;
                let message = rlp.at(2)?;
                match id {
                    MessageID::GetHeaders
                    | MessageID::GetBodies
                    | MessageID::GetStateHead
                    | MessageID::GetStateChunk => {
                        Ok(Message::Request(request_id, RequestMessage::decode(id, &message)?))
                    }

                    MessageID::Headers | MessageID::Bodies | MessageID::StateHead | MessageID::StateChunk => {
                        Ok(Message::Response(request_id, ResponseMessage::decode(id, &message)?))
                    }
                    _ => Err(DecoderError::Custom("Unknown message id detected")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::H256;

    /// For a type that does not have PartialEq, uses Debug instead.
    fn assert_eq_by_debug<T: std::fmt::Debug>(a: &T, b: &T) {
        assert_eq!(format!("{:?}", a), format!("{:?}", b));
    }

    #[test]
    fn status_message_rlp() {
        let status_message = Message::Status {
            total_score: U256::default(),
            best_hash: H256::default().into(),
            genesis_hash: H256::default().into(),
        };
        let encoded = rlp::encode(&status_message);
        let decoded: Message = rlp::decode(&encoded).unwrap();

        assert_eq_by_debug(&status_message, &decoded)
    }

    #[test]
    fn request_bodies_message_rlp() {
        let request_id = 10;
        let message = Message::Request(request_id, RequestMessage::Bodies(vec![]));
        let encoded = rlp::encode(&message);
        let decoded: Message = rlp::decode(&encoded).unwrap();

        assert_eq_by_debug(&message, &decoded)
    }

    #[test]
    fn request_state_head_rlp() {
        let request_id = 10;
        let message = Message::Request(request_id, RequestMessage::StateHead(H256::random().into()));
        let encoded = rlp::encode(&message);
        let decoded: Message = rlp::decode(&encoded).unwrap();

        assert_eq_by_debug(&message, &decoded)
    }
}
