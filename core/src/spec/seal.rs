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

use cjson;
use ctypes::H520;
use rlp::RlpStream;

/// Tendermint seal.
pub struct Tendermint {
    /// Seal round.
    pub round: usize,
    /// Proposal seal signature.
    pub proposal: H520,
    /// Precommit seal signatures.
    pub precommits: Vec<H520>,
}

impl Into<Generic> for Tendermint {
    fn into(self) -> Generic {
        let mut stream = RlpStream::new_list(3);
        stream.append(&self.round).append(&self.proposal).append_list(&self.precommits);
        Generic(stream.out())
    }
}

pub struct Generic(pub Vec<u8>);

/// Genesis seal type.
pub enum Seal {
    /// AuthorityRound seal.
    Tendermint(Tendermint),
    /// Generic RLP seal.
    Generic(Generic),
}

impl From<cjson::spec::Seal> for Seal {
    fn from(s: cjson::spec::Seal) -> Self {
        match s {
            cjson::spec::Seal::Tendermint(tender) => Seal::Tendermint(Tendermint {
                round: tender.round.into(),
                proposal: tender.proposal.into(),
                precommits: tender.precommits.into_iter().map(Into::into).collect(),
            }),
            cjson::spec::Seal::Generic(g) => Seal::Generic(Generic(g.into())),
        }
    }
}

impl Into<Generic> for Seal {
    fn into(self) -> Generic {
        match self {
            Seal::Generic(generic) => generic,
            Seal::Tendermint(tender) => tender.into(),
        }
    }
}
