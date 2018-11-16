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

use cjson::uint::Uint;
use ckey::{Error as KeyError, NetworkId};
use ctypes::parcel::IncompleteParcel;

use super::Action;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsignedParcel {
    pub seq: Option<Uint>,
    pub fee: Uint,
    pub network_id: NetworkId,
    pub action: Action,
}

// FIXME: Use TryFrom.
impl From<UnsignedParcel> for Result<(IncompleteParcel, Option<u64>), KeyError> {
    fn from(parcel: UnsignedParcel) -> Self {
        Ok((
            IncompleteParcel {
                fee: parcel.fee.into(),
                network_id: parcel.network_id,
                action: Result::from(parcel.action)?,
            },
            parcel.seq.map(Into::into),
        ))
    }
}
