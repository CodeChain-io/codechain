// Copyright 2019 Kodebox, Inc.
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

use ckey::{NetworkId, Public};
use primitives::Bytes;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq)]
pub enum OutgoingMessage {
    Sync1 {
        initiator_pub_key: Public,
        network_id: NetworkId,
        initiator_port: u16,
    },
    Sync2 {
        initiator_pub_key: Public,
        recipient_pub_key: Public,
        network_id: NetworkId,
        initiator_port: u16,
    },
}

#[derive(Debug, PartialEq)]
pub enum IncomingMessage {
    Ack {
        recipient_pub_key: Public,
        encrypted_nonce: Bytes,
    },
    Nack,
}

const SYNC1_ID: u8 = 0x01;
const SYNC2_ID: u8 = 0x02;
const ACK_ID: u8 = 0x03;
const NACK_ID: u8 = 0x04;

impl Encodable for OutgoingMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            OutgoingMessage::Sync1 {
                initiator_pub_key,
                network_id,
                initiator_port,
            } => {
                s.begin_list(4).append(&SYNC1_ID).append(initiator_pub_key).append(network_id).append(initiator_port);
            }
            OutgoingMessage::Sync2 {
                initiator_pub_key,
                recipient_pub_key,
                network_id,
                initiator_port,
            } => {
                s.begin_list(5)
                    .append(&SYNC2_ID)
                    .append(initiator_pub_key)
                    .append(recipient_pub_key)
                    .append(network_id)
                    .append(initiator_port);
            }
        }
    }
}

impl Decodable for OutgoingMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            SYNC1_ID => {
                let item_count = rlp.item_count()?;
                if item_count != 4 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 4,
                        got: item_count,
                    })
                }
                Ok(OutgoingMessage::Sync1 {
                    initiator_pub_key: rlp.val_at(1)?,
                    network_id: rlp.val_at(2)?,
                    initiator_port: rlp.val_at(3)?,
                })
            }
            SYNC2_ID => {
                let item_count = rlp.item_count()?;
                if item_count != 5 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 5,
                        got: item_count,
                    })
                }
                Ok(OutgoingMessage::Sync2 {
                    initiator_pub_key: rlp.val_at(1)?,
                    recipient_pub_key: rlp.val_at(2)?,
                    network_id: rlp.val_at(3)?,
                    initiator_port: rlp.val_at(4)?,
                })
            }
            _ => Err(DecoderError::Custom("Invalid id")),
        }
    }
}

impl Encodable for IncomingMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            IncomingMessage::Ack {
                recipient_pub_key,
                encrypted_nonce,
            } => {
                s.begin_list(3).append(&ACK_ID).append(recipient_pub_key).append(encrypted_nonce);
            }
            IncomingMessage::Nack => {
                s.begin_list(1).append(&NACK_ID);
            }
        }
    }
}

impl Decodable for IncomingMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            ACK_ID => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(IncomingMessage::Ack {
                    recipient_pub_key: rlp.val_at(1)?,
                    encrypted_nonce: rlp.val_at(2)?,
                })
            }
            NACK_ID => {
                let item_count = rlp.item_count()?;
                if item_count != 1 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 1,
                        got: item_count,
                    })
                }
                Ok(IncomingMessage::Nack)
            }
            _ => Err(DecoderError::Custom("Invalid id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_sync1() {
        rlp_encode_and_decode_test!(OutgoingMessage::Sync1 {
            initiator_pub_key: Public::random(),
            network_id: "ab".into(),
            initiator_port: 3100
        });
    }

    #[test]
    fn encode_and_decode_sync2() {
        rlp_encode_and_decode_test!(OutgoingMessage::Sync2 {
            initiator_pub_key: Public::random(),
            recipient_pub_key: Public::random(),
            network_id: "ab".into(),
            initiator_port: 3100
        });
    }

    #[test]
    fn encode_and_decode_ack() {
        rlp_encode_and_decode_test!(IncomingMessage::Ack {
            recipient_pub_key: Public::random(),
            encrypted_nonce: vec![1, 23, 4, 5, 6],
        });
    }

    #[test]
    fn encode_and_decode_nack() {
        rlp_encode_and_decode_test!(IncomingMessage::Nack);
    }
}
