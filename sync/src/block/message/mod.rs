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

mod request;
mod response;

pub use self::request::RequestMessage;
pub use self::response::ResponseMessage;

const MESSAGE_ID_STATUS: u8 = 0x01;
const MESSAGE_ID_GET_HEADERS: u8 = 0x02;
const MESSAGE_ID_HEADERS: u8 = 0x03;
const MESSAGE_ID_GET_BODIES: u8 = 0x04;
const MESSAGE_ID_BODIES: u8 = 0x05;
const MESSAGE_ID_GET_STATE_HEAD: u8 = 0x06;
const MESSAGE_ID_STATE_HEAD: u8 = 0x07;
const MESSAGE_ID_GET_STATE_CHUNK: u8 = 0x08;
const MESSAGE_ID_STATE_CHUNK: u8 = 0x09;

#[derive(Debug, PartialEq)]
pub enum Message {
    Status {
        total_score: U256,
        best_hash: H256,
        genesis_hash: H256,
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
                s.append(&MESSAGE_ID_STATUS);

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
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let id = rlp.val_at(0)?;
        if id == MESSAGE_ID_STATUS {
            if rlp.item_count()? != 2 {
                return Err(DecoderError::RlpIncorrectListLen)
            }
            let message = rlp.at(1)?;

            if message.item_count()? != 3 {
                return Err(DecoderError::RlpIncorrectListLen)
            }

            Ok(Message::Status {
                total_score: message.val_at(0)?,
                best_hash: message.val_at(1)?,
                genesis_hash: message.val_at(2)?,
            })
        } else {
            if rlp.item_count()? != 3 {
                return Err(DecoderError::RlpIncorrectListLen)
            }
            let request_id = rlp.val_at(1)?;
            let message = rlp.at(2)?;
            match id {
                MESSAGE_ID_GET_HEADERS
                | MESSAGE_ID_GET_BODIES
                | MESSAGE_ID_GET_STATE_HEAD
                | MESSAGE_ID_GET_STATE_CHUNK => Ok(Message::Request(request_id, RequestMessage::decode(id, &message)?)),
                MESSAGE_ID_HEADERS | MESSAGE_ID_BODIES | MESSAGE_ID_STATE_HEAD | MESSAGE_ID_STATE_CHUNK => {
                    Ok(Message::Response(request_id, ResponseMessage::decode(id, &message)?))
                }
                _ => Err(DecoderError::Custom("Unknown message id detected")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ctypes::{H256, U256};

    use super::Message;

    #[test]
    fn test_status_message_rlp() {
        rlp_encode_and_decode_test!(Message::Status {
            total_score: U256::default(),
            best_hash: H256::default(),
            genesis_hash: H256::default(),
        });
    }
}
