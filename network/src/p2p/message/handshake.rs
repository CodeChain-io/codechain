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

use super::super::super::NodeId;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    Sync {
        version: Version,
        port: u16,
        node_id: NodeId,
    },
    Ack(Version),
}

impl Message {
    pub fn sync(port: u16, node_id: NodeId) -> Self {
        Message::Sync {
            version: 0,
            port,
            node_id,
        }
    }

    pub fn ack() -> Self {
        Message::Ack(0)
    }

    #[allow(dead_code)]
    fn version(&self) -> &Version {
        match self {
            Message::Sync {
                version,
                ..
            } => version,
            Message::Ack(version) => version,
        }
    }

    fn protocol_id(&self) -> ProtocolId {
        match self {
            Message::Sync {
                ..
            } => SYNC_ID,
            Message::Ack(_) => ACK_ID,
        }
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Sync {
                version,
                port,
                node_id,
            } => {
                s.begin_list(4).append(version).append(&self.protocol_id()).append(port).append(node_id);
            }
            Message::Ack(version) => {
                s.begin_list(2).append(version).append(&self.protocol_id());
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
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Message::Sync {
                    version,
                    port: rlp.val_at(2)?,
                    node_id: rlp.val_at(3)?,
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

    use super::*;

    #[test]
    fn protocol_id_of_sync_is_0() {
        const PORT: u16 = 1234;
        let node_id = 1000.into();
        assert_eq!(0x00, Message::sync(PORT, node_id).protocol_id());
    }

    #[test]
    fn protocol_id_of_ack_is_1() {
        assert_eq!(0x01, Message::ack().protocol_id());
    }

    #[test]
    fn encode_and_decode_sync() {
        const PORT: u16 = 1234;
        let node_id = 1000.into();
        let sync = Message::sync(PORT, node_id);
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
