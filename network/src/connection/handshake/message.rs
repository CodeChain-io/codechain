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

use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

type Version = u32;
type ProtocolId = u32;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    Sync(Version),
    Ack(Version),
}

const SYNC_ID: ProtocolId = 0x00;
const ACK_ID: ProtocolId = 0x01;

impl Message {
    pub fn sync() -> Self {
        Message::Sync(0)
    }

    pub fn ack() -> Self {
        Message::Ack(0)
    }

    fn version(&self) -> Version {
        match self {
            &Message::Sync(version) => version,
            &Message::Ack(version) => version,
        }
    }

    fn protocol_id(&self) -> ProtocolId {
        match self {
            &Message::Sync(_) => SYNC_ID,
            &Message::Ack(_) => ACK_ID,
        }
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2)
            .append(&self.version())
            .append(&self.protocol_id());
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let version: Version = rlp.val_at(0)?;
        let protocol_id: ProtocolId = rlp.val_at(1)?;
        match protocol_id {
            SYNC_ID => Ok(Message::Sync(version)),
            ACK_ID => Ok(Message::Ack(version)),
            _ => Err(DecoderError::Custom("invalid protocol id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{ Decodable, Encodable, UntrustedRlp };

    use super::Message;

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;

    #[test]
    fn protocol_id_of_sync_is_0() {
        assert_eq!(0x00, Message::sync().protocol_id());
    }

    #[test]
    fn protocol_id_of_ack_is_1() {
        assert_eq!(0x01, Message::ack().protocol_id());
    }

    #[test]
    fn encode_sync() {
        let sync = Message::sync();
        let result = sync.rlp_bytes();

        assert_eq!(3, result.len());

        assert_eq!(LIST + 2, result[0]);

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
        let mut bytes = vec![LIST + 1 /* version */ + 1 /* protocol id */];

        bytes.push(SINGLE + 0); // version
        bytes.push(SINGLE + 0x00); // protocol id

        assert_eq!(3, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::sync(), message),
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

