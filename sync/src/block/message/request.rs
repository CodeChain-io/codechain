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

use ccore::BlockNumber;
use ctypes::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::{Message, MESSAGE_ID_REQUEST_BODIES, MESSAGE_ID_REQUEST_HEADERS};

#[derive(Clone, Debug, PartialEq)]
pub enum RequestMessage {
    Headers {
        start_number: BlockNumber,
        max_count: u64,
    },
    Bodies(Vec<H256>),
}

impl Into<Message> for RequestMessage {
    fn into(self) -> Message {
        Message::Request(self)
    }
}

impl Encodable for RequestMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);

        s.append(match self {
            &RequestMessage::Headers {
                ..
            } => &MESSAGE_ID_REQUEST_HEADERS,
            &RequestMessage::Bodies {
                ..
            } => &MESSAGE_ID_REQUEST_BODIES,
        });

        match self {
            &RequestMessage::Headers {
                start_number,
                max_count,
            } => {
                s.begin_list(2);
                s.append(&start_number);
                s.append(&max_count);
            }
            &RequestMessage::Bodies(ref hashes) => {
                s.append_list(hashes);
            }
        };
    }
}

impl Decodable for RequestMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let id = rlp.val_at(0)?;
        let message = rlp.at(1)?;
        Ok(match id {
            MESSAGE_ID_REQUEST_HEADERS => {
                if message.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                RequestMessage::Headers {
                    start_number: message.val_at(0)?,
                    max_count: message.val_at(1)?,
                }
            }
            MESSAGE_ID_REQUEST_BODIES => RequestMessage::Bodies(message.as_list()?),
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        })
    }
}

#[cfg(test)]
mod tests {
    use ctypes::H256;
    use rlp::Encodable;

    use super::RequestMessage;

    #[test]
    fn test_request_headers_message_rlp() {
        let message = RequestMessage::Headers {
            start_number: 100,
            max_count: 100,
        };
        assert_eq!(message, ::rlp::decode(message.rlp_bytes().as_ref()));
    }

    #[test]
    fn test_request_bodies_message_rlp() {
        let message = RequestMessage::Bodies(vec![H256::default()]);
        assert_eq!(message, ::rlp::decode(message.rlp_bytes().as_ref()));
    }
}
