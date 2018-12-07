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

use ccore::{Header, UnverifiedParcel};
use rlp::{DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq)]
pub enum ResponseMessage {
    Headers(Vec<Header>),
    Bodies(Vec<Vec<UnverifiedParcel>>),
    StateHead(Vec<u8>),
    StateChunk(Vec<u8>),
}

impl Encodable for ResponseMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            ResponseMessage::Headers(headers) => {
                s.append_list(headers);
            }
            ResponseMessage::Bodies(bodies) => {
                s.begin_list(bodies.len());
                bodies.iter().for_each(|body| {
                    s.append_list(body);
                });
            }
            ResponseMessage::StateHead(bytes) => {
                s.begin_list(1);
                s.append(bytes);
            }
            ResponseMessage::StateChunk(bytes) => {
                s.begin_list(1);
                s.append(bytes);
            }
        };
    }
}

impl ResponseMessage {
    pub fn message_id(&self) -> u8 {
        match self {
            ResponseMessage::Headers {
                ..
            } => super::MESSAGE_ID_HEADERS,
            ResponseMessage::Bodies(..) => super::MESSAGE_ID_BODIES,
            ResponseMessage::StateHead(..) => super::MESSAGE_ID_STATE_HEAD,
            ResponseMessage::StateChunk {
                ..
            } => super::MESSAGE_ID_STATE_CHUNK,
        }
    }

    pub fn decode(id: u8, rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let message = match id {
            super::MESSAGE_ID_HEADERS => ResponseMessage::Headers(rlp.as_list()?),
            super::MESSAGE_ID_BODIES => {
                let mut bodies = Vec::new();
                for item in rlp.into_iter() {
                    bodies.push(item.as_list()?);
                }
                ResponseMessage::Bodies(bodies)
            }
            super::MESSAGE_ID_STATE_HEAD => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                ResponseMessage::StateHead(rlp.val_at(0)?)
            }
            super::MESSAGE_ID_STATE_CHUNK => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                ResponseMessage::StateChunk(rlp.val_at(0)?)
            }
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        };

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use ccore::Header;
    use rlp::{Encodable, UntrustedRlp};

    use super::ResponseMessage;

    pub fn decode_bytes(id: u8, bytes: &[u8]) -> ResponseMessage {
        let rlp = UntrustedRlp::new(bytes);
        ResponseMessage::decode(id, &rlp).unwrap()
    }

    #[test]
    fn headers_message_rlp() {
        let headers = vec![Header::default()];
        headers.iter().for_each(|header| {
            header.hash();
        });

        let message = ResponseMessage::Headers(headers);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn bodies_message_rlp() {
        let message = ResponseMessage::Bodies(vec![vec![]]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn state_head_message_rlp() {
        let message = ResponseMessage::StateHead(vec![]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }

    #[test]
    fn state_chunk_message_rlp() {
        let message = ResponseMessage::StateChunk(vec![]);
        assert_eq!(message, decode_bytes(message.message_id(), message.rlp_bytes().as_ref()));
    }
}
