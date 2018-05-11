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
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::{Message, MESSAGE_ID_BODIES, MESSAGE_ID_HEADERS};

#[derive(Debug, PartialEq)]
pub enum ResponseMessage {
    Headers(Vec<Header>),
    Bodies(Vec<Vec<UnverifiedParcel>>),
}

impl Into<Message> for ResponseMessage {
    fn into(self) -> Message {
        Message::Response(self)
    }
}

impl Encodable for ResponseMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2);

        s.append(match self {
            &ResponseMessage::Headers {
                ..
            } => &MESSAGE_ID_HEADERS,
            &ResponseMessage::Bodies {
                ..
            } => &MESSAGE_ID_BODIES,
        });

        match self {
            &ResponseMessage::Headers(ref headers) => {
                s.append_list(headers);
            }
            &ResponseMessage::Bodies(ref bodies) => {
                s.begin_list(bodies.len());
                bodies.into_iter().for_each(|body| {
                    s.append_list(body);
                });
            }
        };
    }
}

impl Decodable for ResponseMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let id = rlp.val_at(0)?;
        let message = rlp.at(1)?;
        Ok(match id {
            MESSAGE_ID_HEADERS => ResponseMessage::Headers(message.as_list()?),
            MESSAGE_ID_BODIES => {
                let mut bodies = Vec::new();
                for item in message.into_iter() {
                    bodies.push(item.as_list()?);
                }
                ResponseMessage::Bodies(bodies)
            }
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        })
    }
}

#[cfg(test)]
mod tests {
    use ccore::Header;
    use rlp::Encodable;

    use super::ResponseMessage;

    #[test]
    fn test_headers_message_rlp() {
        let headers = vec![Header::default()];
        headers.iter().for_each(|header| {
            header.hash();
        });

        let message = ResponseMessage::Headers(headers);
        assert_eq!(message, ::rlp::decode(message.rlp_bytes().as_ref()));
    }

    #[test]
    fn test_bodies_message_rlp() {
        let message = ResponseMessage::Bodies(vec![vec![]]);
        assert_eq!(message, ::rlp::decode(message.rlp_bytes().as_ref()));
    }
}
