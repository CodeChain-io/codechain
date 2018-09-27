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

use ckey::{Error as KeyError, NetworkId};
use ctypes::parcel::{Action as ActionType, Parcel};
use primitives::U256;

use super::Action;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsignedParcel {
    pub nonce: Option<U256>,
    pub fee: U256,
    pub network_id: NetworkId,
    pub action: Action,
}

// FIXME: Use TryFrom.
impl From<UnsignedParcel> for Result<Parcel, KeyError> {
    fn from(value: UnsignedParcel) -> Self {
        let nonce = value.nonce.expect("Nonce must exist");
        let fee = value.fee;
        let network_id = value.network_id;
        let action: ActionType = Result::from(value.action)?;

        Ok(Parcel {
            nonce,
            fee,
            network_id,
            action,
        })
    }
}
