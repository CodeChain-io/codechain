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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::ExtensionMessage;
use super::HandshakeMessage;
use super::NegotiationMessage;

#[derive(Debug)]
pub enum Message {
    Extension(ExtensionMessage),
    Handshake(HandshakeMessage),
    Negotiation(NegotiationMessage),
}

use super::ACK_ID;
use super::ALLOWED_ID;
use super::ENCRYPTED_ID;
use super::REQUEST_ID;
use super::SYNC_ID;
use super::UNENCRYPTED_ID;

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Extension(message) => message.rlp_append(s),
            Message::Handshake(message) => message.rlp_append(s),
            Message::Negotiation(message) => message.rlp_append(s),
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let protocol_id = rlp.val_at(1)?;
        match protocol_id {
            SYNC_ID => Ok(Message::Handshake(HandshakeMessage::decode(rlp)?)),
            ACK_ID => Ok(Message::Handshake(HandshakeMessage::decode(rlp)?)),
            REQUEST_ID => Ok(Message::Negotiation(NegotiationMessage::decode(rlp)?)),
            ALLOWED_ID => Ok(Message::Negotiation(NegotiationMessage::decode(rlp)?)),
            ENCRYPTED_ID => Ok(Message::Extension(ExtensionMessage::decode(rlp)?)),
            UNENCRYPTED_ID => Ok(Message::Extension(ExtensionMessage::decode(rlp)?)),
            _ => Err(DecoderError::Custom("unexpected protocol id")),
        }
    }
}
