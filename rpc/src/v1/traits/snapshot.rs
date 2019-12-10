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

use jsonrpc_core::Result;

use super::super::types::BlockNumberAndHash;

#[rpc(server)]
pub trait Snapshot {
    /// Gets list of block numbers and block hashes of the snapshots.
    #[rpc(name = "snapshot_getList")]
    fn get_snapshot_list(&self) -> Result<Vec<BlockNumberAndHash>>;
}
