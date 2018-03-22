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

use ctypes::hash::{H128, H256};
use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

use super::super::session::Session;
pub use super::application::Message as ApplicationMessage;
pub use super::handshake::Message as HandshakeMessage;
pub use super::negotiation::Message as NegotiationMessage;

pub type Version = u32;
pub type ProtocolId = u32;
pub type Seq = u64;
pub type SharedSecret = H256;
pub type Nonce = H128;
pub type SessionKey = (SharedSecret, Nonce);

pub const SYNC_ID: ProtocolId = 0x00;
pub const ACK_ID: ProtocolId = 0x01;
pub const REQUEST_ID: ProtocolId = 0x02;
pub const ALLOWED_ID: ProtocolId = 0x03;
pub const DENIED_ID: ProtocolId = 0x04;
pub const ENCRYPTED_ID: ProtocolId = 0x05;
pub const UNENCRYPTED_ID: ProtocolId = 0x06;

pub enum Message {
    Application(ApplicationMessage),
    Handshake(HandshakeMessage),
    Negotiation(NegotiationMessage),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::Application(ref message) => message.rlp_append(s),
            &Message::Handshake(ref message) => message.rlp_append(s),
            &Message::Negotiation(ref message) => message.rlp_append(s),
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
            DENIED_ID => Ok(Message::Negotiation(NegotiationMessage::decode(rlp)?)),
            ENCRYPTED_ID => Ok(Message::Application(ApplicationMessage::decode(rlp)?)),
            UNENCRYPTED_ID => Ok(Message::Application(ApplicationMessage::decode(rlp)?)),
            _ => Err(DecoderError::Custom("unexpected protocol id")),
        }
    }
}

pub struct SignedMessage {
    pub message: Vec<u8>,
    signature: H256,
}

impl SignedMessage {
    pub fn new(message: Message, session: &Session) -> Option<Self> {
        let message = message.rlp_bytes().into_vec();
        session.sign(&message)
            .map(|signature| {
                Self {
                    message,
                    signature,
                }
            })
    }

    pub fn is_valid(&self, session: &Session) -> bool {
        session.sign(&self.message)
            .map(|signature| signature == self.signature)
            .unwrap_or(false)
    }
}

impl Encodable for SignedMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2)
            .append(&self.message)
            .append(&self.signature);
    }
}

impl Decodable for SignedMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::Custom("invalid message"))
        }
        let message: Vec<u8> = rlp.val_at(0)?;
        let signature: H256 = rlp.val_at(1)?;
        Ok(Self {
            message,
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SYNC_ID;
    use super::ACK_ID;
    use super::REQUEST_ID;
    use super::ALLOWED_ID;
    use super::DENIED_ID;
    use super::ENCRYPTED_ID;
    use super::UNENCRYPTED_ID;

    #[test]
    fn sync_id_is_a_unique() {
        assert_ne!(SYNC_ID, ACK_ID);
        assert_ne!(SYNC_ID, REQUEST_ID);
        assert_ne!(SYNC_ID, ALLOWED_ID);
        assert_ne!(SYNC_ID, DENIED_ID);
        assert_ne!(SYNC_ID, ENCRYPTED_ID);
        assert_ne!(SYNC_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn ack_id_is_a_unique() {
        assert_ne!(ACK_ID, SYNC_ID);
        assert_ne!(ACK_ID, REQUEST_ID);
        assert_ne!(ACK_ID, ALLOWED_ID);
        assert_ne!(ACK_ID, DENIED_ID);
        assert_ne!(ACK_ID, ENCRYPTED_ID);
        assert_ne!(ACK_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn request_id_is_a_unique() {
        assert_ne!(REQUEST_ID, SYNC_ID);
        assert_ne!(REQUEST_ID, ACK_ID);
        assert_ne!(REQUEST_ID, ALLOWED_ID);
        assert_ne!(REQUEST_ID, DENIED_ID);
        assert_ne!(REQUEST_ID, ENCRYPTED_ID);
        assert_ne!(REQUEST_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn allowed_id_is_a_unique() {
        assert_ne!(ALLOWED_ID, SYNC_ID);
        assert_ne!(ALLOWED_ID, ACK_ID);
        assert_ne!(ALLOWED_ID, REQUEST_ID);
        assert_ne!(ALLOWED_ID, DENIED_ID);
        assert_ne!(ALLOWED_ID, ENCRYPTED_ID);
        assert_ne!(ALLOWED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn denied_id_is_a_unique() {
        assert_ne!(DENIED_ID, SYNC_ID);
        assert_ne!(DENIED_ID, ACK_ID);
        assert_ne!(DENIED_ID, REQUEST_ID);
        assert_ne!(DENIED_ID, ALLOWED_ID);
        assert_ne!(DENIED_ID, ENCRYPTED_ID);
        assert_ne!(DENIED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn encrypted_id_is_a_unique() {
        assert_ne!(ENCRYPTED_ID, SYNC_ID);
        assert_ne!(ENCRYPTED_ID, ACK_ID);
        assert_ne!(ENCRYPTED_ID, REQUEST_ID);
        assert_ne!(ENCRYPTED_ID, ALLOWED_ID);
        assert_ne!(ENCRYPTED_ID, DENIED_ID);
        assert_ne!(ENCRYPTED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn unencrypted_id_is_a_unique() {
        assert_ne!(UNENCRYPTED_ID, SYNC_ID);
        assert_ne!(UNENCRYPTED_ID, ACK_ID);
        assert_ne!(UNENCRYPTED_ID, REQUEST_ID);
        assert_ne!(UNENCRYPTED_ID, ALLOWED_ID);
        assert_ne!(UNENCRYPTED_ID, DENIED_ID);
        assert_ne!(UNENCRYPTED_ID, ENCRYPTED_ID);
    }
}
