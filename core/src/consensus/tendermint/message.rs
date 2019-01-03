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

use std::cmp;

use ccrypto::blake256;
use ckey::{verify_schnorr, Public, SchnorrSignature};
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::vote_collector::Message;
use super::types::BitSet;
use super::{BlockHash, Height, Step, View};
use crate::error::Error;
use crate::header::Header;

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

    pub fn is_view(&self, height: Height, view: View) -> bool {
        self.height == height && self.view == view
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

#[derive(Debug, PartialEq)]
pub enum TendermintMessage {
    ConsensusMessage(Bytes),
    ProposalBlock(SchnorrSignature, usize, Bytes),
    StepState {
        vote_step: VoteStep,
        proposal: Option<H256>,
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
}

impl Encodable for TendermintMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            TendermintMessage::ConsensusMessage(message) => {
                s.begin_list(2);
                s.append(&MESSAGE_ID_CONSENSUS_MESSAGE);
                s.append(message);
            }
            TendermintMessage::ProposalBlock(signature, signer_index, bytes) => {
                s.begin_list(4);
                s.append(&MESSAGE_ID_PROPOSAL_BLOCK);
                s.append(signature);
                s.append(signer_index);
                s.append(bytes);
            }
            TendermintMessage::StepState {
                vote_step,
                proposal,
                known_votes,
            } => {
                s.begin_list(4);
                s.append(&MESSAGE_ID_STEP_STATE);
                s.append(vote_step);
                s.append(proposal);
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
        }
    }
}

impl Decodable for TendermintMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let id = rlp.val_at(0)?;
        Ok(match id {
            MESSAGE_ID_CONSENSUS_MESSAGE => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let bytes = rlp.at(1)?;
                TendermintMessage::ConsensusMessage(bytes.as_val()?)
            }
            MESSAGE_ID_PROPOSAL_BLOCK => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let signature = rlp.at(1)?;
                let signer_index = rlp.at(2)?;
                let bytes = rlp.at(3)?;
                TendermintMessage::ProposalBlock(signature.as_val()?, signer_index.as_val()?, bytes.as_val()?)
            }
            MESSAGE_ID_STEP_STATE => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let vote_step = rlp.at(1)?.as_val()?;
                let proposal = rlp.at(2)?.as_val()?;
                let known_votes = rlp.at(3)?.as_val()?;
                TendermintMessage::StepState {
                    vote_step,
                    proposal,
                    known_votes,
                }
            }
            MESSAGE_ID_REQUEST_MESSAGE => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let vote_step = rlp.at(1)?.as_val()?;
                let requested_votes = rlp.at(2)?.as_val()?;
                TendermintMessage::RequestMessage {
                    vote_step,
                    requested_votes,
                }
            }
            MESSAGE_ID_REQUEST_PROPOSAL => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let height = rlp.at(1)?.as_val()?;
                let view = rlp.at(2)?.as_val()?;
                TendermintMessage::RequestProposal {
                    height,
                    view,
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

/// Message transmitted between consensus participants.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, RlpDecodable, RlpEncodable)]
pub struct ConsensusMessage {
    pub on: VoteOn,
    pub signature: SchnorrSignature,
    pub signer_index: usize,
}

impl ConsensusMessage {
    #[cfg(test)]
    fn new(
        signature: SchnorrSignature,
        signer_index: usize,
        height: Height,
        view: View,
        step: Step,
        block_hash: Option<BlockHash>,
    ) -> Self {
        ConsensusMessage {
            signature,
            signer_index,
            on: VoteOn {
                step: VoteStep::new(height, view, step),
                block_hash,
            },
        }
    }

    pub fn new_proposal(
        signature: SchnorrSignature,
        signer_index: usize,
        header: &Header,
    ) -> Result<Self, ::rlp::DecoderError> {
        let height = header.number();
        let view = consensus_view(header)?;

        Ok(ConsensusMessage {
            signature,
            signer_index,
            on: VoteOn {
                step: VoteStep::new(height as Height, view, Step::Propose),
                block_hash: Some(header.hash()),
            },
        })
    }

    pub fn verify(&self, signer_public: &Public) -> Result<bool, Error> {
        let vote_info = message_info_rlp(self.on.step, self.on.block_hash);
        Ok(verify_schnorr(signer_public, &self.signature, &blake256(vote_info))?)
    }
}

/// Header consensus view.
pub fn consensus_view(header: &Header) -> Result<View, ::rlp::DecoderError> {
    let view_rlp = header.seal().get(1).expect("seal passed basic verification; seal has 3 fields; qed");
    UntrustedRlp::new(view_rlp.as_slice()).as_val()
}

pub fn previous_block_view(header: &Header) -> Result<View, ::rlp::DecoderError> {
    let view_rlp = header.seal().get(0).expect("seal passed basic verification; seal has 3 fields; qed");
    UntrustedRlp::new(view_rlp.as_slice()).as_val()
}

impl Message for ConsensusMessage {
    type Round = VoteStep;

    fn signature(&self) -> SchnorrSignature {
        self.signature
    }

    fn signer_index(&self) -> usize {
        self.signer_index
    }

    fn block_hash(&self) -> Option<H256> {
        self.on.block_hash
    }

    fn round(&self) -> &VoteStep {
        &self.on.step
    }

    fn is_broadcastable(&self) -> bool {
        self.on.step.step.is_pre()
    }
}

pub fn message_info_rlp(step: VoteStep, block_hash: Option<BlockHash>) -> Bytes {
    let vote_on = VoteOn {
        step,
        block_hash,
    };
    vote_on.rlp_bytes().into_vec()
}

pub fn message_hash(step: VoteStep, block_hash: H256) -> H256 {
    let vote_on = VoteOn {
        step,
        block_hash: Some(block_hash),
    };
    blake256(&vote_on.rlp_bytes())
}

#[cfg(test)]
mod tests {
    use rlp::{self, rlp_encode_and_decode_test};

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
        rlp_encode_and_decode_test!(TendermintMessage::ConsensusMessage(vec![1u8, 2u8]));
    }

    #[test]
    fn encode_and_decode_tendermint_message_2() {
        rlp_encode_and_decode_test!(TendermintMessage::ProposalBlock(
            SchnorrSignature::random(),
            0x1234,
            vec![1u8, 2u8]
        ));
    }

    #[test]
    fn encode_and_decode_tendermint_message_3() {
        let mut bit_set = BitSet::new();
        bit_set.set(2);
        rlp_encode_and_decode_test!(TendermintMessage::StepState {
            vote_step: VoteStep::new(10, 123, Step::Prevote),
            proposal: Some(Default::default()),
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
    fn encode_and_decode_consensus_message_1() {
        let message = ConsensusMessage::default();
        rlp_encode_and_decode_test!(message);
    }

    #[test]
    fn encode_and_decode_consensus_message_2() {
        let message = ConsensusMessage::new(
            SchnorrSignature::random(),
            0x1234,
            2usize,
            3usize,
            Step::Commit,
            Some(H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050")),
        );
        rlp_encode_and_decode_test!(message);
    }

    #[test]
    fn encode_and_decode_consensus_message_3() {
        let height = 2usize;
        let view = 3usize;
        let step = Step::Commit;
        let signature = SchnorrSignature::random();
        let index = 0x1234;
        let block_hash = Some(H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050"));
        let consensus_message = ConsensusMessage::new(signature, index, height, view, step, block_hash);
        let encoded = consensus_message.rlp_bytes();
        let decoded = rlp::decode::<ConsensusMessage>(&encoded);
        assert_eq!(consensus_message, decoded);
    }
}
