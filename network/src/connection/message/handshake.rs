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

use super::ProtocolId;
use super::Version;

use super::ACK_ID;
use super::SYNC_ID;

use super::super::super::session::Nonce;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    Sync(Version, Nonce),
    Ack(Version),
}

impl Message {
    pub fn sync(nonce: Nonce) -> Self {
        Message::Sync(0, nonce)
    }

    pub fn ack() -> Self {
        Message::Ack(0)
    }

    fn version(&self) -> Version {
        match self {
            &Message::Sync(version, _) => version,
            &Message::Ack(version) => version,
        }
    }

    fn protocol_id(&self) -> ProtocolId {
        match self {
            &Message::Sync(..) => SYNC_ID,
            &Message::Ack(_) => ACK_ID,
        }
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::Sync(version, nonce) => {
                s.begin_list(3).append(&version).append(&self.protocol_id()).append(&nonce);
            }
            &Message::Ack(version) => {
                s.begin_list(2).append(&version).append(&self.protocol_id());
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let protocol_id: ProtocolId = rlp.val_at(1)?;
        match protocol_id {
            SYNC_ID => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let nonce = rlp.val_at(2)?;
                Ok(Message::Sync(version, nonce))
            }
            ACK_ID => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Message::Ack(version))
            }
            _ => Err(DecoderError::Custom("invalid protocol id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{Decodable, Encodable, UntrustedRlp};

    use super::{Message, Nonce};

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;

    #[test]
    fn protocol_id_of_sync_is_0() {
        const NONCE: Nonce = 1000;
        assert_eq!(0x00, Message::sync(NONCE).protocol_id());
    }

    #[test]
    fn protocol_id_of_ack_is_1() {
        assert_eq!(0x01, Message::ack().protocol_id());
    }

    #[test]
    fn encode_sync() {
        const NONCE: Nonce = 1000;
        let sync = Message::sync(NONCE);
        let result = sync.rlp_bytes();

        assert_eq!(6, result.len());

        assert_eq!(LIST + 5, result[0]);

        assert_eq!(SINGLE + sync.version() as u8, result[1]);

        assert_eq!(SINGLE + sync.protocol_id() as u8, result[2]);
    }

    #[test]
    fn encode_ack() {
        let ack = Message::ack();
        let result = ack.rlp_bytes();

        assert_eq!(3, result.len());

        assert_eq!(LIST + 2, result[0]);

        assert_eq!(SINGLE + ack.version() as u8, result[1]);

        assert_eq!(ack.protocol_id() as u8, result[2]);
    }

    #[test]
    fn decode_sync() {
        let mut bytes = vec![LIST + 1 /* version */ + 1 /* protocol id */ + 1 /* nonce */];

        bytes.push(SINGLE + 0); // version
        bytes.push(SINGLE + 0x00); // protocol id

        const NONCE: Nonce = 3;
        bytes.push(NONCE as u8); // nonce

        assert_eq!(4, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::sync(NONCE), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn decode_ack() {
        let mut bytes = vec![LIST + 1 /* version */ + 1 /* protocol id */];

        bytes.push(SINGLE + 0); // version
        bytes.push(0x01); // protocol id

        assert_eq!(3, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ack(), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}
