// Copyright 2018-2019 Kodebox, Inc.
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

use std::sync::Arc;

use ccrypto::Blake;
use ckey::{recover, Address, Signature};
use client::ConsensusClient;
use consensus::vote_collector::Message;
use consensus::ValidatorSet;
use ctypes::errors::SyntaxError;
use ctypes::CommonParams;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

const ACTION_TAG_TRANSFER_CCS: u8 = 1;
const ACTION_TAG_DELEGATE_CCS: u8 = 2;
const ACTION_TAG_REVOKE: u8 = 3;
const ACTION_TAG_SELF_NOMINATE: u8 = 4;
const ACTION_TAG_REPORT_DOUBLE_VOTE: u8 = 5;
const ACTION_TAG_CHANGE_PARAMS: u8 = 0xFF;

#[derive(Debug, PartialEq)]
pub enum Action<M: Message> {
    TransferCCS {
        address: Address,
        quantity: u64,
    },
    DelegateCCS {
        address: Address,
        quantity: u64,
    },
    Revoke {
        address: Address,
        quantity: u64,
    },
    SelfNominate {
        deposit: u64,
        metadata: Bytes,
    },
    ChangeParams {
        metadata_seq: u64,
        params: Box<CommonParams>,
        signatures: Vec<Signature>,
    },
    ReportDoubleVote {
        message1: M,
        message2: M,
    },
}

impl<M: Message> Action<M> {
    pub fn verify(
        &self,
        current_params: &CommonParams,
        client: Option<Arc<ConsensusClient>>,
        validators: Option<Arc<ValidatorSet>>,
    ) -> Result<(), SyntaxError> {
        match self {
            Action::TransferCCS {
                ..
            } => {}
            Action::DelegateCCS {
                ..
            } => {}
            Action::Revoke {
                ..
            } => {}
            Action::SelfNominate {
                metadata,
                ..
            } => {
                if metadata.len() > current_params.max_candidate_metadata_size() {
                    return Err(SyntaxError::InvalidCustomAction(format!(
                        "Too long candidate metadata: the size limit is {}",
                        current_params.max_candidate_metadata_size()
                    )))
                }
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => {
                let current_network_id = current_params.network_id();
                let transaction_network_id = params.network_id();
                if current_network_id != transaction_network_id {
                    return Err(SyntaxError::InvalidCustomAction(format!(
                        "The current network id is {} but the transaction tries to change the network id to {}",
                        current_network_id, transaction_network_id
                    )))
                }
                params.verify().map_err(SyntaxError::InvalidCustomAction)?;
                let action = Action::<M>::ChangeParams {
                    metadata_seq: *metadata_seq,
                    params: params.clone(),
                    signatures: vec![],
                };
                let encoded_action = H256::blake(rlp::encode(&action));
                for signature in signatures {
                    // XXX: Signature recovery is an expensive job. Should we do it twice?
                    recover(&signature, &encoded_action).map_err(|err| {
                        SyntaxError::InvalidCustomAction(format!("Cannot decode the signature: {}", err))
                    })?;
                }
            }
            Action::ReportDoubleVote {
                message1,
                message2,
            } => {
                if message1 == message2 {
                    return Err(SyntaxError::InvalidCustomAction(String::from("Messages are duplicated")))
                }
                if message1.round() != message2.round() {
                    return Err(SyntaxError::InvalidCustomAction(String::from(
                        "The messages are from two different voting rounds",
                    )))
                }

                let signer_idx1 = message1.signer_index();
                let signer_idx2 = message2.signer_index();

                if signer_idx1 != signer_idx2 {
                    return Err(SyntaxError::InvalidCustomAction(format!(
                        "Two messages have different signer indexes: {}, {}",
                        signer_idx1, signer_idx2
                    )))
                }

                assert_eq!(
                    message1.height(),
                    message2.height(),
                    "Heights of both messages must be same because message1.round() == message2.round()"
                );
                let signed_block_height = message1.height();
                let (client, validators) = (
                    client.expect("Client should be initialized"),
                    validators.expect("ValidatorSet should be initialized"),
                );
                if signed_block_height == 0 {
                    return Err(SyntaxError::InvalidCustomAction(String::from(
                        "Double vote on the genesis block does not make sense",
                    )))
                }
                let parent_hash = client
                    .block_header(&(signed_block_height - 1).into())
                    .ok_or_else(|| {
                        SyntaxError::InvalidCustomAction(format!(
                            "Cannot get header from the height {}",
                            signed_block_height
                        ))
                    })?
                    .hash();
                let signer = validators.get(&parent_hash, signer_idx1);
                if message1.verify(&signer) != Ok(true) || message2.verify(&signer) != Ok(true) {
                    return Err(SyntaxError::InvalidCustomAction(String::from("Schnorr signature verification fails")))
                }
            }
        }
        Ok(())
    }
}

impl<M: Message> Encodable for Action<M> {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::TransferCCS {
                address,
                quantity,
            } => {
                s.begin_list(3).append(&ACTION_TAG_TRANSFER_CCS).append(address).append(quantity);
            }
            Action::DelegateCCS {
                address,
                quantity,
            } => {
                s.begin_list(3).append(&ACTION_TAG_DELEGATE_CCS).append(address).append(quantity);
            }
            Action::Revoke {
                address,
                quantity,
            } => {
                s.begin_list(3).append(&ACTION_TAG_REVOKE).append(address).append(quantity);
            }
            Action::SelfNominate {
                deposit,
                metadata,
            } => {
                s.begin_list(3).append(&ACTION_TAG_SELF_NOMINATE).append(deposit).append(metadata);
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => {
                s.begin_list(3 + signatures.len())
                    .append(&ACTION_TAG_CHANGE_PARAMS)
                    .append(metadata_seq)
                    .append(&**params);
                for signature in signatures {
                    s.append(signature);
                }
            }
            Action::ReportDoubleVote {
                message1,
                message2,
            } => {
                s.begin_list(3).append(&ACTION_TAG_REPORT_DOUBLE_VOTE).append(message1).append(message2);
            }
        };
    }
}

impl<M: Message> Decodable for Action<M> {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        match tag {
            ACTION_TAG_TRANSFER_CCS => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(Action::TransferCCS {
                    address: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
                })
            }
            ACTION_TAG_DELEGATE_CCS => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(Action::DelegateCCS {
                    address: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
                })
            }
            ACTION_TAG_REVOKE => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(Action::Revoke {
                    address: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
                })
            }
            ACTION_TAG_SELF_NOMINATE => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpInvalidLength {
                        expected: 3,
                        got: item_count,
                    })
                }
                Ok(Action::SelfNominate {
                    deposit: rlp.val_at(1)?,
                    metadata: rlp.val_at(2)?,
                })
            }
            ACTION_TAG_CHANGE_PARAMS => {
                let item_count = rlp.item_count()?;
                if item_count < 4 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        expected: 4,
                        got: item_count,
                    })
                }
                let metadata_seq = rlp.val_at(1)?;
                let params = Box::new(rlp.val_at(2)?);
                let signatures = (3..item_count).map(|i| rlp.val_at(i)).collect::<Result<_, _>>()?;
                Ok(Action::ChangeParams {
                    metadata_seq,
                    params,
                    signatures,
                })
            }
            ACTION_TAG_REPORT_DOUBLE_VOTE => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        expected: 3,
                        got: item_count,
                    })
                }
                let message1 = rlp.val_at(1)?;
                let message2 = rlp.val_at(2)?;
                Ok(Action::ReportDoubleVote {
                    message1,
                    message2,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected Tendermint Stake Action Type")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccrypto::blake256;
    use ckey::sign_schnorr;
    use client::TestBlockChainClient;
    use consensus::solo::SoloMessage;
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn decode_fail_if_change_params_have_no_signatures() {
        let action = Action::ChangeParams {
            metadata_seq: 3,
            params: CommonParams::default_for_test().into(),
            signatures: vec![],
        };
        assert_eq!(
            Err(DecoderError::RlpIncorrectListLen {
                expected: 4,
                got: 3,
            }),
            UntrustedRlp::new(&rlp::encode(&action)).as_val::<Action>::<_>()
        );
    }

    #[test]
    fn rlp_of_change_params() {
        rlp_encode_and_decode_test!(Action::ChangeParams {
            metadata_seq: 3,
            params: CommonParams::default_for_test().into(),
            signatures: vec![Signature::random(), Signature::random()],
        });
    }
}
