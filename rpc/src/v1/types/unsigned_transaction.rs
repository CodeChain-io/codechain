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

use super::super::errors;
use super::Action;
use cjson::uint::Uint;
use ckey::NetworkId;
use ctypes::transaction::IncompleteTransaction;
use jsonrpc_core::Error;
use std::convert::{TryFrom, TryInto};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsignedTransaction {
    pub seq: Option<u64>,
    pub fee: Uint,
    pub network_id: NetworkId,
    pub action: Action,
}

impl TryFrom<UnsignedTransaction> for (IncompleteTransaction, Option<u64>) {
    type Error = Error;
    fn try_from(tx: UnsignedTransaction) -> Result<Self, Self::Error> {
        Ok((
            IncompleteTransaction {
                fee: tx.fee.into(),
                network_id: tx.network_id,
                action: tx.action.try_into().map_err(errors::conversion)?,
            },
            tx.seq,
        ))
    }
}
