// Copyright 2019 Kodebox, Inc.
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
use primitives::Bytes;

use super::super::types::IBCQueryResult;


#[rpc(server)]
pub trait IBC {
    #[rpc(name = "ibc_query_client_consensus_state")]
    fn query_client_consensus_state(
        &self,
        client_id: String,
        block_number: Option<u64>,
    ) -> Result<Option<IBCQueryResult>>;

    #[rpc(name = "ibc_query_header")]
    fn query_header(&self, block_number: Option<u64>) -> Result<Option<String>>;

    /// Gets the other chain's root saved in the light client
    #[rpc(name = "ibc_query_client_root")]
    fn query_client_root(
        &self,
        client_id: String,
        other_block_number: u64,
        this_block_number: Option<u64>,
    ) -> Result<Option<IBCQueryResult>>;
}
