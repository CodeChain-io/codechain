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

use ccrypto::Blake;
use ckey::{recover, Address, Signature};
use ctypes::errors::SyntaxError;
use ctypes::CommonParams;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

const ACTION_TAG_TRANSFER_CCS: u8 = 1;
const ACTION_TAG_DELEGATE_CCS: u8 = 2;
const ACTION_TAG_REVOKE: u8 = 3;
const ACTION_TAG_SELF_NOMINATE: u8 = 4;
const ACTION_TAG_CHANGE_PARAMS: u8 = 0xFF;

#[derive(Debug, PartialEq)]
pub enum Action {
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
}

impl Action {
    pub fn verify(&self, current_params: &CommonParams) -> Result<(), SyntaxError> {
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
                        "Too long candidata metadata: the size limit is {}",
                        current_params.max_candidate_metadata_size()
                    )))
                }
            }
            Action::ChangeParams {
                metadata_seq,
                params,
                signatures,
            } => {
                let action = Action::ChangeParams {
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
        }
        Ok(())
    }
}

impl Encodable for Action {
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
        };
    }
}

impl Decodable for Action {
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
            _ => Err(DecoderError::Custom("Unexpected Tendermint Stake Action Type")),
        }
    }
}

#[cfg(test)]
mod tests {
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
            UntrustedRlp::new(&rlp::encode(&action)).as_val::<Action>()
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
