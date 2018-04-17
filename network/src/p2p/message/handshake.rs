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
    Sync {
        version: Version,
        session_id: Nonce,
    },
    Ack(Version),
}

impl Message {
    #[allow(dead_code)]
    pub fn sync(session_id: Nonce) -> Self {
        Message::Sync {
            version: 0,
            session_id,
        }
    }

    #[allow(dead_code)]
    pub fn ack() -> Self {
        Message::Ack(0)
    }

    #[allow(dead_code)]
    fn version(&self) -> Version {
        match self {
            &Message::Sync {
                version,
                ..
            } => version,
            &Message::Ack(version) => version,
        }
    }

    fn protocol_id(&self) -> ProtocolId {
        match self {
            &Message::Sync {
                ..
            } => SYNC_ID,
            &Message::Ack(_) => ACK_ID,
        }
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::Sync {
                version,
                ref session_id,
            } => {
                s.begin_list(3).append(&version).append(&self.protocol_id()).append(session_id);
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
                let session_id = rlp.val_at(2)?;
                Ok(Message::Sync {
                    version,
                    session_id,
                })
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

    #[test]
    fn protocol_id_of_sync_is_0() {
        let session_id = Nonce::from(1000);
        assert_eq!(0x00, Message::sync(session_id).protocol_id());
    }

    #[test]
    fn protocol_id_of_ack_is_1() {
        assert_eq!(0x01, Message::ack().protocol_id());
    }

    #[test]
    fn encode_and_decode_sync() {
        let session_id = Nonce::from(1000);
        let sync = Message::sync(session_id);
        let bytes = sync.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(sync, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_ack() {
        let ack = Message::ack();
        let bytes = ack.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(ack, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}
