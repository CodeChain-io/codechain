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
use primitives::H520;
use rlp::RlpStream;

/// Tendermint seal.
pub struct Tendermint {
    /// Parent block's view
    pub prev_view: usize,
    /// Current block's view
    pub cur_view: usize,
    /// Precommit seal signatures.
    pub precommits: Vec<H520>,
}

impl From<Tendermint> for Generic {
    fn from(tendermint: Tendermint) -> Self {
        let mut stream = RlpStream::new_list(3);
        stream.append(&tendermint.prev_view).append(&tendermint.cur_view).append_list(&tendermint.precommits);
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

impl From<cjson::scheme::Seal> for Seal {
    fn from(s: cjson::scheme::Seal) -> Self {
        match s {
            cjson::scheme::Seal::Tendermint(tender) => Seal::Tendermint(Tendermint {
                prev_view: tender.prev_view.into(),
                cur_view: tender.cur_view.into(),
                precommits: tender.precommits.into_iter().map(Into::into).collect(),
            }),
            cjson::scheme::Seal::Generic(g) => Seal::Generic(Generic(g.into())),
        }
    }
}

impl From<Seal> for Generic {
    fn from(seal: Seal) -> Self {
        match seal {
            Seal::Generic(generic) => generic,
            Seal::Tendermint(tender) => tender.into(),
        }
    }
}
