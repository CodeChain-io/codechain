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

use ckey::NetworkId;
use primitives::U256;

use super::{Action, Parcel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteParcel {
    /// Amount of CCC to be paid as a cost for distributing this parcel to the network.
    pub fee: U256,
    /// Network Id
    pub network_id: NetworkId,

    pub action: Action,
}

impl IncompleteParcel {
    pub fn complete(self, seq: U256) -> Parcel {
        Parcel {
            seq,
            fee: self.fee,
            network_id: self.network_id,
            action: self.action,
        }
    }
}
