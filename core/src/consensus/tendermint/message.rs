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

use std::cmp;

use ccrypto::blake256;
use ckey::{verify_schnorr, Error as KeyError, Public, SchnorrSignature};
use ctypes::BlockHash;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use snap;

pub use super::super::sortition::PriorityMessage;
use super::super::BitSet;
use super::{Height, Step, View};

/// Complete step of the consensus process.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, RlpDecodable, RlpEncodable)]
pub struct VoteStep {
    pub height: Height,
    pub view: View,
    pub step: Step,
}

impl VoteStep {
    pub fn new(height: Height, view: View, step: Step) -> Self {
        VoteStep {
            height,
            view,
            step,
        }
    }

    pub fn is_step(&self, height: Height, view: View, step: Step) -> bool {
        self.height == height && self.view == view && self.step == step
    }
}

impl Default for VoteStep {
    fn default() -> Self {
        VoteStep::new(0, 0, Step::Propose)
    }
}

impl PartialOrd for VoteStep {
    fn partial_cmp(&self, m: &VoteStep) -> Option<cmp::Ordering> {
        Some(self.cmp(m))
    }
}

impl Ord for VoteStep {
    fn cmp(&self, m: &VoteStep) -> cmp::Ordering {
        if self.height != m.height {
            self.height.cmp(&m.height)
        } else if self.view != m.view {
            self.view.cmp(&m.view)
        } else {
            self.step.number().cmp(&m.step.number())
        }
    }
}

const MESSAGE_ID_CONSENSUS_MESSAGE: u8 = 0x01;
const MESSAGE_ID_PROPOSAL_BLOCK: u8 = 0x02;
const MESSAGE_ID_STEP_STATE: u8 = 0x03;
const MESSAGE_ID_REQUEST_MESSAGE: u8 = 0x04;
const MESSAGE_ID_REQUEST_PROPOSAL: u8 = 0x05;
const MESSAGE_ID_REQUEST_COMMIT: u8 = 0x06;
const MESSAGE_ID_COMMIT: u8 = 0x07;
const MESSAGE_ID_PRIORITY: u8 = 0x08;

#[derive(Debug, PartialEq)]
pub enum TendermintMessage {
    ConsensusMessage(Vec<Bytes>),
    PriorityMessage {
        height: Height,
        view: View,
        signer_idx: usize,
        message: PriorityMessage,
    },
    ProposalBlock {
        signature: SchnorrSignature,
        view: View,
        message: Bytes,
    },
    StepState {
        vote_step: VoteStep,
        proposal: Option<BlockHash>,
        lock_view: Option<View>,
        known_votes: BitSet,
    },
    RequestMessage {
        vote_step: VoteStep,
        requested_votes: BitSet,
    },
    RequestProposal {
        height: Height,
        view: View,
    },
    RequestCommit {
        height: Height,
    },
    Commit {
        block: Bytes,
        votes: Vec<ConsensusMessage>,
    },
}

impl Encodable for TendermintMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            TendermintMessage::ConsensusMessage(messages) => {
                s.begin_list(2);
                s.append(&MESSAGE_ID_CONSENSUS_MESSAGE);
                s.append_list::<Bytes, Bytes>(messages);
            }
            TendermintMessage::ProposalBlock {
                signature,
                view,
                message,
            } => {
                s.begin_list(4);
                s.append(&MESSAGE_ID_PROPOSAL_BLOCK);
                s.append(signature);
                s.append(view);

                let compressed = {
                    // TODO: Cache the Encoder object
                    let mut snappy_encoder = snap::Encoder::new();
                    snappy_encoder.compress_vec(message).expect("Compression always succeed")
                };
                s.append(&compressed);
            }
            TendermintMessage::StepState {
                vote_step,
                proposal,
                lock_view,
                known_votes,
            } => {
                s.begin_list(5);
                s.append(&MESSAGE_ID_STEP_STATE);
                s.append(vote_step);
                s.append(proposal);
                s.append(lock_view);
                s.append(known_votes);
            }
            TendermintMessage::RequestMessage {
                vote_step,
                requested_votes,
            } => {
                s.begin_list(3);
                s.append(&MESSAGE_ID_REQUEST_MESSAGE);
                s.append(vote_step);
                s.append(requested_votes);
            }
            TendermintMessage::RequestProposal {
                height,
                view,
            } => {
                s.begin_list(3);
                s.append(&MESSAGE_ID_REQUEST_PROPOSAL);
                s.append(height);
                s.append(view);
            }
            TendermintMessage::RequestCommit {
                height,
            } => {
                s.begin_list(2);
                s.append(&MESSAGE_ID_REQUEST_COMMIT);
                s.append(height);
            }
            TendermintMessage::Commit {
                block,
                votes,
            } => {
                s.begin_list(3);
                s.append(&MESSAGE_ID_COMMIT);
                s.append(block);
                s.append_list(votes);
            }
            TendermintMessage::PriorityMessage {
                height,
                view,
                signer_idx,
                message,
            } => {
                s.begin_list(5);
                s.append(&MESSAGE_ID_PRIORITY).append(height).append(view).append(signer_idx).append(message);
            }
        }
    }
}

impl Decodable for TendermintMessage {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let id = rlp.val_at(0)?;
        Ok(match id {
            MESSAGE_ID_CONSENSUS_MESSAGE => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                TendermintMessage::ConsensusMessage(rlp.list_at(1)?)
            }
            MESSAGE_ID_PROPOSAL_BLOCK => {
                let item_count = rlp.item_count()?;
                if item_count != 4 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 4,
                    })
                }
                let signature = rlp.at(1)?;
                let view = rlp.at(2)?;
                let compressed_message: Vec<u8> = rlp.val_at(3)?;
                let uncompressed_message = {
                    // TODO: Cache the Decoder object
                    let mut snappy_decoder = snap::Decoder::new();
                    snappy_decoder.decompress_vec(&compressed_message).map_err(|err| {
                        cwarn!(SYNC, "Decompression failed while decoding a body response: {}", err);
                        DecoderError::Custom("Invalid compression format")
                    })?
                };

                TendermintMessage::ProposalBlock {
                    signature: signature.as_val()?,
                    view: view.as_val()?,
                    message: uncompressed_message,
                }
            }
            MESSAGE_ID_STEP_STATE => {
                let item_count = rlp.item_count()?;
                if item_count != 5 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 5,
                    })
                }
                let vote_step = rlp.at(1)?.as_val()?;
                let proposal = rlp.at(2)?.as_val()?;
                let lock_view = rlp.at(3)?.as_val()?;
                let known_votes = rlp.at(4)?.as_val()?;
                TendermintMessage::StepState {
                    vote_step,
                    proposal,
                    lock_view,
                    known_votes,
                }
            }
            MESSAGE_ID_REQUEST_MESSAGE => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                let vote_step = rlp.at(1)?.as_val()?;
                let requested_votes = rlp.at(2)?.as_val()?;
                TendermintMessage::RequestMessage {
                    vote_step,
                    requested_votes,
                }
            }
            MESSAGE_ID_REQUEST_PROPOSAL => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                let height = rlp.at(1)?.as_val()?;
                let view = rlp.at(2)?.as_val()?;
                TendermintMessage::RequestProposal {
                    height,
                    view,
                }
            }
            MESSAGE_ID_REQUEST_COMMIT => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                let height = rlp.at(1)?.as_val()?;
                TendermintMessage::RequestCommit {
                    height,
                }
            }
            MESSAGE_ID_COMMIT => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                let block = rlp.at(1)?.as_val()?;
                let votes = rlp.at(2)?.as_list()?;
                TendermintMessage::Commit {
                    block,
                    votes,
                }
            }
            MESSAGE_ID_PRIORITY => {
                let item_count = rlp.item_count()?;
                if item_count != 5 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 5,
                    })
                }
                let height = rlp.at(1)?.as_val()?;
                let view = rlp.at(2)?.as_val()?;
                let signer_idx = rlp.at(3)?.as_val()?;
                let message = rlp.at(4)?.as_val()?;
                TendermintMessage::PriorityMessage {
                    height,
                    view,
                    signer_idx,
                    message,
                }
            }
            _ => return Err(DecoderError::Custom("Unknown message id detected")),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, RlpDecodable, RlpEncodable)]
pub struct VoteOn {
    pub step: VoteStep,
    pub block_hash: Option<BlockHash>,
}

impl VoteOn {
    pub fn hash(&self) -> H256 {
        blake256(&self.rlp_bytes())
    }
}

/// Message transmitted between consensus participants.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, RlpDecodable, RlpEncodable)]
pub struct ConsensusMessage {
    pub on: VoteOn,
    pub signature: SchnorrSignature,
    pub signer_index: usize,
}

impl ConsensusMessage {
    pub fn signature(&self) -> SchnorrSignature {
        self.signature
    }

    pub fn signer_index(&self) -> usize {
        self.signer_index
    }

    pub fn block_hash(&self) -> Option<BlockHash> {
        self.on.block_hash
    }

    pub fn round(&self) -> &VoteStep {
        &self.on.step
    }

    pub fn height(&self) -> u64 {
        self.on.step.height
    }

    pub fn verify(&self, signer_public: &Public) -> Result<bool, KeyError> {
        verify_schnorr(signer_public, &self.signature, &self.on.hash())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ccrypto::sha256;
    use ckey::Private;
    use parking_lot::RwLock;
    use rlp::{self, rlp_encode_and_decode_test};
    use vrf::openssl::{CipherSuite, ECVRF};

    use super::super::super::sortition::vrf_sortition::VRFSortition;
    use super::super::Step;
    use super::*;

    #[test]
    fn step_ordering() {
        assert!(VoteStep::new(10, 123, Step::Precommit) < VoteStep::new(11, 123, Step::Precommit));
        assert!(VoteStep::new(10, 123, Step::Propose) < VoteStep::new(11, 123, Step::Precommit));
        assert!(VoteStep::new(10, 122, Step::Propose) < VoteStep::new(11, 123, Step::Propose));
    }

    #[test]
    fn encode_and_decode_tendermint_message_1() {
        rlp_encode_and_decode_test!(TendermintMessage::ConsensusMessage(vec![vec![1u8, 2u8]]));
    }

    #[test]
    fn encode_and_decode_tendermint_message_1_2() {
        rlp_encode_and_decode_test!(TendermintMessage::ConsensusMessage(vec![vec![1u8, 2u8], vec![3u8, 4u8]]));
    }

    #[test]
    fn encode_and_decode_tendermint_message_2() {
        rlp_encode_and_decode_test!(TendermintMessage::ProposalBlock {
            signature: SchnorrSignature::random(),
            view: 1,
            message: vec![1u8, 2u8]
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_3() {
        let mut bit_set = BitSet::new();
        bit_set.set(2);
        rlp_encode_and_decode_test!(TendermintMessage::StepState {
            vote_step: VoteStep::new(10, 123, Step::Prevote),
            proposal: Some(Default::default()),
            lock_view: Some(2),
            known_votes: bit_set
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_4() {
        let mut bit_set = BitSet::new();
        bit_set.set(1);
        rlp_encode_and_decode_test!(TendermintMessage::RequestMessage {
            vote_step: VoteStep::new(10, 123, Step::Prevote),
            requested_votes: bit_set,
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_5() {
        rlp_encode_and_decode_test!(TendermintMessage::RequestProposal {
            height: 10,
            view: 123,
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_6() {
        rlp_encode_and_decode_test!(TendermintMessage::RequestCommit {
            height: 3,
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_7() {
        rlp_encode_and_decode_test!(TendermintMessage::Commit {
            block: vec![1u8, 2u8],
            votes: vec![
                ConsensusMessage {
                    signature: SchnorrSignature::random(),
                    signer_index: 0x1234,
                    on: VoteOn {
                        step: VoteStep::new(2, 3, Step::Commit),
                        block_hash: Some(
                            H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").into()
                        ),
                    },
                },
                ConsensusMessage {
                    signature: SchnorrSignature::random(),
                    signer_index: 0x1235,
                    on: VoteOn {
                        step: VoteStep::new(2, 3, Step::Commit),
                        block_hash: Some(
                            H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").into()
                        ),
                    },
                }
            ]
        });
    }

    #[test]
    fn encode_and_decode_tendermint_message_8() {
        let priv_key: Private = sha256("secret_key").into();

        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 71.85,
            vrf_inst: ec_vrf,
        };
        let voting_power = 50;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key, voting_power).unwrap().unwrap();

        let priority_message = PriorityMessage {
            seed,
            info: priority_info,
        };
        rlp_encode_and_decode_test!(TendermintMessage::PriorityMessage {
            height: 2187,
            view: 1,
            signer_idx: 11,
            message: priority_message
        });
    }

    #[test]
    fn encode_and_decode_consensus_message_1() {
        let message = ConsensusMessage::default();
        rlp_encode_and_decode_test!(message);
    }

    #[test]
    fn encode_and_decode_consensus_message_2() {
        let message = ConsensusMessage {
            signature: SchnorrSignature::random(),
            signer_index: 0x1234,
            on: VoteOn {
                step: VoteStep::new(2, 3, Step::Commit),
                block_hash: Some(H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").into()),
            },
        };
        rlp_encode_and_decode_test!(message);
    }

    #[test]
    fn encode_and_decode_consensus_message_3() {
        let height = 2;
        let view = 3;
        let step = Step::Commit;
        let signature = SchnorrSignature::random();
        let signer_index = 0x1234;
        let block_hash = Some(H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").into());
        let consensus_message = ConsensusMessage {
            signature,
            signer_index,
            on: VoteOn {
                step: VoteStep::new(height, view, step),
                block_hash,
            },
        };
        let encoded = consensus_message.rlp_bytes();
        let decoded = rlp::decode::<ConsensusMessage>(&encoded).unwrap();
        assert_eq!(consensus_message, decoded);
    }
}
