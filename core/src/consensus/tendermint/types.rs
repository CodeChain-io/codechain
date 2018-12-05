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
use primitives::Bytes;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

pub type Height = usize;
pub type View = usize;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Step {
    Propose,
    Prevote,
    Precommit,
    Commit,
}

impl Step {
    pub fn is_pre(self) -> bool {
        match self {
            Step::Prevote | Step::Precommit => true,
            _ => false,
        }
    }

    pub fn number(self) -> u8 {
        match self {
            Step::Propose => 0,
            Step::Prevote => 1,
            Step::Precommit => 2,
            Step::Commit => 3,
        }
    }
}

impl Decodable for Step {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.as_val()? {
            0u8 => Ok(Step::Propose),
            1 => Ok(Step::Prevote),
            2 => Ok(Step::Precommit),
            // FIXME: Step::Commit case is not necessary if Api::send_local_message does not serialize message.
            3 => Ok(Step::Commit),
            _ => Err(DecoderError::Custom("Invalid step.")),
        }
    }
}

impl Encodable for Step {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_single_value(&self.number());
    }
}

pub struct ProposalSeal<'a> {
    view: &'a View,
    signature: &'a Signature,
}

impl<'a> ProposalSeal<'a> {
    pub fn new(view: &'a View, signature: &'a Signature) -> Self {
        Self {
            view,
            signature,
        }
    }

    pub fn seal_fields(&self) -> Vec<Bytes> {
        vec![
            ::rlp::encode(&*self.view).into_vec(),
            ::rlp::encode(&*self.signature).into_vec(),
            ::rlp::EMPTY_LIST_RLP.to_vec(),
        ]
    }
}

pub struct RegularSeal<'a> {
    view: &'a View,
    signatures: &'a [Signature],
}

impl<'a> RegularSeal<'a> {
    pub fn new(view: &'a View, signatures: &'a [Signature]) -> Self {
        Self {
            view,
            signatures,
        }
    }

    pub fn seal_fields(&self) -> Vec<Bytes> {
        vec![
            ::rlp::encode(&*self.view).into_vec(),
            ::rlp::NULL_RLP.to_vec(),
            ::rlp::encode_list(self.signatures).into_vec(),
        ]
    }
}
