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

use ckey::Signature;
use ctypes::parcel::Action;
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq)]
pub enum Message {
    Action(Action),
    Signatures {
        action_hash: H256,
        signatures: Vec<Signature>,
    },
    RequestAction(H256),
}

impl Message {
    fn num_of_items(&self) -> usize {
        match self {
            Message::Action(_) => 1,
            Message::Signatures {
                ..
            } => 2,
            Message::RequestAction(_) => 1,
        }
    }
}

const ACTION_ID: u8 = 1;
const SIGNATURES_ID: u8 = 2;
const REQUEST_ACTION_ID: u8 = 3;

fn is_change_shard_state(action: &Action) -> bool {
    match action {
        Action::ChangeShardState {
            ..
        } => true,
        _ => false,
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let message_id = rlp.val_at(0)?;
        let message = match message_id {
            ACTION_ID => {
                let action: Action = rlp.val_at(1)?;

                if !is_change_shard_state(&action) {
                    return Err(DecoderError::Custom("Invalid action"))
                }

                Message::Action(action)
            }
            SIGNATURES_ID => {
                let signatures: Vec<Signature> = rlp.list_at(2)?;
                Message::Signatures {
                    action_hash: rlp.val_at(1)?,
                    signatures: signatures.into_iter().map(From::from).collect(),
                }
            }
            REQUEST_ACTION_ID => Message::RequestAction(rlp.val_at(1)?),
            _ => return Err(DecoderError::Custom("Invalid message id")),
        };
        if rlp.item_count()? != 1 + message.num_of_items() {
            return Err(DecoderError::RlpInvalidLength)
        }
        Ok(message)
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(1 + self.num_of_items());
        match self {
            Message::Action(action) => {
                debug_assert!(is_change_shard_state(action), "{:?} is not ChangeShardState action", action);
                s.append(&ACTION_ID);
                s.append(action);
            }
            Message::Signatures {
                action_hash,
                signatures,
            } => {
                s.append(&SIGNATURES_ID);
                s.append(action_hash);
                s.begin_list(signatures.len());
                for signature in signatures.iter() {
                    s.append(&Signature::from(signature.clone()));
                }
            }
            Message::RequestAction(action_hash) => {
                s.append(&REQUEST_ACTION_ID);
                s.append(action_hash);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ckey::Signature;

    #[test]
    fn encode_and_decode_action() {
        rlp_encode_and_decode_test!(Message::Action(Action::ChangeShardState {
            transactions: vec![],
            changes: vec![],
            signatures: vec![],
        }));
    }

    #[test]
    fn encode_and_decode_empty_signatures() {
        rlp_encode_and_decode_test!(Message::Signatures {
            action_hash: H256::random(),
            signatures: vec![],
        });
    }

    #[test]
    fn encode_and_decode_with_single_signatures() {
        let signature = Signature::random();
        rlp_encode_and_decode_test!(Message::Signatures {
            action_hash: H256::random(),
            signatures: vec![signature.into()],
        });
    }

    #[test]
    fn encode_and_decode_with_two_signatures() {
        let signature1 = Signature::random();
        let signature2 = Signature::random();
        rlp_encode_and_decode_test!(Message::Signatures {
            action_hash: H256::random(),
            signatures: vec![signature1.into(), signature2.into()],
        });
    }

    #[test]
    fn encode_and_decode_with_multiple_signatures() {
        let signature1 = Signature::random();
        let signature2 = Signature::random();
        let signature3 = Signature::random();
        let signature4 = Signature::random();
        let signature5 = Signature::random();
        let signature6 = Signature::random();
        let signature7 = Signature::random();
        rlp_encode_and_decode_test!(Message::Signatures {
            action_hash: H256::random(),
            signatures: vec![
                signature1.into(),
                signature2.into(),
                signature3.into(),
                signature4.into(),
                signature5.into(),
                signature6.into(),
                signature7.into(),
            ],
        });
    }

    #[test]
    fn encode_and_decode_request() {
        rlp_encode_and_decode_test!(Message::RequestAction(H256::random()));
    }
}
